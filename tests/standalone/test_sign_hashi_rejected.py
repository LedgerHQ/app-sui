# Transactions that look like Hashi, or reuse the Move calls Hashi support admits, but must
# not be clear-signed. Each one lands on "This transaction cannot be clear-signed" (blind
# signing is off) and is refused with NotSupported.
#
# Most are the SDK fixtures of test_sign_hashi.py with one value patched; the others are
# built by tests/tools/hashi-fixtures with the same pinned SDK.

import pytest

from hashi_utils import (
    CALL_PACKAGE,
    CLOCK,
    HASHI_OBJECT,
    OTHER,
    SENDER,
    TYPE_PACKAGE,
    fixture,
    patch_last,
    patch_once,
    sign_expect_not_recognized,
)


def _withdraw():
    return fixture("withdraw_ab_p2wpkh")[0]


def _flip_last(b):
    return b[:-1] + bytes([b[-1] ^ 1])


def _call_package_not_pinned():
    # Every Move call of the withdrawal targets the pinned v2 package; point them all at
    # a package that is not pinned.
    return _withdraw().replace(CALL_PACKAGE, _flip_last(CALL_PACKAGE))


def _call_package_original():
    # The original package id is the one in type tags, but calls must go to a pinned
    # upgrade: the original's code is not what was reviewed.
    return _withdraw().replace(CALL_PACKAGE, TYPE_PACKAGE)


def _wrong_hashi_object():
    return patch_once(_withdraw(), HASHI_OBJECT, _flip_last(HASHI_OBJECT))


def _wrong_clock():
    # The clock is the shared object 0x6 at initial version 1.
    clock = CLOCK + (1).to_bytes(8, "little")
    return patch_once(_withdraw(), clock, bytes(31) + b"\x05" + (1).to_bytes(8, "little"))


def _address_bad_prefix():
    # Pure(21) holding vector<u8> of length 19 (+ a stray byte): not a 20-byte program.
    tx = _withdraw()
    i = tx.index(b"\x00\x15\x14")
    return tx[: i + 2] + b"\x13" + tx[i + 3 :]


def _address_option_tag():
    # Pure(33) whose first byte reads as an Option tag rather than a 32-byte length.
    tx = fixture("withdraw_ab_p2tr")[0]
    i = tx.index(b"\x00\x21\x20")
    return tx[: i + 2] + b"\x01" + tx[i + 3 :]


def _change_to_other():
    # The coin path's change send_funds names someone else than the sender.
    tx = fixture("withdraw_coins_change")[0]
    return patch_once(tx, b"\x00\x20" + SENDER, b"\x00\x20" + OTHER)


def _deposit_none_path():
    # derivation_path: None mints nothing.
    tx = fixture("deposit_1")[0]
    return patch_once(tx, b"\x00\x21\x01" + SENDER, b"\x00\x21\x00" + SENDER)


def _deposit_zero_amount():
    tx = fixture("deposit_1")[0]
    return patch_once(tx, b"\x00\x08" + (50_000).to_bytes(8, "little"), b"\x00\x08" + bytes(8))


def _deposit_mixed_recipients():
    # One review shows one recipient, so every UTXO must credit the same account.
    tx = fixture("deposit_4")[0]
    return patch_last(tx, b"\x00\x21\x01" + SENDER, b"\x00\x21\x01" + OTHER)


PATCHED = [
    pytest.param("withdraw_ab_p2wpkh", _call_package_not_pinned, id="call_package_not_pinned"),
    pytest.param("withdraw_ab_p2wpkh", _call_package_original, id="call_package_original"),
    pytest.param("withdraw_ab_p2wpkh", _wrong_hashi_object, id="wrong_hashi_object"),
    pytest.param("withdraw_ab_p2wpkh", _wrong_clock, id="wrong_clock"),
    pytest.param("withdraw_ab_p2wpkh", _address_bad_prefix, id="address_bad_prefix"),
    pytest.param("withdraw_ab_p2tr", _address_option_tag, id="address_option_tag"),
    pytest.param("withdraw_coins_change", _change_to_other, id="change_to_other"),
    pytest.param("deposit_1", _deposit_none_path, id="deposit_none_path"),
    pytest.param("deposit_1", _deposit_zero_amount, id="deposit_zero_amount"),
    pytest.param("deposit_4", _deposit_mixed_recipients, id="deposit_mixed_recipients"),
]


@pytest.mark.parametrize("name, patch", PATCHED)
def test_sign_hashi_patched_rejected(name, patch, backend, firmware, navigator):
    sign_expect_not_recognized(name, backend, firmware, navigator, tx=patch())


BUILT = [
    # A 25-byte destination: only 20 (P2WPKH) and 32 (P2TR) can be paid out.
    pytest.param("withdraw_bad_len", id="withdraw_bad_len"),
    # The cancel refund goes to someone else than the requester.
    pytest.param("cancel_other_recipient", id="cancel_other_recipient"),
    # More UTXOs than one review holds.
    pytest.param("deposit_5", id="deposit_5"),
    # A Hashi flow mixed with a SUI transfer, in either order.
    pytest.param("withdraw_then_sui_transfer", id="withdraw_then_sui_transfer"),
    pytest.param("sui_transfer_then_withdraw", id="sui_transfer_then_withdraw"),
    # Two withdrawals in one transaction.
    pytest.param("double_withdraw", id="double_withdraw"),
    # A withdrawal locking a USDC balance instead of hBTC.
    pytest.param("withdraw_usdc_balance", id="withdraw_usdc_balance"),
    # A ValidDuring naming Sui mainnet on the testnet deployment.
    pytest.param("withdraw_valid_during_mainnet", id="withdraw_valid_during_mainnet"),
    # Non-Hashi transactions using the 0x2 calls Hashi support admits. All were
    # unrecognised before; none may become clear-signable.
    pytest.param("nonhashi_usdc_send_funds_input", id="nonhashi_usdc_send_funds_input"),
    pytest.param("nonhashi_btc_send_funds_input", id="nonhashi_btc_send_funds_input"),
    pytest.param("nonhashi_into_balance_send", id="nonhashi_into_balance_send"),
    pytest.param("nonhashi_btc_redeem_input_send", id="nonhashi_btc_redeem_input_send"),
]


@pytest.mark.parametrize("name", BUILT)
def test_sign_hashi_rejected(name, backend, firmware, navigator):
    sign_expect_not_recognized(name, backend, firmware, navigator)
