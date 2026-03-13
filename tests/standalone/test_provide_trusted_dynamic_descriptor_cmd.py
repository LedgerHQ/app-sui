import pytest
import concurrent.futures
import time
import base64
import ragger

from application_client.client import Client
from ragger.navigator import NavIns, NavInsID
from utils import run_apdu_and_nav_tasks_concurrently

def test_provide_trusted_dynamic_descriptor_cmd(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)

    client.provide_dynamic_token("DEEP", 6, "0xdeeb7a4662eec9f2f3def03fb937a663dddaa2e215b8078a284d026b7946c270", "deep", "DEEP")

