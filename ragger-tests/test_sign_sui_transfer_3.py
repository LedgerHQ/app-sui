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

# built_tx AAAFAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAjoAwAAAAAAAAEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAI0AcAAAAAAAAAIG+yH+6tAn2khzKVr/1sTzYY/hdvovvz57XvHZRjsx4hAwIBAAABAQEAAgECAAEBAwABAgMAAAAAAwEAAAABBAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAANALTAAAAAAAAA==
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": null,
#   "gasData": {
#     "budget": "4983760",
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
#         "bytes": "6AMAAAAAAAA="
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
#         "bytes": "0AcAAAAAAAA="
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
#           "Input": 0
#         },
#         "amounts": [
#           {
#             "Input": 1
#           }
#         ]
#       }
#     },
#     {
#       "SplitCoins": {
#         "coin": {
#           "Input": 2
#         },
#         "amounts": [
#           {
#             "Input": 3
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
#             "NestedResult": [
#               1,
#               0
#             ]
#           }
#         ],
#         "address": {
#           "Input": 4
#         }
#       }
#     }
#   ]
# }

def test_sign_tx_sui_two_split_transfer(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAFAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAjoAwAAAAAAAAEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAI0AcAAAAAAAAAIG+yH+6tAn2khzKVr/1sTzYY/hdvovvz57XvHZRjsx4hAwIBAAABAQEAAgECAAEBAwABAgMAAAAAAwEAAAABBAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAANALTAAAAAAAAA==')

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
                instructions=[ NavInsID.RIGHT_CLICK # Transfer SUI
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # To ...
                               , NavInsID.RIGHT_CLICK # Amount
                               , NavInsID.RIGHT_CLICK # Max Gas
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

# built_tx AAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAjoAwAAAAAAAAEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiEDAgEAAAEBAQADAwAAAAABAQIAAQEDAAAAAAEDAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAEOUtAAAAAAAA
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
#         "bytes": "6AMAAAAAAAA="
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
#           "Input": 0
#         },
#         "amounts": [
#           {
#             "Input": 1
#           }
#         ]
#       }
#     },
#     {
#       "MergeCoins": {
#         "destination": {
#           "NestedResult": [
#             0,
#             0
#           ]
#         },
#         "sources": [
#           {
#             "Input": 2
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
#           }
#         ],
#         "address": {
#           "Input": 3
#         }
#       }
#     }
#   ]
# }

def test_sign_tx_sui_split_merge(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAjoAwAAAAAAAAEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiEDAgEAAAEBAQADAwAAAAABAQIAAQEDAAAAAAEDAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAEOUtAAAAAAAA')

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
                instructions=[ NavInsID.RIGHT_CLICK # Transfer SUI
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # To ...
                               , NavInsID.RIGHT_CLICK # Amount
                               , NavInsID.RIGHT_CLICK # Max Gas
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

# built_tx AAADAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAhQwwAAAAAAAAAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiEDAgEAAAEBAQADAAEDAAAAAAEBAAECAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAh+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbocEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAHi+LQAAAAAAAA==
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
#       },
#       {
#         "objectId": "0x1c12be5429384d00eeef61242f3aebabeac3012549dd6f888dc1087c4d00da80",
#         "version": "289568466",
#         "digest": "G9KngE3q7fpBfZtrmoEFdjZC4Ebb4TR7mZ1NYpf2xqaJ"
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
#         "bytes": "UMMAAAAAAAA="
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
#           "Input": 0
#         },
#         "amounts": [
#           {
#             "Input": 1
#           }
#         ]
#       }
#     },
#     {
#       "MergeCoins": {
#         "destination": {
#           "GasCoin": true
#         },
#         "sources": [
#           {
#             "NestedResult": [
#               0,
#               0
#             ]
#           }
#         ]
#       }
#     },
#     {
#       "TransferObjects": {
#         "objects": [
#           {
#             "GasCoin": true
#           }
#         ],
#         "address": {
#           "Input": 2
#         }
#       }
#     }
#   ]
# }

def test_sign_tx_sui_split_merge_to_gas(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAADAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAhQwwAAAAAAAAAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiEDAgEAAAEBAQADAAEDAAAAAAEBAAECAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAh+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbocEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAHi+LQAAAAAAAA==')

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
                instructions=[ NavInsID.RIGHT_CLICK # Transfer SUI
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # To ...
                               , NavInsID.RIGHT_CLICK # Amount
                               , NavInsID.RIGHT_CLICK # Max Gas
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

# built_tx AAAFAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAhg6gAAAAAAAAEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAIkF8BAAAAAAAAIG+yH+6tAn2khzKVr/1sTzYY/hdvovvz57XvHZRjsx4hBQIBAAABAQEAAgECAAEBAwADAAEDAAAAAAMAAQMBAAAAAQEAAQQAHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgC6/8WtNIIGrBtHVJRyYgghkHlxQHH+ovc6ci3sJCLp2vVdkIRAAAAACA7/wR8yg26EmuQ9efw9yarvaOHVlIb3BOm8pv5J42w5B+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAEOUtAAAAAAAA
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
#         "objectId": "0xebff16b4d2081ab06d1d5251c988208641e5c501c7fa8bdce9c8b7b0908ba76b",
#         "version": "289568469",
#         "digest": "53CbPjHczNtV9Kids6JdGt9bkPbSeJ34dc9TX2W2g6tT"
#       },
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
#         "bytes": "YOoAAAAAAAA=" # 60000
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
#         "bytes": "kF8BAAAAAAA=" # 90000
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
#           "Input": 0
#         },
#         "amounts": [
#           {
#             "Input": 1
#           }
#         ]
#       }
#     },
#     {
#       "SplitCoins": {
#         "coin": {
#           "Input": 2
#         },
#         "amounts": [
#           {
#             "Input": 3
#           }
#         ]
#       }
#     },
#     {
#       "MergeCoins": {
#         "destination": {
#           "GasCoin": true
#         },
#         "sources": [
#           {
#             "NestedResult": [
#               0,
#               0
#             ]
#           }
#         ]
#       }
#     },
#     {
#       "MergeCoins": {
#         "destination": {
#           "GasCoin": true
#         },
#         "sources": [
#           {
#             "NestedResult": [
#               1,
#               0
#             ]
#           }
#         ]
#       }
#     },
#     {
#       "TransferObjects": {
#         "objects": [
#           {
#             "GasCoin": true
#           }
#         ],
#         "address": {
#           "Input": 4
#         }
#       }
#     }
#   ]
# }

def test_sign_tx_sui_two_split_merge_to_gas(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAFAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAhg6gAAAAAAAAEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAIkF8BAAAAAAAAIG+yH+6tAn2khzKVr/1sTzYY/hdvovvz57XvHZRjsx4hBQIBAAABAQEAAgECAAEBAwADAAEDAAAAAAMAAQMBAAAAAQEAAQQAHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgC6/8WtNIIGrBtHVJRyYgghkHlxQHH+ovc6ci3sJCLp2vVdkIRAAAAACA7/wR8yg26EmuQ9efw9yarvaOHVlIb3BOm8pv5J42w5B+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAEOUtAAAAAAAA')

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
                instructions=[ NavInsID.RIGHT_CLICK # Transfer SUI
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # To ...
                               , NavInsID.RIGHT_CLICK # Amount
                               , NavInsID.RIGHT_CLICK # Max Gas
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
