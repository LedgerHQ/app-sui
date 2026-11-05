# Mainnet hBTC (Hashi's Coin<package::btc::BTC>) is in KNOWN_COINS: a transfer of it
# reviews as "hBTC" with 8 decimals without a dynamic token descriptor. Without the
# entry the review shows the raw coin type and amount instead, so the goldens tell
# the two apart.

from application_client.client import Client
from application_client.sui_utils import (
    FEES,
    FOREIGN_ADDRESS,
    HBTC_AMOUNT,
    HBTC_MAINNET_PACKAGE,
    OWNED_ADDRESS,
    btc_coins_by_amount,
)
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently

PATH = "m/44'/784'/12345'"


def test_sign_hbtc_mainnet_known_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    _, public_key, _, address = client.get_public_key(path=PATH)
    assert "0x" + address.hex() == OWNED_ADDRESS

    transaction, object_list = client.build_usdc_simple_transaction_empty_gas_payment(
        OWNED_ADDRESS, FOREIGN_ADDRESS, HBTC_AMOUNT, FEES,
        btc_coins_by_amount(HBTC_MAINNET_PACKAGE),
    )

    def apdu_task():
        return client.sign_tx(path=PATH, transaction=transaction, object_list=object_list)

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
