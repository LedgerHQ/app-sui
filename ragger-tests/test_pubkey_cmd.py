import pytest

from application_client.client import Client, Errors
from contextlib import contextmanager
from ragger.bip import calculate_public_key_and_chaincode, CurveChoice
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID, NavIns
from utils import ROOT_SCREENSHOT_PATH, run_apdu_and_nav_tasks_concurrently


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
