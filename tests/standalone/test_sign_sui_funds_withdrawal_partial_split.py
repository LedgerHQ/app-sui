# Security regression for B2CA-2793 finding 6: SIP-58 withdrawal_split ignores the
# split amount, clear-signs the full reservation.
#
# Background: `0x2::funds_accumulator::withdrawal_split(reservation, amount)` returns
# only `amount` (argument 1), a portion of the upper-bound `reservation` carried by
# the FundsWithdrawal input (argument 0). The parser used to read only the
# reservation and ignore argument 1's actual value entirely, so a transaction could
# reserve B but split/send only A < B while the device displayed and swap-checked
# the larger, never-validated B.
#
# This transaction reuses the exact structure of the passing
# `test_sign_tx_sui_funds_withdrawal` (which reserves and splits the same amount,
# 612_000 MIST) but changes only the split argument (Input 1) to 400_000 MIST --
# less than the 612_000 reservation. Confirmed against the vulnerable parser (this
# fix reverted): the device displayed and signed "Amount SUI 0.000612" (the
# reservation) while only 400_000 MIST is actually sent. With the fix the device
# must display and sign the true, smaller split amount instead.

import base64

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently


def test_sign_tx_sui_funds_withdrawal_partial_split(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    # Same as test_sign_tx_sui_funds_withdrawal, but Input 1 (the withdrawal_split
    # amount) is 400_000 MIST instead of 612_000 -- strictly less than the
    # reservation carried by Input 0 (FundsWithdrawal amount 612_000).
    transaction = base64.b64decode(
        "AAAAAAADAgCgVgkAAAAAAAAHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQAAAAiAGgYAAAAAAAAgHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACEWZ1bmRzX2FjY3VtdWxhdG9yEHdpdGhkcmF3YWxfc3BsaXQBBwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACB2JhbGFuY2UHQmFsYW5jZQEHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQACAQAAAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACB2JhbGFuY2UMcmVkZWVtX2Z1bmRzAQcAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAECAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIHYmFsYW5jZQpzZW5kX2Z1bmRzAQcAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAICAQABAgBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQFADb3P7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2QhEAAAAAIGbFq2VJip03FgAaA0gV/0q8p2X39vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiHoAwAAAAAAACChBwAAAAAAAA=="
    )
    # Gas object for payment (same as test_sign_tx_sui_funds_withdrawal)
    object_list = [
        base64.b64decode(
            "AAEB03ZCEQAAAAAoQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvIYR4MtEtyGd4QDpQ5AAAAAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeISB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA"
        )
    ]

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
                    NavInsID.RIGHT_CLICK,  # Amount -- must read 0.0004, not 0.000612
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
