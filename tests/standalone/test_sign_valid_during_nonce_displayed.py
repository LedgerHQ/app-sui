# The SIP-58 ValidDuring replay domain must reach the review: which network the
# transaction is scoped to, and the nonce distinguishing it from otherwise identical
# ones.
#
# ValidDuring carries `chain`, which scopes a transaction to one network, and
# `nonce`, which the caller sets to distinguish otherwise identical transactions.
# Both were parsed and discarded, so two signing requests differing only in their
# replay domain were shown identically: the user could not tell a retry of the same
# transaction from a distinct one, or a mainnet transaction from a testnet one.
#
# This fixture is test_sign_tx_sui_valid_during_address_balance_gas's transaction
# with only its replay domain changed. Everything the old review displayed -- from,
# to, amount, max gas -- is byte-identical to that test's, so the two are
# distinguishable only if the replay domain itself is on screen.
#
# Two things assert it: the navigation pages through the review searching for the
# nonce as displayed and gives up if no screen carries it, and the goldens pin the
# screens themselves. Dropping the field trips the goldens first, on the page count.

import base64

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently

PATH = "m/44'/784'/0'/0'/0'"

NONCE = 0xDEADBEEF
NONCE_AS_DISPLAYED = "3735928559"

# Sui mainnet's genesis checkpoint digest, which is what ValidDuring.chain carries.
# Its first four bytes, 35834a8a, are the chain identifier Sui publishes.
MAINNET_CHAIN = bytes.fromhex(
    "35834a8ac17ca48fb14ac8f99c17c98747e95dd07294ae41a46b382246a4499b"
)
CHAIN_OFFSET = 168
MAINNET_AS_DISPLAYED = "Sui Mainnet"

# Same transaction as the upstream address-balance test, with the replay domain
# changed: nonce 42 -> 0xDEADBEEF, and chain (all zeros there) -> mainnet.
VALID_DURING_TX = base64.b64decode(
    "AAAAAAACACAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAAIQEIPAAAAAAACAgABAQEA"
    "AQECAAABAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQBvsh/urQJ9pIcyla/9bE82"
    "GP4Xb6L78+e17x2UY7MeIegDAAAAAAAA8EkCAAAAAAACAQAAAAAAAAAAAQEAAAAAAAAAAAAgNYNK"
    "isF8pI+xSsj5nBfJh0fpXdBylK5BpGs4IkakSZvvvq3e"
)


def _sanity_check_fixture():
    """The nonce must be the value the navigation searches for."""
    assert int.from_bytes(VALID_DURING_TX[-4:], "little") == NONCE, (
        "fixture nonce does not match the value this test looks for on screen"
    )
    assert str(NONCE) == NONCE_AS_DISPLAYED
    assert VALID_DURING_TX[CHAIN_OFFSET:CHAIN_OFFSET + 32] == MAINNET_CHAIN, (
        "fixture chain is not the mainnet digest this test looks for on screen"
    )


def _approve_after_finding(text, firmware, navigator, scenario_navigator):
    """Page through the review until `text` shows, then approve.

    Paging by text rather than by a fixed click count: the review length depends on
    which optional fields are present, so a hard-coded list would need retuning
    whenever one is added. If the field is missing this gives up rather than
    approving something it never saw.
    """
    nano = firmware.device.startswith("nano")
    navigator.navigate_until_text_and_compare(
        NavInsID.RIGHT_CLICK if nano else NavInsID.USE_CASE_REVIEW_TAP,
        [NavInsID.BOTH_CLICK] if nano else [NavInsID.USE_CASE_REVIEW_CONFIRM],
        text,
        scenario_navigator.screenshot_path,
        scenario_navigator.test_name,
        screen_change_after_last_instruction=False,
    )


def test_valid_during_nonce_reaches_the_review(
    backend, scenario_navigator, firmware, navigator
):
    _sanity_check_fixture()
    client = Client(backend, use_block_protocol=True)
    _, public_key, _, _ = client.get_public_key(path=PATH)

    def apdu_task():
        return client.sign_tx(
            path=PATH, transaction=VALID_DURING_TX, object_list=[]
        )

    def nav_task():
        _approve_after_finding(
            NONCE_AS_DISPLAYED, firmware, navigator, scenario_navigator
        )

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, VALID_DURING_TX)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)


# A chain identifier the app knows is shown by name. Devnet and local networks are
# regenerated and have no fixed digest, so those keep showing the raw value -- the
# address-balance tests, whose chain is all zeros, cover that path.
def test_valid_during_known_chain_shown_by_name(
    backend, scenario_navigator, firmware, navigator
):
    _sanity_check_fixture()
    client = Client(backend, use_block_protocol=True)
    _, public_key, _, _ = client.get_public_key(path=PATH)

    def apdu_task():
        return client.sign_tx(path=PATH, transaction=VALID_DURING_TX, object_list=[])

    def nav_task():
        _approve_after_finding(
            MAINNET_AS_DISPLAYED, firmware, navigator, scenario_navigator
        )

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, VALID_DURING_TX)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)
