# A sponsored transaction, where this device pays the gas but some other account
# is the sender.
#
#   TransactionData.sender = SPONSORED_SENDER (not this device)
#   GasData.owner          = this device
#   PTB                    = request_add_stake(system, Argument::GasCoin, validator)
#
# Sui accepts a gas-owner signature as authorization in its own right, and
# Argument::GasCoin resolves to the *sponsor's* coin. So this stakes this device's
# SUI, and the unused StakedSui result accrues to the sender rather than to the
# signer. Sponsorship is a legitimate Sui feature and stays signable; the security
# requirement is that the review says whose transaction it is.
#
# Before the fix it could not: gas_data_parser discarded GasData.owner and the
# review rendered "From" off the signing path, so a sponsored stake was
# indistinguishable from the user staking their own SUI.
#
# The fixture is test_sign_stake_gas_coin's transaction with GasData.owner
# rewritten to the address of the signing path (m/44'/784'/0'/0'/1' = f65c72ab...,
# confirmed against the device). Its sender is simply the one the upstream fixture
# already had; nothing about that account is special.

import base64

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently

PATH = "m/44'/784'/0'/0'/1'"
LEDGER_ADDRESS = "f65c72abf52307bc1bd3c199534aaf04eb56c7be96ae2e74271b09508412e8fb"
SPONSORED_SENDER = "1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488"

SPONSORED_TX = base64.b64decode(
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


def _sanity_check_fixture():
    """The fixture must actually carry the sponsored shape."""
    tail = SPONSORED_TX[-49:]  # owner(32) || price(8) || budget(8) || expiration(1)
    assert tail[:32].hex() == LEDGER_ADDRESS, "GasData.owner is not the Ledger address"
    assert bytes.fromhex(SPONSORED_SENDER) in SPONSORED_TX, "sender was lost"


# Sponsoring stays possible - Sui supports it and refusing would remove a real
# feature - so the requirement is disclosure: the review must name the account
# that actually sent the transaction, and must not read as an ordinary stake by
# the signing address. The golden snapshots are the assertion; they must contain
# a "Sent by" field carrying SPONSORED_SENDER.
def test_sponsored_stake_of_ledger_gas_coin(
    backend, scenario_navigator, firmware, navigator
):
    _sanity_check_fixture()
    client = Client(backend, use_block_protocol=True)

    _, public_key, _, _ = client.get_public_key(path=PATH)
    assert public_key.hex() != ""

    outcome = {}

    def apdu_task():
        try:
            outcome["signature"] = client.sign_tx(
                path=PATH, transaction=SPONSORED_TX, object_list=OBJECT_LIST
            )
        except Exception as exc:  # noqa: BLE001
            outcome["error"] = exc
        return outcome

    def nav_task():
        # Unlike the older sign tests this does not hard-code a per-device click
        # count: the disclosure adds screens, and on nano an address spans several,
        # so a fixed list would need retuning whenever a field moves. Drive to the
        # confirmation screen by its text instead. scenario_navigator.review_approve
        # is not used because on nano it walks past that screen onto "Reject".
        try:
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
        except Exception as exc:  # noqa: BLE001 - a rejection means no review to drive
            outcome["nav_error"] = exc

    def check_result(_result):
        pass

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    sig = outcome.get("signature")
    valid = (
        check_signature_validity(public_key, sig, SPONSORED_TX)
        if sig is not None
        else None
    )
    print(f"\nSPONSORED_OUTCOME signed={sig is not None} sig_valid_over_tx={valid} "
          f"error={outcome.get('error')!r} nav_error={outcome.get('nav_error')!r}")

    # Signing is allowed; what must not happen is signing it silently. The user
    # accepted a review that named the sender, so a signature here is correct.
    assert "error" not in outcome, (
        f"Sponsored transaction was rejected outright: {outcome['error']!r}. "
        "Sponsorship is a supported Sui feature; the fix is to disclose it."
    )
    assert sig is not None and valid, (
        "Device did not produce a valid signature after the sponsored review was "
        "approved"
    )
