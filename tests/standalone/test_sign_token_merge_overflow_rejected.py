# Security regression for B2CA-2793 follow-up finding 1 (CWE-190): unchecked u64
# aggregation must not wrap the clear-signed amount below what the transaction moves.
#
# Background: `handle_merge_coins` accumulated the destination balance and every
# merged source with plain `+=`. Both cargo profiles set `overflow-checks = false`,
# so that addition wraps modulo 2^64 instead of trapping, and the wrapped total was
# written back as the destination coin's balance. A later transfer of that coin then
# displays -- and, in swap mode with no UI at all, compares against the authenticated
# quote -- an amount far below the value actually moved. The accumulation is now
# checked and any overflow is rejected as an ambiguity.
#
# This is `test_sign_tx_usdc_merge_three` (MergeCoins{dest: Input 0, sources:
# [Input 1, Input 2]} then TransferObjects{[Input 0]}) with the three USDC object
# balances rewritten so the merged total crosses u64::MAX by exactly one unit, and
# the transferred input's object digests re-pointed at the modified objects.
# Confirmed against the vulnerable parser (this fix reverted): the device displayed
# and signed "Amount USDC 0.0" for a transaction that merges and hands over
# 18446744073709551617 base units. With the fix the device must treat this as an
# unrecognized tx instead.

import base64
import hashlib

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import run_apdu_and_nav_tasks_concurrently

U64_MAX = 2 ** 64 - 1


def _read_uleb128(data: bytes, offset: int):
    result = shift = 0
    while True:
        b = data[offset]
        offset += 1
        result |= (b & 0x7F) << shift
        if not b & 0x80:
            return result, offset
        shift += 7


def _with_balance(obj: bytes, balance: int) -> bytes:
    """Rewrite the 8-byte LE balance of a Coin object blob, keeping every other byte
    intact. Layout: 0x00 0x03 0x07 | addr[32] | module(vec) | struct(vec) |
    type_params(vec) | has_public_transfer | version | contents(vec: uid[32] |
    balance[8]) | owner | digest | rebate.
    """
    assert obj[0:3] == bytes([0x00, 0x03, 0x07]), "expected Move/Coin/Struct object"
    o = 3 + 32
    mlen, o = _read_uleb128(obj, o)
    o += mlen
    nlen, o = _read_uleb128(obj, o)
    o += nlen
    params, o = _read_uleb128(obj, o)
    assert params == 0, "expected a non-generic coin type"
    o += 1  # has_public_transfer
    o += 8  # version
    contents_len, o = _read_uleb128(obj, o)
    assert contents_len == 40, "expected a 40-byte Coin contents (uid + balance)"
    off = o + 32
    return obj[:off] + balance.to_bytes(8, byteorder='little') + obj[off + 8:]


def _object_digest(obj: bytes) -> bytes:
    return hashlib.blake2b(b"Object::" + obj, digest_size=32).digest()


def test_sign_tx_usdc_merge_overflow_rejected(
        backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    client.provide_dynamic_token(
        "USDC", 6,
        "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7",
        "usdc", "USDC")

    # Same transaction/objects as the passing `test_sign_tx_usdc_merge_three`.
    transaction = bytearray(base64.b64decode(
        'AAAAAAAEAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDAQCLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRTUyph0AAAAAID6RSJBTwuLCyFHo8Wr624IT/QLOofOgJMYDmGxMFJY3AQCOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0uZ5pB0AAAAAIOQ/CZZSDz1bJFP7ynAKkMqm7brpDXAdHJsO5qFAXW6cACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQIDAQAAAgEBAAECAAEBAQAAAQMADy+N1J4mnaBm83Yk0zqLTtGyYtal/fK3Nb45+NfFrPUB77Y+i7giCZ99h4t6xRVFae7Oh2ZEbd1a+4VmC6+gEYeO0akdAAAAACBXcC5VVvwySa8vVlwcbyuYmJVaB0hIsTvpOzYHNqbHng8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz17gIAAAAAAABg4xYAAAAAAAA='))
    coin_objs = [
        base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAF2L6QdAAAAACjTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFubeGAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1ICV9oiz28QN2+VFgs3VVcob35zoaZgQf5WcAe9gWdNyWoC0UAAAAAAA='),
        base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAE1MqYdAAAAACiLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRQHiAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IOV7R/YfpK7xICsKift4S9G6tE2+t4MyPAX4gSGmkRIYoC0UAAAAAAA='),
        base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAHmeaQdAAAAACiOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0gHbAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IK02+4bxem3JcKC41NNAanTDoQBzwHsLO6uVhtAiJfqCoC0UAAAAAAA='),
    ]

    # Destination holds u64::MAX - 1; the two merged sources add 1 each, so the
    # parser's running total reaches u64::MAX and then wraps to 0 on the last one.
    balances = [U64_MAX - 1, 1, 1]
    assert sum(balances) == U64_MAX + 1

    object_list = []
    for coin_obj, balance in zip(coin_objs, balances):
        patched = _with_balance(coin_obj, balance)
        original_digest = _object_digest(coin_obj)
        pos = transaction.find(original_digest)
        assert pos != -1, "could not locate object digest in transaction"
        transaction[pos:pos + 32] = _object_digest(patched)
        object_list.append(patched)
    transaction = bytes(transaction)

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
        pytest.fail('a MergeCoins total that overflows u64 must not be clear-signed '
                    'as the wrapped amount')

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0
