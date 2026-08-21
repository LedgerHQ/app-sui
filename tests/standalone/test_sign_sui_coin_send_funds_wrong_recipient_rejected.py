# Security regression for the SIP-58 `coin::send_funds` arm (LIVE-35798 / TSD-11433).
#
# The displayed amount/recipient come only from TransferObjects; the `send_funds` change coin is
# invisible to the UI (and to swap param-matching). A hostile host could therefore redeem the
# signer's whole address balance, transfer a dust amount to the shown recipient, and route the large
# `send_funds` remainder to itself:
#   redeem_funds(all) -> split(dust) -> send_funds(remainder -> ATTACKER) -> transferObjects(dust -> recipient)
#
# The `coin::send_funds` arm records the send_funds recipient and TransactionDataParser rejects unless
# it == the transaction sender, so the change provably returns to the signer. Rejecting routes the tx
# to the not-recognized/blind-sign path (and hard-rejects in swap mode) instead of clear-signing it,
# so the misleading "dust -> recipient" review is never shown.
#
# Fixture: the passing coin::send_funds transfer (test_sign_sui_transfer_coin_send_funds.py) with
# Input(3) — the send_funds recipient — patched from the sender (0x6fb2..b31e21) to 0xaa..aa; the
# TransactionData `sender` field is left untouched.

import base64

import pytest
from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import run_apdu_and_nav_tasks_concurrently


def test_sign_tx_sui_coin_send_funds_wrong_recipient_rejected(
    backend, scenario_navigator, firmware, navigator
):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode(
        "AAAAAAAEACD2XHKr9SMHvBvTwZlTSq8E61bHvpauLnQnGwlQhBLo+wIAAOH1BQAAAAAABwAAAAAAAAAA"
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACA3N1aQNTVUkAAAAIAOH1BQAAAAAAIKqqqqqqqqqqqqqqqqqq"
        "qqqqqqqqqqqqqqqqqqqqqqqqBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgRjb2luDHJl"
        "ZGVlbV9mdW5kcwEHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQABAQEAAgIA"
        "AAEBAgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIEY29pbgpzZW5kX2Z1bmRzAQcAAAAA"
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAICAAABAwABAQMBAAAAAQAAb7If7q0C"
        "faSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiEBv6wjzVqOT7ulbb+RAVs/dLooSvQAOTpBmx2Gnc/SuM8"
        "yskgZAAAAACDIki1iy4BHSQfs/MFkOEe5dZdmdZ+8Xgl40gzFjL7qYW+yH+6tAn2khzKVr/1sTzYY/h"
        "dvovvz57XvHZRjsx4h6AMAAAAAAABAS0wAAAAAAAA="
    )
    object_list = []

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[
                    NavInsID.RIGHT_CLICK,
                    NavInsID.RIGHT_CLICK,
                    NavInsID.RIGHT_CLICK,
                    NavInsID.BOTH_CLICK,
                ],
                timeout=10,
                test_case_name=scenario_navigator.test_name,
                path=scenario_navigator.screenshot_path,
                screen_change_before_first_instruction=True,
                screen_change_after_last_instruction=False,
            )
        else:
            # "This transaction cannot be clear-signed" -> choose "Reject transaction".
            navigator.navigate(
                [NavInsID.USE_CASE_CHOICE_REJECT],
                screen_change_before_first_instruction=False,
                screen_change_after_last_instruction=False,
            )

    def check_result(result):
        pytest.fail("a coin::send_funds change to a non-sender address must not be clear-signed")

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)
    assert len(e.value.data) == 0
