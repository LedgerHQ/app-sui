# Sui Ledger App: Complete Migration from Alamgu

This directory contains a complete guide for migrating the Sui Ledger app from the Alamgu framework to a standalone implementation with **zero Alamgu dependencies**.

## Migration Goals

✅ **Complete removal** of `alamgu_async_block` dependency  
✅ **Complete removal** of `ledger-parser-combinators` dependency  
✅ **Synchronous implementation** (no async/await)  
✅ **100% client compatibility** (preserve block protocol on the wire)  
✅ **Cleaner architecture** with full control over security-critical code  

## Reading Order

Follow these guides in order:

### 1. Strategy & Overview
📄 **[MIGRATION-STRATEGY.md](MIGRATION-STRATEGY.md)** - Read this first!
- Why we're doing this
- What changes and what stays
- Timeline and phases
- Risk assessment
- Technical decisions

### 2. Core Implementation
📄 **[MIGRATION-GUIDE-BLOCK-PROTOCOL.md](MIGRATION-GUIDE-BLOCK-PROTOCOL.md)** - Implement this first!
- Complete reimplementation of block protocol
- Synchronous state machine
- ~500 lines of code
- Week 1 deliverable

**⚠️ All other guides depend on completing this one first!**

### 3. Command Migrations

Once the block protocol is working, port commands:

📄 **[MIGRATION-GUIDE-GETVERSION.md](MIGRATION-GUIDE-GETVERSION.md)** - Simplest command
- No inputs, no crypto, no UI
- Perfect for validation
- 1 day

📄 **[MIGRATION-GUIDE-GETPUBKEY.md](MIGRATION-GUIDE-GETPUBKEY.md)** - Medium complexity  
- Small inputs via block protocol
- Crypto operations (Ed25519, Blake2b)
- Device UI integration
- 2-3 days

📄 **[MIGRATION-GUIDE-SIGN.md](MIGRATION-GUIDE-SIGN.md)** - Most complex
- Large inputs via chunked reader
- 2000+ line transaction parser
- Complex UI flows
- Swap mode integration
- 1-2 weeks

## Timeline Summary

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| **Phase 1**: Block Protocol + BCS Utils | 1 week | Synchronous `BlockProtocolHandler` + BCS parsing utilities |
| **Phase 2**: Simple Commands | 1 week | GetVersion + GetPubkey working (no parser-combinators) |
| **Phase 3**: Sign Command | 1-2 weeks | Full transaction parsing + all commands migrated |
| **Phase 4**: Cleanup | 3-5 days | Remove both Alamgu dependencies from Cargo.toml |
| **Total** | **5-6 weeks** | Production-ready app |

## Architecture Changes

### Before (Alamgu)
```
┌─────────────────────────────────────┐
│  alamgu_async_block (external dep)  │
│  - HostIO / HostIOState             │
│  - poll_apdu_handlers()             │
│  - ByteStream (async)               │
│  - Future management                │
├─────────────────────────────────────┤
│  ledger-parser-combinators          │
│  - AsyncParser trait                │
│  - BCS parsing (async)              │
│  - 2000+ lines in parser/tx.rs      │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│  APDU Handlers (async)              │
│  - handle_apdu_async()              │
│  - get_address_apdu() async         │
│  - sign_apdu() async                │
└─────────────────────────────────────┘
```

### After (Standalone)
```
┌─────────────────────────────────────┐
│  Block Protocol (owned)             │
│  - BlockProtocolHandler             │
│  - ChunkedReader                    │
│  - CommandContext                   │
│  - Synchronous state machine        │
├─────────────────────────────────────┤
│  BCS Parsing (owned)                │
│  - parse_uleb128(), read_u64_le()   │
│  - parse_bip32_path()               │
│  - parse_transaction() - sync       │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│  APDU Handlers (sync)               │
│  - handle_apdu()                    │
│  - get_address_cmd()                │
│  - sign_cmd()                       │
└─────────────────────────────────────┘
```

## Key Files to Create

Based on guides, you will create **9 new files**:

```
rust-app/src/
  block_protocol/                  [Defined in: MIGRATION-GUIDE-BLOCK-PROTOCOL.md]
    mod.rs                         # BlockProtocolHandler, state machine, all 9 commands
    reader.rs                      # ChunkedReader for incremental data reading
    context.rs                     # CommandContext for stateful execution coordinator
  parser/                          [Defined in: Multiple guides]
    bcs_sync.rs                    # Synchronous BCS parsing utilities (ULEB128, arrays, etc.)
                                   # [Defined in: MIGRATION-GUIDE-BLOCK-PROTOCOL.md Step 6]
    bip32.rs                       # BIP32 path parsing (replaces parser-combinators)
                                   # [Defined in: MIGRATION-GUIDE-BLOCK-PROTOCOL.md Step 6]
    tx_sync.rs                     # Synchronous transaction parser (replaces async tx.rs)
                                   # [Defined in: MIGRATION-GUIDE-SIGN.md Step 7]
  commands/ (or handlers/)         [Note: Guides use handlers/ path, equivalent to commands/]
    get_version.rs                 # Migrated GetVersion command
                                   # [Defined in: MIGRATION-GUIDE-GETVERSION.md]
    get_pubkey.rs                  # Migrated GetPubkey/VerifyAddress commands
                                   # [Defined in: MIGRATION-GUIDE-GETPUBKEY.md as handlers/get_public_key.rs]
    sign.rs                        # Migrated Sign command
                                   # [Defined in: MIGRATION-GUIDE-SIGN.md as handlers/sign_tx.rs]
```

**Note on file paths**: The guides use `handlers/` directory (e.g., `handlers/get_public_key.rs`), which is equivalent to the `commands/` structure shown above. Choose whichever organization fits your project better - they serve the same purpose.

## Testing Strategy

Each phase includes testing:

1. **Unit tests** - State transitions, hash verification
2. **Integration tests** - Full APDU exchanges with Speculos
3. **Client tests** - Real Sui Wallet compatibility
4. **All devices** - Nano S+, X, Stax, Flex, Apex P

## Success Criteria

✅ All existing Ragger tests pass  
✅ Sui Wallet can sign transactions  
✅ Swap mode works  
✅ No performance regression  
✅ `alamgu_async_block` removed from `Cargo.toml`  
✅ `ledger-parser-combinators` removed from `Cargo.toml`  
✅ Code is clearer and easier to maintain  

## Security Considerations

⚠️ **Critical**: The block protocol includes SHA256 verification to prevent data tampering.

When reimplementing:
- ✅ Verify every chunk hash before processing
- ✅ Reject mismatched hashes immediately
- ✅ Test tampering scenarios
- ✅ Code review by security experts

## Questions?
 
- **Q**: Why removing both `alamgu_async_block` AND `ledger-parser-combinators`?  
  **A**: Complete independence - both are async-based Alamgu libraries. Replacing them gives you full control and simpler synchronous code.

- **Q**: Can I keep `ledger-parser-combinators` and only remove `alamgu_async_block`?  
  **A**: No - `ledger-parser-combinators` requires `ByteStream` from `alamgu_async_block` and all parsers are async.

- **Q**: Is writing a BCS parser from scratch hard?  
  **A**: No - BCS is simple: ULEB128 for lengths, little-endian integers, and nested structures. See MIGRATION-GUIDE-BLOCK-PROTOCOL.md Step 6 for utilities.

- **Q**: Is this safe to do?  
  **A**: Yes, with careful implementation and testing. Start with blind signing, add parsers incrementally, test extensively
  **A**: Yes, with careful implementation and testing. The block protocol is well-specified.

- **Q**: How long will this take?  
  **A**: 2-3 weeks with one experienced developer

- **Q**: Can I do this incrementally?  
  **A**: Yes! Test each phase before proceeding.

## Additional Resources

- [docs/block-protocol.md](block-protocol.md) - Wire protocol specification
- [docs/apdu.md](apdu.md) - APDU command reference
- Alamgu source code - Reference for state transitions

---

**Ready to start?** → Begin with [MIGRATION-STRATEGY.md](MIGRATION-STRATEGY.md)
