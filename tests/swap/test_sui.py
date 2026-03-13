import pytest
from hashlib import blake2b

from ledger_app_clients.exchange.test_runner import ALL_TESTS_EXCEPT_MEMO_AND_THORSWAP, ExchangeTestRunner
from application_client.sui import SuiClient, ErrorType
from cal_helper import SUI_CURRENCY_CONFIGURATION, SUI_USDC_CURRENCY_CONFIGURATION
from application_client.sui_utils import *

# ExchangeTestRunner implementation for Sui
class GenericSuiTests(ExchangeTestRunner):
    currency_configuration = SUI_CURRENCY_CONFIGURATION
    valid_destination_1 = FOREIGN_ADDRESS
    valid_destination_2 = FOREIGN_ADDRESS_2
    valid_refund = OWNED_ADDRESS
    valid_send_amount_1 = AMOUNT
    valid_send_amount_2 = AMOUNT_2
    valid_fees_1 = FEES
    valid_fees_2 = FEES_2
    fake_refund = FOREIGN_ADDRESS
    fake_payout = FOREIGN_ADDRESS
    signature_refusal_error_code = ErrorType.SUI_SWAP_TX_PARAM_MISMATCH[0]

    partner_name = "Partner name"
    fund_user_id = "Daft Punk"
    fund_account_name = "Account 0"

    def perform_final_tx(self, destination, send_amount, fees, _memo):
        sui = SuiClient(self.backend, verbose=False)
        tx = sui.build_simple_transaction(OWNED_ADDRESS, destination, send_amount, fees)
        signature = sui.sign_transaction(SUI_PACKED_DERIVATION_PATH_LE, tx)

        public_key_bytes = bytes.fromhex(OWNED_PUBLIC_KEY)
        verify_signature(public_key_bytes, blake2b(tx, digest_size=32).digest(), signature)

class SuiSwaptTokenTests(ExchangeTestRunner):
    currency_configuration = SUI_USDC_CURRENCY_CONFIGURATION
    valid_destination_1 = FOREIGN_ADDRESS
    valid_destination_2 = FOREIGN_ADDRESS_2
    valid_refund = OWNED_ADDRESS
    valid_send_amount_1 = USDC_AMOUNT
    valid_send_amount_2 = USDC_AMOUNT_2
    valid_fees_1 = FEES
    valid_fees_2 = FEES_2
    fake_refund = FOREIGN_ADDRESS
    fake_payout = FOREIGN_ADDRESS
    signature_refusal_error_code = ErrorType.SUI_SWAP_TX_PARAM_MISMATCH[0]

    partner_name = "Partner name"
    fund_user_id = "Daft Punk"
    fund_account_name = "Account 0"

    def perform_final_tx(self, destination, send_amount, fees, _memo):
        sui = SuiClient(self.backend, verbose=False)
        [tx, obj_list] = sui.build_simple_transaction_with_object_list(OWNED_ADDRESS, destination, send_amount, fees)
        signature = sui.sign_transaction(SUI_PACKED_DERIVATION_PATH_LE, tx, obj_list)

        public_key_bytes = bytes.fromhex(OWNED_PUBLIC_KEY)
        verify_signature(public_key_bytes, blake2b(tx, digest_size=32).digest(), signature)


class TestsSui:
    @pytest.mark.parametrize('test_to_run', ALL_TESTS_EXCEPT_MEMO_AND_THORSWAP)
    def test_sui(self, backend, exchange_navigation_helper, test_to_run):
        GenericSuiTests(backend, exchange_navigation_helper).run_test(test_to_run)

    @pytest.mark.parametrize('test_to_run', ALL_TESTS_EXCEPT_MEMO_AND_THORSWAP)
    def test_sui_tokens(self, backend, exchange_navigation_helper, test_to_run):
        SuiSwaptTokenTests(backend, exchange_navigation_helper).run_test(test_to_run)
