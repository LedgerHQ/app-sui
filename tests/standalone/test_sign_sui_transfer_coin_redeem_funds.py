# SIP-58 address-balance native-SUI transfer built the way @mysten/sui's
# `coinWithBalance` emits it: `0x2::coin::redeem_funds<SUI>(FundsWithdrawal)` -> Coin,
# then `TransferObjects([coin], recipient)`.
#
# Regression for the on-device UNKNOWN_ERROR (0x8) (LIVE-35798 / TSD-11433): before the
# `coin::redeem_funds` parser arm, this idiom fell through to `reject_on(NotSupported)`
# (SW 0x0008) because the parser only recognized the `0x2::balance::redeem_funds` /
# `send_funds` idiom. With the fix it is recognized as a plain SUI transfer.
#
# Fixture built offline with @mysten/sui@2.9.0 (the version pinned in ledger-live coin-sui):
#   tx.setGasPayment([]); coinWithBalance({ balance: 1 SUI })(tx);
#   tx.transferObjects([coin], recipient)
# The trailing 58-byte ValidDuring expiration is spliced on after `tx.build()`: a SIP-58 tx with
# an empty gas payment (gas from the address balance) is rejected by the app unless its expiration
# is ValidDuring (see parser/tx.rs "Empty gas payment requires ValidDuring expiration"). The raw
# `tx.build()` output leaves expiration = None, so on its own it would fail that check, not the
# coin::redeem_funds arm this test targets.

import base64

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently


def test_sign_tx_sui_transfer_coin_redeem_funds(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    # coin::redeem_funds(FundsWithdrawal 1 SUI) -> Coin<SUI>; transferObjects([coin], recipient)
    transaction = base64.b64decode(
        "AAAAAAACACBvsh/urQJ9pIcyla/1sTzYY/hdvovvz57de8dlGOzHiAIAAMqaOwAAAAAABwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACA3N1aQNTVUkAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIEY29pbgxyZWRlZW1fZnVuZHMBBwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACA3N1aQNTVUkAAQEBAAEBAwAAAAABAADXgJJ9qOf/ltsDmfFnOLvOll7vkz1IfwRlxTK9Hu2w0wDXgJJ9qOf/ltsDmfFnOLvOll7vkz1IfwRlxTK9Hu2w0+gDAAAAAAAAAOH1BQAAAAACAQAAAAAAAAAAAQEAAAAAAAAAAAAgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAqAAAA"
    )
    object_list = []  # Empty: transfer + gas both sourced from the SIP-58 address balance

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
