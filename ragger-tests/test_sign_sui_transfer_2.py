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

# {"data":{"objects":{"nodes":[{"digest":"53CbPjHczNtV9Kids6JdGt9bkPbSeJ34dc9TX2W2g6tT","bcs":"AAEB1XZCEQAAAAAo6/8WtNIIGrBtHVJRyYgghkHlxQHH+ovc6ci3sJCLp2tAnHECAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAuq6BxxXPwIbLsDoXWJN6/Emi0EtUzGJnln5pJL4iDYWATDwAAAAAA"}]}}}

# built_tx AAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zAQDr/xa00ggasG0dUlHJiCCGQeXFAcf6i9zpyLewkIuna9V2QhEAAAAAIDv/BHzKDboSa5D15/D3Jqu9o4dWUhvcE6bym/knjbDkACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQMDAQAAAQEBAAMBAAABAQIAAQEBAAABAwAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAICEHgAAAAAAAA==
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": null,
#   "gasData": {
#     "budget": "2000000",
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
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0xebff16b4d2081ab06d1d5251c988208641e5c501c7fa8bdce9c8b7b0908ba76b",
#           "version": "289568469",
#           "digest": "53CbPjHczNtV9Kids6JdGt9bkPbSeJ34dc9TX2W2g6tT"
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
#       "MergeCoins": {
#         "destination": {
#           "Input": 0
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
#             "Input": 0
#           }
#         ],
#         "address": {
#           "Input": 3
#         }
#       }
#     }
#   ]
# }

def test_sign_tx_sui_two_merge_input_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zAQDr/xa00ggasG0dUlHJiCCGQeXFAcf6i9zpyLewkIuna9V2QhEAAAAAIDv/BHzKDboSa5D15/D3Jqu9o4dWUhvcE6bym/knjbDkACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQMDAQAAAQEBAAMBAAABAQIAAQEBAAABAwAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAICEHgAAAAAAAA==')

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

# built_tx AAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zAQDr/xa00ggasG0dUlHJiCCGQeXFAcf6i9zpyLewkIuna9V2QhEAAAAAIDv/BHzKDboSa5D15/D3Jqu9o4dWUhvcE6bym/knjbDkACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQMDAQAAAQEBAAMBAgABAQAAAQEBAgABAwAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAICEHgAAAAAAAA==
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": null,
#   "gasData": {
#     "budget": "2000000",
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
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0xebff16b4d2081ab06d1d5251c988208641e5c501c7fa8bdce9c8b7b0908ba76b",
#           "version": "289568469",
#           "digest": "53CbPjHczNtV9Kids6JdGt9bkPbSeJ34dc9TX2W2g6tT"
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
#       "MergeCoins": {
#         "destination": {
#           "Input": 2
#         },
#         "sources": [
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

def test_sign_tx_sui_two_merge_input_coin_2(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zAQDr/xa00ggasG0dUlHJiCCGQeXFAcf6i9zpyLewkIuna9V2QhEAAAAAIDv/BHzKDboSa5D15/D3Jqu9o4dWUhvcE6bym/knjbDkACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQMDAQAAAQEBAAMBAgABAQAAAQEBAgABAwAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAAICEHgAAAAAAAA==')

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

# built_tx AAACAQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQIDAAEBAAABAQABAQAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAKpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9H4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOrTdkIRAAAAACAhwBvcKbCI07AVHqsBdv+VAeBKtJ4AEkpbb64aBfNRuh0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SI6AMAAAAAAAAYqx4AAAAAAAA=
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": null,
#   "gasData": {
#     "budget": "2009880",
#     "price": "1000",
#     "owner": null,
#     "payment": [
#       {
#         "objectId": "0xa93f6c464f8fb8b98fb3d02112902060c8f85ea4d71cfc7777dfdbd75e68ab6d",
#         "version": "289568468",
#         "digest": "Cbin2kMMWzjtPER7GZ7ne81Dhpk2tS31MwinvTwjMEZi"
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
#       "TransferObjects": {
#         "objects": [
#           {
#             "GasCoin": true
#           }
#         ],
#         "address": {
#           "Input": 1
#         }
#       }
#     }
#   ]
# }

def test_sign_tx_sui_merge_input_coin_plus_two_gas(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAACAQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQIDAAEBAAABAQABAQAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAKpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9H4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOrTdkIRAAAAACAhwBvcKbCI07AVHqsBdv+VAeBKtJ4AEkpbb64aBfNRuh0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SI6AMAAAAAAAAYqx4AAAAAAAA=')

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

# built_tx AAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zAAjoAwAAAAAAAAAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiEDAwEAAAEBAQACAQAAAQECAAEBAwEAAAABAwAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAABDlLQAAAAAAAA==
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
#         "bytes": "6AMAAAAAAAA="
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
#       "SplitCoins": {
#         "coin": {
#           "Input": 0
#         },
#         "amounts": [
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
#               1,
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

def test_sign_tx_sui_merge_split(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AQAcEr5UKThNAO7vYSQvOuur6sMBJUndb4iNwQh8TQDagNJ2QhEAAAAAIOEBMYheOzDny0Vh1Tlw1Vy/aUwJnsbSx4my8tySyn/zAAjoAwAAAAAAAAAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiEDAwEAAAEBAQACAQAAAQECAAEBAwEAAAABAwAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEfh2/wFEOG3PTohsXeU7MmxxjMEiHhzOpx74qmIxpA6tN2QhEAAAAAICHAG9wpsIjTsBUeqwF2/5UB4Eq0ngASSltvrhoF81G6HT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IjoAwAAAAAAABDlLQAAAAAAAA==')

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

# built_tx AAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9ACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAQEBAAABAQABAQECAAEDAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAEOUtAAAAAAAA
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

def test_sign_tx_sui_two_transfer(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9ACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEAHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oDSdkIRAAAAACDhATGIXjsw58tFYdU5cNVcv2lMCZ7G0seJsvLcksp/8wAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAQEBAAABAQABAQECAAEDAB0/JkMwV2AiblGMm1qWFlODgI3Zd5cfc96pcVQ7C+SIAR+Hb/AUQ4bc9OiGxd5TsybHGMwSIeHM6nHviqYjGkDq03ZCEQAAAAAgIcAb3CmwiNOwFR6rAXb/lQHgSrSeABJKW2+uGgXzUbodPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAEOUtAAAAAAAA')

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

# built_tx AAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAhQwwAAAAAAAAAIgDgBAAAAAAAAIG+yH+6tAn2khzKVr/1sTzYY/hdvovvz57XvHZRjsx4hAgIBAAACAQEAAQIAAQIDAAAAAAMAAAEAAQMAHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgCH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOrTdkIRAAAAACAhwBvcKbCI07AVHqsBdv+VAeBKtJ4AEkpbb64aBfNRuhwSvlQpOE0A7u9hJC8666vqwwElSd1viI3BCHxNANqA0nZCEQAAAAAg4QExiF47MOfLRWHVOXDVXL9pTAmextLHibLy3JLKf/MdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAOOVLAAAAAAAA
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x1d3f2643305760226e518c9b5a96165383808dd977971f73dea971543b0be488",
#   "expiration": null,
#   "gasData": {
#     "budget": "4973880",
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
#         "bytes": "gDgBAAAAAAA="
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
#           },
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
#           },
#           {
#             "NestedResult": [
#               0,
#               1
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

def test_sign_tx_sui_split_transfer_two(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/1'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAEAQCpP2xGT4+4uY+z0CESkCBgyPhepNcc/Hd339vXXmirbdR2QhEAAAAAIKxVilfj/jgKnYFZ7xpWQAJRbmvG2wSuNQ8nqczFoK+9AAhQwwAAAAAAAAAIgDgBAAAAAAAAIG+yH+6tAn2khzKVr/1sTzYY/hdvovvz57XvHZRjsx4hAgIBAAACAQEAAQIAAQIDAAAAAAMAAAEAAQMAHT8mQzBXYCJuUYybWpYWU4OAjdl3lx9z3qlxVDsL5IgCH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOrTdkIRAAAAACAhwBvcKbCI07AVHqsBdv+VAeBKtJ4AEkpbb64aBfNRuhwSvlQpOE0A7u9hJC8666vqwwElSd1viI3BCHxNANqA0nZCEQAAAAAg4QExiF47MOfLRWHVOXDVXL9pTAmextLHibLy3JLKf/MdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiOgDAAAAAAAAOOVLAAAAAAAA')

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
