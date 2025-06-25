import pytest
import concurrent.futures
import time

from application_client.client import Client, Errors
from contextlib import contextmanager
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavIns, NavInsID
from utils import ROOT_SCREENSHOT_PATH, check_signature_validity, run_apdu_and_nav_tasks_concurrently

def test_sign_tx_short_tx(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/535348'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction="smalltx".encode('utf-8')

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[NavInsID.BOTH_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.BOTH_CLICK]
                , timeout=10
                , path=scenario_navigator.screenshot_path
                , test_case_name="test_sign_tx_short_tx"
                , screen_change_before_first_instruction=False
                , screen_change_after_last_instruction=False
            )
        else:
            # Dismiss the "Blind signing ahead" screen
            navigator.navigate_and_compare(
                instructions=[NavInsID.USE_CASE_CHOICE_REJECT]
                , timeout=20
                , path=scenario_navigator.screenshot_path
                , test_case_name="test_sign_tx_short_tx_1"
                , screen_change_before_first_instruction=True
                , screen_change_after_last_instruction=True
            )
            # Below is similar to scenario_navigator.review_approve()
            # But screen_change_before_first_instruction=True causes hang
            navigator.navigate_until_text_and_compare(
                navigate_instruction=NavInsID.SWIPE_CENTER_TO_LEFT
                , validation_instructions=[NavInsID.USE_CASE_REVIEW_CONFIRM, NavInsID.USE_CASE_STATUS_DISMISS]
                , text="^Hold to sign$"
                , timeout=20
                , path=scenario_navigator.screenshot_path
                , test_case_name="test_sign_tx_short_tx_2"
                , screen_change_before_first_instruction=False
                , screen_change_after_last_instruction=True
            )

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, transaction)

    with blind_sign_enabled(firmware, navigator):
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

def test_sign_tx_long_tx(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/535348'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)

    transaction=("looongtx" * 100).encode('utf-8')

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[NavInsID.BOTH_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.BOTH_CLICK]
                , timeout=10
                , path=scenario_navigator.screenshot_path
                , test_case_name="test_sign_tx_long_tx"
                , screen_change_before_first_instruction=False
                , screen_change_after_last_instruction=False
            )
        else:
            # Dismiss the "Blind signing ahead" screen
            navigator.navigate_and_compare(
                instructions=[NavInsID.USE_CASE_CHOICE_REJECT]
                , timeout=20
                , path=scenario_navigator.screenshot_path
                , test_case_name="test_sign_tx_long_tx_1"
                , screen_change_before_first_instruction=True
                , screen_change_after_last_instruction=True
            )
            # Below is similar to scenario_navigator.review_approve()
            # But screen_change_before_first_instruction=True causes hang
            navigator.navigate_until_text_and_compare(
                navigate_instruction=NavInsID.SWIPE_CENTER_TO_LEFT
                , validation_instructions=[NavInsID.USE_CASE_REVIEW_CONFIRM, NavInsID.USE_CASE_STATUS_DISMISS]
                , text="^Hold to sign$"
                , timeout=20
                , path=scenario_navigator.screenshot_path
                , test_case_name="test_sign_tx_long_tx_2"
                , screen_change_before_first_instruction=False
                , screen_change_after_last_instruction=True
            )

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, transaction)

    with blind_sign_enabled(firmware, navigator):
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

# Transaction signature refused test
# The test will ask for a transaction signature that will be refused on screen
def test_sign_tx_refused(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/535348'/0'"

    transaction="smalltx".encode('utf-8')

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[NavInsID.BOTH_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.BOTH_CLICK]
                , timeout=10
                , path=scenario_navigator.screenshot_path
                , test_case_name="test_sign_tx_refused"
                , screen_change_before_first_instruction=False
                , screen_change_after_last_instruction=False
            )
        else:
            # Dismiss the "Blind signing ahead" screen
            navigator.navigate([NavInsID.USE_CASE_CHOICE_REJECT],
                            screen_change_before_first_instruction=False,
                            screen_change_after_last_instruction=False)
            scenario_navigator.review_reject()

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, transaction)

    with pytest.raises(ExceptionRAPDU) as e:
        with blind_sign_enabled(firmware, navigator):
            run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    # Assert that we have received a refusal
    # assert e.value.status == Errors.SW_DENY
    assert len(e.value.data) == 0

def test_sign_tx_blindsign_disabled(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/535348'/0'"

    transaction="smalltx".encode('utf-8')

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.BOTH_CLICK]
                , timeout=10
                , path=scenario_navigator.screenshot_path
                , test_case_name="test_sign_tx_blindsign_disabled"
                , screen_change_before_first_instruction=False
                , screen_change_after_last_instruction=False
            )
        else:
            # Dismiss the "Enable Blind signing" screen
            navigator.navigate([NavInsID.USE_CASE_CHOICE_REJECT],
                            screen_change_before_first_instruction=False,
                            screen_change_after_last_instruction=False)

    def check_result(result):
        assert len(result) == 64

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    # Assert that we have received a refusal
    # assert e.value.status == Errors.SW_DENY
    assert len(e.value.data) == 0

@contextmanager
def blind_sign_enabled(firmware, navigator):
    toggle_blind_sign(firmware, navigator)
    try:
        yield
    except:
        # Don't re-enable if we hit an exception
        raise
    else:
        toggle_blind_sign(firmware, navigator)

def toggle_blind_sign(firmware, navigator):
    if firmware.device.startswith("nano"):
        navigator.navigate(
            instructions=[NavInsID.RIGHT_CLICK, NavInsID.BOTH_CLICK, NavInsID.BOTH_CLICK, NavInsID.RIGHT_CLICK, NavInsID.BOTH_CLICK, NavInsID.LEFT_CLICK]
            , timeout=10
            , screen_change_before_first_instruction=False
        )
    else:
        navigator.navigate([NavInsID.USE_CASE_HOME_SETTINGS,
                            NavIns(NavInsID.TOUCH, (200, 113)),
                            NavInsID.USE_CASE_SUB_SETTINGS_EXIT],
                            timeout=10,
                            screen_change_before_first_instruction=False,
                            screen_change_after_last_instruction=False)
