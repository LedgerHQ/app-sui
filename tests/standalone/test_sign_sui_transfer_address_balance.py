# ValidDuring (SIP-58) Sui transfer: gas from address balance, empty gas_data.payment

import base64

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently


# ValidDuring (SIP-58) transfer: gas from address balance, empty gas_data.payment.
# Transaction built by tools/valid-during-tx (same sender as test_sign_tx_sui_whole_gas_coin).
def test_sign_tx_sui_valid_during_address_balance_gas(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    # ValidDuring transfer: 1 SUI to recipient, gas from address balance (transfer_sui pattern)
    transaction = base64.b64decode(
        "AAAAAAACACAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAAIQEIPAAAAAAACAgABAQEAAQECAAABAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIegDAAAAAAAA8EkCAAAAAAACAQAAAAAAAAAAAQEAAAAAAAAAAAAgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAqAAAA"
    )
    object_list = []  # Empty: gas from address balance

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_until_text_and_compare(
                NavInsID.RIGHT_CLICK,
                [NavInsID.BOTH_CLICK],
                "Sign transaction",
                scenario_navigator.screenshot_path,
                scenario_navigator.test_name,
                screen_change_after_last_instruction=False,
            )
        else:
            scenario_navigator.review_approve()

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, transaction)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)
