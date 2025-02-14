# Covers various scenarios for valid Sui transfer txs

import pytest
import concurrent.futures
import time
import base64

from application_client.client import Client, Errors
from contextlib import contextmanager
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavIns, NavInsID
from utils import ROOT_SCREENSHOT_PATH, check_signature_validity, run_apdu_and_nav_tasks_concurrently

# Balances of objects used in these tests
# 0xa93f6c464f8fb8b98fb3d02112902060c8f85ea4d71cfc7777dfdbd75e68ab6d 0.03
# 0x1f876ff0144386dcf4e886c5de53b326c718cc1221e1ccea71ef8aa6231a40ea 0.02
# 0x1c12be5429384d00eeef61242f3aebabeac3012549dd6f888dc1087c4d00da80 0.01
# 0xebff16b4d2081ab06d1d5251c988208641e5c501c7fa8bdce9c8b7b0908ba76b 0.041

# can sign a Sui transfer with entire gas coin

# {"data":{"objects":{"nodes":[{"digest":"7vBQVjLUYjJ2kiA4YYhwrHey38LqDVU3FMC3PtUgpotV","bcs":"AAEB03ZCEQAAAAAoQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvIYR4MtEtyGd4QDpQ5AAAAAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeISB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA"}]}}}⏎
# built_tx AAABACAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEBAQABAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQFADb3P7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2QhEAAAAAIGbFq2VJip03FgAaA0gV/0q8p2X39vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiHoAwAAAAAAAOCXLQAAAAAAAA==
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x6fb21feead027da4873295affd6c4f3618fe176fa2fbf3e7b5ef1d9463b31e21",
#   "expiration": null,
#   "gasData": {
#     "budget": "2988000",
#     "price": "1000",
#     "owner": null,
#     "payment": [
#       {
#         "objectId": "0x400dbdcfeda8e1e64ebbf5710ccf5a67bfa6e7f945c92bc8611e0cb44b7219de",
#         "version": "289568467",
#         "digest": "7vBQVjLUYjJ2kiA4YYhwrHey38LqDVU3FMC3PtUgpotV"
#       }
#     ]
#   },
#   "inputs": [
#     {
#       "Pure": {
#         "bytes": "HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5Ig="
#       }
#     }
#   ],
#   "commands": [
#     {
#       "TransferObjects": {
#         "objects": [
#           {
#             "GasCoin": true
#           }
#         ],
#         "address": {
#           "Input": 0
#         }
#       }
#     }
#   ]
# }
def test_sign_tx_sui_whole_gas_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    # 4 A prepended
    transaction = base64.b64decode('AAAAAAABACAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEBAQABAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQFADb3P7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2QhEAAAAAIGbFq2VJip03FgAaA0gV/0q8p2X39vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiHoAwAAAAAAAOCXLQAAAAAAAA==')

    object_list = [base64.b64decode('AAEB03ZCEQAAAAAoQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvIYR4MtEtyGd4QDpQ5AAAAAABvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeISB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA')]

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[ NavInsID.RIGHT_CLICK # Transfer SUI
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # To ...
                               , NavInsID.RIGHT_CLICK # Amount
                               , NavInsID.RIGHT_CLICK # Max Gas
                               , NavInsID.RIGHT_CLICK # Sign Transaction?
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


# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": {
#     "None": true
#   },
#   "gasData": {
#     "budget": "2997880",
#     "price": "1000",
#     "owner": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
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
#           "objectId": "0x1c12be5429384d00eeef61242f3aebabeac3012549dd6f888dc1087c4d00da80",
#           "version": "289568466",
#           "digest": "G9KngE3q7fpBfZtrmoEFdjZC4Ebb4TR7mZ1NYpf2xqaJ"
#         }
#       }
#     },
#     {
#       "Pure": {
#         "bytes": "b7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiE="
#       }
#     }
#   ],
#   "commands": [
#     {
#       "TransferObjects": {
#         "objects": [
#           {
#             "GasCoin": true
#           },
#           {
#             "Input": 0
#           }
#         ],
#         "address": {
#           "Input": 1
#         }
#       }
#     }
#   ]
# }
# 'AAACAQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAgABAAABAQAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAHi+LQAAAAAAAA=='


def test_sign_tx_sui_whole_gas_plus_input_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    # 4 A prepended
    transaction = base64.b64decode('AAAAAAACAQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAgABAAABAQAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAHi+LQAAAAAAAA==')

    object_list = [ base64.b64decode('AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L4YEUXWATDwAAAAAA')
      , base64.b64decode('AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA')
       ]



    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[ NavInsID.RIGHT_CLICK # Transfer SUI
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # To ...
                               , NavInsID.RIGHT_CLICK # Amount
                               , NavInsID.RIGHT_CLICK # Max Gas
                               , NavInsID.RIGHT_CLICK # Sign Transaction?
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



# AAADAAhkAAAAAAAAAAEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAgABAQAAAQIDAAAAAAEBAAECAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAA2NE8AAAAAAAA
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": null,
#   "gasData": {
#     "budget": "3985880",
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
#       "Pure": {
#         "bytes": "ZAAAAAAAAAA="
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
#         "bytes": "b7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiE="
#       }
#     }
#   ],
#   "commands": [
#     {
#       "SplitCoins": {
#         "coin": {
#           "GasCoin": true
#         },
#         "amounts": [
#           {
#             "Input": 0
#           }
#         ]
#       }
#     },
#     {
#       "TransferObjects": {
#         "objects": [
#           {
#             "NestedResult": [
#               0,
#               0
#             ]
#           },
#           {
#             "Input": 1
#           }
#         ],
#         "address": {
#           "Input": 2
#         }
#       }
#     }
#   ]
# }


def test_sign_tx_sui_split_gas_plus_input_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAADAAhkAAAAAAAAAAEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAgABAQAAAQIDAAAAAAEBAAECAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAA2NE8AAAAAAAA')

    object_list = [ base64.b64decode('AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L4YEUXWATDwAAAAAA')
      , base64.b64decode('AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA')
       ]



    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[ NavInsID.RIGHT_CLICK # Transfer SUI
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # To ...
                               , NavInsID.RIGHT_CLICK # Amount
                               , NavInsID.RIGHT_CLICK # Max Gas
                               , NavInsID.RIGHT_CLICK # Sign Transaction?
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


# {"data":{"objects":{"nodes":[{"digest":"3GkMekAY5KQqiop61rRCnQjK57ztStksBSuZsUPf62JM","bcs":"AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA"}]}}}
# {"data":{"objects":{"nodes":[{"digest":"G9KngE3q7fpBfZtrmoEFdjZC4Ebb4TR7mZ1NYpf2xqaJ","bcs":"AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L4YEUXWATDwAAAAAA"}]}}}

#  AAACAQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAeL4tAAAAAAAA
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": null,
#   "gasData": {
#     "budget": "2997880",
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
#           "objectId": "0x1c12be5429384d00eeef61242f3aebabeac3012549dd6f888dc1087c4d00da80",
#           "version": "289568466",
#           "digest": "G9KngE3q7fpBfZtrmoEFdjZC4Ebb4TR7mZ1NYpf2xqaJ"
#         }
#       }
#     },
#     {
#       "Pure": {
#         "bytes": "b7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiE="
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
#     }
#   ]
# }

def test_sign_tx_sui_whole_input_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAACAQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAeL4tAAAAAAAA')

    object_list = [ base64.b64decode('AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L4YEUXWATDwAAAAAA')
      , base64.b64decode('AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA')
       ]



    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[ NavInsID.RIGHT_CLICK # Transfer SUI
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # To ...
                               , NavInsID.RIGHT_CLICK # Amount
                               , NavInsID.RIGHT_CLICK # Max Gas
                               , NavInsID.RIGHT_CLICK # Sign Transaction?
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


# built_tx AAADAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAgEAAAEBAAECAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAEOUtAAAAAAAA

# {"data":{"objects":{"nodes":[{"digest":"Cbin2kMMWzjtPER7GZ7ne81Dhpk2tS31MwinvTwjMEZi","bcs":"AAEB1HZCEQAAAAAoqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq22Aw8kBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAfVAIamErRVJt4BuqoZFY2dBaAKAaQzrxvVjuLcgrqZmATDwAAAAAA"}]}}}
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
#         "bytes": "b7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiE="
#       }
#     }
#   ],
#   "commands": [
#     {
#       "TransferObjects": {
#         "objects": [
#           {
#             "Input": 0
#           },
#           {
#             "Input": 1
#           }
#         ],
#         "address": {
#           "Input": 2
#         }
#       }
#     }
#   ]
# }

def test_sign_tx_sui_whole_two_input_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAADAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAgEAAAEBAAECAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAEOUtAAAAAAAA')

    object_list = [ base64.b64decode('AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L4YEUXWATDwAAAAAA')
      , base64.b64decode('AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA')
      , base64.b64decode('AAEB1HZCEQAAAAAoqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq22Aw8kBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAfVAIamErRVJt4BuqoZFY2dBaAKAaQzrxvVjuLcgrqZmATDwAAAAAA')
       ]



    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[ NavInsID.RIGHT_CLICK # Transfer SUI
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # To ...
                               , NavInsID.RIGHT_CLICK # Amount
                               , NavInsID.RIGHT_CLICK # Max Gas
                               , NavInsID.RIGHT_CLICK # Sign Transaction?
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

# built_tx AAADAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQIDAQAAAQEBAAEBAQAAAQIAHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgBH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOrTdkIRAAAAACAhwBvcKbCI07AVHqsBdv+VAeBKtJ4AEkpbb64aBfNRuh0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SI6AMAAAAAAACw0R4AAAAAAAA=

# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": null,
#   "gasData": {
#     "budget": "2019760",
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
#         "bytes": "b7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiE="
#       }
#     }
#   ],
#   "commands": [
#     {
#       "MergeCoins": {
#         "destination": {
#           "Input": 0
#         },
#         "sources": [
#           {
#             "Input": 1
#           }
#         ]
#       }
#     },
#     {
#       "TransferObjects": {
#         "objects": [
#           {
#             "Input": 0
#           }
#         ],
#         "address": {
#           "Input": 2
#         }
#       }
#     }
#   ]
# }

def test_sign_tx_sui_merge_two_input_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAADAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQIDAQAAAQEBAAEBAQAAAQIAHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgBH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOrTdkIRAAAAACAhwBvcKbCI07AVHqsBdv+VAeBKtJ4AEkpbb64aBfNRuh0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SI6AMAAAAAAACw0R4AAAAAAAA=')

    object_list = [ base64.b64decode('AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L4YEUXWATDwAAAAAA')
      , base64.b64decode('AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA')
      , base64.b64decode('AAEB1HZCEQAAAAAoqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq22Aw8kBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAfVAIamErRVJt4BuqoZFY2dBaAKAaQzrxvVjuLcgrqZmATDwAAAAAA')
       ]



    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[ NavInsID.RIGHT_CLICK # Transfer SUI
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # To ...
                               , NavInsID.RIGHT_CLICK # Amount
                               , NavInsID.RIGHT_CLICK # Max Gas
                               , NavInsID.RIGHT_CLICK # Sign Transaction?
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
