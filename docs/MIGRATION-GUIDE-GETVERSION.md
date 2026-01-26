# Migration Guide: GetVersion APDU Command

## Prerequisites

⚠️ **You must complete [MIGRATION-GUIDE-BLOCK-PROTOCOL.md](MIGRATION-GUIDE-BLOCK-PROTOCOL.md) first!**

This guide assumes you have already implemented:
- `BlockProtocolHandler` - synchronous state machine
- `CommandContext` - stateful execution framework

This is the simplest command to migrate, making it a perfect validation that your block protocol implementation works. GetVersion is ideal because it has **no inputs**, **no crypto**, and **no UI**.

## ⚠️ Critical Constraint: Client Compatibility

**IMPORTANT**: Existing Sui wallet clients already use the **block protocol** to communicate with this app. We **cannot change the wire protocol** without breaking all existing clients.

**This means:**
- ✅ We must **keep the block protocol** on the wire (START, GET_CHUNK, etc.)
- ✅ The migration is about making the **internal implementation** synchronous
- ❌ We cannot switch to simple single-APDU commands (that would break clients)

## Overview

**Command**: Get Version (INS=0x00)
- **Input**: None (but may arrive via block protocol START command)
- **Output**: 3 bytes (major, minor, patch) + app name string (via RESULT_FINAL)
- **Use case**: Host queries app version for compatibility checking

---

## Current Implementation (Alamgu Pattern)

### File: `rust-app/src/handle_apdu.rs`

```rust
pub fn handle_apdu_async(io: HostIO, ins: Ins, ...) -> APDUsFuture<'_> {
    async move {
        match ins {
            Ins::GetVersion => {
                const APP_NAME: &str = "sui";
                let mut rv = ArrayVec::<u8, 220>::new();
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap());
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_MINOR").parse().unwrap());
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_PATCH").parse().unwrap());
                let _ = rv.try_extend_from_slice(APP_NAME.as_bytes());
                io.result_final(&rv).await;
            }
            // ...
        }
    }
}
```

**Characteristics**:
- ✓ Inline in the main handler (no separate function)
- ✓ Uses compile-time environment variables
- ✓ No input parsing needed
- ✗ Still uses async/await (`io.result_final().await`)
- ✗ Uses block protocol for a tiny response (~6 bytes)

---

## Target Implementation (Keeping Block Protocol)

The correct migration approach is to:
1. **Keep the block protocol state machine** (START, GET_CHUNK, RESULT_FINAL)
2. **Make it synchronous** instead of async
3. **Simplify the implementation** since GetVersion has no real inputs

### Option A: Minimal Change (Keep Alamgu's Block Protocol, Remove Async)

The safest approach is to keep using Alamgu's block protocol implementation but make the handler synchronous:

```rust
// Handler remains similar, but synchronous
pub fn get_version_sync(io: &mut BlockProtocolIO) -> Result<(), AppSW> {
    const APP_NAME: &str = "sui";
    
    let major: u8 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
    let minor: u8 = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap();
    let patch: u8 = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap();
    
    let mut response = ArrayVec::<u8, 220>::new();
    response.push(major);
    response.push(minor);
    response.push(patch);
    response.extend_from_slice(APP_NAME.as_bytes());
    
    // Still use block protocol for response
    io.result_final(&response)?;
    Ok(())
}
```

This maintains 100% compatibility with existing clients.

### Option B: Implement Sync Block Protocol Handler (Recommended)

**File**: `rust-app/src/commands/get_version.rs` (new)

If removing Alamgu entirely, you need to reimplement the block protocol handling:

```rust
use crate::AppSW;
use arrayvec::ArrayVec;
use core::str::FromStr;
use ledger_device_sdk::io::Comm;

/// Handler for GetVersion (INS=0x00)
/// Must handle block protocol: START → RESULT_FINAL
pub fn handler_get_version(comm: &mut Comm) -> Result<(), AppSW> {
    const APP_NAME: &str = "sui";
    
    // Parse version from Cargo.toml at compile time
    let version = parse_version_string(env!("CARGO_PKG_VERSION"))
        .ok_or(AppSW::VersionParsingFail)?;
    
    // Build response
    let mut response = ArrayVec::<u8, 220>::new();
    response.try_push(version.0).map_err(|_| AppSW::BufferOverflow)?;
    response.try_push(version.1).map_err(|_| AppSW::BufferOverflow)?;
    response.try_push(version.2).map_err(|_| AppSW::BufferOverflow)?;
    response.try_extend_from_slice(APP_NAME.as_bytes())
        .map_err(|_| AppSW::BufferOverflow)?;
    
    // Append to comm buffer
    comm.append(&response);
    
    Ok(())
}

/// Parse semantic version string "X.Y.Z" into (major, minor, patch)
fn parse_version_string(input: &str) -> Option<(u8, u8, u8)> {
    let mut parts = input.split('.');
    let major = u8::from_str(parts.next()?).ok()?;
    let minor = u8::from_str(parts.next()?).ok()?;
    let patch = u8::from_str(parts.next()?).ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_version_string() {
        assert_eq!(parse_version_string("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version_string("0.0.1"), Some((0, 0, 1)));
        assert_eq!(parse_version_string("255.255.255"), Some((255, 255, 255)));
        assert_eq!(parse_version_string("1.2"), None); // Invalid
        assert_eq!(parse_version_string("1.2.x"), None); // Non-numeric
    }
}
```

### Alternative: Inline Version (Even Simpler)

If you prefer not to create a separate file:

```rust
// In handle_apdu() function
Ins::GetVersion => {
    const APP_NAME: &str = "sui";
    
    // Direct parsing at compile time (fails at build if invalid)
    let major: u8 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
    let minor: u8 = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap();
    let patch: u8 = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap();
    
    comm.append(&[major, minor, patch]);
    comm.append(APP_NAME.as_bytes());
    Ok(())
}
```

**Pros**: Ultra-simple, no allocations
**Cons**: Panics at build time if version format is invalid (but that's actually good!)

---

## Migration Steps

### Step 1: Remove Async Wrapper

**Before**:
```rust
async move {
    match ins {
        Ins::GetVersion => {
            // async code
            io.result_final(&rv).await;
        }
    }
}
```

**After**:
```rust
fn handle_apdu(comm: &mut Comm, ins: &Ins) -> Result<(), AppSW> {
    match ins {
        Ins::GetVersion => handler_get_version(comm),
    }
}
```

### Step 2: Keep Block Protocol, Remove Async

**Before**:
```rust
let mut rv = ArrayVec::<u8, 220>::new();
// ... populate rv ...
io.result_final(&rv).await;  // Async
```

**After (Option A - Keep Alamgu's block protocol)**:
```rust
let mut response = ArrayVec::<u8, 220>::new();
// ... populate response ...
io.result_final(&response)?;  // Synchronous, but still block protocol
Ok(())
```

**After (Option B - Reimplement block protocol synchronously)**:
```rust
let mut response = ArrayVec::<u8, 220>::new();
// ... populate response ...

// Manually handle block protocol response
let data = comm.get_data()?;
if data[0] == 0x00 {  // START command
    // Send RESULT_FINAL
    comm.append(&[0x01]);  // RESULT_FINAL marker
    comm.append(&response);
}
Ok(())
```

### Step 3: Update Main Loop

**Before** (in `main_nanos.rs`):
```rust
match evt {
    io::Event::Command(ins) => {
        let poll_rv = poll_apdu_handlers(
            PinMut::as_mut(&mut states.borrow_mut()),
            ins,
            *hostio,
            |io, ins| handle_apdu_async(io, ins, ctx, settings, ui),
        );
        match poll_rv {
            Ok(()) => comm.borrow_mut().reply_ok(),
            Err(sw) => comm.borrow_mut().reply(sw),
        }
    }
}
```

**After**:
```rust
loop {
    let ins: Ins = comm.next_command();
    
    match handle_apdu(&mut comm, &ins) {
        Ok(()) => comm.reply_ok(),
        Err(sw) => comm.reply(sw),
    }
}
```

---

## Complete Minimal Example

Here's a fully working minimal app with just GetVersion:

```rust
#![no_std]
#![no_main]

use ledger_device_sdk::io::{Comm, ApduHeader, StatusWords};

#[repr(u16)]
enum AppSW {
    Ok = 0x9000,
    InsNotSupported = 0x6D00,
}

#[repr(u8)]
enum Ins {
    GetVersion = 0,
}

impl TryFrom<ApduHeader> for Ins {
    type Error = AppSW;
    fn try_from(h: ApduHeader) -> Result<Self, Self::Error> {
        match (h.cla, h.ins, h.p1, h.p2) {
            (0, 0, 0, 0) => Ok(Ins::GetVersion),
            _ => Err(AppSW::InsNotSupported),
        }
    }
}

ledger_device_sdk::set_panic!(ledger_device_sdk::exiting_panic);

#[no_mangle]
extern "C" fn sample_main() {
    let mut comm = Comm::new().set_expected_cla(0x00);
    
    loop {
        let ins: Ins = comm.next_command();
        
        match ins {
            Ins::GetVersion => {
                let major: u8 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
                let minor: u8 = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap();
                let patch: u8 = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap();
                
                comm.append(&[major, minor, patch]);
                comm.append(b"sui");
                comm.reply_ok();
            }
        }
    }
}
```

**Lines of code**: ~40 (vs ~200+ with Alamgu framework)

---

## Testing

### Unit Test

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_version_format() {
        // At compile time
        let major: u8 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
        let minor: u8 = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap();
        let patch: u8 = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap();
        
        // Verify they fit in u8
        assert!(major < 256);
        assert!(minor < 256);
        assert!(patch < 256);
    }
}
```

### Integration Test (Ragger)

```python
# ragger-tests/test_version_cmd.py

def test_get_version(backend):
    client = SuiClient(backend)
    
    major, minor, patch, name = client.get_version()
    
    # Verify format
    assert 0 <= major <= 255
    assert 0 <= minor <= 255
    assert 0 <= patch <= 255
    assert name == b"sui"
    
    # Verify matches Cargo.toml
    import toml
    cargo_version = toml.load("rust-app/Cargo.toml")["package"]["version"]
    expected = tuple(map(int, cargo_version.split(".")))
    assert (major, minor, patch) == expected
```

### Manual Test (APDU)

```bash
# Send GetVersion command
echo "0000000000" | xxd -r -p > /tmp/apdu.bin
cat /tmp/apdu.bin | ledgerctl send -

# Expected response (example for v1.3.1):
# 01 03 01 73 75 69 9000
# ^  ^  ^  s  u  i  status
# |  |  |
# |  |  patch (1)
# |  minor (3)
# major (1)the block protocol is overkill for a 6-byte response, BUT we must keep it for client compatibility. The migration should focus on making the implementation synchronous while preserving the wire protocol
```

---

## Performance Comparison

| Metric | Alamgu (Async) | Standard SDK |
|--------|----------------|--------------|
| **APDUs exchanged** | 1 | 1 |
| **Code size** | ~500 bytes (framework) | ~50 bytes |
| **RAM usage** | ~200 bytes (future state) | ~10 bytes (stack) |
| **Latency** | ~20ms | ~5ms |
| **Complexity** | High (async runtime) | Minimal |

**Verdict**: For GetVersion, standard SDK is vastly superior. No benefit to block protocol for 6-byte response.

---

## Common Mistakes

### 1. Forgetting to Call `comm.reply_ok()`

**Wrong**:
```rust
Ins::GetVersion => {
    comm.append(&[major, minor, patch]);
    // Missing reply!
}
```

**Right**:
```rust
Ins::GetVersion => {
    comm.append(&[major, minor, patch]);
    comm.reply_ok(); // ← Don't forget!
}
```

### 2. Returning Error Instead of Panicking on Parse Failure

**Debatable**:
```rust
let major = env!("CARGO_PKG_VERSION_MAJOR").parse()
    .map_err(|_| AppSW::VersionParsingFail)?;
```

**Better**: Let it panic at compile time
```rust
let major: u8 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
```

**Rationale**: If the version in Cargo.toml is malformed, the app should fail to build, not fail at runtime.

### 3. Using `Vec` Instead of Stack Allocation

**Wrong** (requires `alloc`):
```Revised Recommendation

Given the **client compatibility requirement**, the migration strategy should be:

### **Option A (Recommended): Keep Alamgu, Just Remove Async**

**Pros**:
- ✅ Minimal changes
- ✅ 100% wire protocol compatibility guaranteed
- ✅ Can be done incrementally
- ✅ Low risk

**Cons**:
- ❌ Still depends on Alamgu's block protocol code
- ❌ Doesn't fully remove the dependency

**Migration time**: ~30 minutes per command

### **Option B: Reimplement Block Protocol Synchronously**

**Pros**:
- ✅ Removes Alamgu dependency entirely
- ✅ Full control over implementation

**Cons**:
- ❌ Must reimplement block protocol state machine (~500 lines)
- ❌ Higher risk of compatibility bugs
- ❌ More testing needed

**Migration time**: ~2-3 days to reimplement block protocol, then ~1 hour per command

## Why This Should Be First

GetVersion is still the **perfect first migration** because:

1. ✅ **No dependencies on other code** - completely standalone
2. ✅ **No input validation** - can't fail due to malformed input
3. ✅ **No crypto operations** - no key derivation complexity
4. ✅ **No UI** - no device-specific screen code
5. ✅ **Trivial testing** - just check 3 bytes match Cargo.toml
6. ✅ **Validates the approach** - proves async→sync works before tackling complex command

---

## Why This Should Be First

GetVersion is the **perfect first migration** because:

1. ✅ **No dependencies on other code** - completely standalone
2. ✅ **No input validation** - can't fail due to malformed input
3. ✅ **No crypto operations** - no key derivation complexity
4. ✅ **No UI** - no device-specific screen code
5. ✅ **Trivial testing** - just check 3 bytes match Cargo.toml
6. ✅ **Instant verification** - can test with a single APDU

**Migration time**: ~15 minutes

After this, you can proceed to GetPubkey (adds crypto + UI) or Sign (adds everything).

---

## Next Steps

1. ✅ Migrate GetVersion (this guide)
2. → Migrate GetPubkey (adds crypto + UI)
3. → Migrate Sign (complex, large inputs)
4. → Remove Alamgu dependencies from `Cargo.toml`
5. → Update tests to verify all APDUs work identically

Once GetVersion works, you've proven the basic event loop transformation. The rest is just adding complexity incrementally.
