# Security regression for B2CA-2793 finding 1: CallArg::Pure length desync.
#
# Background: a `CallArg::Pure` carries its own ULEB128-encoded byte length, but the
# BCS `Option<u64>` sub-parser used for the `1 | 9`-length case ignored that declared
# length entirely -- it just read a tag byte and, on `Some`, greedily read 8 more
# bytes regardless of what the declared length said. A length of 1 whose lone
# content byte is the `Some` tag (0x01) therefore made the parser consume 9 bytes
# instead of 1, silently swallowing the next 8 bytes of the stream as the `Some`
# payload. Because the reviewed parse and the signed byte range are not otherwise
# tied together (finding 2), this is the mechanism a malicious host could use to
# desync what the device displays from what it actually signs.
#
# This test reuses the exact transaction and objects of the passing clear-sign stake
# `test_sign_stake_mul_coin` (request_add_stake_mul_coin with stake_amount = None,
# encoded as `Pure { bytes: [0x00] }`, declared length 1). It flips that single
# content byte to 0x01 (the `Some` tag) and inserts 8 extra bytes right after it, so
# that the vulnerable parser -- which reads the tag plus 8 more bytes regardless of
# the declared length of 1 -- lands exactly back on the original, unmodified
# validator-address input and the rest of the transaction, and so happily signs a
# transaction whose `stake_amount` is actually `Some(0)` rather than `None`. This
# was confirmed against the vulnerable parser (git commit 4287823, i.e. this fix
# reverted): the device displayed and produced a valid signature for this crafted
# transaction via the normal review-and-approve flow -- no blind-signing fallback,
# no error. With the fix, a declared length of 1 can only ever legitimately hold
# `None`, so the device must reject instead.

import base64

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavInsID
from utils import run_apdu_and_nav_tasks_concurrently


def test_sign_stake_mul_coin_pure_option_length_tag_mismatch_rejected(
        backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    # Same transaction as the passing `test_sign_stake_mul_coin`: MakeMoveVec + a
    # sui_system::request_add_stake_mul_coin MoveCall whose `stake_amount` argument
    # (Input 3) is `Pure { bytes: [0x00] }` i.e. `Option<u64>::None`, declared length 1.
    transaction = bytearray(base64.b64decode(
        'AAAAAAAFAQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh'
        '1Tlw1Vy/aUwJnsbSx4my8tySyn/zAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEA'
        'AAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
        'AAAAAAAAAAAABQEAAAAAAAAAAQABAAAgNfXxVPARdGTjN5xFx/PLay1O/t8wCsrf+Kfo6eOhUQkCBQAC'
        'AQAAAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADCnN1aV9zeXN0ZW0acmVxdWVzdF9h'
        'ZGRfc3Rha2VfbXVsX2NvaW4ABAECAAIAAAEDAAEEAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7'
        'C+SIAuv/FrTSCBqwbR1SUcmIIIZB5cUBx/qL3OnIt7CQi6dr1XZCEQAAAAAgO/8EfMoNuhJrkPXn8Pcm'
        'q72jh1ZSG9wTpvKb+SeNsOQfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHA'
        'G9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL'
        '5IjoAwAAAAAAAHjgAQAAAAAAAA=='))

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

    # Locate the stake_amount input: Pure(len=1, content=0x00) immediately followed
    # by the validator-address input, Pure(len=32, <address>).
    validator_address = base64.b64decode('NfXxVPARdGTjN5xFx/PLay1O/t8wCsrf+Kfo6eOhUQk=')
    marker = bytes([0x00, 0x01, 0x00, 0x00, 0x20]) + validator_address
    pos = transaction.find(marker)
    assert pos != -1, "could not locate the stake_amount Pure input in the transaction"

    # Flip the content byte to the `Some` tag (0x01) and insert 8 filler bytes right
    # after it. The declared length stays 1 (unchanged) -- still claiming `None` --
    # but the vulnerable parser ignores that and reads tag + 8 more bytes regardless,
    # landing exactly back on the untouched validator-address input and the rest of
    # the transaction. The fixed parser, by contrast, reads only the declared 1 byte
    # and must reject on seeing a tag other than 0x00.
    transaction[pos + 2] = 0x01
    transaction = bytes(transaction[:pos + 3]) + bytes(8) + bytes(transaction[pos + 3:])

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
        pytest.fail('a Pure input whose declared length disagrees with its embedded '
                     'Option<u64> tag must not be clear-signed')

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0
