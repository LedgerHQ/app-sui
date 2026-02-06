# Covers various scenarios for token transfer txs supported by the app

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
# 0xd3baa97a46a20d65f1be60cbaa160856e6aae5078e16545071e2fd3e314105b9 USDC  100023 (0.100023 USDC)
# 0x8ba6495be346c1be89e5c35cdabab145ea18990f48f2b0629ff016024c9ded45 USDC  123393 (0.123393 USDC)
# 0x8ebf87449ed242216a3c94de4935c21792618850fc781222e866dc60bb1bc8d2 USDC  121601 (0.121601 USDC)
# 0x2bf6f455be6dca289d0c6d3c65b14b36da4a4029b1d6718768a469594b32ab73 wUSDC 3 (0.000003 wUSDC)

# built_tx AAAEAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDAQCLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRTUyph0AAAAAID6RSJBTwuLCyFHo8Wr624IT/QLOofOgJMYDmGxMFJY3AQCOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0uZ5pB0AAAAAIOQ/CZZSDz1bJFP7ynAKkMqm7brpDXAdHJsO5qFAXW6cACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQIDAQAAAgEBAAECAAEBAQAAAQMADy+N1J4mnaBm83Yk0zqLTtGyYtal/fK3Nb45+NfFrPUB77Y+i7giCZ99h4t6xRVFae7Oh2ZEbd1a+4VmC6+gEYeO0akdAAAAACBXcC5VVvwySa8vVlwcbyuYmJVaB0hIsTvpOzYHNqbHng8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz17gIAAAAAAABg4xYAAAAAAAA=
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x0f2f8dd49e269da066f37624d33a8b4ed1b262d6a5fdf2b735be39f8d7c5acf5",
#   "expiration": null,
#   "gasData": {
#     "budget": "1500000",
#     "price": "750",
#     "owner": null,
#     "payment": [
#       {
#         "objectId": "0xefb63e8bb822099f7d878b7ac5154569eece8766446ddd5afb85660bafa01187",
#         "version": "497668494",
#         "digest": "6tKgd63ZMmb4CA74EjWN7Wv4yx3hCiBqo2PnKTU4hr9T"
#       }
#     ]
#   },
#   "inputs": [
#     {
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0xd3baa97a46a20d65f1be60cbaa160856e6aae5078e16545071e2fd3e314105b9",
#           "version": "497299318",
#           "digest": "BWyUBdTBr7uMyQ1nRsQkkB5RXLNL6QWprGaqJ7atMJTC"
#         }
#       }
#     },
#     {
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0x8ba6495be346c1be89e5c35cdabab145ea18990f48f2b0629ff016024c9ded45",
#           "version": "497431093",
#           "digest": "5DEmU82eeP1wJ6L7q8V7bLbAr4Vbjat2XkT3nbf5NmSN"
#         }
#       }
#     },
#     {
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0x8ebf87449ed242216a3c94de4935c21792618850fc781222e866dc60bb1bc8d2",
#           "version": "497318374",
#           "digest": "GMyhw5NFFz4FNZHqaU1tbnzx6EYRa2pEzbGvKL4f6SFd"
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
# {
#   dataType: 'moveObject',
#   type: '0x2::coin::Coin<0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC>',
#   hasPublicTransfer: true,
#   version: 497431093,
#   bcsBytes: 'i6ZJW+NGwb6J5cNc2rqxReoYmQ9I8rBin/AWAkyd7UUB4gEAAAAAAA=='
# }
# {
#   dataType: 'moveObject',
#   type: '0x2::coin::Coin<0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC>',
#   hasPublicTransfer: true,
#   version: 497318374,
#   bcsBytes: 'jr+HRJ7SQiFqPJTeSTXCF5JhiFD8eBIi6GbcYLsbyNIB2wEAAAAAAA=='
# }
def test_sign_tx_usdc_merge_three(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    client.provide_dynamic_token("USDC", 6, "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7", "usdc", "USDC")

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAEAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDAQCLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRTUyph0AAAAAID6RSJBTwuLCyFHo8Wr624IT/QLOofOgJMYDmGxMFJY3AQCOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0uZ5pB0AAAAAIOQ/CZZSDz1bJFP7ynAKkMqm7brpDXAdHJsO5qFAXW6cACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQIDAQAAAgEBAAECAAEBAQAAAQMADy+N1J4mnaBm83Yk0zqLTtGyYtal/fK3Nb45+NfFrPUB77Y+i7giCZ99h4t6xRVFae7Oh2ZEbd1a+4VmC6+gEYeO0akdAAAAACBXcC5VVvwySa8vVlwcbyuYmJVaB0hIsTvpOzYHNqbHng8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz17gIAAAAAAABg4xYAAAAAAAA=')

    object_list = [base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAF2L6QdAAAAACjTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFubeGAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1ICV9oiz28QN2+VFgs3VVcob35zoaZgQf5WcAe9gWdNyWoC0UAAAAAAA='),
                   base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAE1MqYdAAAAACiLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRQHiAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IOV7R/YfpK7xICsKift4S9G6tE2+t4MyPAX4gSGmkRIYoC0UAAAAAAA='),
                   base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAHmeaQdAAAAACiOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0gHbAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IK02+4bxem3JcKC41NNAanTDoQBzwHsLO6uVhtAiJfqCoC0UAAAAAAA=')
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


# built_tx AAAFAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDAQCLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRTUyph0AAAAAID6RSJBTwuLCyFHo8Wr624IT/QLOofOgJMYDmGxMFJY3AQCOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0uZ5pB0AAAAAIOQ/CZZSDz1bJFP7ynAKkMqm7brpDXAdHJsO5qFAXW6cAAg4RAEAAAAAAAAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiEDAwEAAAIBAQABAgACAQAAAQEDAAEBAwEAAAABBAAPL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9QHvtj6LuCIJn32Hi3rFFUVp7s6HZkRt3Vr7hWYLr6ARh47RqR0AAAAAIFdwLlVW/DJJry9WXBxvK5iYlVoHSEixO+k7Ngc2pseeDy+N1J4mnaBm83Yk0zqLTtGyYtal/fK3Nb45+NfFrPXuAgAAAAAAAGDjFgAAAAAAAA==
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x0f2f8dd49e269da066f37624d33a8b4ed1b262d6a5fdf2b735be39f8d7c5acf5",
#   "expiration": null,
#   "gasData": {
#     "budget": "1500000",
#     "price": "750",
#     "owner": null,
#     "payment": [
#       {
#         "objectId": "0xefb63e8bb822099f7d878b7ac5154569eece8766446ddd5afb85660bafa01187",
#         "version": "497668494",
#         "digest": "6tKgd63ZMmb4CA74EjWN7Wv4yx3hCiBqo2PnKTU4hr9T"
#       }
#     ]
#   },
#   "inputs": [
#     {
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0xd3baa97a46a20d65f1be60cbaa160856e6aae5078e16545071e2fd3e314105b9",
#           "version": "497299318",
#           "digest": "BWyUBdTBr7uMyQ1nRsQkkB5RXLNL6QWprGaqJ7atMJTC"
#         }
#       }
#     },
#     {
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0x8ba6495be346c1be89e5c35cdabab145ea18990f48f2b0629ff016024c9ded45",
#           "version": "497431093",
#           "digest": "5DEmU82eeP1wJ6L7q8V7bLbAr4Vbjat2XkT3nbf5NmSN"
#         }
#       }
#     },
#     {
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0x8ebf87449ed242216a3c94de4935c21792618850fc781222e866dc60bb1bc8d2",
#           "version": "497318374",
#           "digest": "GMyhw5NFFz4FNZHqaU1tbnzx6EYRa2pEzbGvKL4f6SFd"
#         }
#       }
#     },
#     {
#       "Pure": {
#         "bytes": "OEQBAAAAAAA="
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
#           },
#           {
#             "Input": 2
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

def test_sign_tx_usdc_merge_split(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    client.provide_dynamic_token("USDC", 6, "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7", "usdc", "USDC")

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAFAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDAQCLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRTUyph0AAAAAID6RSJBTwuLCyFHo8Wr624IT/QLOofOgJMYDmGxMFJY3AQCOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0uZ5pB0AAAAAIOQ/CZZSDz1bJFP7ynAKkMqm7brpDXAdHJsO5qFAXW6cAAg4RAEAAAAAAAAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiEDAwEAAAIBAQABAgACAQAAAQEDAAEBAwEAAAABBAAPL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9QHvtj6LuCIJn32Hi3rFFUVp7s6HZkRt3Vr7hWYLr6ARh47RqR0AAAAAIFdwLlVW/DJJry9WXBxvK5iYlVoHSEixO+k7Ngc2pseeDy+N1J4mnaBm83Yk0zqLTtGyYtal/fK3Nb45+NfFrPXuAgAAAAAAAGDjFgAAAAAAAA==')

    object_list = [base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAF2L6QdAAAAACjTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFubeGAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1ICV9oiz28QN2+VFgs3VVcob35zoaZgQf5WcAe9gWdNyWoC0UAAAAAAA='),
                   base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAE1MqYdAAAAACiLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRQHiAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IOV7R/YfpK7xICsKift4S9G6tE2+t4MyPAX4gSGmkRIYoC0UAAAAAAA='),
                   base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAHmeaQdAAAAACiOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0gHbAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IK02+4bxem3JcKC41NNAanTDoQBzwHsLO6uVhtAiJfqCoC0UAAAAAAA=')
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

# built_tx AAAEAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEAi6ZJW+NGwb6J5cNc2rqxReoYmQ9I8rBin/AWAkyd7UU1MqYdAAAAACA+kUiQU8LiwshR6PFq+tuCE/0CzqHzoCTGA5hsTBSWNwAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAQEBAAABAQABAQECAAEDAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHjtGpHQAAAAAgV3AuVVb8MkmvL1ZcHG8rmJiVWgdISLE76Ts2Bzamx54PL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAEF4mAAAAAAAA
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x0f2f8dd49e269da066f37624d33a8b4ed1b262d6a5fdf2b735be39f8d7c5acf5",
#   "expiration": null,
#   "gasData": {
#     "budget": "2514448",
#     "price": "750",
#     "owner": null,
#     "payment": [
#       {
#         "objectId": "0xefb63e8bb822099f7d878b7ac5154569eece8766446ddd5afb85660bafa01187",
#         "version": "497668494",
#         "digest": "6tKgd63ZMmb4CA74EjWN7Wv4yx3hCiBqo2PnKTU4hr9T"
#       }
#     ]
#   },
#   "inputs": [
#     {
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0xd3baa97a46a20d65f1be60cbaa160856e6aae5078e16545071e2fd3e314105b9",
#           "version": "497299318",
#           "digest": "BWyUBdTBr7uMyQ1nRsQkkB5RXLNL6QWprGaqJ7atMJTC"
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
#           "objectId": "0x8ba6495be346c1be89e5c35cdabab145ea18990f48f2b0629ff016024c9ded45",
#           "version": "497431093",
#           "digest": "5DEmU82eeP1wJ6L7q8V7bLbAr4Vbjat2XkT3nbf5NmSN"
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
def test_sign_tx_usdc_transfer_two(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    client.provide_dynamic_token("USDC", 6, "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7", "usdc", "USDC")

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAEAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEAi6ZJW+NGwb6J5cNc2rqxReoYmQ9I8rBin/AWAkyd7UU1MqYdAAAAACA+kUiQU8LiwshR6PFq+tuCE/0CzqHzoCTGA5hsTBSWNwAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAQEBAAABAQABAQECAAEDAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHjtGpHQAAAAAgV3AuVVb8MkmvL1ZcHG8rmJiVWgdISLE76Ts2Bzamx54PL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAEF4mAAAAAAAA')

    object_list = [base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAF2L6QdAAAAACjTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFubeGAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1ICV9oiz28QN2+VFgs3VVcob35zoaZgQf5WcAe9gWdNyWoC0UAAAAAAA='),
                   base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAE1MqYdAAAAACiLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRQHiAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IOV7R/YfpK7xICsKift4S9G6tE2+t4MyPAX4gSGmkRIYoC0UAAAAAAA='),
                   base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAHmeaQdAAAAACiOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0gHbAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IK02+4bxem3JcKC41NNAanTDoQBzwHsLO6uVhtAiJfqCoC0UAAAAAAA=')
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
