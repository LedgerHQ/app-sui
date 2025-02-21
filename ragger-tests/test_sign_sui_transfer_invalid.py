# Covers various scenarios for Sui transfer txs not supported for clear signing

import pytest
import concurrent.futures
import time
import base64

from application_client.client import Client, Errors
from contextlib import contextmanager
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavIns, NavInsID
from utils import ROOT_SCREENSHOT_PATH, check_signature_validity, run_apdu_and_nav_tasks_concurrently

# built_tx AAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9ACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAgh49RW7lor2RC2Q0/dbt7liaWOySGZpRZy6q897zeN7wCAQEBAAABAQABAQECAAEDAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAEOUtAAAAAAAA
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": null,
#   "gasData": {
#     "budget": "3007760",
#     "price": "1000",
#     "owner": null,
#     "payment": [
#       {
#         "objectId": "0x1f876ff0144386dcf4e886c5de53b326c718cc1221e1ccea71ef8aa6231a40ea",
#         "version": "289568467",
#         "digest": "3GkMekAY5KQqiop61rRCnQjK57ztStksBSuZsUPf62JM"
#       }
#     ]
#   },
#   "inputs": [
#     {
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0xa93f6c464f8fb8b98fb3d02112902060c8f85ea4d71cfc7777dfdbd75e68ab6d",
#           "version": "289568468",
#           "digest": "Cbin2kMMWzjtPER7GZ7ne81Dhpk2tS31MwinvTwjMEZi"
#         }
#       }
#     },
#     {
#       "Pure": {
#         "bytes": "b7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiE="
#       }
#     },
#     {
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0x1c12be5429384d00eeef61242f3aebabeac3012549dd6f888dc1087c4d00da80",
#           "version": "289568466",
#           "digest": "G9KngE3q7fpBfZtrmoEFdjZC4Ebb4TR7mZ1NYpf2xqaJ"
#         }
#       }
#     },
#     {
#       "Pure": {
#         "bytes": "h49RW7lor2RC2Q0/dbt7liaWOySGZpRZy6q897zeN7w="
#       }
#     }
#   ],
#   "commands": [
#     {
#       "TransferObjects": {
#         "objects": [
#           {
#             "Input": 0
#           }
#         ],
#         "address": {
#           "Input": 1
#         }
#       }
#     },
#     {
#       "TransferObjects": {
#         "objects": [
#           {
#             "Input": 2
#           }
#         ],
#         "address": {
#           "Input": 3
#         }
#       }
#     }
#   ]
# }

def test_sign_tx_sui_multi_recipient(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    transaction = base64.b64decode('AAAAAAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9ACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAgh49RW7lor2RC2Q0/dbt7liaWOySGZpRZy6q897zeN7wCAQEBAAABAQABAQECAAEDAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAEOUtAAAAAAAA')

    object_list = [ base64.b64decode('AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L4YEUXWATDwAAAAAA')
      , base64.b64decode('AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA')
      , base64.b64decode('AAEB1HZCEQAAAAAoqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq22Aw8kBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAfVAIamErRVJt4BuqoZFY2dBaAKAaQzrxvVjuLcgrqZmATDwAAAAAA')
      , base64.b64decode('AAEB1XZCEQAAAAAo6/8WtNIIGrBtHVJRyYgghkHlxQHH+ovc6ci3sJCLp2tAnHECAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAuq6BxxXPwIbLsDoXWJN6/Emi0EtUzGJnln5pJL4iDYWATDwAAAAAA')
       ]


    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK]
                , timeout=10
                , test_case_name=scenario_navigator.test_name
                , path=scenario_navigator.screenshot_path
                , screen_change_before_first_instruction=True
                , screen_change_after_last_instruction=False
            )
        else:
            # Dismiss the "Enable Blind signing" screen
            navigator.navigate([NavInsID.USE_CASE_CHOICE_REJECT],
                            screen_change_before_first_instruction=False,
                            screen_change_after_last_instruction=False)

    def check_result(result):
        pytest.fail('should not happen')

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0


# GasCoin does not exist in object_list
def test_sign_tx_sui_whole_gas_coin_missing_obj(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    transaction = base64.b64decode('AAAAAAABACAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEBAQABAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQFADb3P7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2QhEAAAAAIGbFq2VJip03FgAaA0gV/0q8p2X39vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiHoAwAAAAAAAOCXLQAAAAAAAA==')

    object_list = [ base64.b64decode('AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L4YEUXWATDwAAAAAA')
      , base64.b64decode('AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA')
      , base64.b64decode('AAEB1HZCEQAAAAAoqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq22Aw8kBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAfVAIamErRVJt4BuqoZFY2dBaAKAaQzrxvVjuLcgrqZmATDwAAAAAA')
      , base64.b64decode('AAEB1XZCEQAAAAAo6/8WtNIIGrBtHVJRyYgghkHlxQHH+ovc6ci3sJCLp2tAnHECAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAuq6BxxXPwIbLsDoXWJN6/Emi0EtUzGJnln5pJL4iDYWATDwAAAAAA')
       ]

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK]
                , timeout=10
                , test_case_name=scenario_navigator.test_name
                , path=scenario_navigator.screenshot_path
                , screen_change_before_first_instruction=True
                , screen_change_after_last_instruction=False
            )
        else:
            # Dismiss the "Enable Blind signing" screen
            navigator.navigate([NavInsID.USE_CASE_CHOICE_REJECT],
                            screen_change_before_first_instruction=False,
                            screen_change_after_last_instruction=False)

    def check_result(result):
        pytest.fail('should not happen')

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0

# Coin referred by TransferObjects does not exist in object_list
def test_sign_tx_sui_whole_input_coin_missing_obj(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    transaction = base64.b64decode('AAAAAAACAQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAeL4tAAAAAAAA')

    object_list = [ base64.b64decode('AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA')
      , base64.b64decode('AAEB1HZCEQAAAAAoqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq22Aw8kBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAfVAIamErRVJt4BuqoZFY2dBaAKAaQzrxvVjuLcgrqZmATDwAAAAAA')
      , base64.b64decode('AAEB1XZCEQAAAAAo6/8WtNIIGrBtHVJRyYgghkHlxQHH+ovc6ci3sJCLp2tAnHECAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAuq6BxxXPwIbLsDoXWJN6/Emi0EtUzGJnln5pJL4iDYWATDwAAAAAA')
       ]

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK]
                , timeout=10
                , test_case_name=scenario_navigator.test_name
                , path=scenario_navigator.screenshot_path
                , screen_change_before_first_instruction=True
                , screen_change_after_last_instruction=False
            )
        else:
            # Dismiss the "Enable Blind signing" screen
            navigator.navigate([NavInsID.USE_CASE_CHOICE_REJECT],
                            screen_change_before_first_instruction=False,
                            screen_change_after_last_instruction=False)

    def check_result(result):
        pytest.fail('should not happen')

    with pytest.raises(ExceptionRAPDU) as e:
        run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)

    assert len(e.value.data) == 0
