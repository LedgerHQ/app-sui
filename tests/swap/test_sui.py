import base64
import pytest
from hashlib import blake2b

from ledger_app_clients.exchange.test_runner import ALL_TESTS_EXCEPT_MEMO_AND_THORSWAP, ExchangeTestRunner
from ragger.error import ExceptionRAPDU
from application_client.client import Client, Errors
from cal_helper import (
    SUI_CURRENCY_CONFIGURATION,
    SUI_USDC_CURRENCY_CONFIGURATION,
    SUI_UNKNOWN_CURRENCY_CONFIGURATION,
)
from application_client.sui_utils import *

# Coin type backing the USDC token objects reused by the dynamic-descriptor tests.
USDC_DYNAMIC_TOKEN_ADDRESS = "0x909cba62ce96d54de25bec9502de5ca7b4f28901747bbf96b76c2e63ec5f1cba"
USDC_DYNAMIC_TOKEN_MODULE = "coin"
USDC_DYNAMIC_TOKEN_STRUCT = "COIN"

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

# Security regression: a swap whose ticker is unknown (not in KNOWN_COINS) and for
# which no dynamic token descriptor is loaded must NOT be satisfiable by a liquid SUI
# transfer. Previously coin_type_from_ticker fell back to SUI for unknown tickers, so a
# host-supplied SUI transfer with the expected amount/fee/destination passed the swap
# check and got signed without any prompt. The negative path is asserted explicitly in
# TestsSui.test_sui_unknown_token_rejects_sui below.
class SuiSwapUnknownTokenRejectsSuiTests(ExchangeTestRunner):
    currency_configuration = SUI_UNKNOWN_CURRENCY_CONFIGURATION
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
        # Malicious host substitutes a liquid SUI transfer (matching amount/fee/dest)
        # for an unknown-token swap. The device must reject on coin type mismatch.
        tx = client.build_simple_transaction(OWNED_ADDRESS, destination, send_amount, fees)
        client.sign_tx("m/44'/784'/12345'", tx)


# Positive counterpart: the same unknown ticker IS accepted once a matching signed
# dynamic token descriptor is provided, proving the rejection above is due to the
# ticker/coin-type check and not an incidental failure.
class SuiSwapUnknownTokenDynamicTests(ExchangeTestRunner):
    currency_configuration = SUI_UNKNOWN_CURRENCY_CONFIGURATION
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

        # Dynamic descriptor maps the unknown ticker "TADA" to the token objects' coin type.
        client.provide_dynamic_token("TADA", 6, USDC_DYNAMIC_TOKEN_ADDRESS,
                                     USDC_DYNAMIC_TOKEN_MODULE, USDC_DYNAMIC_TOKEN_STRUCT)

        [tx, obj_list] = client.build_usdc_simple_transaction_empty_gas_payment(OWNED_ADDRESS, destination, send_amount, fees)
        signature = client.sign_tx("m/44'/784'/12345'", tx, obj_list)

        public_key_bytes = bytes.fromhex(OWNED_PUBLIC_KEY)
        verify_signature(public_key_bytes, blake2b(tx, digest_size=32).digest(), signature)


# Negative: the host presents `TransferObjects{objects: [GasCoin], address}` -- the
# send-max idiom -- with a gas object whose balance is exactly the quoted amount.
# The pre-execution balance therefore matches the quote, but the recipient receives
# that balance minus the gas actually consumed, so the swap must not be bound to it
# (B2CA-2793 follow-up finding 2).
class SuiSwapWholeGasCoinTests(ExchangeTestRunner):
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

        [tx, obj_list] = client.build_whole_gas_coin_transaction(OWNED_ADDRESS, destination, send_amount, fees)
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

    # Negative: set up a valid swap for an unknown ticker, then have the malicious host
    # present a liquid SUI transfer. With no dynamic descriptor loaded the device must
    # refuse with SUI_SWAP_TX_PARAM_MISMATCH instead of signing the SUI spend.
    def test_sui_unknown_token_rejects_sui(self, backend, exchange_navigation_helper):
        runner = SuiSwapUnknownTokenRejectsSuiTests(backend, exchange_navigation_helper)
        runner.perform_valid_swap_from_custom(runner.valid_destination_1,
                                              runner.valid_send_amount_1,
                                              runner.valid_fees_1,
                                              "")
        with pytest.raises(ExceptionRAPDU) as e:
            runner.perform_coin_specific_final_tx(runner.valid_destination_1,
                                                  runner.valid_send_amount_1,
                                                  runner.valid_fees_1,
                                                  "")
        assert e.value.status == Errors.SUI_SWAP_TX_PARAM_MISMATCH

    # Negative: a whole-GasCoin (send-max) transfer must never satisfy a swap quote,
    # even when the gas coin's pre-execution balance equals the expected amount
    # exactly -- gas is charged out of that balance, so the counterparty would be
    # underpaid by the gas actually consumed, with no UI shown.
    def test_sui_swap_rejects_whole_gas_coin(self, backend, exchange_navigation_helper):
        runner = SuiSwapWholeGasCoinTests(backend, exchange_navigation_helper)
        runner.perform_valid_swap_from_custom(runner.valid_destination_1,
                                              runner.valid_send_amount_1,
                                              runner.valid_fees_1,
                                              "")
        with pytest.raises(ExceptionRAPDU) as e:
            runner.perform_coin_specific_final_tx(runner.valid_destination_1,
                                                  runner.valid_send_amount_1,
                                                  runner.valid_fees_1,
                                                  "")
        assert e.value.status == Errors.SUI_SWAP_TX_PARAM_MISMATCH

    # Positive: the unknown ticker is accepted once a matching dynamic descriptor is provided.
    @pytest.mark.parametrize('test_to_run', ALL_TESTS_EXCEPT_MEMO_AND_THORSWAP)
    def test_sui_unknown_token_dynamic(self, backend, exchange_navigation_helper, test_to_run):
        SuiSwapUnknownTokenDynamicTests(backend, exchange_navigation_helper).run_test(test_to_run)