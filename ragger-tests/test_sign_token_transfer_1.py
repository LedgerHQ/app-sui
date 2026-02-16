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

# The Txs are built from mainnet data, and contains valid object data for all
# coins, and as such does not belong to the Ledger account used for signing them
# in these tests
# But as we currently don't check that the sender matches the ledger address
# these can be used as it is for the purpose of testing

# Balances of objects used in these tests
# 0xd3baa97a46a20d65f1be60cbaa160856e6aae5078e16545071e2fd3e314105b9 USDC  100023 (0.100023 USDC)
# 0x8ba6495be346c1be89e5c35cdabab145ea18990f48f2b0629ff016024c9ded45 USDC  123393 (0.123393 USDC)
# 0x8ebf87449ed242216a3c94de4935c21792618850fc781222e866dc60bb1bc8d2 USDC  121601 (0.121601 USDC)
# 0x2bf6f455be6dca289d0c6d3c65b14b36da4a4029b1d6718768a469594b32ab73 wUSDC 3 (0.000003 wUSDC)

# {"data":{"objects":{"nodes":[{"digest":"BWyUBdTBr7uMyQ1nRsQkkB5RXLNL6QWprGaqJ7atMJTC","bcs":"AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAF2L6QdAAAAACjTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFubeGAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1ICV9oiz28QN2+VFgs3VVcob35zoaZgQf5WcAe9gWdNyWoC0UAAAAAAA="}]}}}

# built_tx AAACAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHjtGpHQAAAAAgV3AuVVb8MkmvL1ZcHG8rmJiVWgdISLE76Ts2Bzamx54PL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAaComAAAAAAAA
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x0f2f8dd49e269da066f37624d33a8b4ed1b262d6a5fdf2b735be39f8d7c5acf5",
#   "expiration": null,
#   "gasData": {
#     "budget": "2501224",
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
# {
#   dataType: 'moveObject',
#   type: '0x2::coin::Coin<0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC>',
#   hasPublicTransfer: true,
#   version: 497299318,
#   bcsBytes: '07qpekaiDWXxvmDLqhYIVuaq5QeOFlRQceL9PjFBBbm3hgEAAAAAAA=='
# }

def test_sign_tx_usdc_whole_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    client.provide_dynamic_token("USDC", 6, "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7", "usdc", "USDC")

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAACAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHjtGpHQAAAAAgV3AuVVb8MkmvL1ZcHG8rmJiVWgdISLE76Ts2Bzamx54PL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAaComAAAAAAAA')

    object_list = [base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAF2L6QdAAAAACjTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFubeGAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1ICV9oiz28QN2+VFgs3VVcob35zoaZgQf5WcAe9gWdNyWoC0UAAAAAAA=')]

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

# {"data":{"objects":{"nodes":[{"digest":"2uQ3BCM51Km8pf7PCqiCBy1KFv5rZmpbkXEwu7KeS61C","bcs":"AAMHXUswJQZkXDf/EzuYxLUKWuFIQWWXONbXM9WdDSF6k78EY29pbgRDT0lOAAHH2GQcAAAAACgr9vRVvm3KKJ0MbTxlsUs22kpAKbHWcYdopGlZSzKrcwMAAAAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IPCmh/QdnSazrWlro9Zp1vvv08RlqS3ABJXMdPtQZjnIoC0UAAAAAAA="}]}}}
# built_tx AAACAQAr9vRVvm3KKJ0MbTxlsUs22kpAKbHWcYdopGlZSzKrc8fYZBwAAAAAIBxH8AZqM4yJjDK6SsqpgGiylfKUNUZzw90sis725pe3ACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHjtGpHQAAAAAgV3AuVVb8MkmvL1ZcHG8rmJiVWgdISLE76Ts2Bzamx54PL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAaComAAAAAAAA
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x0f2f8dd49e269da066f37624d33a8b4ed1b262d6a5fdf2b735be39f8d7c5acf5",
#   "expiration": null,
#   "gasData": {
#     "budget": "2501224",
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
#           "objectId": "0x2bf6f455be6dca289d0c6d3c65b14b36da4a4029b1d6718768a469594b32ab73",
#           "version": "476371143",
#           "digest": "2uQ3BCM51Km8pf7PCqiCBy1KFv5rZmpbkXEwu7KeS61C"
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
# {
#   dataType: 'moveObject',
#   type: '0x2::coin::Coin<0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN>',
#   hasPublicTransfer: true,
#   version: 476371143,
#   bcsBytes: 'K/b0Vb5tyiidDG08ZbFLNtpKQCmx1nGHaKRpWUsyq3MDAAAAAAAAAA=='
# }

def test_sign_tx_wusdc_whole_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    client.provide_dynamic_token("wUSDC", 6, "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf", "coin", "COIN")

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAACAQAr9vRVvm3KKJ0MbTxlsUs22kpAKbHWcYdopGlZSzKrc8fYZBwAAAAAIBxH8AZqM4yJjDK6SsqpgGiylfKUNUZzw90sis725pe3ACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHjtGpHQAAAAAgV3AuVVb8MkmvL1ZcHG8rmJiVWgdISLE76Ts2Bzamx54PL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAaComAAAAAAAA')

    object_list = [base64.b64decode('AAMHXUswJQZkXDf/EzuYxLUKWuFIQWWXONbXM9WdDSF6k78EY29pbgRDT0lOAAHH2GQcAAAAACgr9vRVvm3KKJ0MbTxlsUs22kpAKbHWcYdopGlZSzKrcwMAAAAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IPCmh/QdnSazrWlro9Zp1vvv08RlqS3ABJXMdPtQZjnIoC0UAAAAAAA=')]

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

# {"data":{"objects":{"nodes":[{"digest":"5DEmU82eeP1wJ6L7q8V7bLbAr4Vbjat2XkT3nbf5NmSN","bcs":"AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAE1MqYdAAAAACiLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRQHiAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IOV7R/YfpK7xICsKift4S9G6tE2+t4MyPAX4gSGmkRIYoC0UAAAAAAA="}]}}}
# {"data":{"objects":{"nodes":[{"digest":"GMyhw5NFFz4FNZHqaU1tbnzx6EYRa2pEzbGvKL4f6SFd","bcs":"AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAHmeaQdAAAAACiOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0gHbAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IK02+4bxem3JcKC41NNAanTDoQBzwHsLO6uVhtAiJfqCoC0UAAAAAAA="}]}}}
# built_tx AAAEAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDAQCLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRTUyph0AAAAAID6RSJBTwuLCyFHo8Wr624IT/QLOofOgJMYDmGxMFJY3AQCOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0uZ5pB0AAAAAIOQ/CZZSDz1bJFP7ynAKkMqm7brpDXAdHJsO5qFAXW6cACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAwEAAAEBAAECAAEDAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHjtGpHQAAAAAgV3AuVVb8MkmvL1ZcHG8rmJiVWgdISLE76Ts2Bzamx54PL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAuJEmAAAAAAAA
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x0f2f8dd49e269da066f37624d33a8b4ed1b262d6a5fdf2b735be39f8d7c5acf5",
#   "expiration": null,
#   "gasData": {
#     "budget": "2527672",
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
#       "TransferObjects": {
#         "objects": [
#           {
#             "Input": 0
#           },
#           {
#             "Input": 1
#           },
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

def test_sign_tx_three_usdc_whole_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    client.provide_dynamic_token("USDC", 6, "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7", "usdc", "USDC")

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAAEAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDAQCLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRTUyph0AAAAAID6RSJBTwuLCyFHo8Wr624IT/QLOofOgJMYDmGxMFJY3AQCOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0uZ5pB0AAAAAIOQ/CZZSDz1bJFP7ynAKkMqm7brpDXAdHJsO5qFAXW6cACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAwEAAAEBAAECAAEDAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHjtGpHQAAAAAgV3AuVVb8MkmvL1ZcHG8rmJiVWgdISLE76Ts2Bzamx54PL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAuJEmAAAAAAAA')

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

# built_tx AAADAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDAAiAOAEAAAAAAAAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAgEAAAEBAQABAQMAAAAAAQIADy+N1J4mnaBm83Yk0zqLTtGyYtal/fK3Nb45+NfFrPUB77Y+i7giCZ99h4t6xRVFae7Oh2ZEbd1a+4VmC6+gEYeO0akdAAAAACBXcC5VVvwySa8vVlwcbyuYmJVaB0hIsTvpOzYHNqbHng8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz17gIAAAAAAAAIWDoAAAAAAAA=
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x0f2f8dd49e269da066f37624d33a8b4ed1b262d6a5fdf2b735be39f8d7c5acf5",
#   "expiration": null,
#   "gasData": {
#     "budget": "3823624",
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
#           "Input": 2
#         }
#       }
#     }
#   ]
# }
# {
#   dataType: 'moveObject',
#   type: '0x2::coin::Coin<0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC>',
#   hasPublicTransfer: true,
#   version: 497299318,
#   bcsBytes: '07qpekaiDWXxvmDLqhYIVuaq5QeOFlRQceL9PjFBBbm3hgEAAAAAAA=='
# }
def test_sign_tx_usdc_split_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    client.provide_dynamic_token("USDC", 6, "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7", "usdc", "USDC")

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAADAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDAAiAOAEAAAAAAAAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAgEAAAEBAQABAQMAAAAAAQIADy+N1J4mnaBm83Yk0zqLTtGyYtal/fK3Nb45+NfFrPUB77Y+i7giCZ99h4t6xRVFae7Oh2ZEbd1a+4VmC6+gEYeO0akdAAAAACBXcC5VVvwySa8vVlwcbyuYmJVaB0hIsTvpOzYHNqbHng8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz17gIAAAAAAAAIWDoAAAAAAAA=')

    object_list = [base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAF2L6QdAAAAACjTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFubeGAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1ICV9oiz28QN2+VFgs3VVcob35zoaZgQf5WcAe9gWdNyWoC0UAAAAAAA=')]

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

# built_tx AAACAQAl2pb6fxLL9X+shfBLjCm9ldGecSNTxvRQ+Lu4pvLIQ6d2ZxwAAAAAIFQCzwkzjdDT/z8SxH21mdVkCljCe1cOGGqYLppob8NUACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHv0GqHQAAAAAgGT+sWlGna1S+/DUWLOV4sUtGm3TqNlwLgzE03ikr8NwPL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAmCsmAAAAAAAA
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x0f2f8dd49e269da066f37624d33a8b4ed1b262d6a5fdf2b735be39f8d7c5acf5",
#   "expiration": null,
#   "gasData": {
#     "budget": "2501528",
#     "price": "750",
#     "owner": null,
#     "payment": [
#       {
#         "objectId": "0xefb63e8bb822099f7d878b7ac5154569eece8766446ddd5afb85660bafa01187",
#         "version": "497697215",
#         "digest": "2hZWK92WKpHjLxxzsEseGzYtS6oU6WTBCimcuxp7wHCo"
#       }
#     ]
#   },
#   "inputs": [
#     {
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0x25da96fa7f12cbf57fac85f04b8c29bd95d19e712353c6f450f8bbb8a6f2c843",
#           "version": "476542631",
#           "digest": "6ewjNycgiXo5TuKZCN2fQRdwyuqKawffYyFfpsDQBFSs"
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
# {
#   dataType: 'moveObject',
#   type: '0x2::coin::Coin<0x8993129d72e733985f7f1a00396cbd055bad6f817fee36576ce483c8bbb8b87b::sudeng::SUDENG>',
#   hasPublicTransfer: true,
#   version: 476542631,
#   bcsBytes: 'JdqW+n8Sy/V/rIXwS4wpvZXRnnEjU8b0UPi7uKbyyENDjFkYAAAAAA=='
# }
# {"data":{"objects":{"nodes":[{"digest":"6ewjNycgiXo5TuKZCN2fQRdwyuqKawffYyFfpsDQBFSs","bcs":"AAMHiZMSnXLnM5hffxoAOWy9BVutb4F/7jZXbOSDyLu4uHsGc3VkZW5nBlNVREVORwABp3ZnHAAAAAAoJdqW+n8Sy/V/rIXwS4wpvZXRnnEjU8b0UPi7uKbyyENDjFkYAAAAAAAPL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9SClNBIS+ZkxFHxisw25kf37mBCLpipUHR8VhNn5c0lmk2CkFAAAAAAA"}]}}}
# balance: 408521795
def test_sign_tx_HIPPO_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    client.provide_dynamic_token("HIPPO", 9, "0x8993129d72e733985f7f1a00396cbd055bad6f817fee36576ce483c8bbb8b87b", "sudeng", "SUDENG")

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAACAQAl2pb6fxLL9X+shfBLjCm9ldGecSNTxvRQ+Lu4pvLIQ6d2ZxwAAAAAIFQCzwkzjdDT/z8SxH21mdVkCljCe1cOGGqYLppob8NUACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHv0GqHQAAAAAgGT+sWlGna1S+/DUWLOV4sUtGm3TqNlwLgzE03ikr8NwPL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAmCsmAAAAAAAA')

    object_list = [base64.b64decode('AAMHiZMSnXLnM5hffxoAOWy9BVutb4F/7jZXbOSDyLu4uHsGc3VkZW5nBlNVREVORwABp3ZnHAAAAAAoJdqW+n8Sy/V/rIXwS4wpvZXRnnEjU8b0UPi7uKbyyENDjFkYAAAAAAAPL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9SClNBIS+ZkxFHxisw25kf37mBCLpipUHR8VhNn5c0lmk2CkFAAAAAAA')]

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

def test_sign_tx_HIPPO_coin_fallback_internal_list(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAACAQAl2pb6fxLL9X+shfBLjCm9ldGecSNTxvRQ+Lu4pvLIQ6d2ZxwAAAAAIFQCzwkzjdDT/z8SxH21mdVkCljCe1cOGGqYLppob8NUACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHv0GqHQAAAAAgGT+sWlGna1S+/DUWLOV4sUtGm3TqNlwLgzE03ikr8NwPL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAmCsmAAAAAAAA')

    object_list = [base64.b64decode('AAMHiZMSnXLnM5hffxoAOWy9BVutb4F/7jZXbOSDyLu4uHsGc3VkZW5nBlNVREVORwABp3ZnHAAAAAAoJdqW+n8Sy/V/rIXwS4wpvZXRnnEjU8b0UPi7uKbyyENDjFkYAAAAAAAPL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9SClNBIS+ZkxFHxisw25kf37mBCLpipUHR8VhNn5c0lmk2CkFAAAAAAA')]

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

# built_tx AAACAQAkOlErOjssUas7B1ipByHf2etJJYdwBbMTSEy5doj0VgHzIR4AAAAAIN8vTwL8rbbLzRfsdy1PyOXvcrij9n34ovKk2/o0N3wNACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAG02HZIA+4tmm1GrxPh4dKNj3Fry/X/O0WxUh1ovpxisAQkHEsRdw5dbbdY9esFx0S8xZ3rE61Q5gJ3SV2OdlnYVFxwwHgAAAAAgDI4TkhHnVDhJiSloJl/c9O1pBEyKpv0JUSJ/mmyKbuVtNh2SAPuLZptRq8T4eHSjY9xa8v1/ztFsVIdaL6cYrOkCAAAAAAAA8AMmAAAAAAAA
# Transaction Commands: {
#   "version": 2,
#   "sender": "0x6d361d9200fb8b669b51abc4f87874a363dc5af2fd7fced16c54875a2fa718ac",
#   "expiration": null,
#   "gasData": {
#     "budget": "2491376",
#     "price": "745",
#     "owner": null,
#     "payment": [
#       {
#         "objectId": "0x090712c45dc3975b6dd63d7ac171d12f31677ac4eb5439809dd257639d967615",
#         "version": "506469399",
#         "digest": "r1Ye5Jfy67g4rgqQUybYzXFAQFApgsry1YL3u1uczKN"
#       }
#     ]
#   },
#   "inputs": [
#     {
#       "Object": {
#         "ImmOrOwnedObject": {
#           "objectId": "0x243a512b3a3b2c51ab3b0758a90721dfd9eb4925877005b313484cb97688f456",
#           "version": "505541377",
#           "digest": "G2DktCyX91sGmyLDFbvuRJdk9PpJnAFhowHCQenc2kpc"
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
def test_sign_tx_unrecognized_coin(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    _, public_key, _, _ = client.get_public_key(path=path)
    assert len(public_key) == 32

    transaction = base64.b64decode('AAAAAAACAQAkOlErOjssUas7B1ipByHf2etJJYdwBbMTSEy5doj0VgHzIR4AAAAAIN8vTwL8rbbLzRfsdy1PyOXvcrij9n34ovKk2/o0N3wNACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEBAQEAAAEBAG02HZIA+4tmm1GrxPh4dKNj3Fry/X/O0WxUh1ovpxisAQkHEsRdw5dbbdY9esFx0S8xZ3rE61Q5gJ3SV2OdlnYVFxwwHgAAAAAgDI4TkhHnVDhJiSloJl/c9O1pBEyKpv0JUSJ/mmyKbuVtNh2SAPuLZptRq8T4eHSjY9xa8v1/ztFsVIdaL6cYrOkCAAAAAAAA8AMmAAAAAAAA')

    object_list = [base64.b64decode('AAMHkOjF9XYq+mdulvsw4s0Mm/3IQNQy7OWHk/VPQtmYGwkFa2lsbGEFS0lMTEEAAQHzIR4AAAAAKCQ6USs6OyxRqzsHWKkHId/Z60klh3AFsxNITLl2iPRW5Rb4fgVVAAAAbTYdkgD7i2abUavE+Hh0o2PcWvL9f87RbFSHWi+nGKwgeQhHBdsHFvOvjCyMxNjkg1Ue4ypBA1B5GpIVylbqy2UAaRQAAAAAAA==')]

    def apdu_task():
        return client.sign_tx(path=path, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[ NavInsID.RIGHT_CLICK # Review transfer
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # From ...
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # To ...
                               , NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK # Coin
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
