# Covers various scenarios for valid Sui stake txs

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

# built_tx AAAFAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAiAYYwAAAAAAAEBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAUBAAAAAAAAAAEAAQAAIDX18VTwEXRk4zecRcfzy2stTv7fMArK3/in6OnjoVEJBAMAAQEAAAIAAQEBAAUAAgMBAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADCnN1aV9zeXN0ZW0acmVxdWVzdF9hZGRfc3Rha2VfbXVsX2NvaW4ABAECAAICAAEDAAEEAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIA+v/FrTSCBqwbR1SUcmIIIZB5cUBx/qL3OnIt7CQi6dr1XZCEQAAAAAgO/8EfMoNuhJrkPXn8Pcmq72jh1ZSG9wTpvKb+SeNsOQfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8x0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SI6AMAAAAAAAB44AEAAAAAAAA=
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": null,
#   "gasData": {
#     "budget": "123000",
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
#         "bytes": "gGGMAAAAAAA="
#       }
#     },
#     {
#       "Object": {
#         "SharedObject": {
#           "objectId": "0x0000000000000000000000000000000000000000000000000000000000000005",
#           "initialSharedVersion": 1,
#           "mutable": true
#         }
#       }
#     },
#     {
#       "Pure": {
#         "bytes": "AA=="
#       }
#     },
#     {
#       "Pure": {
#         "bytes": "NfXxVPARdGTjN5xFx/PLay1O/t8wCsrf+Kfo6eOhUQk="
#       }
#     }
#   ],
#   "commands": [
#     {
#       "MergeCoins": {
#         "destination": {
#           "GasCoin": true
#         },
#         "sources": [
#           {
#             "Input": 0
#           }
#         ]
#       }
#     },
#     {
#       "SplitCoins": {
#         "coin": {
#           "GasCoin": true
#         },
#         "amounts": [
#           {
#             "Input": 1
#           }
#         ]
#       }
#     },
#     {
#       "MakeMoveVec": {
#         "type": null,
#         "elements": [
#           {
#             "NestedResult": [
#               1,
#               0
#             ]
#           },
#           {
#             "Input": 0
#           }
#         ]
#       }
#     },
#     {
#       "MoveCall": {
#         "package": "0x0000000000000000000000000000000000000000000000000000000000000003",
#         "module": "sui_system",
#         "function": "request_add_stake_mul_coin",
#         "typeArguments": [],
#         "arguments": [
#           {
#             "Input": 2
#           },
#           {
#             "Result": 2
#           },
#           {
#             "Input": 3
#           },
#           {
#             "Input": 4
#           }
#         ]
#       }
#     }
#   ]
# }

def test_sign_stake_mul_coin_gas_merge_split(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAFAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAiAYYwAAAAAAAEBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAUBAAAAAAAAAAEAAQAAIDX18VTwEXRk4zecRcfzy2stTv7fMArK3/in6OnjoVEJBAMAAQEAAAIAAQEBAAUAAgMBAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADCnN1aV9zeXN0ZW0acmVxdWVzdF9hZGRfc3Rha2VfbXVsX2NvaW4ABAECAAICAAEDAAEEAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIA+v/FrTSCBqwbR1SUcmIIIZB5cUBx/qL3OnIt7CQi6dr1XZCEQAAAAAgO/8EfMoNuhJrkPXn8Pcmq72jh1ZSG9wTpvKb+SeNsOQfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8x0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SI6AMAAAAAAAB44AEAAAAAAAA=')

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
                instructions=[ NavInsID.RIGHT_CLICK # Review transfer
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

# built_tx AAACAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABQEAAAAAAAAAAQEAqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq23UdkIRAAAAACCsVYpX4/44Cp2BWe8aVkACUW5rxtsErjUPJ6nMxaCvvQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMKc3VpX3N5c3RlbRZyZXF1ZXN0X3dpdGhkcmF3X3N0YWtlAAIBAAABAQAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAPr/xa00ggasG0dUlHJiCCGQeXFAcf6i9zpyLewkIuna9V2QhEAAAAAIDv/BHzKDboSa5D15/D3Jqu9o4dWUhvcE6bym/knjbDkH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOrTdkIRAAAAACAhwBvcKbCI07AVHqsBdv+VAeBKtJ4AEkpbb64aBfNRuhwSvlQpOE0A7u9hJC8666vqwwElSd1viI3BCHxNANqA0nZCEQAAAAAg4QExiF47MOfLRWHVOXDVXL9pTAmextLHibLy3JLKf/MdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAeOABAAAAAAAA
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": null,
#   "gasData": {
#     "budget": "123000",
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
#         "SharedObject": {
#           "objectId": "0x0000000000000000000000000000000000000000000000000000000000000005",
#           "initialSharedVersion": 1,
#           "mutable": true
#         }
#       }
#     },
#     {
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0xa93f6c464f8fb8b98fb3d02112902060c8f85ea4d71cfc7777dfdbd75e68ab6d",
#           "version": "289568468",
#           "digest": "Cbin2kMMWzjtPER7GZ7ne81Dhpk2tS31MwinvTwjMEZi"
#         }
#       }
#     }
#   ],
#   "commands": [
#     {
#       "MoveCall": {
#         "package": "0x0000000000000000000000000000000000000000000000000000000000000003",
#         "module": "sui_system",
#         "function": "request_withdraw_stake",
#         "typeArguments": [],
#         "arguments": [
#           {
#             "Input": 0
#           },
#           {
#             "Input": 1
#           }
#         ]
#       }
#     }
#   ]
# }

def test_sign_unstake_whole_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAACAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABQEAAAAAAAAAAQEAqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq23UdkIRAAAAACCsVYpX4/44Cp2BWe8aVkACUW5rxtsErjUPJ6nMxaCvvQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMKc3VpX3N5c3RlbRZyZXF1ZXN0X3dpdGhkcmF3X3N0YWtlAAIBAAABAQAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAPr/xa00ggasG0dUlHJiCCGQeXFAcf6i9zpyLewkIuna9V2QhEAAAAAIDv/BHzKDboSa5D15/D3Jqu9o4dWUhvcE6bym/knjbDkH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOrTdkIRAAAAACAhwBvcKbCI07AVHqsBdv+VAeBKtJ4AEkpbb64aBfNRuhwSvlQpOE0A7u9hJC8666vqwwElSd1viI3BCHxNANqA0nZCEQAAAAAg4QExiF47MOfLRWHVOXDVXL9pTAmextLHibLy3JLKf/MdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAeOABAAAAAAAA')

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
                instructions=[ NavInsID.RIGHT_CLICK # Review transfer
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
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

# built_tx AAADAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAhANB0FAAAAAAEBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAUBAAAAAAAAAAECAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADDHN0YWtpbmdfcG9vbAVzcGxpdAACAQAAAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADCnN1aV9zeXN0ZW0WcmVxdWVzdF93aXRoZHJhd19zdGFrZQACAQIAAgAAHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgD6/8WtNIIGrBtHVJRyYgghkHlxQHH+ovc6ci3sJCLp2vVdkIRAAAAACA7/wR8yg26EmuQ9efw9yarvaOHVlIb3BOm8pv5J42w5B+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbocEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAHjgAQAAAAAAAA==
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": null,
#   "gasData": {
#     "budget": "123000",
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
#         "bytes": "QDQdBQAAAAA="
#       }
#     },
#     {
#       "Object": {
#         "SharedObject": {
#           "objectId": "0x0000000000000000000000000000000000000000000000000000000000000005",
#           "initialSharedVersion": 1,
#           "mutable": true
#         }
#       }
#     }
#   ],
#   "commands": [
#     {
#       "MoveCall": {
#         "package": "0x0000000000000000000000000000000000000000000000000000000000000003",
#         "module": "staking_pool",
#         "function": "split",
#         "typeArguments": [],
#         "arguments": [
#           {
#             "Input": 0
#           },
#           {
#             "Input": 1
#           }
#         ]
#       }
#     },
#     {
#       "MoveCall": {
#         "package": "0x0000000000000000000000000000000000000000000000000000000000000003",
#         "module": "sui_system",
#         "function": "request_withdraw_stake",
#         "typeArguments": [],
#         "arguments": [
#           {
#             "Input": 2
#           },
#           {
#             "Result": 0
#           }
#         ]
#       }
#     }
#   ]
# }
def test_sign_unstake_split_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAADAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAhANB0FAAAAAAEBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAUBAAAAAAAAAAECAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADDHN0YWtpbmdfcG9vbAVzcGxpdAACAQAAAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADCnN1aV9zeXN0ZW0WcmVxdWVzdF93aXRoZHJhd19zdGFrZQACAQIAAgAAHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgD6/8WtNIIGrBtHVJRyYgghkHlxQHH+ovc6ci3sJCLp2vVdkIRAAAAACA7/wR8yg26EmuQ9efw9yarvaOHVlIb3BOm8pv5J42w5B+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbocEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAHjgAQAAAAAAAA==')

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
                instructions=[ NavInsID.RIGHT_CLICK # Review transfer
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
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


def test_sign_unstake_staked_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAACAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABQEAAAAAAAAAAQEA0mPbzJ8T6TAri2KVGNKhLFvnVaugyXhgbMKRNnBZAV33Gw4AAAAAACDBfsqjWZzYr3r/y2WeSuQSi3FgoCPQgT6mxxHSO8kQ5QEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMKc3VpX3N5c3RlbRZyZXF1ZXN0X3dpdGhkcmF3X3N0YWtlAAIBAAABAQASVdCTBMXMgkue9P2RlvW46WFntPsCZvcN02gprVGdXQFJOVTGQ6Kqlqo9Ac8Sgh5tNhLeYFGge4cWg+gqQAjTrMTrOQAAAAAAIIngHh+/pmtHFNCd1AH4TjwGkS9C+EyKHI4xqky+3PLSElXQkwTFzIJLnvT9kZb1uOlhZ7T7Amb3DdNoKa1RnV3oAwAAAAAAAEhYpgAAAAAAAA==')

    object_list = [ base64.b64decode('AAEBxOs5AAAAAAAoSTlUxkOiqpaqPQHPEoIebTYS3mBRoHuHFoPoKkAI06w4de91AAAAAAASVdCTBMXMgkue9P2RlvW46WFntPsCZvcN02gprVGdXSCdZIW6l8e6/2RKKj/87f3lTuqOuLzaxGn2e737cI8V3WATDwAAAAAA')
      , base64.b64decode('AAIA9xsOAAAAAABQ0mPbzJ8T6TAri2KVGNKhLFvnVaugyXhgbMKRNnBZAV0LvEBHHC4MlHdvB1gaCcmbDBkRKg+8at3mmywuqL6PTwkAAAAAAAAAAF7QsgAAAAAAElXQkwTFzIJLnvT9kZb1uOlhZ7T7Amb3DdNoKa1RnV0gyhECC0cu3eeOqn5ga2K/jYGQXZsSfeBpV/9v3ftQBQTgthMAAAAAAA==')
       ]

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[ NavInsID.RIGHT_CLICK # Review transfer
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
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
