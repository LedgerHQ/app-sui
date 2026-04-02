"""
Test that sends APDUs from test_data/test_tx.apdu, rejects the transaction on device,
then sends APDUs from test_data/test_getaddress.apdu.

Equivalent to:
  ledgercomm-send file test_tx.apdu   # (user rejects on device)
  ledgercomm-send file test_getaddress.apdu
"""
from pathlib import Path

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import run_apdu_and_nav_tasks_concurrently

# Path to APDU files (next to this test)
TEST_DATA = Path(__file__).resolve().parent / "test_data"
TEST_TX_APDU = TEST_DATA / "test_tx.apdu"
TEST_GETADDRESS_APDU = TEST_DATA / "test_getaddress.apdu"


def load_apdus_from_file(path: Path) -> list[bytes]:
    """Parse APDU file (ledgercomm format: lines starting with '=> ' followed by hex)."""
    apdus = []
    for line in path.read_text().strip().splitlines():
        line = line.strip()
        if line.startswith("=> "):
            hex_str = line[3:].replace(" ", "")
            apdus.append(bytes.fromhex(hex_str))
    return apdus


def send_apdu_file(client: Client, path: Path) -> list[bytes]:
    """Send all APDUs from file and return responses (raises on first error)."""
    apdus = load_apdus_from_file(path)
    responses = []
    for apdu in apdus:
        data = client.exchange_raw(apdu)
        responses.append(data)
    return responses


def test_apdu_file_reject_tx_then_getaddress(
    backend, scenario_navigator, firmware, navigator
):
    """Send test_tx.apdu, reject the tx on device, then send test_getaddress.apdu."""
    assert TEST_TX_APDU.exists(), f"Missing {TEST_TX_APDU}"
    assert TEST_GETADDRESS_APDU.exists(), f"Missing {TEST_GETADDRESS_APDU}"

    client = Client(backend, use_block_protocol=True)

    def apdu_task():
        return send_apdu_file(client, TEST_TX_APDU)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[
                    NavInsID.RIGHT_CLICK,  # Transfer SUI / first screen
                    NavInsID.RIGHT_CLICK,
                    NavInsID.RIGHT_CLICK,  # From ...
                    NavInsID.RIGHT_CLICK,
                    NavInsID.RIGHT_CLICK,  # To ...
                    NavInsID.RIGHT_CLICK,  # Amount
                    NavInsID.RIGHT_CLICK,  # Max Gas
                    NavInsID.RIGHT_CLICK,  # Confirm
                    NavInsID.BOTH_CLICK,   # Reject
                ],
                timeout=15,
                test_case_name="test_apdu_file_reject_tx_then_getaddress",
                path=scenario_navigator.screenshot_path,
                screen_change_before_first_instruction=True,
                screen_change_after_last_instruction=False,
            )
        else:
            scenario_navigator.review_reject()

    def check_result(_result):
        pytest.fail("should not happen (tx should be rejected)")

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0

    # Now send test_getaddress.apdu
    apdus = load_apdus_from_file(TEST_GETADDRESS_APDU)
    for apdu in apdus:
        rapdu = backend.exchange_raw(data=apdu)
        assert rapdu.status == 0x9000, f"GetAddress APDU failed: {rapdu.status:04x}"
