from enum import IntEnum
from typing import Dict, List, Optional, Tuple
from hashlib import sha256
from struct import unpack

from ragger.backend.interface import BackendInterface, RAPDU
from ragger.error import ExceptionRAPDU
from bip_utils import Bip32Utils

from typing import List, Generator, Optional
from enum import IntEnum
from contextlib import contextmanager

from .tlv import format_tlv
from .sui_keychain import Key, sign_data

# https://ledgerhq.atlassian.net/wiki/spaces/TA/pages/6196265191/Sui+Token+Dynamic+Descriptor#2nd-solution
class DynamicTokenTag_TUID(IntEnum):
    TAG_TOKEN_PACKAGE_ADDRESS = 0x10
    TAG_TOKEN_MODULE_NAME = 0x11
    TAG_TOKEN_STRUCT_NAME = 0x12


# https://ledgerhq.atlassian.net/wiki/spaces/TA/pages/5603262535/Token+Dynamic+Descriptor#Sui
class DynamicTokenTag(IntEnum):
    STRUCTURE_TYPE = 0x01
    VERSION = 0x02
    COIN_TYPE = 0x03
    APP = 0x04
    TICKER = 0x05
    MAGNITUDE = 0x06
    TUID = 0x07
    SIGNATURE = 0x08

class CertificatePubKeyUsage(IntEnum):
    CERTIFICATE_PUBLIC_KEY_USAGE_COIN_META            = 0x08,

MAX_APDU_LEN: int = 255

CLA: int = 0x00
P1: int = 0x00
P2: int = 0x00

class InsType(IntEnum):
    GET_VERSION                         = 0x00
    GET_APP_NAME                        = 0x00
    VERIFY_ADDRESS                      = 0x01
    GET_PUBLIC_KEY                      = 0x02
    SIGN_TX                             = 0x03
    PROVIDE_TRUSTED_DYNAMIC_DESCRIPTOR  = 0x22

class Errors(IntEnum):
    SW_DENY                    = 0x6985
    SW_WRONG_P1P2              = 0x6A86
    SW_INS_NOT_SUPPORTED       = 0x6D00
    SW_CLA_NOT_SUPPORTED       = 0x6E00
    SW_WRONG_APDU_LENGTH       = 0x6E03
    SW_WRONG_RESPONSE_LENGTH   = 0xB000
    SW_DISPLAY_BIP32_PATH_FAIL = 0xB001
    SW_DISPLAY_ADDRESS_FAIL    = 0xB002
    SW_DISPLAY_AMOUNT_FAIL     = 0xB003
    SW_WRONG_TX_LENGTH         = 0xB004
    SW_TX_PARSING_FAIL         = 0xB005
    SW_TX_HASH_FAIL            = 0xB006
    SW_BAD_STATE               = 0xB007
    SW_SIGNATURE_FAIL          = 0xB008

DYNAMIC_TOKEN = 0x90

def split_message(message: bytes, max_size: int) -> List[bytes]:
    return [message[x:x + max_size] for x in range(0, len(message), max_size)]

class PKIClient:
    _CLA: int = 0xB0
    _INS: int = 0x06

    def __init__(self, client: BackendInterface) -> None:
        self._client = client

    def send_certificate(self, key_usage: CertificatePubKeyUsage, payload: bytes) -> RAPDU:
        response = self.send_raw(key_usage, payload)

    def send_raw(self, key_usage: CertificatePubKeyUsage, payload: bytes) -> RAPDU:
        header = bytearray()
        header.append(self._CLA)
        header.append(self._INS)
        header.append(key_usage)
        header.append(0x00)
        header.append(len(payload))
        return self._client.exchange_raw(header + payload)

class Client:
    def __init__(self, backend: BackendInterface, use_block_protocol: bool=False) -> None:
        self.backend = backend

        self._pki_client = PKIClient(self.backend)

        if use_block_protocol:
            self.send_fn = self.send_with_blocks
        else:
            self.send_fn = self.send_chunks

    def set_use_block_protocol(self, v):
        if v:
            self.send_fn = self.send_with_blocks
        else:
            self.send_fn = self.send_chunks

    def get_app_and_version(self) -> Tuple[Tuple[int, int, int], str]:
        response = self.send_fn(cla=CLA,
                            ins=InsType.GET_VERSION,
                            p1=P1,
                            p2=P2,
                            payload=[b""])
        print(response)
        major, minor, patch = unpack("BBB", response[:3])
        return ((major, minor, patch), response[3:].decode("ascii"))

    def get_public_key(self, path: str) -> Tuple[int, bytes, int, bytes]:
        return self.get_public_key_impl(InsType.GET_PUBLIC_KEY, path)

    def get_public_key_with_confirmation(self, path: str) -> Tuple[int, bytes, int, bytes]:
        return self.get_public_key_impl(InsType.VERIFY_ADDRESS, path)


    def get_public_key_impl(self, ins, path: str) -> Tuple[int, bytes, int, bytes]:
        response = self.send_fn(cla=CLA,
                                ins=ins,
                                p1=P1,
                                p2=P2,
                                payload=[pack_derivation_path(path)])
        response, pub_key_len, pub_key = pop_size_prefixed_buf_from_buf(response)
        response, chain_code_len, chain_code = pop_size_prefixed_buf_from_buf(response)
        return pub_key_len, pub_key, chain_code_len, chain_code
    
    def provide_dynamic_token(self,
                              ticker: str,
                              magnitude: int,
                              address: str,
                              module: str,
                              struct: str):
        tuid = format_tlv(DynamicTokenTag_TUID.TAG_TOKEN_PACKAGE_ADDRESS, address)
        tuid += format_tlv(DynamicTokenTag_TUID.TAG_TOKEN_MODULE_NAME, module)
        tuid += format_tlv(DynamicTokenTag_TUID.TAG_TOKEN_STRUCT_NAME, struct)

        print(tuid)

        payload = format_tlv(DynamicTokenTag.STRUCTURE_TYPE, DYNAMIC_TOKEN)
        payload += format_tlv(DynamicTokenTag.VERSION, 1)
        payload += format_tlv(DynamicTokenTag.COIN_TYPE, 0x80000310)
        payload += format_tlv(DynamicTokenTag.APP, "Sui")
        payload += format_tlv(DynamicTokenTag.TICKER, ticker)
        payload += format_tlv(DynamicTokenTag.MAGNITUDE, magnitude)
        payload += format_tlv(DynamicTokenTag.TUID, tuid)
        payload += format_tlv(DynamicTokenTag.SIGNATURE, sign_data(Key.DYNAMIC_TOKEN, payload))

        p_len = (len(payload)).to_bytes(2, byteorder='little')
        payload = p_len + payload

        # send PKI certificate
        # pylint: disable=line-too-long
        if self.backend.device.name == "nanosp":
            cert_apdu = "01010102010211040000000212010013020002140101160400000000200D44796E616D69635F546F6B656E3002000E310108320121332102E3C05B637A0626AB382004A9350BEB1DE47958A3B9FC1EFC6E16A72104C05FE73401013501031546304402207756CFA4C31D0732F45FE12AD824262C1359BD49A89EE847A5322D99023E1D2802204D6B36F57BE3DD3D0FF2BBD5ECC71FAAE5D52899742673200D9F8236A58913E3"  # noqa: E501
        elif self.backend.device.name == "nanox":
            cert_apdu = "01010102010211040000000212010013020002140101160400000000200D44796E616D69635F546F6B656E3002000E310108320121332102E3C05B637A0626AB382004A9350BEB1DE47958A3B9FC1EFC6E16A72104C05FE734010135010215473045022100B483BB3A8D6B4EEF83DD75E7ADBFB4330F5C46BBCFAF343B6997B964C6DA1DCB022077F1F41F2C7931CF191364D0A4071E1DB8CE4FA510BC0E7E7517E7E973F4A1DA"  # noqa: E501
        elif self.backend.device.name == "stax":
            cert_apdu = "01010102010211040000000212010013020002140101160400000000200D44796E616D69635F546F6B656E3002000E310108320121332102E3C05B637A0626AB382004A9350BEB1DE47958A3B9FC1EFC6E16A72104C05FE734010135010415463044022037C48A6217CE3EA24351B4DEB714A8420A64F985DF62E1820D421C6AC8AFF75F0220290A3296A4263346866F52520197C6FA2A5E6E32E464833D3598706EE9158DB0"  # noqa: E501
        elif self.backend.device.name == "flex":
            cert_apdu = "01010102010211040000000212010013020002140101160400000000200D44796E616D69635F546F6B656E3002000E310108320121332102E3C05B637A0626AB382004A9350BEB1DE47958A3B9FC1EFC6E16A72104C05FE734010135010515473045022100B31EF0A2398641FE938C55568DFC5F1A4297244C765A0C0659821361B812A4C6022023559941A0D58FEDA2B37008DDE3B1B27C85A4AEEAFF7D74BC68E093CEC4585E"  # noqa: E501
        elif self.backend.device.name == "apex_p":
            cert_apdu = "01010102010211040000000212010013020002140101160400000000200D44796E616D69635F546F6B656E3002000E310108320121332102E3C05B637A0626AB382004A9350BEB1DE47958A3B9FC1EFC6E16A72104C05FE73401013501061547304502210082871ACD246B31207A6D526628D0F4EB702F5DF60A906502AC94E8D5FF2C7A89022016C84B407E53113F26916C574C0B6DC4FC129C401E44FEBCB64C01C1BACC9E82"  # noqa: E501
        # pylint: enable=line-too-long

        self._pki_client.send_certificate(CertificatePubKeyUsage.CERTIFICATE_PUBLIC_KEY_USAGE_COIN_META,
                                          bytes.fromhex(cert_apdu))

        self.send_fn(cla=CLA,
                    ins=InsType.PROVIDE_TRUSTED_DYNAMIC_DESCRIPTOR,
                    p1=P1,
                    p2=P2,
                    payload=payload)

    def sign_tx(self, path: str, transaction: bytes, object_list: Optional[list[bytes]] = None) -> bytes:
        if object_list is None:
            object_list = []
        tx_len = (len(transaction)).to_bytes(4, byteorder='little')
        tx_data = tx_len + transaction
        path_data = pack_derivation_path(path)

        num_items = len(object_list).to_bytes(4, byteorder='little')  # First byte is number of items
        list_data = bytearray(num_items)

        # Add each item with its length prefix
        for item in object_list:
            item_len = len(item).to_bytes(4, byteorder='little')  # Length of each item
            list_data.extend(item_len)
            list_data.extend(item)

        if len(object_list) > 0:
            payload = [tx_data, path_data, bytes(list_data)]
        else:
            payload = [tx_data, path_data]

        return self.send_fn(cla=CLA,
                     ins=InsType.SIGN_TX,
                     p1=P1,
                     p2=P2,
                     payload=payload)

    def get_async_response(self) -> Optional[RAPDU]:
        return self.backend.last_async_response

    def send_chunks(self, cla, ins, p1, p2, payload: [bytes]) -> bytes:
        messages = split_message(b''.join(payload), MAX_APDU_LEN)
        if messages == []:
            messages = [b'']

        result = b''

        for msg in messages:
            # print(f"send_chunks {msg}")
            rapdu = self.backend.exchange(cla=cla,
                                           ins=ins,
                                           p1=p1,
                                           p2=p2,
                                           data=msg)
            # print(f"send_chunks after {msg}")
            result = rapdu.data

        return result

    def exchange_raw(self, payload: bytes) -> bytes:
        rapdu = self.backend.exchange_raw(data=payload)
        return rapdu.data

    # Block Protocol
    def send_with_blocks(self, cla, ins, p1, p2, payload: [bytes], extra_data: Dict[str, bytes] = {}) -> bytes:
        chunk_size = 180
        parameter_list = []

        if not isinstance(payload, list):
            payload = [payload]

        data = {}

        if extra_data:
            data.update(extra_data)

        for item in payload:
            chunk_list = []
            for i in range(0, len(item), chunk_size):
                chunk = item[i:i + chunk_size]
                chunk_list.append(chunk)

            last_hash = b'\x00' * 32

            for chunk in reversed(chunk_list):
                linked_chunk = last_hash + chunk
                last_hash = sha256(linked_chunk).digest()
                data[last_hash.hex()] = linked_chunk

            parameter_list.append(last_hash)

        initialPayload = HostToLedger.START.to_bytes(1, byteorder='little') + b''.join(parameter_list)

        return self.handle_block_protocol(cla, ins, p1, p2, initialPayload, data)

    def handle_block_protocol(self, cla, ins, p1, p2, initialPayload: bytes, data: Dict[str, bytes]) -> bytes:
        payload = initialPayload
        rv_instruction = -1
        result = b''

        while (rv_instruction != LedgerToHost.RESULT_FINAL):
            rapdu = self.backend.exchange(cla=cla,
                                     ins=ins,
                                     p1=p1,
                                     p2=p2,
                                     data=payload)
            rv = rapdu.data
            rv_instruction = rv[0]
            rv_payload = rv[1:]

            if rv_instruction == LedgerToHost.RESULT_ACCUMULATING:
                result = result + rv_payload
                payload = HostToLedger.RESULT_ACCUMULATING_RESPONSE.to_bytes(1, byteorder='little')
            elif rv_instruction == LedgerToHost.RESULT_FINAL:
                result = result + rv_payload
            elif rv_instruction == LedgerToHost.GET_CHUNK:
                chunk_hash = rv_payload.hex()
                if chunk_hash in data:
                    chunk = data[rv_payload.hex()]
                    payload = HostToLedger.GET_CHUNK_RESPONSE_SUCCESS.to_bytes(1, byteorder='little') + chunk
                else:
                    payload = HostToLedger.GET_CHUNK_RESPONSE_FAILURE.to_bytes(1, byteorder='little')
            elif rv_instruction == LedgerToHost.PUT_CHUNK:
                data[sha256(rv_payload).hexdigest()] = rv_payload
                payload = HostToLedger.PUT_CHUNK_RESPONSE.to_bytes(1, byteorder='little')
            else:
                raise RuntimeError("Unknown instruction returned from ledger")

        return result

class LedgerToHost(IntEnum):
    RESULT_ACCUMULATING = 0
    RESULT_FINAL = 1
    GET_CHUNK = 2
    PUT_CHUNK = 3

class HostToLedger(IntEnum):
    START = 0
    GET_CHUNK_RESPONSE_SUCCESS = 1
    GET_CHUNK_RESPONSE_FAILURE = 2
    PUT_CHUNK_RESPONSE = 3
    RESULT_ACCUMULATING_RESPONSE = 4

def pack_derivation_path(derivation_path: str) -> bytes:
    split = derivation_path.split("/")

    if split[0] != "m":
        raise ValueError("Error master expected")

    path_bytes: bytes = (len(split) - 1).to_bytes(1, byteorder='little')
    for value in split[1:]:
        if value == "":
            raise ValueError(f'Error missing value in split list "{split}"')
        if value.endswith('\''):
            path_bytes += Bip32Utils.HardenIndex(int(value[:-1])).to_bytes(4, byteorder='little')
        else:
            path_bytes += int(value).to_bytes(4, byteorder='little')
    return path_bytes

# remainder, data_len, data
def pop_sized_buf_from_buffer(buffer:bytes, size:int) -> Tuple[bytes, bytes]:
    return buffer[size:], buffer[0:size]

# remainder, data_len, data
def pop_size_prefixed_buf_from_buf(buffer:bytes) -> Tuple[bytes, int, bytes]:
    data_len = buffer[0]
    return buffer[1+data_len:], data_len, buffer[1:data_len+1]
