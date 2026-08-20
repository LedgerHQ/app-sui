# SIP-58 FundsWithdrawal: transfer SUI from address balance to recipient.
# Transaction built by tools/valid-during-tx (build_funds_withdrawal_sui_transfer).
# Same sender/recipient/gas as test_sign_sui_transfer_1.py::test_sign_tx_sui_whole_gas_coin.

import base64

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently


def test_sign_tx_sui_funds_withdrawal(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    # FundsWithdrawal transfer: 612_000 MIST from sender address balance to recipient.
    # PTB with CallArg::FundsWithdrawal in inputs.
    # Generate: tools/valid-during-tx (build_tx_sui_funds_withdrawal)
    #
    # withdrawal_split's second argument (Input 1) is fixed to the correct BCS
    # encoding here: a plain 8-byte u64 Pure equal to the split amount (612_000,
    # matching the reservation), not the original tool output's 32-byte Pure with
    # the same value zero-padded. That 32-byte encoding was never actually a valid
    # `u64` and only "worked" because the parser used to ignore this argument's
    # value entirely (B2CA-2793 finding 6); now that it's validated against the
    # reservation, it must be shaped like the real Move `u64` parameter it is.
    transaction = base64.b64decode(
        "AAAAAAADAgCgVgkAAAAAAAAHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQAAAAigVgkAAAAAAAAgHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACEWZ1bmRzX2FjY3VtdWxhdG9yEHdpdGhkcmF3YWxfc3BsaXQBBwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACB2JhbGFuY2UHQmFsYW5jZQEHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQACAQAAAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACB2JhbGFuY2UMcmVkZWVtX2Z1bmRzAQcAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAECAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIHYmFsYW5jZQpzZW5kX2Z1bmRzAQcAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAICAQABAgBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQFADb3P7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2QhEAAAAAIGbFq2VJip03FgAaA0gV/0q8p2X39vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiHoAwAAAAAAACChBwAAAAAAAA=="
    )
    # Gas object for payment (same as test_sign_tx_sui_whole_gas_coin)
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
