# Security/regression test for a bug found in code review of B2CA-2793 findings 3/5:
# the GasCoin net-adjustment subtracted the SplitCoins delta before the real
# gas-coin balance was known, so a bare split off the gas coin (with no
# compensating MergeCoins) spuriously underflowed and rejected an otherwise
# valid, common transaction pattern -- forcing an unnecessary blind-sign
# fallback instead of the accurate clear-signing findings 3/5 were meant to
# enable.
#
# This transaction is built from the exact objects and structure of the
# passing `test_sign_tx_sui_whole_gas_coin` (transfer the whole GasCoin,
# real balance 0.96600424 SUI), with one command inserted: split 0.003 SUI
# off the GasCoin first (kept by the sender as unspent change, Input(1) ->
# NestedResult(0, 0), unused), then transfer the (now reduced) GasCoin
# itself -- no MergeCoins into GasCoin anywhere in this transaction.
#
# Confirmed against the vulnerable parser (this fix reverted): the device
# rejected the transaction outright (checked_sub(0, 3_000_000) underflows
# because the real gas balance hadn't been added in yet at that point), even
# though the transaction is perfectly valid. With the fix, the device must
# clear-sign it and display the correct reduced amount, 0.96300424 SUI
# (0.96600424 - 0.003), not the stale, pre-split balance.

import base64

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently


def test_sign_tx_sui_transfer_gas_coin_split_reused(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    # Same sender/recipient/gas-payment-coin as the passing
    # `test_sign_tx_sui_whole_gas_coin`, but with an extra SplitCoins command:
    # inputs = [recipient, Pure(amount=0.003 SUI)]
    # commands = [SplitCoins(GasCoin, [Input(1)]), TransferObjects([GasCoin], Input(0))]
    transaction = base64.b64decode(
        'AAAAAAACACAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAAIwMYtAAAAAAACAgABAQEAAQEA'
        'AQAAb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiEBQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvI'
        'YR4MtEtyGd7TdkIRAAAAACBmxatlSYqdNxYAGgNIFf9KvKdl9/byN1zJHSrdt5wgim+yH+6tAn2khzKV'
        'r/1sTzYY/hdvovvz57XvHZRjsx4h6AMAAAAAAADgly0AAAAAAAA='
    )
    object_list = [base64.b64decode(
        'AAEB03ZCEQAAAAAoQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvIYR4MtEtyGd4QDpQ5AAAAAABvsh/urQJ9'
        'pIcyla/9bE82GP4Xb6L78+e17x2UY7MeISB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2AT'
        'DwAAAAAA'
    )]

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[NavInsID.RIGHT_CLICK  # Transfer SUI
                              , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK  # From ...
                              , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK  # To ...
                              , NavInsID.RIGHT_CLICK  # Amount -- must read 0.96300424, not 0.96600424
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
