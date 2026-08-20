# Security regression for B2CA-2793 finding 3: SplitCoins leaves a stale source
# balance.
#
# Background: SplitCoins subtracts the split amounts from its source coin on the
# real network, but the parser only recorded the newly created outputs -- the
# source's own remaining balance was never updated. If that same source coin is
# referenced again later (e.g. transferred), the device re-read its pre-split
# balance from the object data, overstating what the signed transaction actually
# delivers to the recipient.
#
# This transaction is built from the exact objects and structure of the passing
# `test_sign_tx_sui_whole_input_coin` (transfer coin 0x1c12..., balance 0.01 SUI),
# with one command inserted: split 0.003 SUI off that coin first, then transfer
# the same coin (now reduced to 0.007 SUI) instead of the untouched original.
# Confirmed against the vulnerable parser (this fix reverted): the device
# displayed and signed "Amount SUI 0.01" -- the stale, pre-split balance -- while
# only 0.007 SUI is actually delivered to the recipient (the other 0.003 SUI stays
# with the sender as a new, untransferred coin). With the fix the device must
# display and sign the true, reduced amount instead.

import base64

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently


def test_sign_tx_sui_transfer_split_source_reused(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    # Same coin (0x1c12..., 0.01 SUI) and gas payment as the passing
    # `test_sign_tx_sui_whole_input_coin`, but with an extra SplitCoins command:
    # inputs = [coin, Pure(amount=0.003 SUI), Pure(recipient)]
    # commands = [SplitCoins(Input(0), [Input(1)]), TransferObjects([Input(0)], Input(2))]
    transaction = base64.b64decode(
        'AAAAAAADAQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh'
        '1Tlw1Vy/aUwJnsbSx4my8tySyn/zAAjAxi0AAAAAAAAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8d'
        'lGOzHiECAgEAAAEBAQABAQEAAAECAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AU'
        'Q4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJK'
        'W2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAeL4tAAAAAAAA')

    object_list = [base64.b64decode(
        'AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZDMFdg'
        'Im5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L4YEUXWAT'
        'DwAAAAAA'),
        base64.b64decode(
            'AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZD'
            'MFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9a'
            'sLM9x2ATDwAAAAAA'),
    ]

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[NavInsID.RIGHT_CLICK  # Transfer SUI
                              , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK  # From ...
                              , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK  # To ...
                              , NavInsID.RIGHT_CLICK  # Amount -- must read 0.007, not 0.01
                              , NavInsID.RIGHT_CLICK  # Max Gas
                              , NavInsID.BOTH_CLICK
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
