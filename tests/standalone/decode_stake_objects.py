#!/usr/bin/env python3
"""
Decode object_list from test_sign_stake_gas_coin in test_sign_sui_stake_1.py.
Extracts ObjectRef (id, version, digest) for each gas coin and decodes the validator address.
"""

import base64
import struct

# Payment array from transaction (index -> objectId)
PAYMENT_ORDER = [
    "0xebff16b4d2081ab06d1d5251c988208641e5c501c7fa8bdce9c8b7b0908ba76b",  # [0]
    "0xa93f6c464f8fb8b98fb3d02112902060c8f85ea4d71cfc7777dfdbd75e68ab6d",  # [1]
    "0x1f876ff0144386dcf4e886c5de53b326c718cc1221e1ccea71ef8aa6231a40ea",  # [2]
    "0x1c12be5429384d00eeef61242f3aebabeac3012549dd6f888dc1087c4d00da80",  # [3]
]

# Expected ObjectRefs from transaction comment
EXPECTED_REFS = {
    "0xebff16b4d2081ab06d1d5251c988208641e5c501c7fa8bdce9c8b7b0908ba76b": (289568469, "53CbPjHczNtV9Kids6JdGt9bkPbSeJ34dc9TX2W2g6tT"),
    "0xa93f6c464f8fb8b98fb3d02112902060c8f85ea4d71cfc7777dfdbd75e68ab6d": (289568468, "Cbin2kMMWzjtPER7GZ7ne81Dhpk2tS31MwinvTwjMEZi"),
    "0x1f876ff0144386dcf4e886c5de53b326c718cc1221e1ccea71ef8aa6231a40ea": (289568467, "3GkMekAY5KQqiop61rRCnQjK57ztStksBSuZsUPf62JM"),
    "0x1c12be5429384d00eeef61242f3aebabeac3012549dd6f888dc1087c4d00da80": (289568466, "G9KngE3q7fpBfZtrmoEFdjZC4Ebb4TR7mZ1NYpf2xqaJ"),
}


def read_uleb128(data: bytes, offset: int) -> tuple[int, int]:
    """Read ULEB128-encoded integer, return (value, new_offset)."""
    result = 0
    shift = 0
    while offset < len(data):
        byte = data[offset]
        result |= (byte & 0x7F) << shift
        offset += 1
        if (byte & 0x80) == 0:
            break
        shift += 7
    return result, offset


def parse_sui_object_bcs(bcs_bytes: bytes) -> dict | None:
    """
    Parse BCS-encoded Sui Object (ObjectData::Move with GasCoin).
    Structure: ObjectData enum (0=Move) | MoveObjectType (1=GasCoin) | has_public_transfer | version (u64) | contents (ULEB128 len + bytes)
    Contents for Coin: ObjectID (32 bytes) + balance (u64)
    """
    if len(bcs_bytes) < 12:
        return None
    offset = 0
    obj_data_variant = bcs_bytes[offset]
    offset += 1
    if obj_data_variant != 0:  # Move
        return None
    move_type = bcs_bytes[offset]
    offset += 1
    if move_type != 1:  # GasCoin
        return None
    has_public_transfer = bcs_bytes[offset]
    offset += 1
    if offset + 8 > len(bcs_bytes):
        return None
    version = struct.unpack_from("<Q", bcs_bytes, offset)[0]
    offset += 8
    contents_len, offset = read_uleb128(bcs_bytes, offset)
    if offset + contents_len > len(bcs_bytes):
        return None
    contents = bcs_bytes[offset : offset + contents_len]
    offset += contents_len
    if contents_len < 40:  # ObjectID (32) + balance (8)
        return None
    object_id_bytes = contents[:32]
    object_id = "0x" + object_id_bytes.hex()
    balance = struct.unpack_from("<Q", contents, 32)[0]
    return {
        "object_id": object_id,
        "version": version,
        "balance": balance,
        "remaining_bytes": len(bcs_bytes) - offset,
    }


def decode_validator_address(base64_pure: str) -> str:
    """Decode validator address from Pure bytes (base64). Sui addresses are 32 bytes."""
    raw = base64.b64decode(base64_pure)
    if len(raw) != 32:
        return f"Invalid length {len(raw)} (expected 32)"
    return "0x" + raw.hex()


def main():
    object_list_b64 = [
        "AAEB0nZCEQAAAAAoHBK+VCk4TQDu72EkLzrrq+rDASVJ3W+IjcEIfE0A2oCAlpgAAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAdWxm/zBGpPolm35Bn6wJKCXKBWKegYpW9ZT1L4YEUXWATDwAAAAAA",
        "AAEB03ZCEQAAAAAoH4dv8BRDhtz06IbF3lOzJscYzBIh4czqce+KpiMaQOoALTEBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2ATDwAAAAAA",
        "AAEB1HZCEQAAAAAoqT9sRk+PuLmPs9AhEpAgYMj4XqTXHPx3d9/b115oq22Aw8kBAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAfVAIamErRVJt4BuqoZFY2dBaAKAaQzrxvVjuLcgrqZmATDwAAAAAA",
        "AAEB1XZCEQAAAAAo6/8WtNIIGrBtHVJRyYgghkHlxQHH+ovc6ci3sJCLp2tAnHECAAAAAAAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiCAuq6BxxXPwIbLsDoXWJN6/Emi0EtUzGJnln5pJL4iDYWATDwAAAAAA",
    ]

    print("=" * 70)
    print("ObjectRef (id, version, digest) for each gas coin in object_list")
    print("=" * 70)

    for i, b64 in enumerate(object_list_b64):
        bcs = base64.b64decode(b64)
        parsed = parse_sui_object_bcs(bcs)
        if parsed:
            obj_id = parsed["object_id"]
            version = parsed["version"]
            balance = parsed["balance"]
            digest = EXPECTED_REFS.get(obj_id, (None, "?"))[1]
            id_suffix = obj_id[2:10] if obj_id.startswith("0x") else obj_id[:8]  # first 8 hex chars
            print(f"\nObject {i + 1} (payment suffix ...{id_suffix}):")
            print(f"  objectId:  {obj_id}")
            print(f"  version:   {version}")
            print(f"  digest:    {digest}")
            print(f"  balance:   {balance} MIST ({balance / 1e9:.6f} SUI)")
            print(f"  ObjectRef: ({obj_id}, {version}, {digest})")
        else:
            print(f"\nObject {i + 1}: Failed to parse")

    print("\n" + "=" * 70)
    print("Match to payment array (ebff16b4, a93f6c46, 1f876ff0, 1c12be54)")
    print("=" * 70)
    print("  Payment order in tx: [0]ebff16b4 [1]a93f6c46 [2]1f876ff0 [3]1c12be54")
    print("  object_list order:   by version (ascending)")
    for i, b64 in enumerate(object_list_b64):
        bcs = base64.b64decode(b64)
        parsed = parse_sui_object_bcs(bcs)
        if parsed:
            obj_id = parsed["object_id"]
            suffix = obj_id[2:10] if obj_id.startswith("0x") else obj_id[:8]
            payment_idx = PAYMENT_ORDER.index(obj_id) if obj_id in PAYMENT_ORDER else "?"
            print(f"  object_list[{i}] -> ...{suffix} -> payment[{payment_idx}] -> {obj_id}")

    print("\n" + "=" * 70)
    print("Validator address from Pure bytes")
    print("=" * 70)
    validator_b64 = "NfXxVPARdGTjN5xFx/PLay1O/t8wCsrf+Kfo6eOhUQk="
    validator_addr = decode_validator_address(validator_b64)
    print(f"  Pure bytes (base64): {validator_b64}")
    print(f"  Decoded address:    {validator_addr}")


if __name__ == "__main__":
    main()
