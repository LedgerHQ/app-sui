# A sponsored transaction that splits value off the gas coin must not be
# clear-signed.
#
# In a sponsored transaction the gas coin belongs to the sponsor (this device)
# while the other inputs belong to the sender. The parser tracks aggregate amounts
# rather than per-coin ownership, and applies its gas-coin deltas only when the
# reviewed command itself involves the gas coin. So SplitCoins(GasCoin, X) followed
# by a merge into a sender-owned coin moves X out of this device's coin with nothing
# on screen to account for it: the review shows an unrelated transfer plus max gas.
#
# Until coin ownership is modelled end to end, the app fails closed on the
# combination. That routes to the not-recognized path, so with blind signing off the
# command is refused; sponsorship itself keeps working, and so do non-sponsored
# splits and merges (test_sign_sui_transfer_1 / _3 cover those).
#
# The fixture is test_sign_tx_sui_split_gas_plus_input_coin's transaction with
# GasData.owner rewritten to the address of the signing path
# (m/44'/784'/0'/0'/1' = f65c72ab..., confirmed against the device). Its PTB is
# SplitCoins(GasCoin, 100) then TransferObjects([split, Input1], Input2) — a benign
# instance of the refused class, which is the point: the device cannot tell it apart
# from the hostile one, so it declines both.

import base64

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import run_apdu_and_nav_tasks_concurrently

PATH = "m/44'/784'/0'/0'/1'"
LEDGER_ADDRESS = "f65c72abf52307bc1bd3c199534aaf04eb56c7be96ae2e74271b09508412e8fb"
SPONSORED_SENDER = "1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488"

SPONSORED_SPLIT_TX = base64.b64decode(
    "AAAAAAADAAhkAAAAAAAAAAEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAA"
    "ACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAgb7If7q0CfaSHMpWv/WxPNhj+F2+i"
    "+/Pnte8dlGOzHiECAgABAQAAAQIDAAAAAAEBAAECAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96p"
    "cVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOw"
    "FR6rAXb/lQHgSrSeABJKW2+uGgXzUbr2XHKr9SMHvBvTwZlTSq8E61bHvpauLnQnGwlQhBLo++gD"
    "AAAAAAAA2NE8AAAAAAAA"
)

OBJECT_LIST = [
    base64.b64decode(
        "AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZD"
        "MFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L"
        "4YEUXWATDwAAAAAA"
    ),
    base64.b64decode(
        "AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZD"
        "MFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9a"
        "sLM9x2ATDwAAAAAA"
    ),
]


def _sanity_check_fixture():
    """The fixture must carry both halves of the refused combination."""
    tail = SPONSORED_SPLIT_TX[-49:]  # owner(32) || price(8) || budget(8) || expiration(1)
    assert tail[:32].hex() == LEDGER_ADDRESS, "GasData.owner is not the Ledger address"
    assert bytes.fromhex(SPONSORED_SENDER) in SPONSORED_SPLIT_TX, "sender was lost"
    # 0x02 == Command::SplitCoins, followed by Argument::GasCoin (0x00).
    assert b"\x02\x02\x00\x01\x01\x00" in SPONSORED_SPLIT_TX, "SplitCoins(GasCoin) missing"


def test_sponsored_gas_coin_split_not_clear_signed(
    backend, scenario_navigator, firmware, navigator
):
    _sanity_check_fixture()
    client = Client(backend, use_block_protocol=True)

    def apdu_task():
        return client.sign_tx(
            path=PATH, transaction=SPONSORED_SPLIT_TX, object_list=OBJECT_LIST
        )

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
        pytest.fail(
            "a sponsored transaction that splits the gas coin must not be "
            "clear-signed: the split value leaves this device's coin with nothing "
            "on screen to account for it"
        )

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0
