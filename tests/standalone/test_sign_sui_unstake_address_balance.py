# ValidDuring (SIP-58) Sui unstake: gas from address balance, empty gas_data.payment

import base64

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently


# ValidDuring (SIP-58) unstake: gas from address balance, delegation from owned StakedSui.
# Transaction built by tools/valid-during-tx (same sender as test_sign_unstake_whole_coin).
def test_sign_unstake_valid_during_address_balance_gas(
    backend, scenario_navigator, firmware, navigator
):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    # ValidDuring unstake: gas from address balance, delegation 0xa93f6c46...
    transaction = base64.b64decode(
        "AAAAAAACAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABQEAAAAAAAAAAQEAqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq23UdkIRAAAAACCsVYpX4/44Cp2BWe8aVkACUW5rxtsErjUPJ6nMxaCvvQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMKc3VpX3N5c3RlbRZyZXF1ZXN0X3dpdGhkcmF3X3N0YWtlAAIBAAABAQAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAA8EkCAAAAAAACAQAAAAAAAAAAAQEAAAAAAAAAAAAgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAsAAAA"
    )
    # StakedSui object (0xa93f6c46...) - object at index 2 has digest matching tx
    object_list = [
        base64.b64decode(
            "AAEB1HZCEQAAAAAoqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq22Aw8kBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAfVAIamErRVJt4BuqoZFY2dBaAKAaQzrxvVjuLcgrqZmATDwAAAAAA"
        )
    ]

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[
                    NavInsID.RIGHT_CLICK,  # Review unstake
                    NavInsID.RIGHT_CLICK,
                    NavInsID.RIGHT_CLICK,  # From ...
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
