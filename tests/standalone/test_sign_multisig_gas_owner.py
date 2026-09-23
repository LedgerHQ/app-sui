# A gas owner this device's key belongs to without equalling it: a Sui multisig.
#
#   TransactionData.sender = SPONSORED_SENDER  (not this device)
#   GasData.owner          = a multisig whose constituents are this device's key
#                            and one key the attacker holds, threshold 1
#
# A multisig address is blake2b-256 over the multisig flag, the threshold and each
# constituent's (flag || public key || weight), so it equals no constituent's own
# Ed25519 address. The device's share of such a signature still authorizes the
# multisig -- with threshold 1 it authorizes it alone -- so the host can spend the
# multisig's gas coin with a signature this device produced.
#
# Selecting the sponsorship disclosure by comparing the derived address to
# GasData.owner therefore missed exactly this case: the addresses differ, so the
# review dropped the disclosure, named the constituent key as the transaction's
# "From" account and titled itself an ordinary transaction. The user saw their own
# account sending a transaction that was in fact sent by another account and funded
# by a multisig they only hold one key of.
#
# Both fixtures below are existing tests' transactions with GasData.owner rewritten
# to the multisig address, which is the whole edit. Bases:
# test_sponsored_stake_of_sender_coin_names_stake_owner (a stake funded from the
# sender's own coins) and test_sign_tx_sui_two_merge_input_coin (a transfer of
# merged input coins). Neither touches the gas coin, so neither is caught by the
# sponsorship gates that reject the value-extracting shapes outright -- these are
# the sponsored transactions that do reach a review.
#
# Three assertions, each a separate signing run because the navigation searches
# forward for one text and gives up if no screen carries it:
#
#   * the review is a sponsored one, so the transaction cannot reach an
#     ordinary/non-sponsored review,
#   * it names the derived address as the signing key rather than as an account,
#     and
#   * it names the account funding the transaction.
#
# A fourth refuses the review, since the disclosure lengthens it. The goldens pin
# the screens themselves.

import base64

import pytest

from application_client.client import Client
from pysui.abstracts.client_keypair import SignatureScheme
from pysui.sui.sui_crypto import BaseMultiSig, SuiPublicKey
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently

PATH = "m/44'/784'/0'/0'/1'"
LEDGER_ADDRESS = "f65c72abf52307bc1bd3c199534aaf04eb56c7be96ae2e74271b09508412e8fb"
SPONSORED_SENDER = "1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488"

# The other constituent, standing in for the key an attacker holds: the public key
# from RFC 8032's first Ed25519 test vector, so it is a real point on the curve and
# demonstrably not this device's.
ATTACKER_PUBKEY = bytes.fromhex(
    "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
)

# request_add_stake(system, Input0, validator), where Input0 is a coin owned by
# SPONSORED_SENDER: the gas owner pays only the gas, which is what sponsoring means.
SENDER_COIN_STAKE_TX = base64.b64decode(
    "AAAAAAADAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABQEAAAAAAAAAAQEAqT9sRk+P"
    "uLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq23UdkIRAAAAACCsVYpX4/44Cp2BWe8aVkACUW5rxtsE"
    "rjUPJ6nMxaCvvQAgNfXxVPARdGTjN5xFx/PLay1O/t8wCsrf+Kfo6eOhUQkBAAAAAAAAAAAAAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAAADCnN1aV9zeXN0ZW0RcmVxdWVzdF9hZGRfc3Rha2UAAwEAAAEB"
    "AAECAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIA+v/FrTSCBqwbR1SUcmIIIZB5cUB"
    "x/qL3OnIt7CQi6dr1XZCEQAAAAAgO/8EfMoNuhJrkPXn8Pcmq72jh1ZSG9wTpvKb+SeNsOQfh2/w"
    "FEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0"
    "ngASSltvrhoF81G6HBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGI"
    "Xjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8/Zccqv1Iwe8G9PBmVNKrwTrVse+lq4udCcbCVCE"
    "Euj76AMAAAAAAAB44AEAAAAAAAA="
)

# MergeCoins of two input coins into a third, then TransferObjects of the result.
# No command touches the gas coin.
MERGE_INPUT_COINS_TRANSFER_TX = base64.b64decode(
    "AAAAAAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgK"
    "nYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDa"
    "gNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zAQDr/xa00ggasG0dUlHJ"
    "iCCGQeXFAcf6i9zpyLewkIuna9V2QhEAAAAAIDv/BHzKDboSa5D15/D3Jqu9o4dWUhvcE6bym/kn"
    "jbDkACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQMDAQAAAQEBAAMBAAABAQIAAQEB"
    "AAABAwAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEfh2/wFEOG3PTohsXeU7MmxxjM"
    "EiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8m"
    "QzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAICEHgAAAAAAAA=="
)

OBJECT_LIST = [
    base64.b64decode(
        "AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZD"
        "MFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L"
        "4YEUXWATDwAAAAAA"
    ),
    base64.b64decode(
        "AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZD"
        "MFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9a"
        "sLM9x2ATDwAAAAAA"
    ),
    base64.b64decode(
        "AAEB1HZCEQAAAAAoqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq22Aw8kBAAAAAAAdPyZD"
        "MFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAfVAIamErRVJt4BuqoZFY2dBaAKAaQzrxvVjuL"
        "cgrqZmATDwAAAAAA"
    ),
    base64.b64decode(
        "AAEB1XZCEQAAAAAo6/8WtNIIGrBtHVJRyYgghkHlxQHH+ovc6ci3sJCLp2tAnHECAAAAAAAdPyZD"
        "MFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAuq6BxxXPwIbLsDoXWJN6/Emi0EtUzGJnln5pJ"
        "L4iDYWATDwAAAAAA"
    ),
]

# GasData is the transaction's tail:
#   payment(vec of ObjectRef) || owner(32) || price(8) || budget(8)
# followed by a 1-byte expiration, so the gas owner always sits 49 bytes from the
# end. The sender is the 32 bytes immediately preceding GasData, at an offset that
# depends on how many gas payment objects the fixture carries; deriving it from the
# payment count both locates the sender and checks the tail layout is what these
# assertions assume.
GAS_OWNER_FROM_END = 49
OBJECT_REF_LEN = 73  # id(32) || version(8) || digest(1 length byte + 32)


def _sender_offset(tx):
    owner_at = len(tx) - GAS_OWNER_FROM_END
    for count in range(1, 5):
        payment_at = owner_at - count * OBJECT_REF_LEN - 1
        if payment_at >= 32 and tx[payment_at] == count:
            return payment_at - 32
    raise AssertionError("could not locate GasData.payment in the fixture")


def _multisig_gas_owner(device_pubkey):
    """The address of a 1-of-2 multisig over this device's key and the attacker's.

    Derived by pysui, which is the reference implementation: blake2b-256 over the
    multisig flag, the threshold as a little-endian u16, and each constituent's
    scheme flag, public key and weight.
    """
    keys = [
        SuiPublicKey(SignatureScheme.ED25519, device_pubkey),
        SuiPublicKey(SignatureScheme.ED25519, ATTACKER_PUBKEY),
    ]
    multisig = BaseMultiSig(keys, [1, 1], 1)
    assert multisig.threshold == 1, "the device's share alone must authorize this multisig"
    assert device_pubkey in [k.key_bytes for k in multisig.public_keys], (
        "the device key is not a constituent, so this is not the case V-158 describes"
    )
    address = bytes.fromhex(multisig.address.removeprefix("0x"))
    assert len(address) == 32
    return address


def _sponsor_with(tx, gas_owner):
    """`tx` with GasData.owner rewritten, and the resulting shape checked."""
    patched = tx[: -GAS_OWNER_FROM_END] + gas_owner + tx[-(GAS_OWNER_FROM_END - 32) :]
    assert len(patched) == len(tx)

    assert patched[-GAS_OWNER_FROM_END:-17] == gas_owner
    owner_hex = gas_owner.hex()
    assert owner_hex != LEDGER_ADDRESS, (
        "the gas owner equals the derived address, so the old single-key comparison "
        "would have disclosed it and the fixture proves nothing"
    )

    off = _sender_offset(patched)
    sender_hex = patched[off : off + 32].hex()
    assert sender_hex == SPONSORED_SENDER, (
        "TransactionData.sender is not the other account, so the fixture is not sponsored"
    )
    assert sender_hex != owner_hex, "sender and gas owner must differ"
    assert sender_hex != LEDGER_ADDRESS
    return patched


def _prepare(backend, tx):
    """A client, the device key, and `tx` re-pointed at a multisig gas owner."""
    client = Client(backend, use_block_protocol=True)
    _, public_key, _, address = client.get_public_key(path=PATH)
    assert len(public_key) == 32
    # Pins the constant the shape checks compare against to the address the device
    # actually derives, so those checks cannot pass against a stale value.
    assert address.hex() == LEDGER_ADDRESS

    return client, public_key, _sponsor_with(tx, _multisig_gas_owner(public_key))


def _sign_and_find(text, tx, case, backend, scenario_navigator, firmware, navigator):
    """Sign `tx`, paging through the review until `text` shows, then approve.

    Paging by text rather than by a fixed click count: if the review never carries
    the text this gives up rather than approving something it never saw, so the
    disclosure is asserted by the navigation itself and not only by the goldens.

    `case` names the fixture in the golden path. `scenario_navigator.test_name` is
    the test function's name with no parameter id, so the two fixtures would
    otherwise share one set of goldens and each replay would be compared against
    the other's screens.
    """
    client, public_key, transaction = _prepare(backend, tx)

    def apdu_task():
        return client.sign_tx(
            path=PATH, transaction=transaction, object_list=OBJECT_LIST
        )

    def nav_task():
        nano = firmware.device.startswith("nano")
        navigator.navigate_until_text_and_compare(
            NavInsID.RIGHT_CLICK if nano else NavInsID.USE_CASE_REVIEW_TAP,
            [NavInsID.BOTH_CLICK] if nano else [NavInsID.USE_CASE_REVIEW_CONFIRM],
            text,
            scenario_navigator.screenshot_path,
            f"{scenario_navigator.test_name}_{case}",
            screen_change_after_last_instruction=False,
        )

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, transaction)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)


FIXTURES = [
    pytest.param(SENDER_COIN_STAKE_TX, "stake", id="stake"),
    pytest.param(MERGE_INPUT_COINS_TRANSFER_TX, "transfer", id="transfer"),
]


# The transaction must not reach an ordinary review: a gas owner that is not the
# sender is a sponsorship whether or not the derived address happens to equal one
# of them, and the confirmation names it as such.
@pytest.mark.parametrize("tx, case", FIXTURES)
def test_multisig_gas_owner_reviewed_as_sponsored(
    tx, case, backend, scenario_navigator, firmware, navigator
):
    _sign_and_find(
        "Sign sponsored", tx, case, backend, scenario_navigator, firmware, navigator
    )


# And the derived address is named for what it is. It is neither principal here, so
# labelling it "From" would attribute the transaction to an account that did not
# send it and does not pay for it.
@pytest.mark.parametrize("tx, case", FIXTURES)
def test_multisig_gas_owner_review_names_the_signing_key(
    tx, case, backend, scenario_navigator, firmware, navigator
):
    _sign_and_find(
        "Signing key", tx, case, backend, scenario_navigator, firmware, navigator
    )


# The account funding the transaction is named too. Asserted here rather than left
# to the goldens alone: a bulk snapshot regeneration would absorb the field's
# disappearance, whereas this navigation gives up when no screen carries it.
@pytest.mark.parametrize("tx, case", FIXTURES)
def test_multisig_gas_owner_review_names_the_gas_owner(
    tx, case, backend, scenario_navigator, firmware, navigator
):
    _sign_and_find(
        "Gas paid by", tx, case, backend, scenario_navigator, firmware, navigator
    )


# Refusing the review must reach the device's rejection screen and answer the host
# with an error rather than a signature. One fixture is enough: the rejection path
# does not depend on which transaction is under review, only on the review being
# longer than it used to be, which is what a fixed click count would trip over.
def test_multisig_gas_owner_review_can_be_rejected(
    backend, scenario_navigator, firmware, navigator
):
    client, _public_key, transaction = _prepare(backend, MERGE_INPUT_COINS_TRANSFER_TX)

    def apdu_task():
        return client.sign_tx(
            path=PATH, transaction=transaction, object_list=OBJECT_LIST
        )

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_until_text_and_compare(
                NavInsID.RIGHT_CLICK,
                [NavInsID.BOTH_CLICK],
                "Reject",
                scenario_navigator.screenshot_path,
                scenario_navigator.test_name,
                screen_change_after_last_instruction=False,
            )
        else:
            scenario_navigator.review_reject()

    def check_result(_result):
        pytest.fail("a rejected review must not produce a signature")

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0
