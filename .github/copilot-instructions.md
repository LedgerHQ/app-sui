# Sui Ledger App Development Guide

## Project Overview
This is a Ledger hardware wallet application for the Sui blockchain, built using the [Alamgu](https://github.com/alamgu/) framework. The app is written in embedded Rust (`#![no_std]`) and compiles to native ARM code for Ledger devices (Nano S+, Nano X, Flex, Stax, Apex P).

## Architecture

### Core Structure
- **APDU Handler**: Entry point at [rust-app/src/handle_apdu.rs](rust-app/src/handle_apdu.rs) - implements async APDU command processing
- **Block Protocol**: Application-level protocol ([docs/block-protocol.md](docs/block-protocol.md)) that allows arbitrary-sized data transfers via chunking with SHA256 hash verification. Includes a `usize` length prefix (4 bytes on ARM32, 8 bytes on x86_64) before transaction data.
- **Parser**: Transaction parsing in [rust-app/src/parser/](rust-app/src/parser/) - handles Sui transaction types (transfers, staking, token operations) using ledger-parser-combinators
- **UI Layer**: Device-specific UIs - NBGL for Stax/Flex/Apex P ([rust-app/src/ui/nbgl.rs](rust-app/src/ui/nbgl.rs)), BAGL for Nano S+/X
- **Swap Integration**: Exchange support in [rust-app/src/swap/](rust-app/src/swap/) using Ledger's libcall API

### Device Targeting
Use conditional compilation extensively:
- `target_family = "bolos"` - all Ledger devices
- `target_os = "stax"/"flex"/"apex_p"` - NBGL-based devices (newer UI)
- `target_os = "nanosplus"/"nanox"` - BAGL-based devices (older UI)

Example pattern from [rust-app/src/lib.rs](rust-app/src/lib.rs):
```rust
#[cfg(any(target_os = "stax", target_os = "flex", target_os = "apex_p"))]
pub mod main_stax;
#[cfg(not(any(target_os = "stax", target_os = "flex", target_os = "apex_p")))]
pub mod main_nanos;
```

## Development Workflow

### Build System (Nix-based)
**Primary method**: Use Nix for all builds (required for reproducible builds and official releases)
```bash
# Enter device-specific dev shell
nix-shell -A nanosplus.rustShell
cd rust-app/
cargo build --release --target=$TARGET_JSON
```

Where `DEVICE` is `nanosplus`, `nanox`, `flex`, `stax`, or `apex_p` (not `nanos` - the original Nano S is unsupported).

**Quick iteration**: Use `cargo-ledger` (provided by nix-shell) for rapid dev cycles:
```bash
cargo-ledger ledger -l nanosplus  # Builds, creates hex, and loads to device
```

### Testing
Run all device tests via Speculos emulator:
```bash
./run-ragger-tests.sh  # Tests all devices
pytest ragger-tests --device nanosp  # Single device
```

Tests are in [ragger-tests/](ragger-tests/) using Python's Ragger framework. Test naming convention: `test_<feature>_<scenario>.py`.

**Swap tests**: Exchange integration tests are in [tests/swap/](tests/swap/). These treat the app as a library loaded alongside the Exchange app:
```bash
pytest -v --tb=short tests/swap/ --device flex --golden_run
```
Note: Swap tests set `MAIN_APP_DIR` in conftest.py, which configures the app to load as a library rather than standalone.

### Dependency Management
After updating `Cargo.lock`, run `./update-crate-hashes.sh` to regenerate `crate-hashes.json` - this provides supply-chain integrity for git dependencies in Nix builds.

### Loading to Device
```bash
# Using Nix (downloads/builds then loads)
nix --extra-experimental-features nix-command run -f . nanosplus.loadApp

# Device must be: plugged in, unlocked, on home screen
```

## Critical Patterns

### APDU Commands
All commands use `CLA=0x00`. Key instructions ([rust-app/src/interface.rs](rust-app/src/interface.rs)):
- `0x00` GetVersion
- `0x01` VerifyAddress (shows on device)
- `0x02` GetPubkey (no prompt)
- `0x03` Sign
- `0x22` ProvideTrustedDynamicDescriptor (for token metadata)
- `0xFF` Exit

### Address Derivation
BIP32 path: `m/44'/784'/account'/change'/index'`
Address = `0x` + Blake2b(0x00 || Ed25519_pubkey)[0:32] as hex

Implementation: [rust-app/src/interface.rs](rust-app/src/interface.rs) `SuiPubKeyAddress::get_address()`

### Transaction Format (BCS Encoding)
Sui transactions follow BCS (Binary Canonical Serialization) with this structure:
1. **Intent** (3 ULEB128 values): version, scope, app_id - typically 0x00, 0x00, 0x00
2. **TransactionData enum variant**: 0x00 for V1
3. **TransactionDataV1** tuple:
   - **TransactionKind enum variant**: 0x00 for ProgrammableTransaction
   - **ProgrammableTransaction**: inputs count (ULEB128), inputs array, commands count (ULEB128), commands array
   - **Sender**: 32-byte SUI address
   - **GasData**: payment objects, owner, price, budget
   - **Expiration**: enum (0x00=None, 0x01=Epoch)

**Critical**: When parsing via block protocol, skip the length prefix first. This is handled in [rust-app/src/implementation.rs](rust-app/src/implementation.rs#L126).

### Unstable Rust Features
Required nightly features ([rust-app/src/lib.rs](rust-app/src/lib.rs)):
- `stmt_expr_attributes`, `adt_const_params`, `type_alias_impl_trait`
- `cfg_version` for version-conditional compilation
- `custom_test_frameworks` for on-device testing

### Async Execution
Uses `alamgu-async-block` for cooperative async without a runtime. All APDU handlers return `impl Future` types defined with `#[define_opaque]`.

## Project Conventions

### File Organization
- Device entry points: `main_nanos.rs` vs `main_stax.rs`
- UI abstractions: [rust-app/src/ui.rs](rust-app/src/ui.rs) re-exports device-specific implementations
- Parser modules: `common.rs` (shared types), `tx.rs` (transaction), `object.rs`, `tuid.rs`
- Dual IO modules: `io_legacy` (default) vs `io_new` (opt-in via feature flag)

### Memory Constraints
Embedded environment with limited stack/heap:
- Use `ArrayVec` instead of `Vec`
- Profile builds use `opt-level = 3` and `lto = "fat"` even in dev mode
- Never allocate on heap in hot paths

### Error Handling
Use `ledger_device_sdk::io::StatusWords` for APDU errors. Panics call `exit_app(1)` due to `#[panic_handler]` in [rust-app/src/lib.rs](rust-app/src/lib.rs).

### Logging
Use `ledger-log` crate with feature flags:
- Default: no logging
- `--features speculos,ledger-log/log_info` - enable info-level logging in emulator
- `--features extra_debug` - trace-level logging

## External References
- [Alamgu framework](https://github.com/alamgu/)
- [Ledger device SDK](https://github.com/LedgerHQ/ledger-device-rust-sdk)
- [Speculos emulator](https://github.com/ledgerHQ/speculos)
- [APDU protocol reference](https://developers.ledger.com/docs/nano-app/application-structure/)
