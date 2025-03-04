# Covers scenarios for valid token transfer txs, that are not supported by the app

import pytest
import concurrent.futures
import time
import base64

from application_client.client import Client, Errors
from contextlib import contextmanager
from ragger.error import ExceptionRAPDU
from ragger.navigator import NavIns, NavInsID
from utils import ROOT_SCREENSHOT_PATH, check_signature_validity, run_apdu_and_nav_tasks_concurrently

# built_tx AAAEAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEAK/b0Vb5tyiidDG08ZbFLNtpKQCmx1nGHaKRpWUsyq3PH2GQcAAAAACAcR/AGajOMiYwyukrKqYBospXylDVGc8PdLIrO9uaXtwAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAQEBAAABAQABAQECAAEDAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHjtGpHQAAAAAgV3AuVVb8MkmvL1ZcHG8rmJiVWgdISLE76Ts2Bzamx54PL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAEF4mAAAAAAAA
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
def test_sign_tx_usdc_and_wusdc(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    transaction = base64.b64decode('AAAAAAAEAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEAK/b0Vb5tyiidDG08ZbFLNtpKQCmx1nGHaKRpWUsyq3PH2GQcAAAAACAcR/AGajOMiYwyukrKqYBospXylDVGc8PdLIrO9uaXtwAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAQEBAAABAQABAQECAAEDAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHjtGpHQAAAAAgV3AuVVb8MkmvL1ZcHG8rmJiVWgdISLE76Ts2Bzamx54PL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAEF4mAAAAAAAA')

    object_list = [base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAF2L6QdAAAAACjTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFubeGAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1ICV9oiz28QN2+VFgs3VVcob35zoaZgQf5WcAe9gWdNyWoC0UAAAAAAA='),
                   base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAE1MqYdAAAAACiLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRQHiAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IOV7R/YfpK7xICsKift4S9G6tE2+t4MyPAX4gSGmkRIYoC0UAAAAAAA='),
                   base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAHmeaQdAAAAACiOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0gHbAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IK02+4bxem3JcKC41NNAanTDoQBzwHsLO6uVhtAiJfqCoC0UAAAAAAA='),
                   base64.b64decode('AAMHXUswJQZkXDf/EzuYxLUKWuFIQWWXONbXM9WdDSF6k78EY29pbgRDT0lOAAHH2GQcAAAAACgr9vRVvm3KKJ0MbTxlsUs22kpAKbHWcYdopGlZSzKrcwMAAAAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IPCmh/QdnSazrWlro9Zp1vvv08RlqS3ABJXMdPtQZjnIoC0UAAAAAAA=')
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

# built_tx AAAEAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQEAK/b0Vb5tyiidDG08ZbFLNtpKQCmx1nGHaKRpWUsyq3PH2GQcAAAAACAcR/AGajOMiYwyukrKqYBospXylDVGc8PdLIrO9uaXtwAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAQEBAAABAQABAQECAAEDAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1Ae+2Pou4IgmffYeLesUVRWnuzodmRG3dWvuFZguvoBGHjtGpHQAAAAAgV3AuVVb8MkmvL1ZcHG8rmJiVWgdISLE76Ts2Bzamx54PL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9e4CAAAAAAAAEF4mAAAAAAAA
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

# built_tx AAADAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAQEBAAABAQABAQABAgAPL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9QHvtj6LuCIJn32Hi3rFFUVp7s6HZkRt3Vr7hWYLr6ARh79Bqh0AAAAAIBk/rFpRp2tUvvw1FizleLFLRpt06jZcC4MxNN4pK/DcDy+N1J4mnaBm83Yk0zqLTtGyYtal/fK3Nb45+NfFrPXuAgAAAAAAAGgqJgAAAAAAAA==
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
#         "version": "497697215",
#         "digest": "2hZWK92WKpHjLxxzsEseGzYtS6oU6WTBCimcuxp7wHCo"
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
def test_sign_tx_usdc_and_sui(backend, scenario_navigator, firmware, navigator):
    client = Client(backend, use_block_protocol=True)
    path = "m/44'/784'/0'/0'/0'"

    transaction = base64.b64decode('AAAAAAADAQDTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFuXYvpB0AAAAAIJxCj+NcqbGRfo0O7b01nRHCGKviMzj24EOLiWE+lIpDACBvsh/urQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeIQAgb7If7q0CfaSHMpWv/WxPNhj+F2+i+/Pnte8dlGOzHiECAQEBAAABAQABAQABAgAPL43UniadoGbzdiTTOotO0bJi1qX98rc1vjn418Ws9QHvtj6LuCIJn32Hi3rFFUVp7s6HZkRt3Vr7hWYLr6ARh79Bqh0AAAAAIBk/rFpRp2tUvvw1FizleLFLRpt06jZcC4MxNN4pK/DcDy+N1J4mnaBm83Yk0zqLTtGyYtal/fK3Nb45+NfFrPXuAgAAAAAAAGgqJgAAAAAAAA==')

    object_list = [base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAF2L6QdAAAAACjTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFubeGAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1ICV9oiz28QN2+VFgs3VVcob35zoaZgQf5WcAe9gWdNyWoC0UAAAAAAA='),
                   base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAE1MqYdAAAAACiLpklb40bBvonlw1zaurFF6hiZD0jysGKf8BYCTJ3tRQHiAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IOV7R/YfpK7xICsKift4S9G6tE2+t4MyPAX4gSGmkRIYoC0UAAAAAAA='),
                   base64.b64decode('AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAHmeaQdAAAAACiOv4dEntJCIWo8lN5JNcIXkmGIUPx4EiLoZtxguxvI0gHbAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IK02+4bxem3JcKC41NNAanTDoQBzwHsLO6uVhtAiJfqCoC0UAAAAAAA='),
                   base64.b64decode('AAMHXUswJQZkXDf/EzuYxLUKWuFIQWWXONbXM9WdDSF6k78EY29pbgRDT0lOAAHH2GQcAAAAACgr9vRVvm3KKJ0MbTxlsUs22kpAKbHWcYdopGlZSzKrcwMAAAAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1IPCmh/QdnSazrWlro9Zp1vvv08RlqS3ABJXMdPtQZjnIoC0UAAAAAAA=')
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
