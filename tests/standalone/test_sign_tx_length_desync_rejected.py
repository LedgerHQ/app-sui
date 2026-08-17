# Security regression for B2CA-2793 finding 2: SIGN_TX signed range not bound to
# the displayed parse.
#
# Background: the host sends a 4-byte length used only to bound the BLAKE2b-256
# hash that gets signed. The parse that builds the review screen ran on a separate
# clone of the same byte stream, bounded only by the grammar's own structure (it
# just stops once a recognized tx shape is fully parsed), never checked against
# that declared length. So a host could append arbitrary extra bytes after an
# otherwise complete, valid, displayed transaction, declare the padded length, and
# have the device happily sign a hash that covers bytes the display never showed --
# an undisclosed appendage to whatever the user approved.
#
# This test reuses the exact transaction and objects of the passing clear-sign stake
# `test_sign_stake_mul_coin` and appends 16 arbitrary trailing bytes, so the review
# parse (which stops at the structurally complete end of the real tx) consumes 16
# fewer bytes than the host-declared signed length. With the fix, the device must
# treat this mismatch as an unrecognized tx (the same fail-safe already used for
# parse ambiguity elsewhere) rather than silently signing the padded byte range.

import base64

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import run_apdu_and_nav_tasks_concurrently


def test_sign_stake_mul_coin_trailing_bytes_rejected(
        backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    # Same transaction as the passing `test_sign_stake_mul_coin`.
    transaction = base64.b64decode(
        'AAAAAAAFAQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh'
        '1Tlw1Vy/aUwJnsbSx4my8tySyn/zAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEA'
        'AAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
        'AAAAAAAAAAAABQEAAAAAAAAAAQABAAAgNfXxVPARdGTjN5xFx/PLay1O/t8wCsrf+Kfo6eOhUQkCBQAC'
        'AQAAAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADCnN1aV9zeXN0ZW0acmVxdWVzdF9h'
        'ZGRfc3Rha2VfbXVsX2NvaW4ABAECAAIAAAEDAAEEAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7'
        'C+SIAuv/FrTSCBqwbR1SUcmIIIZB5cUBx/qL3OnIt7CQi6dr1XZCEQAAAAAgO/8EfMoNuhJrkPXn8Pcm'
        'q72jh1ZSG9wTpvKb+SeNsOQfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHA'
        'G9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL'
        '5IjoAwAAAAAAAHjgAQAAAAAAAA=='
    )

    object_list = [base64.b64decode(
        'AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZDMFdg'
        'Im5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L4YEUXWAT'
        'DwAAAAAA'),
        base64.b64decode(
            'AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZD'
            'MFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9a'
            'sLM9x2ATDwAAAAAA'),
        base64.b64decode(
            'AAEB1HZCEQAAAAAoqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq22Aw8kBAAAAAAAdPyZD'
            'MFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAfVAIamErRVJt4BuqoZFY2dBaAKAaQzrxvVjuL'
            'cgrqZmATDwAAAAAA'),
        base64.b64decode(
            'AAEB1XZCEQAAAAAo6/8WtNIIGrBtHVJRyYgghkHlxQHH+ovc6ci3sJCLp2tAnHECAAAAAAAdPyZD'
            'MFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAuq6BxxXPwIbLsDoXWJN6/Emi0EtUzGJnln5pJ'
            'L4iDYWATDwAAAAAA'),
    ]

    # Append 16 arbitrary bytes. The reviewed parse stops at the structurally
    # complete end of the real transaction, so it consumes 16 fewer bytes than the
    # host-declared signed length (client.sign_tx always sets tx_len == len(tx)).
    transaction = transaction + bytes(range(16))

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

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
        pytest.fail('a transaction with trailing bytes beyond the reviewed parse '
                     'must not be clear-signed')

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0
