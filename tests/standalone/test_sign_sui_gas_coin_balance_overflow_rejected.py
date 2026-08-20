# Security regression for B2CA-2793 follow-up finding 1 (CWE-190): the sum of the gas
# payment balances must not wrap the clear-signed GasCoin transfer amount.
#
# Background: the parser summed every gas payment object's balance with plain `+`
# into `total_gas_amount`. Both cargo profiles set `overflow-checks = false`, so with
# several payment objects that sum wraps modulo 2^64 instead of trapping. When the PTB
# transfers `Argument::GasCoin` by value, that total *is* the amount the device
# asserts, so a wrapped sum understates -- here down to zero -- the value actually
# handed to the recipient. The sum is now checked, and an unrepresentable total leaves
# the gas balance unknown (None), which routes the transaction to the same
# unrecognized-tx fail-safe already used when a payment object cannot be resolved.
#
# This is the `TransferObjects{objects:[GasCoin], address}` shape of the passing
# `test_sign_tx_sui_whole_gas_coin`, with gas_data.payment extended to two objects
# whose balances sum to u64::MAX + 1. Confirmed against the vulnerable parser (this
# fix reverted): the device displayed and signed "Amount SUI 0.0" for a transaction
# handing over both gas coins. With the fix the device must refuse to clear-sign it.

import base64
import hashlib

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import run_apdu_and_nav_tasks_concurrently

U64_MAX = 2 ** 64 - 1

# gas_data.payment[0] of the base transaction.
GAS_OBJECT_ID = bytes.fromhex(
    '400dbdcfeda8e1e64ebbf5710ccf5a67bfa6e7f945c92bc8611e0cb44b7219de')
OBJECT_REF_LEN = 32 + 8 + 33  # objectId | version | digest(vec: 0x20 | 32 bytes)


def _read_uleb128(data: bytes, offset: int):
    result = shift = 0
    while True:
        b = data[offset]
        offset += 1
        result |= (b & 0x7F) << shift
        if not b & 0x80:
            return result, offset
        shift += 7


def _gas_coin_contents_offset(obj: bytes) -> int:
    """Offset of a GasCoin object blob's contents (uid[32] | balance[8]).
    Layout: 0x00 0x01 | has_public_transfer | version | contents(vec) | owner | ...
    """
    assert obj[0:2] == bytes([0x00, 0x01]), "expected a Move/GasCoin object"
    o = 2 + 1 + 8  # variant | has_public_transfer | version
    contents_len, o = _read_uleb128(obj, o)
    assert contents_len == 40, "expected a 40-byte Coin contents (uid + balance)"
    return o


def _with_uid_and_balance(obj: bytes, uid: bytes, balance: int) -> bytes:
    off = _gas_coin_contents_offset(obj)
    return (obj[:off] + uid + balance.to_bytes(8, byteorder='little')
            + obj[off + 40:])


def _object_digest(obj: bytes) -> bytes:
    return hashlib.blake2b(b"Object::" + obj, digest_size=32).digest()


def _object_ref(object_id: bytes, version: bytes, obj: bytes) -> bytes:
    return object_id + version + bytes([0x20]) + _object_digest(obj)


def test_sign_tx_sui_whole_gas_coin_balance_overflow_rejected(
        backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    # Same TransferObjects{[GasCoin], Input(0)} shape and gas object as the passing
    # `test_sign_tx_sui_whole_gas_coin`.
    transaction = base64.b64decode(
        'AAAAAAABACAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEBAQABAABvsh/urQJ9pIcyla/9'
        'bE82GP4Xb6L78+e17x2UY7MeIQFADb3P7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2QhEAAAAA'
        'IGbFq2VJip03FgAaA0gV/0q8p2X39vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8d'
        'lGOzHiHoAwAAAAAAAOCXLQAAAAAAAA==')
    gas_obj = base64.b64decode(
        'AAEB03ZCEQAAAAAoQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvIYR4MtEtyGd4QDpQ5AAAAAABvsh/urQJ9'
        'pIcyla/9bE82GP4Xb6L78+e17x2UY7MeISB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2AT'
        'DwAAAAAA')

    # Two payment objects whose balances sum to u64::MAX + 1: the running total
    # reaches u64::MAX - 1 on the first and wraps to 0 on the second.
    second_object_id = GAS_OBJECT_ID[:31] + bytes([GAS_OBJECT_ID[31] ^ 0xFF])
    gas_obj_1 = _with_uid_and_balance(gas_obj, GAS_OBJECT_ID, U64_MAX - 1)
    gas_obj_2 = _with_uid_and_balance(gas_obj, second_object_id, 2)
    assert (U64_MAX - 1) + 2 == U64_MAX + 1

    # Replace gas_data.payment = [ref] with [ref(gas_obj_1), ref(gas_obj_2)].
    pos = transaction.find(GAS_OBJECT_ID)
    assert pos != -1, "could not locate the gas payment object id in transaction"
    assert transaction[pos - 1] == 0x01, "expected a single-entry gas payment vector"
    version = transaction[pos + 32:pos + 40]
    transaction = (transaction[:pos - 1]
                   + bytes([0x02])
                   + _object_ref(GAS_OBJECT_ID, version, gas_obj_1)
                   + _object_ref(second_object_id, version, gas_obj_2)
                   + transaction[pos + OBJECT_REF_LEN:])

    object_list = [gas_obj_1, gas_obj_2]

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
        pytest.fail('a gas payment total that overflows u64 must not be clear-signed '
                    'as the wrapped GasCoin transfer amount')

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0
