import base64
import pytest
from hashlib import blake2b

from ledger_app_clients.exchange.test_runner import ALL_TESTS_EXCEPT_MEMO_AND_THORSWAP, ExchangeTestRunner
from application_client.client import Client, Errors
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
    signature_refusal_error_code = Errors.SUI_SWAP_TX_PARAM_MISMATCH

    partner_name = "Partner name"
    fund_user_id = "Daft Punk"
    fund_account_name = "Account 0"

    def perform_final_tx(self, destination, send_amount, fees, _memo):
        client = Client(self.backend, use_block_protocol=True)
        tx = client.build_simple_transaction(OWNED_ADDRESS, destination, send_amount, fees)
        signature = client.sign_tx("m/44'/784'/12345'", tx)

        public_key_bytes = bytes.fromhex(OWNED_PUBLIC_KEY)
        verify_signature(public_key_bytes, blake2b(tx, digest_size=32).digest(), signature)

class SuiSwapTokenTests(ExchangeTestRunner):
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
    signature_refusal_error_code = Errors.SUI_SWAP_TX_PARAM_MISMATCH

    partner_name = "Partner name"
    fund_user_id = "Daft Punk"
    fund_account_name = "Account 0"

    def perform_final_tx(self, destination, send_amount, fees, _memo):
        client = Client(self.backend, use_block_protocol=True)

        [tx, obj_list] = client.build_usdc_simple_transaction_empty_gas_payment(OWNED_ADDRESS, destination, send_amount, fees)
        signature = client.sign_tx("m/44'/784'/12345'", tx, obj_list)

        public_key_bytes = bytes.fromhex(OWNED_PUBLIC_KEY)
        verify_signature(public_key_bytes, blake2b(tx, digest_size=32).digest(), signature)

class SuiSwapDynamicTokenTests(ExchangeTestRunner):
    currency_configuration = SUI_USDC_CURRENCY_CONFIGURATION
    valid_destination_1 = FOREIGN_ADDRESS
    valid_destination_2 = FOREIGN_ADDRESS_2
    valid_refund = OWNED_ADDRESS
    valid_send_amount_1 = USDC_AMOUNT_3
    valid_send_amount_2 = USDC_AMOUNT_4
    valid_fees_1 = FEES
    valid_fees_2 = FEES_2
    fake_refund = FOREIGN_ADDRESS
    fake_payout = FOREIGN_ADDRESS
    signature_refusal_error_code = Errors.SUI_SWAP_TX_PARAM_MISMATCH

    partner_name = "Partner name"
    fund_user_id = "Daft Punk"
    fund_account_name = "Account 0"

    def perform_final_tx(self, destination, send_amount, fees, _memo):
        client = Client(self.backend, use_block_protocol=True)
    
        client.provide_dynamic_token("USDC", 6, "0x909cba62ce96d54de25bec9502de5ca7b4f28901747bbf96b76c2e63ec5f1cba", "coin", "COIN")

        [tx, obj_list] = client.build_usdc_simple_transaction_empty_gas_payment(OWNED_ADDRESS, destination, send_amount, fees)
        signature = client.sign_tx("m/44'/784'/12345'", tx, obj_list)

        public_key_bytes = bytes.fromhex(OWNED_PUBLIC_KEY)
        verify_signature(public_key_bytes, blake2b(tx, digest_size=32).digest(), signature)

class TestsSui:
    @pytest.mark.parametrize('test_to_run', ALL_TESTS_EXCEPT_MEMO_AND_THORSWAP)
    def test_sui(self, backend, exchange_navigation_helper, test_to_run):
        GenericSuiTests(backend, exchange_navigation_helper).run_test(test_to_run)

    @pytest.mark.parametrize('test_to_run', ALL_TESTS_EXCEPT_MEMO_AND_THORSWAP)
    def test_sui_tokens(self, backend, exchange_navigation_helper, test_to_run):
        SuiSwapTokenTests(backend, exchange_navigation_helper).run_test(test_to_run)

    @pytest.mark.parametrize('test_to_run', ALL_TESTS_EXCEPT_MEMO_AND_THORSWAP)
    def test_sui_dynamic_tokens(self, backend, exchange_navigation_helper, test_to_run):
        SuiSwapDynamicTokenTests(backend, exchange_navigation_helper).run_test(test_to_run)