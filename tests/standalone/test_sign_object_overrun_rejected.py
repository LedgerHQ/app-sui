# Security regression: an object must be parsed from exactly the bytes its digest
# covers, never from the bytes that follow them.
#
# Background: SIGN_TX takes a list of objects, each framed as `length | bytes`. The
# device hashes only the first `length` bytes of an entry and matches that hash against
# the digest the transaction names, which is what makes the coin type and amount
# trustworthy. It then parsed the entry on a separate, unbounded cursor, and that parse
# stops where the object's own embedded lengths say, not where `length` says. An object
# whose hashed range ends early let the parse run on into whatever the host put next in
# the stream -- the following entry's length prefix and body -- and take the coin
# amount from those unverified bytes. The amount shown on the review screen (and the
# one swap's check_tx_params compares) was then not covered by any digest.
#
# This test reuses the clear-sign token transfer `test_sign_tx_unrecognized_coin`
# (0x90e8c5..::killa::KILLA) and truncates the coin object just before its 8-byte
# balance. The digest in the transaction is re-pointed at the truncated range, so the
# hash check passes. The balance is then supplied by the next list entry: its 4-byte
# length prefix and first four bytes form the amount, and the rest of the original
# object follows so the parse still completes. The device must refuse to clear-sign it.

import base64
import hashlib

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import run_apdu_and_nav_tasks_concurrently

# Status the app returns (SyscallError::NotSupported) for a transaction it does not
# recognize while blind signing is off: the refusal must come from that path, not from
# any other failure.
SUI_TX_NOT_RECOGNIZED = 0x0008


def _object_digest(obj: bytes) -> bytes:
    return hashlib.blake2b(b"Object::" + obj, digest_size=32).digest()


def test_sign_tx_object_parsed_past_hashed_length_rejected(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    # Same transaction/object as the passing `test_sign_tx_unrecognized_coin`.
    transaction = bytearray(base64.b64decode(
        'AAAAAAACAQAkOlErOjssUas7B1ipByHf2etJJYdwBbMTSEy5doj0VgHzIR4AAAAAIN8vTwL8rbbLzRfsdy1PyOXvcrij9n34ovKk2/o0N3wNACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAG02HZIA+4tmm1GrxPh4dKNj3Fry/X/O0WxUh1ovpxisAQkHEsRdw5dbbdY9esFx0S8xZ3rE61Q5gJ3SV2OdlnYVFxwwHgAAAAAgDI4TkhHnVDhJiSloJl/c9O1pBEyKpv0JUSJ/mmyKbuVtNh2SAPuLZptRq8T4eHSjY9xa8v1/ztFsVIdaL6cYrOkCAAAAAAAA8AMmAAAAAAAA'))
    coin_obj = base64.b64decode(
        'AAMHkOjF9XYq+mdulvsw4s0Mm/3IQNQy7OWHk/VPQtmYGwkFa2lsbGEFS0lMTEEAAQHzIR4AAAAAKCQ6USs6OyxRqzsHWKkHId/Z60klh3AFsxNITLl2iPRW5Rb4fgVVAAAAbTYdkgD7i2abUavE+Hh0o2PcWvL9f87RbFSHWi+nGKwgeQhHBdsHFvOvjCyMxNjkg1Ue4ypBA1B5GpIVylbqy2UAaRQAAAAAAA==')

    # Offset of the 8-byte balance: the last 8 bytes of the 40-byte contents vector,
    # which is a 0x28 length byte, the 32-byte UID, then the balance.
    amount_offset = 90
    assert coin_obj[amount_offset - 33] == 0x28, "contents vector not where expected"
    real_amount = int.from_bytes(coin_obj[amount_offset:amount_offset + 8], 'little')

    # The hashed range stops right before the balance...
    hashed = coin_obj[:amount_offset]
    # ...and the next entry supplies it. The balance is the entry's 4-byte length prefix
    # plus the first four bytes of its body; the remainder is the rest of the object.
    continuation = bytes(4) + coin_obj[amount_offset + 8:]
    forged_amount = len(continuation)
    assert forged_amount != real_amount

    original_digest = bytes.fromhex(
        'df2f4f02fcadb6cbcd17ec772d4fc8e5ef72b8a3f67df8a2f2a4dbfa34377c0d')
    pos = transaction.find(original_digest)
    assert pos != -1, "could not locate transferred object digest in transaction"
    transaction[pos:pos + 32] = _object_digest(hashed)
    transaction = bytes(transaction)

    object_list = [hashed, continuation]

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK,
                              NavInsID.RIGHT_CLICK, NavInsID.BOTH_CLICK],
                timeout=10,
                test_case_name=scenario_navigator.test_name,
                path=scenario_navigator.screenshot_path,
                screen_change_before_first_instruction=True,
                screen_change_after_last_instruction=False,
            )
        else:
            # Clear signing was refused -> dismiss the "Enable Blind signing" screen.
            navigator.navigate([NavInsID.USE_CASE_CHOICE_REJECT],
                                screen_change_before_first_instruction=False,
                                screen_change_after_last_instruction=False)

    def check_result(result):
        pytest.fail('an object parsed past its hashed length must not be clear-signed')

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert e.value.status == SUI_TX_NOT_RECOGNIZED
    assert len(e.value.data) == 0
