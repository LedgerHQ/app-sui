from enum import IntEnum
from typing import Dict, List, Optional, Tuple
from hashlib import sha256
from struct import unpack

from ragger.backend.interface import BackendInterface, RAPDU
from bip_utils import Bip32Utils


MAX_APDU_LEN: int = 255

CLA: int = 0x00
P1: int = 0x00
P2: int = 0x00

class InsType(IntEnum):
    GET_VERSION    = 0x00
    GET_APP_NAME   = 0x00
    VERIFY_ADDRESS = 0x01
    GET_PUBLIC_KEY = 0x02
    SIGN_TX        = 0x03

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


def split_message(message: bytes, max_size: int) -> List[bytes]:
    return [message[x:x + max_size] for x in range(0, len(message), max_size)]


class Client:
    def __init__(self, backend: BackendInterface, use_block_protocol: bool=False) -> None:
        self.backend = backend
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


    def sign_tx(self, path: str, transaction: bytes) -> bytes:
        tx_len = (len(transaction)).to_bytes(4, byteorder='little')
        payload = [tx_len + transaction, pack_derivation_path(path)]
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
