# Migration Guide: GetPubkey APDU Command

## Prerequisites

⚠️ **You must complete [MIGRATION-GUIDE-BLOCK-PROTOCOL.md](MIGRATION-GUIDE-BLOCK-PROTOCOL.md) first!**

This guide assumes you have already implemented:
- `BlockProtocolHandler` - synchronous state machine
- `ChunkedReader` - incremental data reading
- `CommandContext` - stateful execution framework

## Overview

This guide demonstrates how to port the GetPubkey command to use your new synchronous block protocol implementation.

## ⚠️ Critical Constraint: Client Compatibility

**IMPORTANT**: Existing Sui wallet clients already use the **block protocol** to communicate with this app. We **cannot change the wire protocol** without breaking all existing clients.

**This means:**
- ✅ We must **keep the block protocol** on the wire (START, GET_CHUNK, etc.)
- ✅ The migration uses our reimplemented synchronous block protocol
- ❌ We cannot switch to simple single-APDU commands (that would break clients)
- ⚠️ For GetPubkey, the input is small (~20 bytes), so block protocol is overkill but required for compatibility

## Overview

**Command**: Get Public Key (INS=0x02)
- **Input**: BIP32 derivation path
- **Output**: Public key (32 bytes) + Address (32 bytes)
- **Optional**: Display address on screen for verification (VerifyAddress uses same logic with prompt=true)

---

## Current Implementation (Alamgu Pattern)

### File: `rust-app/src/implementation.rs`

```rust
pub async fn get_address_apdu(io: HostIO, ui: UserInterface, prompt: bool) {
    // 1. Get input parameters via block protocol
    let input = match io.get_params::<1>() {
        Some(v) => v,
        None => reject(SyscallError::InvalidParameter as u16).await,
    };

    // 2. Parse BIP32 path using async parser (ledger-parser-combinators)
    let path = BIP_PATH_PARSER.parse(&mut input[0].clone()).await;

    // 3. Validate path prefix (m/44'/784'/...)
    if !path.starts_with(&BIP32_PREFIX[0..2]) {
        reject::<()>(SyscallError::InvalidParameter as u16).await;
    }

    // 4. Derive key and address
    let mut rv = ArrayVec::<u8, 220>::new();

    if with_public_keys(&path, true, |key, address: &SuiPubKeyAddress| {
        try_option(|| -> Option<()> {
            // 5. Optional UI confirmation
            if prompt {
                ui.confirm_address(address)?;
            }

            // 6. Serialize public key
            let key_bytes = ed25519_public_key_bytes(key);
            rv.try_push(u8::try_from(key_bytes.len()).ok()?).ok()?;
            rv.try_extend_from_slice(key_bytes).ok()?;

            // 7. Serialize address
            let binary_address = address.get_binary_address();
            rv.try_push(u8::try_from(binary_address.len()).ok()?).ok()?;
            rv.try_extend_from_slice(binary_address).ok()?;
            Some(())
        }())
    })
    .is_err()
    {
        reject::<()>(StatusWords::UserCancelled as u16).await;
    }

    // 8. Send response via block protocol
    io.result_final(&rv).await;
}
```

### File: `rust-app/src/handle_apdu.rs`

```rust
pub fn handle_apdu_async(io: HostIO, ins: Ins, ...) -> APDUsFuture<'_> {
    async move {
        match ins {
            Ins::GetPubkey => {
                NoinlineFut(get_address_apdu(io, ui, false)).await;
            }
            Ins::VerifyAddress => {
                NoinlineFut(get_address_apdu(io, ui, true)).await;
            }
            // ...
        }
    }
}
```

### Key Dependencies (Alamgu-specific):
- `alamgu-async-block`: `HostIO`, `io.get_params()`, `io.result_final()`, `reject()`
- `ledger-parser-combinators`: `BIP_PATH_PARSER`, `AsyncParser`, `ByteStream`
- Async/await syntax throughout

---

## Target Implementation (Standard SDK Pattern)

### Step 1: Create Handler Function

**File**: `rust-app/src/handlers/get_public_key.rs` (new file)

**Note**: This is equivalent to `rust-app/src/commands/get_pubkey.rs` in MIGRATION-README.md's file structure. Use whichever path matches your project organization (handlers/ or commands/).

```rust
use crate::utils::Bip32Path;
use crate::interface::{SuiPubKeyAddress, BIP32_PREFIX};
use crate::ui::ui_display_address;
use crate::AppSW;

use arrayvec::ArrayVec;
use ledger_crypto_helpers::common::Address;
use ledger_crypto_helpers::eddsa::{ed25519_public_key_bytes, with_public_keys};
use ledger_device_sdk::io::Comm;

/// Handler for GetPubkey (INS=0x02) and VerifyAddress (INS=0x01)
pub fn handler_get_public_key(comm: &mut Comm, display: bool) -> Result<(), AppSW> {
    // 1. Get raw data from APDU
    let data = comm.get_data().map_err(|_| AppSW::WrongApduLength)?;
    
    // 2. Parse BIP32 path (synchronous)
    let path: Bip32Path = data.try_into()?;
    
    // 3. Validate path prefix
    if !path.as_ref().starts_with(&BIP32_PREFIX[0..2]) {
        return Err(AppSW::InvalidPath);
    }
    
    // 4. Derive key and address
    let mut response = ArrayVec::<u8, 220>::new();
    
    with_public_keys(
        path.as_ref(),
        true, // Ed25519
        |key, address: &SuiPubKeyAddress| -> Result<(), AppSW> {
            // 5. Optional UI confirmation
            if display {
                if !ui_display_address(address)? {
                    return Err(AppSW::Deny);
                }
            }
            
            // 6. Serialize public key
            let key_bytes = ed25519_public_key_bytes(key);
            response.try_push(key_bytes.len() as u8)
                .map_err(|_| AppSW::BufferOverflow)?;
            response.try_extend_from_slice(key_bytes)
                .map_err(|_| AppSW::BufferOverflow)?;
            
            // 7. Serialize address
            let binary_address = address.get_binary_address();
            response.try_push(binary_address.len() as u8)
                .map_err(|_| AppSW::BufferOverflow)?;
            response.try_extend_from_slice(binary_address)
                .map_err(|_| AppSW::BufferOverflow)?;
            
            Ok(())
        },
    ).map_err(|_| AppSW::KeyDeriveFail)??;
    
    // 8. Append response to comm buffer
    comm.append(&response);
    
    Ok(())
}
```

### Step 2: Create BIP32 Path Parser

**File**: `rust-app/src/utils.rs` (new file or add to existing)

```rust
use arrayvec::ArrayVec;
use crate::AppSW;

/// BIP32 path stored as an array of u32 indices
pub type Bip32Path = ArrayVec<u32, 10>;

/// Parse BIP32 path from APDU data
/// Format: [num_elements: 1 byte] [element1: 4 bytes LE] [element2: 4 bytes LE] ...
impl TryFrom<&[u8]> for Bip32Path {
    type Error = AppSW;
    
    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        // Check minimum length (at least 1 byte for count)
        if data.is_empty() {
            return Err(AppSW::WrongApduLength);
        }
        
        let num_elements = data[0] as usize;
        
        // Validate expected data length
        if data.len() != 1 + (num_elements * 4) {
            return Err(AppSW::WrongApduLength);
        }
        
        // Check maximum path length
        if num_elements > 10 {
            return Err(AppSW::InvalidPath);
        }
        
        // Parse path elements (little-endian)
        let mut path = ArrayVec::new();
        for i in 0..num_elements {
            let offset = 1 + (i * 4);
            let element = u32::from_le_bytes(
                data[offset..offset + 4]
                    .try_into()
                    .map_err(|_| AppSW::WrongApduLength)?
            );
            path.push(element);
        }
        
        Ok(path)
    }
}
```

### Step 3: Update Status Words

**File**: `rust-app/src/interface.rs` or create `rust-app/src/app_sw.rs`

```rust
use ledger_device_sdk::io::Reply;

/// Application status words
#[repr(u16)]
#[derive(Clone, Copy, PartialEq)]
pub enum AppSW {
    Deny = 0x6985,
    WrongP1P2 = 0x6A86,
    InvalidPath = 0x6A87,
    InsNotSupported = 0x6D00,
    ClaNotSupported = 0x6E00,
    WrongApduLength = 0x6700,
    BufferOverflow = 0xB001,
    KeyDeriveFail = 0xB002,
    DisplayFail = 0xB003,
    Ok = 0x9000,
}

impl From<AppSW> for Reply {
    fn from(sw: AppSW) -> Reply {
        Reply(sw as u16)
    }
}
```

### Step 4: Update Main Handler

**File**: `rust-app/src/main.rs` (simplified version)

```rust
use crate::handlers::get_public_key::handler_get_public_key;
use crate::interface::{Ins, AppSW};
use ledger_device_sdk::io::Comm;

extern "C" fn sample_main() {
    let mut comm = Comm::new().set_expected_cla(0x00);
    
    loop {
        let ins: Ins = comm.next_command();
        
        let status = match handle_apdu(&mut comm, &ins) {
            Ok(()) => {
                comm.reply_ok();
                AppSW::Ok
            }
            Err(sw) => {
                comm.reply(sw);
                sw
            }
        };
        
        // Show status screen if needed (NBGL devices)
        show_status_if_needed(&ins, status);
    }
}

fn handle_apdu(comm: &mut Comm, ins: &Ins) -> Result<(), AppSW> {
    match ins {
        Ins::GetPubkey => handler_get_public_key(comm, false),
        Ins::VerifyAddress => handler_get_public_key(comm, true),
        // ... other commands
        _ => Err(AppSW::InsNotSupported),
    }
}
```

### Step 5: Create UI Display Function

**File**: `rust-app/src/ui/address.rs` (new file)

```rust
use crate::interface::SuiPubKeyAddress;
use crate::AppSW;
use ledger_device_sdk::nbgl::{NbglAddressReview, NbglReviewStatus, StatusType};

/// Display address on device for user confirmation (NBGL)
#[cfg(any(target_os = "stax", target_os = "flex", target_os = "apex_p"))]
pub fn ui_display_address(address: &SuiPubKeyAddress) -> Result<bool, AppSW> {
    use alloc::format;
    
    let address_str = format!("{}", address); // Uses Display trait -> "0x..."
    
    let review = NbglAddressReview::new()
        .verify_str("Verify Sui Address")
        .addr_str(&address_str);
    
    let approved = review.show();
    
    Ok(approved)
}

/// Display address on device for user confirmation (BAGL - Nano S+/X)
#[cfg(not(any(target_os = "stax", target_os = "flex", target_os = "apex_p")))]
pub fn ui_display_address(address: &SuiPubKeyAddress) -> Result<bool, AppSW> {
    use ledger_device_sdk::ui::bitmaps::CROSSMARK;
    use ledger_device_sdk::ui::gadgets::{Field, MultiFieldReview};
    use alloc::format;
    
    let address_str = format!("{}", address);
    
    let fields = [
        Field {
            name: "Verify Address",
            value: &address_str,
        },
    ];
    
    let review = MultiFieldReview::new(
        &fields,
        &["Approve"],
        Some(&CROSSMARK),
    );
    
    Ok(review.show())
}
```

---

## Migration Checklist

### What Changes:

- [x] **Async → Sync**: Remove all `async`/`.await` keywords
- [x] **Block Protocol → Direct APDU**: Replace `io.get_params()` with `comm.get_data()`
- [x] **Parser Combinators → Manual Parsing**: Replace `BIP_PATH_PARSER.parse()` with `TryFrom<&[u8]>`
- [x] **Error Handling**: Replace `reject().await` with `return Err(AppSW::...)`
- [x] **Response**: Replace `io.result_final()` with `comm.append()`
- [x] **Future Storage**: Remove pinned `Option<Future>` state management
- [x] **HostIO**: Replace with `&mut Comm`

### What Stays the Same:

- ✓ **Cryptography**: `with_public_keys()`, `ed25519_public_key_bytes()` unchanged
- ✓ **Address Derivation**: `SuiPubKeyAddress::get_address()` logic identical
- ✓ **Path Validation**: `BIP32_PREFIX` check logic preserved
- ✓ **Output Format**: Same byte layout (length-prefixed pubkey + address)
- ✓ **Device Targeting**: `#[cfg(target_os = "...")]` patterns remain

---

## Testing Migration

### Unit Test (if using `--target x86_64-unknown-linux-gnu`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_bip32_path() {
        // Input: [5, 0x2C, 0x00, 0x00, 0x80, ...] 
        // = 5 elements, first is 0x8000002C (44' hardened)
        let data = [
            5,  // 5 elements
            0x2C, 0x00, 0x00, 0x80,  // 44' = 0x8000002C (LE)
            0x10, 0x03, 0x00, 0x80,  // 784' = 0x80000310 (LE)
            0x00, 0x00, 0x00, 0x80,  // 0'
            0x00, 0x00, 0x00, 0x00,  // 0
            0x00, 0x00, 0x00, 0x00,  // 0
        ];
        
        let path: Bip32Path = (&data[..]).try_into().unwrap();
        
        assert_eq!(path.len(), 5);
        assert_eq!(path[0], 0x8000002C);
        assert_eq!(path[1], 0x80000310);
    }
}
```

### Integration Test (Ragger):

```python
# ragger-tests/test_pubkey_cmd.py
def test_get_pubkey_standard_path(backend):
    client = SuiClient(backend)
    
    # Standard path: m/44'/784'/0'/0'/0'
    path = "m/44'/784'/0'/0'/0'"
    
    pubkey, address = client.get_public_key(path, display=False)
    
    assert len(pubkey) == 32
    assert len(address) == 32
    assert address.startswith(b'\x00')  # Sui addresses start with 0x00...
```

---

## Performance Comparison

| Metric | Alamgu (Async) | Standard SDK |
|--------|----------------|--------------|
| **APDUs for simple path** | 2-3 (START + GET_CHUNK) | 1 (single APDU) |
| **Code complexity** | High (futures, polling) | Low (linear flow) |
| **Max input size** | Unlimited (chunked) | ~240 bytes |
| **Memory usage** | Higher (future state) | Lower (stack only) |
| **Latency** | ~50-100ms extra | Minimal |

**Note**: For GetPubkey, the path is always <50 bytes, so chunking is unnecessary. Standard SDK is better here.

---

## Common Pitfalls

### 1. Endianness Confusion
**Alamgu**: Uses `U32<{ Endianness::Little }>` in parser
**Standard**: Must manually specify `u32::from_le_bytes()`

### 2. Error Propagation
**Alamgu**: `reject().await` never returns (diverges)
**Standard**: `return Err(...)` propagates to caller

### 3. ArrayVec vs Vec
**Alamgu**: Uses `ArrayVec` (stack allocated)
**Standard**: Can use either, but `ArrayVec` is safer for embedded

### 4. UI Differences
**NBGL**: `NbglAddressReview` - single call, returns bool
**BAGL**: `MultiFieldReview` - handles navigation internally

---

## Removing ledger-parser-combinators Dependency

The current implementation uses `ledger-parser-combinators` for BIP32 path parsing:

```rust
// Current (Alamgu)
pub type BipParserImplT = impl AsyncParser<Bip32Key, ByteStream, Output = ArrayVec<u32, 10>>;
pub const BIP_PATH_PARSER: BipParserImplT = SubInterp(DefaultInterp);

// In handler:
let path = BIP_PATH_PARSER.parse(&mut input[0].clone()).await;
```

**Replace with direct parsing** (see MIGRATION-GUIDE-BLOCK-PROTOCOL.md Step 6):

```rust
// New (Synchronous, no parser-combinators)
use crate::parser::bcs_sync::parse_bip32_path;

// In handler with ChunkedReader:
let path = parse_bip32_path(&mut reader)?;

// Or for simple APDU data:
pub fn parse_bip32_from_apdu(data: &[u8]) -> Result<ArrayVec<u32, 10>, Reply> {
    if data.is_empty() {
        return Err(Reply(0x6700));
    }
    
    let length = data[0] as usize;
    if data.len() != 1 + (length * 4) || length > 10 {
        return Err(Reply(0x6700));
    }
    
    let mut path = ArrayVec::new();
    for i in 0..length {
        let offset = 1 + (i * 4);
        let component = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
        path.push(component);
    }
    
    Ok(path)
}
```

**Benefits**:
- ✅ No async/await required
- ✅ No trait-based parser infrastructure
- ✅ Clear, direct byte manipulation
- ✅ Better for embedded environment
- ✅ One less Alamgu dependency

**Migration checklist for GetPubkey**:
- [ ] Implement `parse_bip32_path` in `parser/bcs_sync.rs`
- [ ] Replace `BIP_PATH_PARSER.parse().await` with sync parsing
- [ ] Test path parsing with various inputs
- [ ] Verify error handling matches original behavior
- [ ] Remove `ledger-parser-combinators` imports from GetPubkey handler

---

## Next Steps

After successfully migrating GetPubkey:
1. **Migrate GetVersion** (simpler, no crypto, no parsing)
2. **Migrate Sign** (complex, needs full transaction parser migration)
3. **Update tests** to verify equivalence
4. **Benchmark** to ensure no performance regression

The Sign command will be the most challenging due to:
- Large transaction data (requires block protocol)
- Complex BCS parsing (2000+ lines in parser/tx.rs to convert)
- Multiple transaction types (transfer, stake, etc.)
- Stateful UI (multiple confirmation screens)

See MIGRATION-GUIDE-SIGN.md for the full transaction parser migration strategy.

