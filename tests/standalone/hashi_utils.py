# Shared helpers for the Hashi tests. Fixtures come from tests/tools/hashi-fixtures
# (pinned @mysten/hashi builders); see that generator for how each one is built.

import base64
import json
from pathlib import Path

import pytest
from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently

_FIXTURES = json.loads((Path(__file__).parent / "hashi_fixtures.json").read_text())

# The fixtures' sender is the address this path derives.
PATH = "m/44'/784'/12345'"
SENDER = bytes.fromhex(_FIXTURES["_meta"]["sender"][2:])
OTHER = bytes.fromhex("5a64eec558ee719741578942714a0b35058ced15d79f4af64da014715ada4497")

HASHI_OBJECT = bytes.fromhex(_FIXTURES["_meta"]["hashiObject"][2:])
TYPE_PACKAGE = bytes.fromhex(_FIXTURES["_meta"]["typePackage"][2:])
# The deployment is at its first version, so Move calls target the type package.
CALL_PACKAGE = TYPE_PACKAGE
# The same for the mainnet deployment, whose fixtures are named `mainnet_*`.
MAINNET_HASHI_OBJECT = bytes.fromhex(_FIXTURES["_meta"]["mainnetHashiObject"][2:])
MAINNET_PACKAGE = bytes.fromhex(_FIXTURES["_meta"]["mainnetTypePackage"][2:])
CLOCK = bytes(31) + b"\x06"
BTC_TYPE_TAIL = b"\x03btc\x03BTC"

# What a rejected, unrecognised transaction answers: `SyscallError::NotSupported`.
# A cancelled review answers 0x6985 instead, so checking this tells a transaction that
# never reached a review apart from one that did and was refused by the navigation.
SW_NOT_SUPPORTED = 0x0008


def fixture(name):
    f = _FIXTURES[name]
    return base64.b64decode(f["tx"]), [base64.b64decode(o) for o in f["objects"]]


def patch_once(tx, old, new):
    """Replace the single occurrence of `old`, failing loudly if it is not unique."""
    assert len(old) == len(new)
    assert tx.count(old) == 1, f"expected exactly one occurrence, found {tx.count(old)}"
    return tx.replace(old, new)


def retarget_calls(tx, package, modules=(b"withdraw", b"utxo", b"deposit"), source=CALL_PACKAGE):
    """Point the Hashi Move calls of `modules` from `source` at `package`, leaving the
    `btc::BTC` type tags alone."""
    n = 0
    for module in modules:
        call = source + bytes([len(module)]) + module
        n += tx.count(call)
        tx = tx.replace(call, package + call[32:])
    assert n > 0, "no Hashi call to retarget"
    return tx


def retype_btc(tx, source, package):
    """Replace every `source::btc::BTC` type tag with `package::btc::BTC`."""
    old = source + BTC_TYPE_TAIL
    assert tx.count(old) > 0, "no hBTC type tag"
    return tx.replace(old, package + BTC_TYPE_TAIL)


def patch_last(tx, old, new):
    assert len(old) == len(new) and tx.count(old) >= 1
    i = tx.rindex(old)
    return tx[:i] + new + tx[i + len(old):]


def _client(backend):
    client = Client(backend, use_block_protocol=True)
    _, public_key, _, address = client.get_public_key(path=PATH)
    assert len(public_key) == 32
    assert address == SENDER, "the fixtures' sender must be the signing account"
    return client, public_key


def sign_and_approve(name, case, backend, scenario_navigator, firmware, navigator, tx=None):
    """Sign a fixture, paging through its review to the final screen, then approve."""
    client, public_key = _client(backend)
    fixture_tx, objects = fixture(name)
    transaction = tx if tx is not None else fixture_tx

    def apdu_task():
        return client.sign_tx(path=PATH, transaction=transaction, object_list=objects)

    def nav_task():
        nano = firmware.device.startswith("nano")
        navigator.navigate_until_text_and_compare(
            NavInsID.RIGHT_CLICK if nano else NavInsID.USE_CASE_REVIEW_TAP,
            [NavInsID.BOTH_CLICK] if nano else [NavInsID.USE_CASE_REVIEW_CONFIRM],
            "Sign transaction",
            scenario_navigator.screenshot_path,
            f"{scenario_navigator.test_name}_{case}",
            screen_change_after_last_instruction=False,
        )

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, transaction)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)


def sign_expect_not_recognized(name, backend, firmware, navigator, tx=None):
    """Sign a transaction that must not be clear-signed: with blind signing off (the
    default), the device warns it cannot be clear-signed and refuses."""
    client, _ = _client(backend)
    fixture_tx, objects = fixture(name)
    transaction = tx if tx is not None else fixture_tx

    def apdu_task():
        return client.sign_tx(path=PATH, transaction=transaction, object_list=objects)

    def nav_task():
        # "This transaction cannot be clear-signed" -> "Reject transaction".
        if firmware.device.startswith("nano"):
            instructions = [NavInsID.RIGHT_CLICK] * 3 + [NavInsID.BOTH_CLICK]
        else:
            instructions = [NavInsID.USE_CASE_CHOICE_REJECT]
        navigator.navigate(
            instructions,
            screen_change_before_first_instruction=False,
            screen_change_after_last_instruction=False,
        )

    def check_result(result):
        pytest.fail("this transaction must not be clear-signed")

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)
    assert e.value.status == SW_NOT_SUPPORTED, hex(e.value.status)
    assert len(e.value.data) == 0
