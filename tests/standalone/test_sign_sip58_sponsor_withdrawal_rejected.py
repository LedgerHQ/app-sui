# A SIP-58 FundsWithdrawal drawing on the gas sponsor's address balance, in a
# transaction this device is only sponsoring, must not be clear-signed.
#
# FundsWithdrawalArg.withdraw_from says whose address balance a withdrawal spends:
# the transaction sender's, or the gas sponsor's. It used to be parsed and thrown
# away, so nothing downstream could tell them apart. The coin::send_funds change
# check compares its recipient against TransactionData.sender only, so for a
# sponsor-funded withdrawal in a sponsored transaction that check passes while the
# money came from the sponsor -- letting the undisplayed remainder go to the sender.
#
# The gate is deliberately narrow. When sender and sponsor are the same account --
# the ordinary case, where you are both sending and paying -- Sponsor and Sender
# resolve to one address, the change check is already correct, and clear signing is
# safe. Only the sponsored combination is refused, so withdrawing from your own
# balance still clear-signs (test_sign_sui_funds_withdrawal and the other SIP-58
# tests cover that) and so does sponsorship itself.
#
# Carrying the withdrawal source through redeem_funds -> split -> send_funds is the
# real fix; until then this routes to the not-recognized path, so with blind signing
# off the command is refused.
#
# The fixture is test_sign_tx_sui_funds_withdrawal's transaction with two edits: the
# withdraw_from byte flipped from 0 (Sender) to 1 (Sponsor), and TransactionData.sender
# changed to another account so that the device -- which remains GasData.owner -- is
# only the sponsor. Note this exercises the gate, not the full exploit chain: this
# transaction uses balance::send_funds, so self_deposit is never set and the
# coin::send_funds remainder diversion is not reproduced here.

import base64

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently

PATH = "m/44'/784'/0'/0'/0'"
LEDGER_ADDRESS = "6fb21feead027da4873295affd6c4f3618fe176fa2fbf3e7b5ef1d9463b31e21"
SPONSORED_SENDER = "1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488"

# Byte offsets within the transaction, all verified by _sanity_check_fixture below.
# withdraw_from follows the reservation amount and the 0x2::sui::SUI type tag;
# sender precedes GasData, which is payment(1 objectref) || owner || price || budget.
WITHDRAW_FROM_OFFSET = 59
SENDER_OFFSET = 476
GAS_OWNER_OFFSET = 582
WITHDRAW_FROM_SPONSOR = 1

SPONSOR_WITHDRAWAL_TX = base64.b64decode(
    "AAAAAAADAgCgVgkAAAAAAAAHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NV"
    "SQABAAigVgkAAAAAAAAgHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgDAAAAAAAAAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACEWZ1bmRzX2FjY3VtdWxhdG9yEHdpdGhkcmF3YWxfc3Bs"
    "aXQBBwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACB2JhbGFuY2UHQmFsYW5jZQEHAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQACAQAAAQEAAAAAAAAAAAAAAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAAACB2JhbGFuY2UMcmVkZWVtX2Z1bmRzAQcAAAAAAAAAAAAAAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAECAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    "AAAAAAAAAAIHYmFsYW5jZQpzZW5kX2Z1bmRzAQcAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    "AAAAAgNzdWkDU1VJAAICAQABAgAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAFADb3P"
    "7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2QhEAAAAAIGbFq2VJip03FgAaA0gV/0q8p2X3"
    "9vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiHoAwAAAAAAACChBwAA"
    "AAAAAA=="
)

OBJECT_LIST = [
    base64.b64decode(
        "AAEB03ZCEQAAAAAoQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvIYR4MtEtyGd4QDpQ5AAAAAABvsh/u"
        "rQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeISB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9a"
        "sLM9x2ATDwAAAAAA"
    )
]


def _sanity_check_fixture():
    """Both halves of the refused combination must actually be present."""
    tx = SPONSOR_WITHDRAWAL_TX
    assert tx[WITHDRAW_FROM_OFFSET] == WITHDRAW_FROM_SPONSOR, (
        "withdraw_from is not WithdrawFrom::Sponsor"
    )
    assert tx[SENDER_OFFSET:SENDER_OFFSET + 32].hex() == SPONSORED_SENDER, (
        "TransactionData.sender is not the other account"
    )
    assert tx[GAS_OWNER_OFFSET:GAS_OWNER_OFFSET + 32].hex() == LEDGER_ADDRESS, (
        "GasData.owner is not this device, so the transaction is not sponsored by it"
    )


def test_sip58_sponsor_withdrawal_not_clear_signed(
    backend, scenario_navigator, firmware, navigator
):
    _sanity_check_fixture()
    client = Client(backend, use_block_protocol=True)

    def apdu_task():
        return client.sign_tx(
            path=PATH, transaction=SPONSOR_WITHDRAWAL_TX, object_list=OBJECT_LIST
        )

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK,
                              NavInsID.RIGHT_CLICK, NavInsID.BOTH_CLICK],
                timeout=10,
                test_case_name=scenario_navigator.test_name,
                path=scenario_navigator.screenshot_path,
                screen_change_before_first_instruction=True,
                screen_change_after_last_instruction=False,
            )
        else:
            # Clear signing failed -> dismiss the "Enable Blind signing" screen.
            navigator.navigate([NavInsID.USE_CASE_CHOICE_REJECT],
                               screen_change_before_first_instruction=False,
                               screen_change_after_last_instruction=False)

    def check_result(result):
        pytest.fail(
            "a sponsored transaction drawing on the sponsor's address balance must "
            "not be clear-signed: the device cannot tell whose funds moved, so the "
            "send_funds change check would compare against the wrong account"
        )

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0


# The same Sponsor-funded withdrawal, but sent by this device rather than merely
# sponsored by it. Sponsor and Sender then name the same account, so there is no
# misbinding to exploit and the review is accurate: this must still clear-sign.
#
# This is the guard against over-rejecting. An earlier version of the fix refused
# WithdrawFrom::Sponsor outright, which turned every legitimate own-balance
# withdrawal using that variant into a blind-signing prompt -- a functional
# regression with no security benefit.
OWN_BALANCE_SPONSOR_VARIANT_TX = base64.b64decode(
    "AAAAAAADAgCgVgkAAAAAAAAHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NV"
    "SQABAAigVgkAAAAAAAAgHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgDAAAAAAAAAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACEWZ1bmRzX2FjY3VtdWxhdG9yEHdpdGhkcmF3YWxfc3Bs"
    "aXQBBwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACB2JhbGFuY2UHQmFsYW5jZQEHAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQACAQAAAQEAAAAAAAAAAAAAAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAAACB2JhbGFuY2UMcmVkZWVtX2Z1bmRzAQcAAAAAAAAAAAAAAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAECAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    "AAAAAAAAAAIHYmFsYW5jZQpzZW5kX2Z1bmRzAQcAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    "AAAAAgNzdWkDU1VJAAICAQABAgBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQFADb3P"
    "7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2QhEAAAAAIGbFq2VJip03FgAaA0gV/0q8p2X3"
    "9vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiHoAwAAAAAAACChBwAA"
    "AAAAAA=="
)


def test_sip58_own_balance_sponsor_variant_still_clear_signed(
    backend, scenario_navigator, firmware, navigator
):
    tx = OWN_BALANCE_SPONSOR_VARIANT_TX
    assert tx[WITHDRAW_FROM_OFFSET] == WITHDRAW_FROM_SPONSOR
    assert tx[SENDER_OFFSET:SENDER_OFFSET + 32] == tx[GAS_OWNER_OFFSET:GAS_OWNER_OFFSET + 32], (
        "fixture must be self-sent: sender and gas owner have to be the same account"
    )
    assert tx[SENDER_OFFSET:SENDER_OFFSET + 32].hex() == LEDGER_ADDRESS

    client = Client(backend, use_block_protocol=True)
    _, public_key, _, _ = client.get_public_key(path=PATH)

    def apdu_task():
        return client.sign_tx(path=PATH, transaction=tx, object_list=OBJECT_LIST)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[
                    NavInsID.RIGHT_CLICK,
                    NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK,
                    NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK,
                    NavInsID.RIGHT_CLICK,
                    NavInsID.RIGHT_CLICK,
                    NavInsID.BOTH_CLICK,
                ],
                timeout=10,
                test_case_name=scenario_navigator.test_name,
                path=scenario_navigator.screenshot_path,
                screen_change_before_first_instruction=True,
                screen_change_after_last_instruction=False,
            )
        else:
            scenario_navigator.review_approve()

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, tx)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)
