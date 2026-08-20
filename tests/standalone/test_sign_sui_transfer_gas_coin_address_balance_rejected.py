# Security regression for B2CA-2793 finding 4: SIP-58 empty gas payment clear-signs
# GasCoin transfers as 0 SUI.
#
# Background: for a SIP-58 transaction with an empty gas_data.payment (gas paid from
# address balance), the parser had no way to know the gas coin's real balance, so it
# seeded the running total as `Some(0)` instead of `None`. If the transaction then
# transfers (or stakes) the whole GasCoin directly, the device clear-signed a
# concrete "Amount SUI 0.000000000" -- a false, specific number for a transfer that
# actually hands over whatever the signer's real gas-coin balance turns out to be.
#
# This transaction is the exact `TransferObjects{objects:[GasCoin], address}` shape
# of the passing `test_sign_tx_sui_whole_gas_coin`, with gas_data.payment emptied
# (SIP-58, gas from address balance) and expiration switched to the required
# ValidDuring. Confirmed against the vulnerable parser (this fix reverted): the
# device displayed and signed "Amount SUI 0.000000000" via the normal review flow
# for a transaction that actually hands over the signer's entire (unknown, non-zero)
# gas balance. With the fix the device must treat this as an unrecognized tx (the
# same fail-safe already used for parse ambiguity elsewhere) rather than
# clear-signing a fabricated zero amount.

import base64

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import run_apdu_and_nav_tasks_concurrently


def test_sign_tx_sui_whole_gas_coin_address_balance_rejected(
        backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    # Same TransferObjects{[GasCoin], Input(0)} shape as `test_sign_tx_sui_whole_gas_coin`,
    # but gas_data.payment = [] (SIP-58, gas from address balance) and
    # expiration = ValidDuring (required for empty gas payment) instead of None.
    transaction = base64.b64decode(
        'AAAAAAABACAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEBAQABAABvsh/urQJ9pIcyla/9'
        'bE82GP4Xb6L78+e17x2UY7MeIQBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIegDAAAAAAAA'
        '4JctAAAAAAACAAAAACAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=')

    object_list = []  # Empty: gas from address balance, no payment objects to resolve

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
        pytest.fail('a GasCoin transfer with an unknown (SIP-58 address-balance) '
                     'gas balance must not be clear-signed as "0 SUI"')

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0
