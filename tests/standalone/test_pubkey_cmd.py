from time import time
import pytest

from application_client.client import Client, Errors
from contextlib import contextmanager
from ragger.bip import calculate_public_key_and_chaincode, CurveChoice
from ragger.navigator import NavInsID, NavIns
from utils import ROOT_SCREENSHOT_PATH, run_apdu_and_nav_tasks_concurrently
import speculos_stderr_tee

# Emulator stderr (alamgu-async-block) must not show these after a malformed chunked APDU;
# they indicate a broken async state rather than a clean protocol error.
_ASYNC_BLOCK_LOG_ERROR_MARKERS = (
    "Start response code received when expecting GetChunkResponse",
    "APDU handler future neither completed nor sent a command",
)


def _assert_no_alamgu_async_block_errors_in_speculos_log() -> None:
    log = speculos_stderr_tee.text()
    for marker in _ASYNC_BLOCK_LOG_ERROR_MARKERS:
        assert marker not in log, (
            "Unexpected alamgu-async-block error on Speculos stderr "
            f"(marker {marker!r}). Tail of log:\n{log[-12000:]}"
        )


# In this test we check that the GET_PUBLIC_KEY works in non-confirmation mode
def test_get_public_key_no_confirm(backend):
    for path in [ "m/44'/784'/0'"]:
        client = Client(backend, use_block_protocol=True)
        _, public_key, _, address = client.get_public_key(path=path)

        assert public_key.hex() == "6fc6f39448ad7af0953b78b16d0f840e6fe718ba4a89384239ff20ed088da2fa"
        assert address.hex() == "56b19e720f3bfa8caaef806afdd5dfaffd0d6ec9476323a14d1638ad734b2ba5"


# In this test we check that the GET_PUBLIC_KEY works in confirmation mode
def test_get_public_key_confirm_accepted(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'"

    def nav_task():
        scenario_navigator.address_review_approve()

    def apdu_task():
        return client.get_public_key_with_confirmation(path=path)

    def check_result(result):
        _, public_key, _, address = result
        assert public_key.hex() == "6fc6f39448ad7af0953b78b16d0f840e6fe718ba4a89384239ff20ed088da2fa"
        assert address.hex() == "56b19e720f3bfa8caaef806afdd5dfaffd0d6ec9476323a14d1638ad734b2ba5"

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

# In this test we check that a partial block-protocol GET_PUBLIC_KEY exchange followed by a
# fresh GET_PUBLIC_KEY does not leave stale HostIO state (GetChunk / requested_block) that would
# break the next command — and does not log alamgu-async-block protocol errors on Speculos stderr.
def test_incomplete_command_does_not_panic_followed_by_next_command(
    backend, scenario_navigator, firmware, navigator
):
    speculos_stderr_tee.clear()
    path = "m/44'/784'/0'"
    client = Client(backend, use_block_protocol=True)

    # First APDU only starts the chunked flow (device may respond with GetChunk + 9000); host does not send the rest.
    client.exchange_raw(bytes.fromhex("0002000021005060b9150c06381181d0f9964338489391a2c45b2134260a8b568f6ada00bf48"))

    # Second command must complete cleanly after firmware clears block state on START (see alamgu-async-block).
    _, public_key, _, address = client.get_public_key(path=path)
    assert public_key.hex() == "6fc6f39448ad7af0953b78b16d0f840e6fe718ba4a89384239ff20ed088da2fa"
    assert address.hex() == "56b19e720f3bfa8caaef806afdd5dfaffd0d6ec9476323a14d1638ad734b2ba5"

    _assert_no_alamgu_async_block_errors_in_speculos_log()
