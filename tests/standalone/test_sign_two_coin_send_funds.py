# Two independent SIP-58 withdraw/send pairs in one transaction.
#
# `0x2::coin::send_funds` moves a redeemed coin into an address balance and never
# appears on the review, which shows only the TransferObjects destination. The
# device therefore has to prove every hidden deposit returns to the sender. It
# records the recipient in a single Option, so two send_funds calls with different
# recipients used to be a hole in one direction: the first (large) deposit to the
# attacker was overwritten by a second (small) deposit back to the sender, and the
# surviving value passed the sender check while the review showed only the small
# visible transfer.
#
#   [0] coin::redeem_funds<SUI>(withdrawal A)   -> the whole reserved balance
#   [1] SplitCoins(coin[0], [visible amount])   -> the only figure on screen
#   [2] coin::send_funds<SUI>(coin[0], X)       -> hidden: remainder of A
#   [3] coin::redeem_funds<SUI>(withdrawal B)
#   [4] coin::send_funds<SUI>(coin[3], Y)       -> hidden: all of B
#   [5] TransferObjects([split], recipient)     -> the visible transfer
#
# With X = attacker and Y = sender the attacker takes A's remainder; the reverse
# order was already refused because the surviving recipient was not the sender. Both
# are now refused, and the sender/sender case still clear-signs.
#
# The PTBs are built here rather than pasted as opaque base64 because the shape is
# the whole point of the test, and because a hand-edited fixture cannot be trusted
# to still be the transaction it claims to be. `test_builder_reproduces_production_fixture`
# pins the encoder: it rebuilds, byte for byte, the @mysten/sui@2.23.1 transaction
# that test_sign_sui_transfer_coin_send_funds signs. If that passes, the crafted PTBs
# below differ from production output only in the ways written out above.

import base64

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently

PATH = "m/44'/784'/0'/0'/0'"
SENDER = "6fb21feead027da4873295affd6c4f3618fe176fa2fbf3e7b5ef1d9463b31e21"
RECIPIENT = "f65c72abf52307bc1bd3c199534aaf04eb56c7be96ae2e74271b09508412e8fb"
ATTACKER = "1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488"

WITHDRAWAL_A = 10_000_000_000  # 10 SUI reserved; only VISIBLE of it is displayed
WITHDRAWAL_B = 1_000_000
VISIBLE = 100_000_000  # 0.1 SUI - the sole amount the review shows

SUI_FRAMEWORK = "00" * 31 + "02"


# ---------------------------------------------------------------- BCS encoding
def _uleb(n):
    out = bytearray()
    while True:
        b, n = n & 0x7F, n >> 7
        out.append(b | 0x80 if n else b)
        if not n:
            return bytes(out)


def _u64(n):
    return n.to_bytes(8, "little")


def _vec(items):
    return _uleb(len(items)) + b"".join(items)


def _str(text):
    return _uleb(len(text.encode())) + text.encode()


def _addr(h):
    return bytes.fromhex(h)


# StructTag: address || module || name || type_params.
SUI_COIN_STRUCT = _addr(SUI_FRAMEWORK) + _str("sui") + _str("SUI") + _vec([])
# TypeTag::Struct and TypeInput::Struct are separate schemas that happen to encode
# identically for a plain struct (variant 7 followed by the StructTag). Named apart
# so the two use sites below say which one they mean.
SUI_TYPE_TAG = _uleb(7) + SUI_COIN_STRUCT    # the withdrawal's type_arg
SUI_TYPE_INPUT = _uleb(7) + SUI_COIN_STRUCT  # a MoveCall type argument

# Argument enum
def _input(i):
    return _uleb(1) + i.to_bytes(2, "little")


def _result(i):
    return _uleb(2) + i.to_bytes(2, "little")


def _nested_result(i, j):
    return _uleb(3) + i.to_bytes(2, "little") + j.to_bytes(2, "little")


# CallArg enum
def _pure(raw):
    return _uleb(0) + _uleb(len(raw)) + raw


def _funds_withdrawal(amount):
    # FundsWithdrawalArg { reservation: enum(0) u64, type_arg: enum(0) TypeTag,
    #                      withdraw_from: enum(0) == Sender }
    return _uleb(2) + _uleb(0) + _u64(amount) + _uleb(0) + SUI_TYPE_TAG + _uleb(0)


# Command enum
def _move_call(module, function, args):
    return (_uleb(0) + _addr(SUI_FRAMEWORK) + _str(module) + _str(function)
            + _vec([SUI_TYPE_INPUT]) + _vec(args))


def _split_coins(coin, amounts):
    return _uleb(2) + coin + _vec(amounts)


def _transfer_objects(objects, recipient):
    return _uleb(1) + _vec(objects) + recipient


def _object_ref(obj_id, version, digest):
    raw = _addr(digest)
    return _addr(obj_id) + _u64(version) + _uleb(len(raw)) + raw


# The gas payment of the production fixture. object_list is empty in these tests, so
# the device cannot resolve its balance and treats the total as unknown -- which is
# what the upstream coin_send_funds test does too, and is fine because none of these
# transactions moves the gas coin itself.
GAS_PAYMENT = [_object_ref(
    "bfac23cd5a8e4fbba56dbf91015b3f74ba284af400393a419b1d869dcfd2b8cf",
    0x1948B232,
    "c8922d62cb80474907ecfcc1643847b9759766759fbc5e0978d20cc58cbeea61",
)]


def _build(inputs, commands, sender=SENDER, gas_owner=SENDER):
    return (b"\x00\x00\x00"      # IntentMessage: TransactionData / V0 / Sui
            + _uleb(0)           # TransactionData::V1
            + _uleb(0)           # TransactionKind::ProgrammableTransaction
            + _vec(inputs) + _vec(commands)
            + _addr(sender)
            + _vec(GAS_PAYMENT) + _addr(gas_owner)
            + _u64(1000) + _u64(5_000_000)
            + b"\x00")           # TransactionExpiration::None


def _two_send_tx(first_deposit, second_deposit):
    """Two withdraw/send pairs plus one visible transfer. Sender pays its own gas,
    so none of the sponsorship gates apply and only the deposit check can fire."""
    inputs = [
        _pure(_addr(RECIPIENT)),         # 0 visible transfer destination
        _funds_withdrawal(WITHDRAWAL_A),  # 1
        _pure(_u64(VISIBLE)),            # 2 split amount == the displayed figure
        _pure(_addr(first_deposit)),     # 3 hidden deposit #1
        _funds_withdrawal(WITHDRAWAL_B),  # 4
        _pure(_addr(second_deposit)),    # 5 hidden deposit #2
    ]
    commands = [
        _move_call("coin", "redeem_funds", [_input(1)]),
        _split_coins(_result(0), [_input(2)]),
        _move_call("coin", "send_funds", [_result(0), _input(3)]),
        _move_call("coin", "redeem_funds", [_input(4)]),
        _move_call("coin", "send_funds", [_result(3), _input(5)]),
        _transfer_objects([_nested_result(1, 0)], _input(0)),
    ]
    return _build(inputs, commands)


MALICIOUS_FIRST = _two_send_tx(ATTACKER, SENDER)
MALICIOUS_LAST = _two_send_tx(SENDER, ATTACKER)
BOTH_TO_SENDER = _two_send_tx(SENDER, SENDER)


# The @mysten/sui@2.23.1 transaction from test_sign_sui_transfer_coin_send_funds.
PRODUCTION_FIXTURE = base64.b64decode(
    "AAAAAAAEACD2XHKr9SMHvBvTwZlTSq8E61bHvpauLnQnGwlQhBLo+wIAAOH1BQAAAAAABwAAAAAAAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACA3N1aQNTVUkAAAAIAOH1BQAAAAAAIG+yH+6tAn2khzKVr/1s"
    "TzYY/hdvovvz57XvHZRjsx4hBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgRjb2luDHJl"
    "ZGVlbV9mdW5kcwEHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIDc3VpA1NVSQABAQEAAgIA"
    "AAEBAgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIEY29pbgpzZW5kX2Z1bmRzAQcAAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgNzdWkDU1VJAAICAAABAwABAQMBAAAAAQAAb7If7q0C"
    "faSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiEBv6wjzVqOT7ulbb+RAVs/dLooSvQAOTpBmx2Gnc/SuM8"
    "yskgZAAAAACDIki1iy4BHSQfs/MFkOEe5dZdmdZ+8Xgl40gzFjL7qYW+yH+6tAn2khzKVr/1sTzYY/h"
    "dvovvz57XvHZRjsx4h6AMAAAAAAABAS0wAAAAAAAA="
)


def test_builder_reproduces_production_fixture():
    """The encoder must agree with real @mysten/sui output, or nothing built here
    can be trusted to be the transaction it claims to be."""
    rebuilt = _build(
        [
            _pure(_addr(RECIPIENT)),
            _funds_withdrawal(VISIBLE),
            _pure(_u64(VISIBLE)),
            _pure(_addr(SENDER)),
        ],
        [
            _move_call("coin", "redeem_funds", [_input(1)]),
            _split_coins(_result(0), [_input(2)]),
            _move_call("coin", "send_funds", [_result(0), _input(3)]),
            _transfer_objects([_nested_result(1, 0)], _input(0)),
        ],
    )
    assert rebuilt == PRODUCTION_FIXTURE, (
        "the BCS encoder no longer reproduces the production fixture byte for byte"
    )


def _sanity_check(tx, first_deposit, second_deposit):
    """The fixture must be the case the test name claims, in the order it claims."""
    # BCS length-prefixed identifiers: 0x0c == len("redeem_funds"), 0x0a == len("send_funds")
    assert tx.count(b"\x0credeem_funds") == 2, "expected two coin::redeem_funds calls"
    assert tx.count(b"\nsend_funds") == 2, "expected two coin::send_funds calls"
    for dep in (first_deposit, second_deposit):
        assert _addr(dep) in tx, "a deposit recipient is missing from the fixture"
    if first_deposit != second_deposit:
        assert tx.index(_addr(first_deposit)) < tx.index(_addr(second_deposit)), (
            "the two deposits appear in the opposite order to the case under test"
        )


def _expect_not_clear_signed(tx, backend, scenario_navigator, firmware, navigator, why):
    client = Client(backend, use_block_protocol=True)

    def apdu_task():
        return client.sign_tx(path=PATH, transaction=tx, object_list=[])

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
        pytest.fail(why)

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0


# The direction the single-Option recipient used to miss: the large hidden deposit
# to the attacker comes first and is overwritten by a small one back to the sender.
def test_two_send_funds_malicious_first_rejected(
    backend, scenario_navigator, firmware, navigator
):
    _sanity_check(MALICIOUS_FIRST, ATTACKER, SENDER)
    _expect_not_clear_signed(
        MALICIOUS_FIRST, backend, scenario_navigator, firmware, navigator,
        "a transaction whose first hidden coin::send_funds pays an account other "
        "than the sender must not be clear-signed, even when a later send_funds "
        "returns to the sender",
    )


# The reverse order. This was already refused before the overwrite guard, because
# the surviving recipient was the attacker and failed the sender check; it is kept
# so that a future change cannot silently lose the easier half.
def test_two_send_funds_malicious_last_rejected(
    backend, scenario_navigator, firmware, navigator
):
    _sanity_check(MALICIOUS_LAST, SENDER, ATTACKER)
    _expect_not_clear_signed(
        MALICIOUS_LAST, backend, scenario_navigator, firmware, navigator,
        "a transaction whose last hidden coin::send_funds pays an account other "
        "than the sender must not be clear-signed",
    )


# The anti-over-rejection control: repeated deposits are refused only when they
# name different recipients, so a transaction whose every hidden send_funds returns
# to the sender stays supported. Nothing leaves the sender's control here, and the
# review is accurate.
def test_two_send_funds_both_to_sender_clear_signs(
    backend, scenario_navigator, firmware, navigator
):
    _sanity_check(BOTH_TO_SENDER, SENDER, SENDER)
    client = Client(backend, use_block_protocol=True)

    _, public_key, _, _ = client.get_public_key(path=PATH)
    assert len(public_key) == 32

    def apdu_task():
        return client.sign_tx(path=PATH, transaction=BOTH_TO_SENDER, object_list=[])

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[
                    NavInsID.RIGHT_CLICK,  # Review transaction to send SUI
                    NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK,  # From ...
                    NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK,  # To ...
                    NavInsID.RIGHT_CLICK,  # Amount
                    NavInsID.RIGHT_CLICK,  # Max Gas
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
        assert check_signature_validity(public_key, result, BOTH_TO_SENDER)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)
