# Security regression for B2CA-2793 follow-up finding 3: generic Sui coin type
# parameters must not be erased during clear-signing.
#
# Background: a Sui type identity is the full `address::module::name<type_params...>`,
# but CoinType only holds (address, module, name). struct_tag_parser parsed the
# StructTag's type parameter vector and then discarded it, so `0xP::m::Token<A>` and
# `0xP::m::Token<B>` collapsed to a single CoinType. Everything downstream compares
# that truncated identity: the KNOWN_COINS table, the UI ticker, the signed dynamic
# token descriptor and (with no UI at all) swap's check_tx_params -- so the device
# could clear-sign a transfer of one generic asset while displaying/validating
# another. struct_tag_parser now rejects any StructTag carrying type parameters,
# because their identity cannot be represented faithfully.
#
# This test reuses the passing clear-sign transfer `test_sign_tx_unrecognized_coin`
# (token 0x90e8c5..::killa::KILLA) and turns its coin object into the generic
# instantiation `killa::KILLA<0x2::sui::SUI>`, re-pointing the transferred input's
# object digest at the modified object. On the vulnerable parser the type parameter
# is skipped and the object yields the very same CoinType as the plain
# `killa::KILLA`, so the transfer is clear-signed exactly as the base test's is.
# With the fix the device must refuse to clear-sign it.

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


def _sui_struct_type_tag() -> bytes:
    """TypeTag::Struct(0x2::sui::SUI), the encoding used inside a StructTag's
    type parameter vector: variant | addr[32] | module(vec) | name(vec) | params(vec).
    """
    addr = bytes(31) + bytes([0x02])
    return (bytes([0x07]) + addr
            + bytes([3]) + b"sui"
            + bytes([3]) + b"SUI"
            + bytes([0x00]))


def _with_type_param(obj: bytes, type_param: bytes) -> bytes:
    """Rebuild a Coin object blob (ObjectData::Move | Coin | TypeTag::Struct | StructTag
    | ...) giving its StructTag a single type parameter, keeping every other byte
    intact so it still parses. Layout: 0x00 0x03 0x07 | addr[32] | module(vec) |
    struct(vec) | type_params(vec) | has_public_transfer | version | contents | owner |
    digest | rebate.
    """
    assert obj[0:3] == bytes([0x00, 0x03, 0x07]), "expected Move/Coin/Struct object"
    o = 3
    addr = obj[o:o + 32]
    o += 32
    mlen, o = _read_uleb128(obj, o)
    module = obj[o:o + mlen]
    o += mlen
    nlen, o = _read_uleb128(obj, o)
    struct_name = obj[o:o + nlen]
    o += nlen
    params, o = _read_uleb128(obj, o)
    assert params == 0, "expected a non-generic coin type to start from"
    tail = obj[o:]
    return (bytes([0x00, 0x03, 0x07]) + addr
            + bytes([mlen]) + module
            + bytes([nlen]) + struct_name
            + bytes([0x01]) + type_param  # type_params: one entry
            + tail)


def _object_digest(obj: bytes) -> bytes:
    return hashlib.blake2b(b"Object::" + obj, digest_size=32).digest()


def test_sign_tx_token_generic_type_param_rejected(
        backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    # Same transaction/object as the passing `test_sign_tx_unrecognized_coin`.
    transaction = bytearray(base64.b64decode(
        'AAAAAAACAQAkOlErOjssUas7B1ipByHf2etJJYdwBbMTSEy5doj0VgHzIR4AAAAAIN8vTwL8rbbLzRfsdy1PyOXvcrij9n34ovKk2/o0N3wNACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAG02HZIA+4tmm1GrxPh4dKNj3Fry/X/O0WxUh1ovpxisAQkHEsRdw5dbbdY9esFx0S8xZ3rE61Q5gJ3SV2OdlnYVFxwwHgAAAAAgDI4TkhHnVDhJiSloJl/c9O1pBEyKpv0JUSJ/mmyKbuVtNh2SAPuLZptRq8T4eHSjY9xa8v1/ztFsVIdaL6cYrOkCAAAAAAAA8AMmAAAAAAAA'))
    coin_obj = base64.b64decode(
        'AAMHkOjF9XYq+mdulvsw4s0Mm/3IQNQy7OWHk/VPQtmYGwkFa2lsbGEFS0lMTEEAAQHzIR4AAAAAKCQ6USs6OyxRqzsHWKkHId/Z60klh3AFsxNITLl2iPRW5Rb4fgVVAAAAbTYdkgD7i2abUavE+Hh0o2PcWvL9f87RbFSHWi+nGKwgeQhHBdsHFvOvjCyMxNjkg1Ue4ypBA1B5GpIVylbqy2UAaRQAAAAAAA==')

    # 0x90e8c5..::killa::KILLA<0x2::sui::SUI>: a distinct on-chain type that the
    # vulnerable parser reduces to the same CoinType as 0x90e8c5..::killa::KILLA.
    generic_obj = _with_type_param(coin_obj, _sui_struct_type_tag())

    original_digest = bytes.fromhex(
        'df2f4f02fcadb6cbcd17ec772d4fc8e5ef72b8a3f67df8a2f2a4dbfa34377c0d')
    pos = transaction.find(original_digest)
    assert pos != -1, "could not locate transferred object digest in transaction"
    transaction[pos:pos + 32] = _object_digest(generic_obj)
    transaction = bytes(transaction)

    object_list = [generic_obj]

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
        pytest.fail('a coin type with type parameters must not be clear-signed under '
                    'an identity that erases them')

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0
