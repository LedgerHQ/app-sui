# Security regression: a TransferObjects command that moves a StakedSui object
# must NOT be clear-signed as an ordinary liquid SUI transfer.
#
# Background: the object parser used to collapse both liquid `Coin<SUI>` (40-byte
# contents) and `StakedSui` (80-byte contents) objects into the same
# `(SUI_COIN_TYPE, amount)` shape. Downstream transfer validation then treated a
# transferred StakedSui position as plain SUI. A malicious host could provide a
# transaction plus matching object data so the device would display/sign the
# movement of a staking position as if it were a normal SUI transfer (especially
# dangerous in swap signing mode, which skips the usual prompts). The parser now
# keeps the stake-vs-coin distinction and rejects a StakedSui used as a coin.
#
# This test reuses the exact transaction and objects of the passing clear-sign
# transfer `test_sign_tx_sui_whole_gas_plus_input_coin`, but turns the
# transferred input object (0x1c12...) into a *StakedSui* object and re-points the
# input's object digest at it. With the fix the device must reject.

import base64
import hashlib

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import run_apdu_and_nav_tasks_concurrently


def _build_staked_sui_from_coin(coin: bytes, principal: int) -> bytes:
    """Turn a real 40-byte-contents `Coin<SUI>` object blob into an 80-byte-contents
    `StakedSui` object blob, keeping the trailing owner/digest/rebate bytes (so it
    still parses) and changing only the MoveObjectType variant and contents.

    Object layout (BCS): ObjectData(0=Move) | MoveObjectType | has_public_transfer
    | version(u64) | contents(ULEB len + bytes) | Owner | TransactionDigest | rebate
    The parser reads the StakedSui principal from the last 8 bytes of the contents.
    """
    assert coin[0] == 0x00, "expected ObjectData::Move"
    assert coin[1] == 0x01, "expected MoveObjectType::GasCoin template"
    assert coin[11] == 0x28, "expected 40-byte coin contents"

    header = bytes([0x00, 0x02]) + coin[2:11] + bytes([0x50])  # Move, StakedSui, ..., len=80
    contents = bytearray(80)
    contents[0:32] = coin[12:44]                       # keep the original UID region
    contents[72:80] = principal.to_bytes(8, "little")  # principal read from the tail
    tail = coin[52:126]                                # owner + tx digest + storage rebate
    return header + bytes(contents) + tail


def _object_digest(obj: bytes) -> bytes:
    # Matches the device: blake2b-256 over the "Object::" salt + object bytes.
    return hashlib.blake2b(b"Object::" + obj, digest_size=32).digest()


def test_sign_tx_sui_transfer_staked_object_rejected(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    # Same transaction as the passing `test_sign_tx_sui_whole_gas_plus_input_coin`:
    # TransferObjects { objects: [GasCoin, Input 0], address: Input 1 }.
    transaction = bytearray(base64.b64decode(
        'AAAAAAACAQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAgABAAABAQAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAHi+LQAAAAAAAA=='))

    # Input 0: originally the liquid SUI coin 0x1c12...; we replace it with a StakedSui.
    coin0 = base64.b64decode(
        'AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L4YEUXWATDwAAAAAA')
    # Gas payment object 0x1f876ff0..., unchanged.
    gas_obj = base64.b64decode(
        'AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA')

    staked_obj = _build_staked_sui_from_coin(coin0, principal=30_000_000)

    # Re-point Input 0's object digest (the 0x1c12 coin digest) at the StakedSui blob.
    coin0_digest = bytes.fromhex(
        'e10131885e3b30e7cb4561d53970d55cbf694c099ec6d2c789b2f2dc92ca7ff3')
    pos = transaction.find(coin0_digest)
    assert pos != -1, "could not locate input object digest in transaction"
    transaction[pos:pos + 32] = _object_digest(staked_obj)
    transaction = bytes(transaction)

    object_list = [staked_obj, gas_obj]

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
            # Clear signing failed -> dismiss the "Enable Blind signing" screen.
            navigator.navigate([NavInsID.USE_CASE_CHOICE_REJECT],
                                screen_change_before_first_instruction=False,
                                screen_change_after_last_instruction=False)

    def check_result(result):
        pytest.fail('transferring a StakedSui object must not be clear-signed')

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0
