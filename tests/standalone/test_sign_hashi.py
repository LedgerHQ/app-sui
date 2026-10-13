# Hashi (Sui-native Bitcoin bridge) transactions are clear-signed: withdrawal (from the
# address balance and from coin objects, to P2WPKH and P2TR), cancel and deposit.
#
# Every transaction is the one the pinned @mysten/hashi SDK builds (version in
# hashi_fixtures.json `_meta`; generator in tests/tools/hashi-fixtures), so the parser is
# checked against the PTB shapes the dApp actually emits:
#   withdraw, address balance: balance::redeem_funds(FundsWithdrawal) -> request_withdrawal
#   withdraw, coins:           [coin::redeem_funds] -> MergeCoins -> SplitCoins ->
#                              coin::into_balance -> coin::send_funds(change, sender) ->
#                              request_withdrawal
#   cancel:                    cancel_withdrawal -> coin::from_balance -> TransferObjects
#   deposit, per UTXO:         utxo::utxo_id -> utxo::utxo -> deposit::deposit
#
# The goldens show what each review must disclose: the hBTC amount and change, the
# Bitcoin address re-encoded from the witness program (bech32 for v0, bech32m for v1),
# the request id, and each UTXO with its txid in display order.

import pytest

from hashi_utils import sign_and_approve

CASES = [
    # 1 234 567 sat to tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx (BIP-173 vector).
    pytest.param("withdraw_ab_p2wpkh", id="withdraw_ab_p2wpkh"),
    # Same amount to tb1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqp3mvzv.
    pytest.param("withdraw_ab_p2tr", id="withdraw_ab_p2tr"),
    # Two coins (700 000 + 900 000 sat) merged, 1 234 567 locked, 365 433 change.
    pytest.param("withdraw_coins_change", id="withdraw_coins_change"),
    # A 500 000 sat coin topped up from the address balance: no change.
    pytest.param("withdraw_coins_topup", id="withdraw_coins_topup"),
    # ValidDuring on the deployment's network: shows Network and Nonce.
    pytest.param("withdraw_valid_during_testnet", id="withdraw_valid_during_testnet"),
    pytest.param("cancel", id="cancel"),
    pytest.param("deposit_1", id="deposit_1"),
    pytest.param("deposit_4", id="deposit_4"),
    # Registering a deposit for another account is allowed (it cannot redirect funds),
    # but the review flags it.
    pytest.param("deposit_other_recipient", id="deposit_other_recipient"),
]


@pytest.mark.parametrize("name", CASES)
def test_sign_hashi(name, backend, scenario_navigator, firmware, navigator):
    sign_and_approve(name, name, backend, scenario_navigator, firmware, navigator)
