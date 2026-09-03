# Sponsored stakes, where this device pays the gas but another account is the
# transaction sender.
#
#   TransactionData.sender = SPONSORED_SENDER (not this device)
#   GasData.owner          = this device
#
# Sui accepts a gas-owner signature as authorization in its own right, so the device
# can be made to fund a transaction it did not send. Two facts decide what is safe:
#
#   * request_add_stake transfers the resulting StakedSui to tx_context::sender(),
#     so on a sponsored stake the position belongs to the sender, never to the
#     signer who paid for it.
#   * Of all the objects a sponsored transaction touches, only the gas coin may
#     belong to the sponsor; every other input has to be owned by the sender.
#
# Together those make `Argument::GasCoin` in the stake principal exactly the case
# where this device supplies the funds and somebody else keeps the position -- and
# since gas is charged out of that same coin, the amount given away is not even
# exactly knowable before execution. The app refuses to clear-sign it, in all three
# ways the gas coin can reach a stake: directly, inside the coin vector of
# request_add_stake_mul_coin, and as the result of a split off it.
#
# A sponsored stake funded from the sender's own coins is a different matter: the
# device only pays gas, which is what sponsoring means. That stays clear-signable,
# and the review names the account the position accrues to.
#
# Each fixture is an existing non-sponsored stake test's transaction with GasData.owner
# rewritten to the address of the signing path (m/44'/784'/0'/0'/1' = f65c72ab...,
# confirmed against the device), which is the whole edit: the fixtures already had
# 1d3f2643... as sender, so moving the gas owner to this device is what makes them
# sponsored. Bases: test_sign_stake_gas_coin, test_sign_stake_mul_coin_gas,
# test_sign_stake_split_gas and test_sign_stake_whole_coin.

import base64

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently

PATH = "m/44'/784'/0'/0'/1'"
LEDGER_ADDRESS = "f65c72abf52307bc1bd3c199534aaf04eb56c7be96ae2e74271b09508412e8fb"
SPONSORED_SENDER = "1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488"

# request_add_stake(system, Argument::GasCoin, validator)
GAS_COIN_STAKE_TX = base64.b64decode(
    "AAAAAAACAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABQEAAAAAAAAAAQAgNfXxVPAR"
    "dGTjN5xFx/PLay1O/t8wCsrf+Kfo6eOhUQkBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    "AAADCnN1aV9zeXN0ZW0RcmVxdWVzdF9hZGRfc3Rha2UAAwEAAAABAQAdPyZDMFdgIm5RjJtalhZT"
    "g4CN2XeXH3PeqXFUOwvkiATr/xa00ggasG0dUlHJiCCGQeXFAcf6i9zpyLewkIuna9V2QhEAAAAA"
    "IDv/BHzKDboSa5D15/D3Jqu9o4dWUhvcE6bym/knjbDkqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3"
    "d9/b115oq23UdkIRAAAAACCsVYpX4/44Cp2BWe8aVkACUW5rxtsErjUPJ6nMxaCvvR+Hb/AUQ4bc"
    "9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJK"
    "W2+uGgXzUbocEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDn"
    "y0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/z9lxyq/UjB7wb08GZU0qvBOtWx76Wri50JxsJUIQS6Pvo"
    "AwAAAAAAAHjgAQAAAAAAAA=="
)

# request_add_stake_mul_coin(system, MakeMoveVec([Input0, GasCoin]), None, validator)
MUL_COIN_GAS_STAKE_TX = base64.b64decode(
    "AAAAAAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgK"
    "nYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    "BQEAAAAAAAAAAQABAAAgNfXxVPARdGTjN5xFx/PLay1O/t8wCsrf+Kfo6eOhUQkCBQACAAEAAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAwpzdWlfc3lzdGVtGnJlcXVlc3RfYWRkX3N0"
    "YWtlX211bF9jb2luAAQBAQACAAABAgABAwAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvk"
    "iAPr/xa00ggasG0dUlHJiCCGQeXFAcf6i9zpyLewkIuna9V2QhEAAAAAIDv/BHzKDboSa5D15/D3"
    "Jqu9o4dWUhvcE6bym/knjbDkH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOrTdkIRAAAA"
    "ACAhwBvcKbCI07AVHqsBdv+VAeBKtJ4AEkpbb64aBfNRuhwSvlQpOE0A7u9hJC8666vqwwElSd1v"
    "iI3BCHxNANqA0nZCEQAAAAAg4QExiF47MOfLRWHVOXDVXL9pTAmextLHibLy3JLKf/P2XHKr9SMH"
    "vBvTwZlTSq8E61bHvpauLnQnGwlQhBLo++gDAAAAAAAAeOABAAAAAAAA"
)

# SplitCoins(GasCoin, [6000000]) then request_add_stake(system, split_result, validator)
SPLIT_GAS_STAKE_TX = base64.b64decode(
    "AAAAAAADAAiAjVsAAAAAAAEBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAUBAAAAAAAA"
    "AAEAIDX18VTwEXRk4zecRcfzy2stTv7fMArK3/in6OnjoVEJAgIAAQEAAAAAAAAAAAAAAAAAAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAwpzdWlfc3lzdGVtEXJlcXVlc3RfYWRkX3N0YWtlAAMBAQADAAAA"
    "AAECAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIBOv/FrTSCBqwbR1SUcmIIIZB5cUB"
    "x/qL3OnIt7CQi6dr1XZCEQAAAAAgO/8EfMoNuhJrkPXn8Pcmq72jh1ZSG9wTpvKb+SeNsOSpP2xG"
    "T4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG"
    "2wSuNQ8nqczFoK+9H4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOrTdkIRAAAAACAhwBvc"
    "KbCI07AVHqsBdv+VAeBKtJ4AEkpbb64aBfNRuhwSvlQpOE0A7u9hJC8666vqwwElSd1viI3BCHxN"
    "ANqA0nZCEQAAAAAg4QExiF47MOfLRWHVOXDVXL9pTAmextLHibLy3JLKf/P2XHKr9SMHvBvTwZlT"
    "Sq8E61bHvpauLnQnGwlQhBLo++gDAAAAAAAAeOABAAAAAAAA"
)

# request_add_stake(system, Input0, validator), where Input0 is a coin object owned
# by SPONSORED_SENDER -- the device's coin pays only the gas.
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
# payment count below both locates the sender and checks the tail layout is what
# these assertions assume.
GAS_OWNER_FROM_END = 49
OBJECT_REF_LEN = 73  # id(32) || version(8) || digest(1 length byte + 32)


def _sender_offset(tx):
    owner_at = len(tx) - GAS_OWNER_FROM_END
    for count in range(1, 5):
        payment_at = owner_at - count * OBJECT_REF_LEN - 1
        if payment_at >= 32 and tx[payment_at] == count:
            return payment_at - 32
    raise AssertionError("could not locate GasData.payment in the fixture")


def _check_sponsored(tx):
    """Both halves of the sponsored shape must actually be present."""
    owner = tx[-GAS_OWNER_FROM_END:-17]
    assert owner.hex() == LEDGER_ADDRESS, "GasData.owner is not the Ledger address"
    off = _sender_offset(tx)
    assert tx[off:off + 32].hex() == SPONSORED_SENDER, (
        "TransactionData.sender is not the other account, so the fixture is not sponsored"
    )


def _expect_not_clear_signed(tx, backend, scenario_navigator, firmware, navigator, why):
    _check_sponsored(tx)
    client = Client(backend, use_block_protocol=True)

    def apdu_task():
        return client.sign_tx(path=PATH, transaction=tx, object_list=OBJECT_LIST)

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


def test_sponsored_stake_of_ledger_gas_coin(
    backend, scenario_navigator, firmware, navigator
):
    _expect_not_clear_signed(
        GAS_COIN_STAKE_TX, backend, scenario_navigator, firmware, navigator,
        "a sponsored stake of this device's gas coin must not be clear-signed: the "
        "device supplies the principal and the sender keeps the resulting position",
    )


def test_sponsored_stake_mul_coin_including_gas_coin(
    backend, scenario_navigator, firmware, navigator
):
    _expect_not_clear_signed(
        MUL_COIN_GAS_STAKE_TX, backend, scenario_navigator, firmware, navigator,
        "the gas coin reaching the stake principal inside a coin vector is the same "
        "value transfer as staking it directly, and must be refused the same way",
    )


def test_sponsored_stake_split_off_gas_coin(
    backend, scenario_navigator, firmware, navigator
):
    _expect_not_clear_signed(
        SPLIT_GAS_STAKE_TX, backend, scenario_navigator, firmware, navigator,
        "staking value split off this device's gas coin for another account's "
        "position must not be clear-signed",
    )


# The anti-over-rejection control, and the disclosure the refusals above make room
# for. Here the stake principal is a coin owned by the sender and this device only
# pays the gas -- which is what sponsoring a transaction means -- so the transaction
# stays clear-signable. The golden snapshots are the assertion: the review must carry
# a "Stake owner" field naming SPONSORED_SENDER, because that account, not the signer
# shown as "From", ends up owning the StakedSui.
def test_sponsored_stake_of_sender_coin_names_stake_owner(
    backend, scenario_navigator, firmware, navigator
):
    _check_sponsored(SENDER_COIN_STAKE_TX)
    client = Client(backend, use_block_protocol=True)

    _, public_key, _, _ = client.get_public_key(path=PATH)
    assert len(public_key) == 32

    def apdu_task():
        return client.sign_tx(
            path=PATH, transaction=SENDER_COIN_STAKE_TX, object_list=OBJECT_LIST
        )

    def nav_task():
        # No hard-coded click count: the disclosure adds a screen, and on nano an
        # address spans several, so a fixed list would need retuning whenever a
        # field moves. scenario_navigator.review_approve is not used on nano
        # because it walks past the confirmation screen onto "Reject".
        if firmware.device.startswith("nano"):
            navigator.navigate_until_text_and_compare(
                NavInsID.RIGHT_CLICK,
                [NavInsID.BOTH_CLICK],
                "Sign sponsored",
                scenario_navigator.screenshot_path,
                scenario_navigator.test_name,
                screen_change_after_last_instruction=False,
            )
        else:
            scenario_navigator.review_approve()

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, SENDER_COIN_STAKE_TX)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)
