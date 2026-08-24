# SIP-58 address-balance native-SUI transfer as @mysten/sui@2.23.1 `coinWithBalance` emits it
# (the version ledger-live coin-sui ships): the withdrawal is split and the *remainder* returned to
# the signer's own address balance, so the PTB is
#   [0] 0x2::coin::redeem_funds<SUI>(FundsWithdrawal) -> Coin
#   [1] SplitCoins(coin[0], [amount])
#   [2] 0x2::coin::send_funds<SUI>(coin[0], sender)      # change back to the signer
#   [3] TransferObjects([split], recipient)
#
# Regression for the on-device UNKNOWN_ERROR (0x8) (LIVE-35798 / TSD-11433). The earlier
# `coin::redeem_funds` fix (test_sign_sui_transfer_coin_redeem_funds.py) used a 2.9.0 fixture with no
# `send_funds`, so it missed command [2]: production 2.23.1 bytes still fell through to
# `reject_on(NotSupported)` (SW 0x0008).
#
# Fixture built offline with @mysten/sui@2.23.1 `coinWithBalance({ balance: 0.1 SUI })` sourced from
# the address balance, gas paid by an explicit owned coin (so it builds without JSON-RPC). Input(3) is
# the sender address (0x6fb2..b31e21), which is the send_funds recipient; TransferObjects sends the
# 0.1 SUI split to recipient 0xf65c..e8fb.

import base64

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently


def test_sign_tx_sui_transfer_coin_send_funds(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode(
        "AAAAAAAEACD2XHKr9SMHvBvTwZlTSq8E61bHvpauLnQnGwlQhBLo+wIAAOH1BQAAAAAABwAAAAAAAAAA"
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACA3N1aQNTVUkAAAAIAOH1BQAAAAAAIG+yH+6tAn2khzKVr/1s"
        "TzYY/hdvovvz57XvHZRjsx4hBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgRjb2luDHJl"
        "ZGVlbV9mdW5kcwEHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQABAQEAAgIA"
        "AAEBAgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIEY29pbgpzZW5kX2Z1bmRzAQcAAAAA"
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAICAAABAwABAQMBAAAAAQAAb7If7q0C"
        "faSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiEBv6wjzVqOT7ulbb+RAVs/dLooSvQAOTpBmx2Gnc/SuM8"
        "yskgZAAAAACDIki1iy4BHSQfs/MFkOEe5dZdmdZ+8Xgl40gzFjL7qYW+yH+6tAn2khzKVr/1sTzYY/h"
        "dvovvz57XvHZRjsx4h6AMAAAAAAABAS0wAAAAAAAA="
    )
    object_list = []  # Transfer sourced from the SIP-58 address balance; gas coin resolved as unknown

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[
                    NavInsID.RIGHT_CLICK,  # Transfer SUI
                    NavInsID.RIGHT_CLICK,
                    NavInsID.RIGHT_CLICK,  # From ...
                    NavInsID.RIGHT_CLICK,
                    NavInsID.RIGHT_CLICK,  # To ...
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
        assert check_signature_validity(public_key, result, transaction)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)
