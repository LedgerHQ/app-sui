import base64
import pytest
from hashlib import blake2b

from ledger_app_clients.exchange.test_runner import ALL_TESTS_EXCEPT_MEMO_AND_THORSWAP, ExchangeTestRunner
from ragger.error import ExceptionRAPDU
from application_client.client import Client, Errors
from cal_helper import (
    SUI_CURRENCY_CONFIGURATION,
    SUI_OVERSIZED_DECIMALS_CURRENCY_CONFIGURATION,
    SUI_USDC_CURRENCY_CONFIGURATION,
    SUI_UNKNOWN_CURRENCY_CONFIGURATION,
)
from application_client.sui_utils import *

# Coin type backing the USDC token objects reused by the dynamic-descriptor tests.
USDC_DYNAMIC_TOKEN_ADDRESS = "0x909cba62ce96d54de25bec9502de5ca7b4f28901747bbf96b76c2e63ec5f1cba"
USDC_DYNAMIC_TOKEN_MODULE = "coin"
USDC_DYNAMIC_TOKEN_STRUCT = "COIN"

# Status the Exchange app returns when a library GET_PRINTABLE_AMOUNT yields no
# amount. Owned by app-exchange, not by this app's Errors enum.
EXCHANGE_PRINTABLE_AMOUNT_MISSING = 0x6A8B

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
# Security regression: the Exchange app supplies the coin config's decimals byte and
# the app used it unvalidated. Rendering an amount divides by 10^decimals, computed
# with u64::pow while overflow-checks are off, so the divisor wraps -- and from 10^64
# it wraps to exactly zero, making the division panic.
#
# That panic lands inside GET_PRINTABLE_AMOUNT, a pre-sign libcall. Libcall startup
# deliberately does not clear the app's .bss (it is shared with the calling Exchange
# app), and SWAP_PANIC_HANDLER -- an Option<fn> whose None is all-zero -- is only
# written by SwapSignTransaction. So the panic handler read whatever the Exchange
# app's live RAM held there and, for any nonzero value, called it as a function
# pointer. Observed before the fix: the app never returned control and the Exchange
# call timed out.
#
# With the config refused at parse, the libcall returns normally with no printable
# amount and Exchange reports it, so the test asserts a clean status rather than a
# timeout.
class SuiSwapOversizedDecimalsTests(ExchangeTestRunner):
    currency_configuration = SUI_OVERSIZED_DECIMALS_CURRENCY_CONFIGURATION
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
        # Never reached: the swap is refused while Exchange is still gathering the
        # printable amount.
        raise AssertionError("swap setup should not have completed")


# Security regression: an object must be parsed from exactly the bytes its digest
# covers. The device used to hash only the first `length` bytes of an object entry but
# parse it on an unbounded cursor, so an object whose hashed range ended early had its
# balance read from whatever the host put next in the stream.
#
# Swap relies on that balance: the transaction splits the quoted amount out of a coin,
# and a split larger than the coin's known balance is refused. Here the coin really
# holds USDC_AMOUNT (the smaller object), but the quote and the split are
# USDC_AMOUNT_2, which would overdraw it. The host truncates the coin object just
# before its balance, points the transaction's digest at that range so the hash check
# passes, and supplies the balance through the next list entry -- its 4-byte length
# prefix and first four bytes -- so the device sees a coin that covers the split. The
# continuation is padded so its length equals the quoted amount.
USDC_COIN_TYPE_PREFIX = bytes([0x00, 0x03, 0x07])


def _usdc_balance_offset(obj: bytes) -> int:
    """Offset of the 8-byte balance in a `Coin<T>` object blob: after the type tag, the
    has_public_transfer flag, the version, the contents length byte and the 32-byte UID."""
    assert obj[:3] == USDC_COIN_TYPE_PREFIX, "expected Move/Coin/Struct object"
    o = 3 + 32
    for _ in range(2):  # module and struct names, single-byte ULEB lengths
        assert obj[o] < 0x80
        o += 1 + obj[o]
    assert obj[o] == 0, "type parameters not expected"
    o += 1 + 1 + 8  # type params count, has_public_transfer, version
    assert obj[o] == 40, "expected a 40-byte coin contents vector"
    return o + 1 + 32


class SuiSwapObjectOverrunTests(ExchangeTestRunner):
    currency_configuration = SUI_USDC_CURRENCY_CONFIGURATION
    valid_destination_1 = FOREIGN_ADDRESS
    valid_destination_2 = FOREIGN_ADDRESS_2
    valid_refund = OWNED_ADDRESS
    valid_send_amount_1 = USDC_AMOUNT_2
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
        tx = bytearray(tx)

        # Swap in the smaller coin: it really holds USDC_AMOUNT < send_amount.
        small_coin = base64.b64decode(USDC_OBJECTS_BY_AMOUNT[USDC_AMOUNT]["obj"])
        offset = _usdc_balance_offset(small_coin)
        hashed = small_coin[:offset]
        continuation = bytes(4) + small_coin[offset + 8:]
        # Balance = len(continuation) as the entry's length prefix, then continuation[:4].
        continuation += bytes(send_amount - len(continuation))
        assert len(continuation) == send_amount

        def digest(obj: bytes) -> bytes:
            return blake2b(b"Object::" + obj, digest_size=32).digest()

        pos = tx.find(digest(obj_list[0]))
        assert pos != -1, "could not locate the coin's digest in the transaction"
        tx[pos:pos + 32] = digest(hashed)

        client.sign_tx("m/44'/784'/12345'", bytes(tx), [hashed, continuation])


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

    # Negative: a coin object parsed past the bytes its digest covers must not let a
    # swap pass. The split overdraws the coin's real balance, and only a balance the
    # host smuggled in after the hashed range makes it look covered. Before the fix the
    # device signed this with no prompt (the call returned instead of raising); now the
    # object is unusable, the transaction is not recognized, and the swap is refused.
    def test_sui_swap_rejects_object_parsed_past_hashed_length(self, backend, exchange_navigation_helper):
        runner = SuiSwapObjectOverrunTests(backend, exchange_navigation_helper)
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

    # Negative: an out-of-range decimals byte in the Exchange-supplied coin config
    # must be refused, not divided by. Before the fix the app panicked inside the
    # GET_PRINTABLE_AMOUNT libcall and never returned control, so this raised
    # TimeoutError instead of the status asserted here -- which is what makes the
    # test discriminate.
    def test_sui_swap_rejects_oversized_decimals(self, backend, exchange_navigation_helper):
        runner = SuiSwapOversizedDecimalsTests(backend, exchange_navigation_helper)
        with pytest.raises(ExceptionRAPDU) as e:
            runner.perform_valid_swap_from_custom(runner.valid_destination_1,
                                                  runner.valid_send_amount_1,
                                                  runner.valid_fees_1,
                                                  "")
        # Exchange's "printable amount should exist": the library call came back
        # cleanly with no amount rather than dying mid-call.
        assert e.value.status == EXCHANGE_PRINTABLE_AMOUNT_MISSING

    # Positive: the unknown ticker is accepted once a matching dynamic descriptor is provided.
    @pytest.mark.parametrize('test_to_run', ALL_TESTS_EXCEPT_MEMO_AND_THORSWAP)
    def test_sui_unknown_token_dynamic(self, backend, exchange_navigation_helper, test_to_run):
        SuiSwapUnknownTokenDynamicTests(backend, exchange_navigation_helper).run_test(test_to_run)