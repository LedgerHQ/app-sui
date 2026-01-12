# Sui Ledger App: Migration Strategy from Alamgu

## Executive Summary

**Goal**: Completely remove `alamgu_async_block` and `ledger-parser-combinators` dependencies

**Critical Constraint**: Must maintain 100% wire protocol compatibility with existing clients

**Recommended Approach**: Reimplement block protocol as synchronous state machine

**Estimated Time**: 5-6 weeks

---

## The Client Compatibility Problem

### Current Reality

Existing Sui wallet clients (Ledger Wallet, web extensions, Third-party wallets) communicate with this Ledger app using the **block protocol**:

1. Client sends `START` command with hashes of chunked data
2. Ledger requests chunks via `GET_CHUNK` (by hash)
3. Client responds with `GET_CHUNK_RESPONSE_SUCCESS` (data) or `GET_CHUNK_RESPONSE_FAILURE`
4. Ledger verifies SHA256 hash of each chunk against requested hash
5. Ledger can optionally store data on host via `PUT_CHUNK` / `PUT_CHUNK_RESPONSE`
6. Ledger can send incremental results via `RESULT_ACCUMULATING` / `RESULT_ACCUMULATING_RESPONSE`
7. Ledger sends final response via `RESULT_FINAL`

**Block Protocol Commands**:
- **Host → Ledger**: `START`, `GET_CHUNK_RESPONSE_SUCCESS`, `GET_CHUNK_RESPONSE_FAILURE`, `PUT_CHUNK_RESPONSE`, `RESULT_ACCUMULATING_RESPONSE`
- **Ledger → Host**: `GET_CHUNK`, `PUT_CHUNK`, `RESULT_ACCUMULATING`, `RESULT_FINAL`

**This protocol is embedded in client code.** We cannot change it without:
- Breaking all existing wallet integrations
- Requiring coordinated updates across multiple client teams
- Creating version compatibility nightmares

### What This Means for Migration

❌ **We CANNOT**:
- Switch to simple single-APDU commands (like boilerplate)
- Change the wire protocol format
- Remove block protocol from any command

✅ **We CAN**:
- Reimplement block protocol as synchronous state machine
- Remove all async/await keywords
- Remove `alamgu_async_block` dependency entirely
- Replace `ledger-parser-combinators` with manual parsing
- Simplify the code structure

---

## Recommended Approach: Full Reimplementation

**Goal**: Complete removal of `alamgu_async_block` dependency

This guide recommends reimplementing the block protocol from scratch as a synchronous state machine. While this is the most work upfront, it provides:

✅ **Complete Independence**: No external async dependencies

✅ **Cleaner Architecture**: Purpose-built for your needs

✅ **Long-term Maintainability**: No dependency on Alamgu framework updates

✅ **Better Understanding**: Forces deep comprehension of security-critical protocol

✅ **Optimization Opportunities**: Can optimize for your specific use cases

### Why Remove All Alamgu Dependencies?

**`alamgu_async_block`**:
1. **Unnecessary async abstraction** - Ledger has simple event loop, async/await adds complexity
2. **Dependency maintenance burden** - Third-party, may become unmaintained
3. **Code clarity** - Synchronous state machine is easier to understand

**`ledger-parser-combinators`**:
1. **Tightly coupled with async** - All parsers are async, work with async ByteStream
2. **2000+ lines of async parsing code** - Major complexity in parser/tx.rs
3. **Overkill for simple parsing** - BIP32 path is just reading bytes
4. **Manual parsing is clearer** - Direct byte reading for embedded systems

**Result**: Full control over all security-critical code

### Implementation Overview

See [MIGRATION-GUIDE-BLOCK-PROTOCOL.md](MIGRATION-GUIDE-BLOCK-PROTOCOL.md) for detailed implementation guide.

**Core Components**:

1. **`BlockProtocolHandler`** - State machine for all protocol commands
   - Handles `START` (receive parameter hashes)
   - Handles `GET_CHUNK_RESPONSE_SUCCESS` / `GET_CHUNK_RESPONSE_FAILURE` (receive data)
   - Handles `PUT_CHUNK_RESPONSE` (acknowledge stored data)
   - Handles `RESULT_ACCUMULATING_RESPONSE` (acknowledge partial result)
   - Sends `GET_CHUNK` (request data by hash)
   - Sends `PUT_CHUNK` (store data on host)
   - Sends `RESULT_ACCUMULATING` (send partial result)
   - Sends `RESULT_FINAL` (send final result)
   - Verifies SHA256 hashes for security
   - Manages state transitions between commands

2. **`ChunkedReader`** - Incremental data reading
   - Fetches chunks on demand
   - Buffers partial data
   - Returns control to main loop between chunks

3. **`CommandContext`** - Stateful command execution
   - Tracks progress through multi-step commands
   - Handles UI interaction
   - Coordinates signing operations

---

## Migration Phases

### Phase 1: Core Block Protocol (Week 1)

**Goals**: Implement synchronous block protocol infrastructure

**Tasks**:
- [ ] Implement `BlockProtocolHandler` state machine
  - START command with parameter hashes
  - GET_CHUNK request/response (SUCCESS and FAILURE)
  - PUT_CHUNK request/response (optional, for storing data on host)
  - RESULT_ACCUMULATING request/response (optional, for large results)
  - RESULT_FINAL response
  - SHA256 hash verification (security-critical!)
  - State transition validation
  
- [ ] Implement `ChunkedReader` for incremental data
  - Fetch chunks by hash
  - Buffer partial reads
  - Handle end-of-stream marker (zero hash)
  
- [ ] Unit tests for state machine
  - Valid state transitions
  - Invalid transitions rejected
  - Hash verification catches tampering
  
- [ ] Mock testing framework
  - Simulate APDU exchanges
  - Test without real device

**Deliverable**: Working block protocol that passes unit tests

**Time**: ~5 days

### Phase 2: Simple Commands (Week 2)

**Goals**: Port commands without large inputs

**Tasks**:
- [ ] Implement `CommandContext` framework
  - Stateful execution model
  - In**Implement synchronous BIP32 path parser**
  - Replace `ledger-parser-combinators` BIP32 parser
  - Simple byte reading: length + array of u32 (little-endian)
  - ~1-2 hours
  
- [ ] Port `GetPubkey` (INS=0x02)
  - BIP32 path input via block protocol
  - Use new synchronous path parser
  - Ed25519 key derivation
  - Address calculation
  - UI for approval
  - ~2 days
  
- [ ] Port `VerifyAddress` (INS=0x01)
  - Similar to GetPubkey
  - Different UI flow
  - ~1 day

**Deliverable**: 3 commands working with real clients, BIP32 parsing without parser-combinator
  - ~2 days
  
- [ ] Port `VerifyAddress` (INS=0x01)
  - Similar to GetPubkey
  - Different UI flow
  - ~1 day

**Deliverable**: 3 commands working with real clients

**Time**: ~7 days
**Design synchronous BCS parser**
  - Replace `ledger-parser-combinators` BCS parsing
  - ULEB128 variable-length integers
  - Arrays, vectors, enums, tuples
  - Work with `ChunkedReader`
  - ~2-3 days
  
- [ ] Design incremental transaction parser
  - Parse as chunks arrive (no full buffer)
  - Handle variable-length fields
  - Support all transaction types
  
- [ ] Port transaction parser to sync
  - Convert 2000+ lines of async parsers from parser/tx.rs
  - Use new synchronous BCS parserser
  - Parse as chunks arrive (no full buffer)
  - Handle variable-length fields
  - Support all transaction types
  
- [ ] Port transaction parser to sync
  - Convert 2000+ lines of async parsers
  - Option: Start with blind signing
  - Add transaction types incrementally
  - ~5-7 days
  
- [ ] Port `Sign` command (INS=0x03)
  - Large inputs via block protocol
  - Transaction parsing
  - UI approval flow
  - Ed25519 signing
  - Swap mode integration
  - ~3-4 days
  
- [ ] Port `ProvideTrustedDynamicDescriptor` (INS=0x22)
  - Token metadata handling
  - ~1 day

**Deliverable**: All commands migrated

**Time**: ~10 days

---

## Phase 4: Testing & Cleanup (Final Days)

**Goals**: Ensure compatibility and remove alamgu_async_block

**Tasks**:
- [ ] Update main loop
  - Remove `HAlamgu dependencies from `Cargo.toml`
  - Remove `alamgu_async_block`
  - Remove `ledger-parser-combinators
  - Remove `poll_apdu_handlers`
  - Use `CommandContext` directly
  
- [ ] Run full test suite
  - All devices (Nano S+, X, Stax, Flex, Apex P)
  - All commands
  - Error cases
  
- [ ] Test with real clients
  - Sui Wallet browser extension
  - Mobile wallets
  - Exchange integration (swap mode)
  
- [ ] Remove `alamgu_async_block` from `Cargo.toml`
  - Verify it compiles
  - Verify tests pass
  - Verify binary size
  
- [ ] Documentation
  - Update README
  - Document new architecture
  - Add inline comments

**Deliverable**: Production-ready app with no alamgu_async_block

**Time**: ~3-5 days

---
Replacing ledger-parser-combinators

**Problem**: 2000 lines of async parser combinators in `parser/tx.rs`, tightly coupled with Alamgu

**Options**:
- **A**: Keep using it (stay dependent on Alamgu)
- **B**: Write manual synchronous parsers
- **C**: Use `serde` + BCS deserializer (requires full buffering)
- **D**: Start with blind signing, add parsers incrementally

**Recommendation**: **Option D then B**
- Start with blind signing (just hash transaction bytes)
- Implement synchronous BCS parsing utilities
- Port transaction parsers one type at a time
- Keep the parser structure (it's well-designed), just make it synchronous

**Key Insight**: The parser-combinators library is essentially helper functions for reading bytes. We can write simpler, direct parsing code for an embedded environment.
- Eliminates async/await complexity

**Implementation**: See [MIGRATION-GUIDE-BLOCK-PROTOCOL.md](MIGRATION-GUIDE-BLOCK-PROTOCOL.md)

### 2. Parser Combinators

**Problem**: 2000 lines of async parser combinators in `parser/tx.rs`

**Options**:
- **A**: Port to sync parser combinators (keep structure)
- **B**: Rewrite with `serde` + BCS deserializer
- **C**: Start with blind signing, add types incrementally

**Recommendation**: **Option C then A**
- Quick validation (blind signing works)
- Then port parsers one transaction type at a time

### 3. Data Buffering Strategy

**Problem**: Can't parse transactions until all chunks arrive

**Options**:
- **A**: Buffer complete transaction in memory
- **B**: Incremental parsing as chunks arrive
- **C**: Parse in two passes (size first, then content)

**Recommendation**: **Option B** (incremental parsing)
- Lower memory usage
- Can reject invalid data earlier
- More complex but more efficient

### 4. Testing Strategy

**Problem**: Must verify exact compatibility

**Options**:
- **A**: Test only final migrated version
- **B**: Test incrementally after each command

**Recommendation**: **Option B**
- Catch regressions early
- Build confidence incrementally
- Easier debugging

---

## Risk Assessment

### High Risk ⚠️⚠️⚠️

**Block protocol reimplementation**
- Crypto-critical: SHA256 verification prevents tampering
- Complex state machine: START → GET_CHUNK → RESULT_FINAL  
- Must handle all edge cases correctly
- Mitigation: 
  - Extensive unit tests for state transitions
  - Test with real clients before deployment
  - Code review by security experts
  - Reference implementation available in alamgu_async_block source

**Transaction parser**
- 2000+ lines of complex parsing logic
- Incorrect parsing → wrong transaction displayed → user loses funds
- Mitigation: 
  - Port incrementally, one transaction type at a time
  - Extensive test suite with known-good transactions
  - Start with blind signing as fallback
  - Cross-reference with Sui SDK parser behavior

### Medium Risk ⚠️⚠️

**State management across APDUs**
- Must maintain state between APDU exchanges
- Memory safety with stateful context
- Mitigation: 
  - Clear state machine design
  - Explicit state transitions
  - Comprehensive state machine tests

**Device-specific UI**
- Different APIs for NBGL vs BAGL
- Mitigation: Test on all device types

**Memory management**
- Embedded environment with limited stack/heap
- Large transaction buffering
- Mitigation:
  - Use `ArrayVec` for bounded collections
  - Incremental parsing to avoid large buffers
  - Memory profiling during testing

### Low Risk ⚠️

**Crypto operations**
- Ed25519, Blake2b are already synchronous
- No changes needed
- ✅ **No `ledger-parser-combinators` dependency in `Cargo.toml`**

**BIP32 path parsing**
- Simple byte parsing
- Easy to test

---

## Success Criteria

### Functional Requirements
- ✅ All existing tests pass without modification
- ✅ Can sign transactions with real Sui Wallet
- ✅ Swap mode works with Ledger Exchange
- ✅ All device types work (Nano S+, X, Stax, Flex, Apex P)
- ✅ **No `alamgu_async_block` dependency in `Cargo.toml`**

### Non-Functional Requirements
- ✅ No performance regression (within 10%)
- ✅ Binary size similar or smaller
- ✅ RAM usage similar or lower
- ✅ Code is clearer and easier to maintain

### Security Requirements
- ✅ Block protocol hash verification prevents tampering
- ✅ No memory safety issues
- ✅ Same or better error handling
- ✅ Transaction parsing behavior identical to current implementation

---

## Conclusion

**The migration to a fully synchronous, alamgu-free implementation is recommended** for these reasons:
Implement synchronous BCS parsing utilities (~2-3 days)
3. Port simple commands with new parsers (~1 week)  
4. Port complex Sign command with incremental parser (~1-2 weeks)
5. Test exhaustively and remove all Alamgu dependencies

**Timeline**: 2-3 weeks for a careful, security-focused implementation

**Key Insights**: 
- The block protocol is ~500 lines of well-defined, security-critical code
- Parser-combinators is just byte reading helpers - we can write simpler direct parsing
- Reimplementing both gives you full control and eliminates async abstraction entirely
- Maintains 100% wire protocol compatibility with existing clients
1. Reimplement block protocol as synchronous state machine (~1 week)
2. Port simple commands to validate approach (~1 week)  
3. Port complex Sign command with incremental parser (~1-2 weeks)
4. Test exhaustively and remove `alamgu_async_block` dependency

**Timeline**: 2-3 weeks for a careful, security-focused implementation

**Key Insight**: The block protocol is ~500 lines of well-defined, security-critical code. Reimplementing it gives you full control and eliminates the async abstraction entirely, while maintaining 100% wire protocol compatibility with existing clients.

**Next Steps**: See [MIGRATION-GUIDE-BLOCK-PROTOCOL.md](MIGRATION-GUIDE-BLOCK-PROTOCOL.md) for detailed implementation guide.

