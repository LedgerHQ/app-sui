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
    MAINNET_HASHI_OBJECT,
    MAINNET_PACKAGE,
    OTHER,
    SENDER,
    TYPE_PACKAGE,
    fixture,
    patch_last,
    patch_once,
    retarget_calls,
    retype_btc,
    sign_expect_not_recognized,
)


def _withdraw():
    return fixture("withdraw_ab_p2wpkh")[0]


def _flip_last(b):
    return b[:-1] + bytes([b[-1] ^ 1])


def _call_package_not_pinned():
    # Point the withdrawal's Hashi calls at a package that is not pinned; the hBTC type
    # tags keep the pinned type package.
    return retarget_calls(_withdraw(), _flip_last(CALL_PACKAGE))


# The testnet deployment @mysten/hashi 0.6.25 targeted, retired by the 0.6.26 redeploy.
RETIRED_CALL_PACKAGE = bytes.fromhex("8f7efd743897fde48cc35b6203cd72c7ad4248f0eb02a9ad378e4a2d39cc2c7e")
RETIRED_TYPE_PACKAGE = bytes.fromhex("fcea10cadbb553c4874201584abf68771592678952efd957b2e82c010c7f4360")
RETIRED_HASHI_OBJECT = bytes.fromhex("22c0ce66ce09df2dc88a31bd320d4177b766518b9b88010368cfbdcd724528f8")


def _retired_deployment():
    # A withdrawal as it was built against the retired deployment: its calls, hBTC type
    # and Hashi object are no longer pinned.
    tx = retarget_calls(_withdraw(), RETIRED_CALL_PACKAGE)
    tx = retype_btc(tx, TYPE_PACKAGE, RETIRED_TYPE_PACKAGE)
    return patch_once(tx, HASHI_OBJECT, RETIRED_HASHI_OBJECT)


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


# Both networks are pinned: a transaction must use one deployment's ids throughout.
def _mainnet_withdraw():
    return fixture("mainnet_withdraw_ab_p2wpkh")[0]


def _mainnet_with_testnet_object():
    return patch_once(_mainnet_withdraw(), MAINNET_HASHI_OBJECT, HASHI_OBJECT)


def _testnet_with_mainnet_object():
    return patch_once(_withdraw(), HASHI_OBJECT, MAINNET_HASHI_OBJECT)


def _mainnet_with_testnet_btc():
    # Mainnet calls locking testnet hBTC from the address balance.
    return retype_btc(_mainnet_withdraw(), MAINNET_PACKAGE, TYPE_PACKAGE)


def _testnet_with_mainnet_btc():
    return retype_btc(_withdraw(), TYPE_PACKAGE, MAINNET_PACKAGE)


def _deposit_mixed_deployments():
    # The UTXO is built by the testnet package and registered with the mainnet one.
    tx = fixture("mainnet_deposit_1")[0]
    return retarget_calls(tx, CALL_PACKAGE, modules=(b"utxo",), source=MAINNET_PACKAGE)


PATCHED = [
    pytest.param("withdraw_ab_p2wpkh", _call_package_not_pinned, id="call_package_not_pinned"),
    pytest.param("withdraw_ab_p2wpkh", _retired_deployment, id="retired_deployment"),
    pytest.param("withdraw_ab_p2wpkh", _wrong_hashi_object, id="wrong_hashi_object"),
    pytest.param("withdraw_ab_p2wpkh", _wrong_clock, id="wrong_clock"),
    pytest.param("withdraw_ab_p2wpkh", _address_bad_prefix, id="address_bad_prefix"),
    pytest.param("withdraw_ab_p2tr", _address_option_tag, id="address_option_tag"),
    pytest.param("withdraw_coins_change", _change_to_other, id="change_to_other"),
    pytest.param("deposit_1", _deposit_none_path, id="deposit_none_path"),
    pytest.param("deposit_1", _deposit_zero_amount, id="deposit_zero_amount"),
    pytest.param("deposit_4", _deposit_mixed_recipients, id="deposit_mixed_recipients"),
    pytest.param("mainnet_withdraw_ab_p2wpkh", _mainnet_with_testnet_object, id="mainnet_with_testnet_object"),
    pytest.param("withdraw_ab_p2wpkh", _testnet_with_mainnet_object, id="testnet_with_mainnet_object"),
    pytest.param("mainnet_withdraw_ab_p2wpkh", _mainnet_with_testnet_btc, id="mainnet_with_testnet_btc"),
    pytest.param("withdraw_ab_p2wpkh", _testnet_with_mainnet_btc, id="testnet_with_mainnet_btc"),
    pytest.param("mainnet_deposit_1", _deposit_mixed_deployments, id="deposit_mixed_deployments"),
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
    # A ValidDuring naming the other Sui network than the deployment's.
    pytest.param("withdraw_valid_during_mainnet", id="withdraw_valid_during_mainnet"),
    pytest.param("mainnet_withdraw_valid_during_testnet", id="mainnet_withdraw_valid_during_testnet"),
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
