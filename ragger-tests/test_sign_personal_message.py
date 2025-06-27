# Covers various scenarios for PersonalMessage signing

import pytest
import concurrent.futures
import time
import base64

from application_client.client import Client, Errors
from contextlib import contextmanager
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavIns, NavInsID
from utils import ROOT_SCREENSHOT_PATH, check_signature_validity, run_apdu_and_nav_tasks_concurrently

def test_sign_short_ascii_message(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AwAASGVsbG8=')

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[ NavInsID.RIGHT_CLICK # Review message
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.BOTH_CLICK
                              ]
                , timeout=10
                , test_case_name=scenario_navigator.test_name
                , path=scenario_navigator.screenshot_path
                , screen_change_before_first_instruction=True
                , screen_change_after_last_instruction=False
            )
        else:
            scenario_navigator.review_approve()

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, transaction)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

def test_sign_short_non_ascii_message(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AwAAAAAABUhlbGxv')

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[ NavInsID.RIGHT_CLICK # Review message
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.BOTH_CLICK
                              ]
                , timeout=10
                , test_case_name=scenario_navigator.test_name
                , path=scenario_navigator.screenshot_path
                , screen_change_before_first_instruction=True
                , screen_change_after_last_instruction=False
            )
        else:
            scenario_navigator.review_approve()

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, transaction)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

def test_sign_long_ascii_message(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AwAAVGhlIGNoZXJyeSBibG9zc29tIG9yIHNha3VyYSBpcyB0aGUgZmxvd2VyIG9mIHRyZWVzIGluIFBydW51cyBzdWJnZW51cyBDZXJhc3VzIFNha3VyYSB1c3VhbGx5IHJlZmVycyB0byBmbG93ZXJzIG9mIG9ybmFtZW50YWwgY2hlcnJ5IHRyZWVzIHN1Y2ggYXMgY3VsdGl2YXJzIG9mIFBydW51cyBzZXJydWxhdGEgbm90IHRyZWVzIGdyb3duIGZvciB0aGVpciBmcnVpdCBDaGVycnkgYmxvc3NvbXMgaGF2ZSBiZWVuIGRlc2NyaWJlZCBhcyBoYXZpbmcgYSB2YW5pbGxhIGxpa2Ugc21lbGwgd2hpY2ggaXMgbWFpbmx5IGF0dHJpYnV0ZWQgdG8gY291bWFyaW4gV2lsZCBzcGVjaWVzIG9mIGNoZXJyeSB0cmVlIGFyZSB3aWRlbHkgZGlzdHJpYnV0ZWQgbWFpbmx5IGluIHRoZSBOb3J0aGVybiBIZW1pc3BoZXJlIFRoZXkgYXJlIGNvbW1vbiBpbiBFYXN0IEFzaWEgZXNwZWNpYWxseSBpbiBKYXBhbiB3aGVyZSB0aGV5IGhhdmUgYmVlbiBjdWx0aXZhdGVkIHByb2R1Y2luZyBtYW55IHZhcmlldGllcyBNb3N0IG9mIHRoZSBvcm5hbWVudGFsIGNoZXJyeSB0cmVlcyBwbGFudGVkIGluIHBhcmtzIGFuZCBvdGhlciBwbGFjZXMgZm9yIHZpZXdpbmcgYXJlIGN1bHRpdmFycyBkZXZlbG9wZWQgZm9yIG9ybmFtZW50YWwgcHVycG9zZXMgZnJvbSB2YXJpb3VzIHdpbGQgc3BlY2llcyBJbiBvcmRlciB0byBjcmVhdGUgYSBjdWx0aXZhciBzdWl0YWJsZSBmb3Igdmlld2luZyBhIHdpbGQgc3BlY2llcyB3aXRoIGNoYXJhY3RlcmlzdGljcyBzdWl0YWJsZSBmb3Igdmlld2luZyBpcyBuZWVkZWQgUHJ1bnVzIHNwZWNpb3NhIE9zaGltYSBjaGVycnkgd2hpY2ggaXMgZW5kZW1pYyB0byBKYXBhbiBwcm9kdWNlcyBtYW55IGxhcmdlIGZsb3dlcnMgaXMgZnJhZ3JhbnQgZWFzaWx5IG11dGF0ZXMgaW50byBkb3VibGUgZmxvd2VycyBhbmQgZ3Jvd3MgcmFwaWRseSBBcyBhIHJlc3VsdCB2YXJpb3VzIGN1bHRpdmFycyBrbm93biBhcyB0aGUgQ2VyYXN1cyBTYXRvIHpha3VyYSBHcm91cCBoYXZlIGJlZW4gcHJvZHVjZWQgc2luY2UgdGhlIDE0dGggY2VudHVyeSBhbmQgY29udGludWUgdG8gY29udHJpYnV0ZSBncmVhdGx5IHRvIHRoZSBkZXZlbG9wbWVudCBvZiBoYW5hbWkgZmxvd2VyIHZpZXdpbmcgY3VsdHVyZSBUaGUgSmFwYW5lc2Ugd29yZCBzYWt1cmEgY2FuIG1lYW4gZWl0aGVyIHRoZSB0cmVlIG9yIGl0cyBmbG93ZXJzIFRoZSBjaGVycnkgYmxvc3NvbSBpcyBjb25zaWRlcmVkIHRoZSBuYXRpb25hbCBmbG93ZXIgb2YgSmFwYW4gYW5kIGlzIGNlbnRyYWwgdG8gdGhlIGN1c3RvbSBvZiBoYW5hbWkgU2FrdXJhIHRyZWVzIGFyZSBvZnRlbiBjYWxsZWQgSmFwYW5lc2UgY2hlcnJ5IGluIEVuZ2xpc2ggVGhpcyBpcyBhbHNvIGEgY29tbW9uIG5hbWUgZm9yIFBydW51cyBzZXJydWxhdGE=')

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[ NavInsID.RIGHT_CLICK # Review message
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.RIGHT_CLICK # Sign Message
                               , NavInsID.BOTH_CLICK
                              ]
                , timeout=10
                , test_case_name=scenario_navigator.test_name
                , path=scenario_navigator.screenshot_path
                , screen_change_before_first_instruction=True
                , screen_change_after_last_instruction=False
            )
        else:
            scenario_navigator.review_approve()

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, transaction)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

def test_sign_short_utf8_message(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AwAAJ0nigJltIGNvbm4=')

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[ NavInsID.RIGHT_CLICK # Review message
                               , NavInsID.RIGHT_CLICK # Message ...
                               , NavInsID.BOTH_CLICK
                              ]
                , timeout=10
                , test_case_name=scenario_navigator.test_name
                , path=scenario_navigator.screenshot_path
                , screen_change_before_first_instruction=True
                , screen_change_after_last_instruction=False
            )
        else:
            scenario_navigator.review_approve()

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, transaction)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)
