import struct, base64, requests
import base58  # pip install base58

def uleb128(n: int) -> bytes:
    result = []
    while True:
        b = n & 0x7F; n >>= 7
        if n: b |= 0x80
        result.append(b)
        if not n: break
    return bytes(result)

def bcs_vec(data: bytes) -> bytes:
    return uleb128(len(data)) + data

def bcs_str(s: str) -> bytes:
    enc = s.encode()
    return uleb128(len(enc)) + enc

def encode_struct_tag(addr_hex: str, module: str, name: str) -> bytes:
    addr = bytes.fromhex(addr_hex.lstrip('0x').zfill(64))
    return addr + bcs_str(module) + bcs_str(name) + b'\x00'  # empty type_params

def encode_digest(base58_str: str) -> bytes:
    raw = base58.b58decode(base58_str)
    assert len(raw) == 32
    return b'\x20' + raw  # BCS Vec<u8>: ULEB128(32) + 32 bytes

def build_obj(rpc_url: str, object_id: str) -> bytes:
    resp = requests.post(rpc_url, json={
        "jsonrpc": "2.0", "id": 1,
        "method": "sui_getObject",
        "params": [object_id, {
            "showBcs": True, "showOwner": True,
            "showPreviousTransaction": True, "showStorageRebate": True
        }]
    }).json()["result"]["data"]

    bcs_data  = resp["bcs"]
    owner     = resp["owner"]
    bcs_bytes = base64.b64decode(bcs_data["bcsBytes"])
    full_type = bcs_data["type"]

    # ObjectDataSchema = 0 (Move)
    out = b'\x00'

    # MoveObjectType
    if full_type == "0x2::coin::Coin<0x2::sui::SUI>":
        out += uleb128(1)   # GasCoin
    elif "<" in full_type:
        inner = full_type[full_type.index("<")+1 : full_type.rindex(">")]
        ia, im, iname = inner.split("::", 2)
        out += uleb128(3)   # Coin(TypeTag)
        out += b'\x07'      # TypeTag::Struct
        out += encode_struct_tag(ia, im, iname)

    out += b'\x01' if bcs_data["hasPublicTransfer"] else b'\x00'
    out += struct.pack('<Q', int(bcs_data["version"]))
    out += bcs_vec(bcs_bytes)

    # OwnerSchema
    if "AddressOwner" in owner:
        out += uleb128(0)
        out += bytes.fromhex(owner["AddressOwner"].lstrip('0x').zfill(64))
    elif "ObjectOwner" in owner:
        out += uleb128(1)
        out += bytes.fromhex(owner["ObjectOwner"].lstrip('0x').zfill(64))
    elif "Shared" in owner:
        out += uleb128(2)
        out += struct.pack('<Q', owner["Shared"]["initial_shared_version"])
    else:   # Immutable
        out += uleb128(3)

    # TransactionDigest: BCS Vec<u8> = 0x20 + 32 bytes
    out += encode_digest(resp["previousTransaction"])

    # StorageRebate: u64 LE
    out += struct.pack('<Q', int(resp["storageRebate"]))

    return out

obj_bytes = build_obj("https://fullnode.mainnet.sui.io", "0xb4df46533386cd95118ce435e315cbb05de02ebd188f7557ecc289b36ce560d9")
print(base64.b64encode(obj_bytes).decode())

