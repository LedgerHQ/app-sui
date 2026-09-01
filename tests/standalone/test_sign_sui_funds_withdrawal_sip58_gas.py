# SIP-58 FundsWithdrawal with address-balance gas: both transfer and gas drawn from the
# sender's address balance (empty gas_data.payment, ValidDuring expiration).
#
# Corresponds to the `test_deposit_and_withdraw` scenario in the Sui e2e test suite:
#   crates/sui-e2e-tests/tests/address_balance_tests.rs::test_deposit_and_withdraw
#
# The production flow: sender deposits SUI into their address balance, then submits a
# funds_accumulator::withdrawal_split → balance::redeem_funds → balance::send_funds PTB
# with gas_data.payment = [] and TransactionExpiration::ValidDuring so the entire
# transaction runs without holding any coin objects.
#
# Regression: before SIP-58 support, this variant was rejected because the parser
# required non-empty gas payment. With SIP-58 it must be accepted and displayed
# identically to the regular-gas funds-withdrawal (same amount/addresses), differing
# only in how gas is funded.
#
# The fixture is derived programmatically from the regular-gas test_sign_sui_funds_withdrawal.py fixture.

import base64

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently

# ObjectRef in transaction BCS: ObjectID[32] + SequenceNumber[8] + Sha3_256Hash[33] = 73 bytes.
# gas_data tail: count[1] + ObjectRef[73] + owner[32] + price[8] + budget[8] + expiration[1]
#              = 123 bytes for a single-coin payment with None expiration.
_GAS_TAIL_LEN = 123
_GAS_COIN_OBJECT_ID = bytes.fromhex(
    "400dbdcfeda8e1e64ebbf5710ccf5a67bfa6e7f945c92bc8611e0cb44b7219de"
)


def _build_tx() -> bytes:
    """
    Derive the SIP-58 variant of the funds-withdrawal transaction from the regular-gas
    fixture by stripping the gas coin ObjectRef from gas_data.payment and replacing
    TransactionExpiration::None (0x00) with ValidDuring (all-zero chain/nonce).
    """
    # Note: `withdrawal_split`'s second argument (Input 1) below is fixed to the
    # correct BCS encoding: a plain 8-byte u64 Pure equal to the split amount
    # (612_000, matching the reservation), not a 32-byte Pure with the same value
    # zero-padded. That 32-byte encoding was never actually a valid `u64` Move
    # argument and only "worked" while the parser ignored this argument's value
    # entirely (B2CA-2793 finding 6); it is now validated against the reservation
    # (see test_sign_sui_funds_withdrawal.py, from which this fixture derives).
    regular = base64.b64decode(
        "AAAAAAADAgCgVgkAAAAAAAAHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQAAAAigVgkAAAAAAAAgHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACEWZ1bmRzX2FjY3VtdWxhdG9yEHdpdGhkcmF3YWxfc3BsaXQBBwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACB2JhbGFuY2UHQmFsYW5jZQEHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQACAQAAAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACB2JhbGFuY2UMcmVkZWVtX2Z1bmRzAQcAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAECAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIHYmFsYW5jZQpzZW5kX2Z1bmRzAQcAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAICAQABAgBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQFADb3P7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2QhEAAAAAIGbFq2VJip03FgAaA0gV/0q8p2X39vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiHoAwAAAAAAACChBwAAAAAAAA=="
    )
    # Sanity-check: payment count byte and first 32 bytes of ObjectRef must match the gas coin.
    assert regular[-_GAS_TAIL_LEN] == 0x01, (
        f"expected payment count=1 at offset -{_GAS_TAIL_LEN}, got {regular[-_GAS_TAIL_LEN]:#04x}"
    )
    assert regular[-_GAS_TAIL_LEN + 1 : -_GAS_TAIL_LEN + 33] == _GAS_COIN_OBJECT_ID, (
        "gas coin ObjectID not at expected tail position — check _GAS_TAIL_LEN"
    )

    # Tail layout (from position -_GAS_TAIL_LEN):
    #   [0]       payment count = 0x01
    #   [1:74]    ObjectRef (73 bytes: ObjectID[32] + SeqNo[8] + Digest[33])
    #   [74:106]  gas_data.owner  (32 bytes)
    #   [106:114] gas_data.price  (u64 LE)
    #   [114:122] gas_data.budget (u64 LE)
    #   [122]     expiration = 0x00 (None)
    tail = regular[-_GAS_TAIL_LEN:]
    owner_price_budget = tail[74:122]

    # ValidDuring: variant(1) + 4×Option::None(4) + chain_id(1+32) + nonce(4) = 42 bytes.
    # All-zero chain and nonce are accepted by the emulator parser (no on-chain validation).
    # The chain identifier is length-prefixed on the wire (ULEB length + bytes), not a
    # bare fixed array (B2CA-2793 finding 2; see ChainIdentifierParser in
    # rust-app/src/parser/tx.rs) -- confirmed against the tool-generated fixture in
    # test_sign_sui_transfer_address_balance.py, which encodes it the same way.
    valid_during = bytes(
        [0x02]       # ValidDuring variant
        + [0x00] * 4   # min_epoch=None, max_epoch=None, min_ts=None, max_ts=None
        + [0x20]       # chain identifier length prefix (ULEB128 32)
        + [0x00] * 32  # chain identifier (all-zero for emulator)
        + [0x00] * 4   # nonce (u32 LE = 0)
    )

    # Reassemble: original PTB + sender, then new gas_data (empty payment), then ValidDuring.
    ptb_and_sender = regular[:-_GAS_TAIL_LEN]
    return ptb_and_sender + bytes([0x00]) + owner_price_budget + valid_during


_TX = _build_tx()


def test_sign_tx_sui_funds_withdrawal_sip58_gas(backend, scenario_navigator, firmware, navigator):
    """
    FundsWithdrawal 612_000 MIST with gas also from address balance (SIP-58).
    """
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = _TX
    object_list = []  # SIP-58: gas from address balance, no coin objects needed

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
