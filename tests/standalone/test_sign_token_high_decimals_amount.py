# A token declaring more decimal places than the amount buffer used to hold.
#
# `get_amount_in_decimals` renders the fractional part one character per decimal
# place into a fixed buffer. That buffer held 12 characters while a signed dynamic
# token descriptor may declare up to MAX_COIN_DECIMALS (19), and the digits past the
# twelfth were dropped silently -- `try_push` returns a Result that was discarded.
#
# The effect was worst for small amounts, because the fraction of one begins with
# zeroes: this transaction moves 80000 units, which at 18 decimals is
# 0.00000000000008, a fraction of 14 characters. Truncated to 12 it rendered as
#
#     USDC 0.000000000000
#
# so the device showed what reads as zero for a transfer that is not zero. The
# buffer is now sized from MAX_COIN_DECIMALS, so the fraction is always complete.
#
# Nothing about the transaction is unusual -- it is test_sign_tx_usdc_split_coin's,
# unchanged. Only the descriptor differs: it declares 18 decimals rather than 6,
# which is what an 18-decimal bridged token would legitimately supply. The golden
# snapshots are the assertion; the amount field must read USDC 0.00000000000008.

import base64

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently

PATH = "m/44'/784'/0'/0'/0'"
USDC_PACKAGE = "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7"

# SplitCoins(coin, [80000]) then TransferObjects([split], recipient).
SPLIT_AMOUNT = 80_000
TRANSACTION = base64.b64decode(
    "AAAAAAADAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGR"
    "fo0O7b01nRHCGKviMzj24EOLiWE+lIpDAAiAOAEAAAAAAAAgb7If7q0CfaSHMpWv/WxPNhj+F2+i"
    "+/Pnte8dlGOzHiECAgEAAAEBAQABAQMAAAAAAQIADy+N1J4mnaBm83Yk0zqLTtGyYtal/fK3Nb45"
    "+NfFrPUB77Y+i7giCZ99h4t6xRVFae7Oh2ZEbd1a+4VmC6+gEYeO0akdAAAAACBXcC5VVvwySa8v"
    "VlwcbyuYmJVaB0hIsTvpOzYHNqbHng8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz17gIA"
    "AAAAAAAIWDoAAAAAAAA="
)

OBJECT_LIST = [
    base64.b64decode(
        "AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAF2L6QdAAAAACjT"
        "uql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFubeGAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07R"
        "smLWpf3ytzW+OfjXxaz1ICV9oiz28QN2+VFgs3VVcob35zoaZgQf5WcAe9gWdNyWoC0UAAAAAAA="
    )
]

DECIMALS = 18
# What the review has to show: 13 zeroes then an 8, i.e. 80000 * 10^-18.
EXPECTED_FRACTION = "00000000000008"


def test_expected_rendering_is_longer_than_the_old_buffer():
    """Pin the arithmetic the golden snapshots are meant to capture, so a change to
    the fixture cannot quietly turn this into a test of a 12-character fraction."""
    fraction = str(SPLIT_AMOUNT).rjust(DECIMALS, "0").rstrip("0")
    assert fraction == EXPECTED_FRACTION
    assert len(EXPECTED_FRACTION) == 14, "fixture no longer exceeds the old 12-char cap"
    assert SPLIT_AMOUNT % 10 ** (DECIMALS - len(EXPECTED_FRACTION)) == 0


def test_sign_tx_token_18_decimals_full_fraction(
    backend, scenario_navigator, firmware, navigator
):
    client = Client(backend, use_block_protocol=True)
    client.provide_dynamic_token("USDC", DECIMALS, USDC_PACKAGE, "usdc", "USDC")

    _, public_key, _, _ = client.get_public_key(path=PATH)
    assert len(public_key) == 32

    def apdu_task():
        return client.sign_tx(
            path=PATH, transaction=TRANSACTION, object_list=OBJECT_LIST
        )

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[
                    NavInsID.RIGHT_CLICK,  # Review transfer
                    NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK,  # From ...
                    NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK,  # To ...
                    NavInsID.RIGHT_CLICK,  # Amount
                    NavInsID.RIGHT_CLICK,  # Max Gas
                    NavInsID.BOTH_CLICK,
                ],
                timeout=10,
                test_case_name=scenario_navigator.test_name,
                path=scenario_navigator.screenshot_path,
                screen_change_before_first_instruction=True,
                screen_change_after_last_instruction=False,
            )
        else:
            scenario_navigator.review_approve()

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, TRANSACTION)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)
