# Security regression: a token whose module/struct name exceeds COIN_STRING_LENGTH
# (32 bytes) must NOT be clear-signed.
#
# Background: CoinType stores module/struct names zero-padded to COIN_STRING_LENGTH and
# reuses those bytes as the canonical asset identity (parsing, aggregation, UI). The
# parser used to silently TRUNCATE longer names to that length, so two distinct token
# types sharing their first COIN_STRING_LENGTH bytes would collide and could be shown /
# aggregated under the wrong identity. struct_tag_parser now rejects names longer than
# COIN_STRING_LENGTH instead of truncating, so such a transaction is not clear-signed: it
# falls back to the not-recognized / blind-sign path (and swaps reject outright).
#
# This test reuses the passing clear-sign transfer `test_sign_tx_unrecognized_coin`
# (token 0x90e8c5..::killa::KILLA) but lengthens the module name past 32 bytes and
# re-points the transferred input's object digest at the modified object. The device
# must refuse to clear-sign it.

import base64
import hashlib

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import run_apdu_and_nav_tasks_concurrently


def _read_uleb128(data: bytes, offset: int):
    result = shift = 0
    while True:
        b = data[offset]
        offset += 1
        result |= (b & 0x7F) << shift
        if not b & 0x80:
            return result, offset
        shift += 7


def _with_module_name(obj: bytes, new_module: bytes) -> bytes:
    """Rebuild a Coin object blob (ObjectData::Move | Coin | TypeTag::Struct | StructTag
    | ... ) replacing the StructTag module name, keeping every trailing byte intact so it
    still parses. Layout: 0x00 0x03 0x07 | addr[32] | module(vec) | struct(vec) |
    type_params(vec) | has_public_transfer | version | contents | owner | digest | rebate.
    """
    assert obj[0:3] == bytes([0x00, 0x03, 0x07]), "expected Move/Coin/Struct object"
    o = 3
    addr = obj[o:o + 32]
    o += 32
    mlen, o = _read_uleb128(obj, o)
    o += mlen  # skip original module
    nlen, o = _read_uleb128(obj, o)
    struct_name = obj[o:o + nlen]
    o += nlen
    _, o = _read_uleb128(obj, o)  # type_params count (0 for these coins)
    tail = obj[o:]
    assert len(new_module) < 0x80, "single-byte ULEB length only"
    return (bytes([0x00, 0x03, 0x07]) + addr
            + bytes([len(new_module)]) + new_module
            + bytes([len(struct_name)]) + struct_name
            + bytes([0x00])  # type_params
            + tail)


def _object_digest(obj: bytes) -> bytes:
    return hashlib.blake2b(b"Object::" + obj, digest_size=32).digest()


def test_sign_tx_token_overlong_module_name_rejected(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    # Same transaction/object as the passing `test_sign_tx_unrecognized_coin`.
    transaction = bytearray(base64.b64decode(
        'AAAAAAACAQAkOlErOjssUas7B1ipByHf2etJJYdwBbMTSEy5doj0VgHzIR4AAAAAIN8vTwL8rbbLzRfsdy1PyOXvcrij9n34ovKk2/o0N3wNACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAG02HZIA+4tmm1GrxPh4dKNj3Fry/X/O0WxUh1ovpxisAQkHEsRdw5dbbdY9esFx0S8xZ3rE61Q5gJ3SV2OdlnYVFxwwHgAAAAAgDI4TkhHnVDhJiSloJl/c9O1pBEyKpv0JUSJ/mmyKbuVtNh2SAPuLZptRq8T4eHSjY9xa8v1/ztFsVIdaL6cYrOkCAAAAAAAA8AMmAAAAAAAA'))
    coin_obj = base64.b64decode(
        'AAMHkOjF9XYq+mdulvsw4s0Mm/3IQNQy7OWHk/VPQtmYGwkFa2lsbGEFS0lMTEEAAQHzIR4AAAAAKCQ6USs6OyxRqzsHWKkHId/Z60klh3AFsxNITLl2iPRW5Rb4fgVVAAAAbTYdkgD7i2abUavE+Hh0o2PcWvL9f87RbFSHWi+nGKwgeQhHBdsHFvOvjCyMxNjkg1Ue4ypBA1B5GpIVylbqy2UAaRQAAAAAAA==')

    # 40-byte module name (> COIN_STRING_LENGTH = 32): previously this would be silently
    # truncated to 32 bytes; the fixed parser rejects it.
    overlong_obj = _with_module_name(coin_obj, b"killa" + b"x" * 35)

    original_digest = bytes.fromhex(
        'df2f4f02fcadb6cbcd17ec772d4fc8e5ef72b8a3f67df8a2f2a4dbfa34377c0d')
    pos = transaction.find(original_digest)
    assert pos != -1, "could not locate transferred object digest in transaction"
    transaction[pos:pos + 32] = _object_digest(overlong_obj)
    transaction = bytes(transaction)

    object_list = [overlong_obj]

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
        pytest.fail('a token with an overlong module name must not be clear-signed')

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0
