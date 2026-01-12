# Migration Guide: Sign Transaction APDU Command

## Prerequisites

⚠️ **You must complete [MIGRATION-GUIDE-BLOCK-PROTOCOL.md](MIGRATION-GUIDE-BLOCK-PROTOCOL.md) first!**

This guide assumes you have already implemented:
- `BlockProtocolHandler` - synchronous state machine  
- `ChunkedReader` - incremental data reading (critical for this command!)
- `CommandContext` - stateful execution framework

This is the **most complex migration** in the Sui app. Sign handles large transaction data (requiring the block protocol), complex parsing (multiple transaction types), and stateful UI (multiple confirmation screens).

## ⚠️ Critical Constraint: Client Compatibility

**IMPORTANT**: Existing Sui wallet clients already use the **block protocol** to communicate with this app. We **cannot change the wire protocol** without breaking all existing clients.

**This means:**
- ✅ We **must keep the block protocol** (START, GET_CHUNK, hash verification, etc.)
- ✅ The migration is about making the **internal implementation** synchronous
- ✅ For Sign, block protocol is **essential** - transactions can be 10KB+
- ✅ The hash-based chunk verification provides security against tampering

**The original "Approach B" (fixed chunking without block protocol) is NOT viable** due to client compatibility requirements.

## Overview

**Command**: Sign Transaction (INS=0x03)
- **Input**: 
  - Transaction data (100-10,000+ bytes, BCS-encoded)
  - BIP32 path
  - Optional: Object metadata (for token operations)
- **Output**: Ed25519 signature (64 bytes)
- **UI Flow**: Parse → Identify type → Show details → User approves → Sign
- **Transaction Types**: Transfer, Stake, Unstake, or Unknown (blind signing)

---

## Current Implementation (Alamgu Pattern)

### File: `rust-app/src/implementation.rs`

```rust
pub async fn sign_apdu(io: HostIO, ctx: &RunCtx, settings: Settings, ui: UserInterface) {
    // 1. Get 3 input parameters via block protocol
    let mut input = match io.get_params::<3>() {
        Some(v) => v,
        None => reject(SyscallError::InvalidParameter as u16).await,
    };
    
    // input[0] = transaction data (with length prefix)
    // input[1] = BIP32 path
    // input[2] = optional object metadata

    // 2. Read transaction length
    let length = usize::from_le_bytes(input[0].read().await);
    
    // 3. Parse transaction asynchronously (fetches chunks as needed)
    let known_txn = {
        let mut txn = input[0].clone();
        let object_data_source = input.get(2).map(|bs| WithObjectData { bs: bs.clone() });
        tx_parser(object_data_source).parse(&mut txn).await
    };
    
    // 4. Handle transaction type
    match known_txn {
        Some(KnownTx::TransferTx { recipient, total_amount, coin_type, gas_budget }) => {
            let path = BIP_PATH_PARSER.parse(&mut input[1].clone()).await;
            
            if ctx.is_swap() {
                // Verify params match exchange expectations
                check_tx_params(ctx.get_swap_tx_params(), &tx_params).await;
            } else {
                // Show UI prompts
                prompt_tx_params(&ui, &path, tx_params, coin_type, ctx).await;
            }
        }
        Some(KnownTx::StakeTx { ... }) => { /* Similar */ }
        Some(KnownTx::UnstakeTx { ... }) => { /* Similar */ }
        None => {
            // Unknown transaction - blind signing
            if !settings.get_blind_sign() {
                ui.warn_tx_not_recognized();
                reject().await;
            }
        }
    }
    
    // 5. Hash the transaction data
    let mut hasher: Blake2b = Hasher::new();
    {
        let mut txn = input[0].clone();
        const CHUNK_SIZE: usize = 128;
        for _ in 0..(length / CHUNK_SIZE) {
            let chunk: [u8; CHUNK_SIZE] = txn.read().await;
            hasher.update(&chunk);
        }
        // Handle remainder...
    }
    let hash = hasher.finalize();
    
    // 6. Sign the hash
    let path = BIP_PATH_PARSER.parse(&mut input[1].clone()).await;
    let sig = eddsa_sign(&path, true, &hash.0).ok()?;
    
    // 7. Send signature
    io.result_final(&sig.0[0..]).await;
    
    ctx.set_swap_sign_success();
}
```

### Key Complexity:
- 🔴 **Block protocol required** - transactions can be 10KB+
- 🔴 **Async parsing** - `tx_parser` is 2000+ lines of combinators
- 🔴 **Streamed hashing** - can't load entire tx into RAM
- 🔴 **Multiple transaction types** - different UI flows
- 🔴 **Swap mode integration** - parameter verification

---

## Migration Strategy: Keep Block Protocol, Make Synchronous

Given the client compatibility constraint, there is really only **one viable approach**:

### **Keep Block Protocol, Convert Async to Sync**

**What this means**:
- ✅ Keep Alamgu's block protocol state machine (or reimplement it synchronously)
- ✅ Convert async handlers to synchronous
- ✅ Convert async parsers to synchronous
- ✅ Maintain exact same wire protocol

**Changes required**:
1. Remove `async`/`.await` keywords
2. Convert `io.get_params()` to synchronous block protocol handling
3. Convert parser combinators from async to sync
4. Keep chunking, hashing, and verification logic

**The Approach B from earlier (fixed chunking) is incorrect** - it would break all existing clients.

---

## Target Implementation (Approach B: Fixed Chunking)

### Step 1: Define Sign Context

**File**: `rust-app/src/handlers/sign_tx.rs` (new file)

**Note**: This is equivalent to `rust-app/src/commands/sign.rs` in MIGRATION-README.md's file structure. Use whichever path matches your project organization (handlers/ or commands/).

```rust
use alloc::vec::Vec;
use crate::utils::Bip32Path;
use crate::parser::tx::SuiTransaction;
use crate::AppSW;

const MAX_TRANSACTION_LEN: usize = 4096; // 4KB max

pub struct SignContext {
    state: SignState,
    path: Bip32Path,
    raw_tx: Vec<u8>,
    parsed_tx: Option<SuiTransaction>,
}

enum SignState {
    ExpectPath,          // Chunk 0: receive derivation path
    AccumulatingTx,      // Chunks 1-N: accumulate transaction data
    ReadyToParse,        // Last chunk received, ready to parse
    AwaitingApproval,    // UI shown, waiting for user
    ReadyToSign,         // User approved, ready to sign
}

impl SignContext {
    pub fn new() -> Self {
        SignContext {
            state: SignState::ExpectPath,
            path: Default::default(),
            raw_tx: Vec::new(),
            parsed_tx: None,
        }
    }
    
    pub fn reset(&mut self) {
        self.state = SignState::ExpectPath;
        self.path = Default::default();
        self.raw_tx.clear();
        self.parsed_tx = None;
    }
}
```

### Step 2: Implement Handler with Chunking

```rust
use ledger_device_sdk::io::Comm;
use ledger_crypto_helpers::hasher::{Blake2b, Hasher};
use ledger_crypto_helpers::eddsa::eddsa_sign;

pub fn handler_sign_tx(
    comm: &mut Comm,
    chunk: u8,
    more: bool,
    ctx: &mut SignContext,
) -> Result<(), AppSW> {
    let data = comm.get_data().map_err(|_| AppSW::WrongApduLength)?;
    
    match (ctx.state, chunk, more) {
        // Chunk 0: Parse BIP32 path
        (SignState::ExpectPath, 0, true) => {
            ctx.reset();
            ctx.path = data.try_into()?;
            ctx.state = SignState::AccumulatingTx;
            Ok(())
        }
        
        // Middle chunks: Accumulate transaction data
        (SignState::AccumulatingTx, n, true) if n > 0 => {
            if ctx.raw_tx.len() + data.len() > MAX_TRANSACTION_LEN {
                return Err(AppSW::TxTooLarge);
            }
            ctx.raw_tx.extend_from_slice(data);
            Ok(())
        }
        
        // Last chunk: Parse and display
        (SignState::AccumulatingTx, n, false) if n > 0 => {
            if ctx.raw_tx.len() + data.len() > MAX_TRANSACTION_LEN {
                return Err(AppSW::TxTooLarge);
            }
            ctx.raw_tx.extend_from_slice(data);
            ctx.state = SignState::ReadyToParse;
            
            // Parse transaction
            ctx.parsed_tx = Some(parse_sui_transaction(&ctx.raw_tx)?);
            
            // Display transaction for approval
            if !ui_display_transaction(&ctx.parsed_tx.as_ref().unwrap())? {
                return Err(AppSW::Deny);
            }
            
            ctx.state = SignState::ReadyToSign;
            
            // Compute signature
            sign_and_respond(comm, ctx)
        }
        
        _ => Err(AppSW::InvalidState),
    }
}

fn sign_and_respond(comm: &mut Comm, ctx: &mut SignContext) -> Result<(), AppSW> {
    // Hash the transaction
    let mut hasher = Blake2b::new();
    hasher.update(&ctx.raw_tx);
    let hash = hasher.finalize();
    
    // Sign with Ed25519
    let signature = eddsa_sign(ctx.path.as_ref(), true, &hash)
        .map_err(|_| AppSW::SigningFailed)?;
    
    // Return signature (64 bytes)
    comm.append(&signature.0);
    
    Ok(())
}
```

### Step 3: Transaction Parser (Simplified)

The full parser is 2000+ lines. Here's a simplified version for the migration:

**File**: `rust-app/src/parser/sui_tx.rs` (new simplified parser)

```rust
use alloc::vec::Vec;
use crate::parser::common::*;
use crate::AppSW;

/// Simplified Sui transaction representation
pub enum SuiTransaction {
    Transfer {
        sender: SuiAddress,
        recipient: SuiAddress,
        amount: u64,
        coin_type: CoinType,
        gas_budget: u64,
    },
    Stake {
        sender: SuiAddress,
        validator: SuiAddress,
        amount: u64,
        gas_budget: u64,
    },
    Unstake {
        sender: SuiAddress,
        amount: u64,
        gas_budget: u64,
    },
    Unknown {
        hash: [u8; 32],
    },
}

pub enum CoinType {
    Sui,
    Token { name: Vec<u8> },
}

/// Parse BCS-encoded Sui transaction
pub fn parse_sui_transaction(data: &[u8]) -> Result<SuiTransaction, AppSW> {
    // This is where the complex parser combinators logic goes
    // For now, a stub that recognizes basic transfers
    
    if data.is_empty() {
        return Err(AppSW::TxParsingFail);
    }
    
    // Try to parse as BCS-encoded TransactionData
    // Real implementation would use BCS deserializer
    
    // Placeholder: Assume it's a transfer for demonstration
    Ok(SuiTransaction::Unknown {
        hash: blake2b_hash(data),
    })
}

fn blake2b_hash(data: &[u8]) -> [u8; 32] {
    use ledger_crypto_helpers::hasher::{Blake2b, Hasher};
    let mut hasher = Blake2b::new();
    hasher.update(data);
    hasher.finalize()
}
```

**Note**: The real parser requires porting 2000+ lines of async parser combinators. Options:
1. **Port incrementally** - Start with Unknown transactions only (blind signing)
2. **Use `serde` + `bcs`** - Leverage Sui's BCS encoding library
3. **Keep Alamgu parser** - Use Approach A instead

### Step 4: UI Display Functions

**File**: `rust-app/src/ui/sign_tx.rs`

```rust
use crate::parser::sui_tx::{SuiTransaction, CoinType};
use crate::AppSW;
use alloc::format;

#[cfg(any(target_os = "stax", target_os = "flex", target_os = "apex_p"))]
pub fn ui_display_transaction(tx: &SuiTransaction) -> Result<bool, AppSW> {
    use ledger_device_sdk::nbgl::NbglReview;
    
    match tx {
        SuiTransaction::Transfer { sender, recipient, amount, coin_type, gas_budget } => {
            let mut review = NbglReview::new()
                .titles("Sign", "Transaction", "Approve");
            
            review.add_field("Type", "Transfer");
            review.add_field("From", &format_address(sender));
            review.add_field("To", &format_address(recipient));
            review.add_field("Amount", &format_amount(*amount, coin_type));
            review.add_field("Gas Budget", &format!("{} SUI", gas_budget));
            
            Ok(review.show())
        }
        
        SuiTransaction::Stake { sender, validator, amount, gas_budget } => {
            let mut review = NbglReview::new()
                .titles("Sign", "Stake", "Approve");
            
            review.add_field("Type", "Stake");
            review.add_field("Validator", &format_address(validator));
            review.add_field("Amount", &format!("{} SUI", amount));
            review.add_field("Gas Budget", &format!("{} SUI", gas_budget));
            
            Ok(review.show())
        }
        
        SuiTransaction::Unknown { hash } => {
            // Blind signing - show hash only
            let mut review = NbglReview::new()
                .titles("Blind Sign", "Warning", "Approve");
            
            review.add_field("Type", "Unknown Transaction");
            review.add_field("Hash", &format_hash(hash));
            
            Ok(review.show())
        }
        
        _ => Ok(true),
    }
}

fn format_address(addr: &SuiAddress) -> String {
    format!("0x{:x}", addr) // Truncate for display
}

fn format_amount(amount: u64, coin_type: &CoinType) -> String {
    match coin_type {
        CoinType::Sui => format!("{}.{} SUI", amount / 1_000_000_000, amount % 1_000_000_000),
        CoinType::Token { name } => format!("{} {:?}", amount, name),
    }
}

fn format_hash(hash: &[u8; 32]) -> String {
    use ledger_crypto_helpers::common::HexSlice;
    format!("0x{}", HexSlice(&hash[..8])) // Show first 8 bytes
}
```

### Step 5: Update Main Handler

**File**: `rust-app/src/main.rs`

```rust
fn handle_apdu(comm: &mut Comm, ins: &Instruction, ctx: &mut TxContext) -> Result<(), AppSW> {
    match ins {
        Instruction::GetVersion => handler_get_version(comm),
        Instruction::GetPubkey { display } => handler_get_public_key(comm, *display),
        Instruction::SignTx { chunk, more } => handler_sign_tx(comm, *chunk, *more, ctx),
        _ => Err(AppSW::InsNotSupported),
    }
}

// Parse P1/P2 for chunking
enum Instruction {
    GetVersion,
    GetPubkey { display: bool },
    SignTx { chunk: u8, more: bool },
}

impl TryFrom<ApduHeader> for Instruction {
    type Error = AppSW;
    
    fn try_from(h: ApduHeader) -> Result<Self, Self::Error> {
        match (h.ins, h.p1, h.p2) {
            (0x00, 0, 0) => Ok(Instruction::GetVersion),
            (0x01, 0, 0) => Ok(Instruction::GetPubkey { display: true }),
            (0x02, 0, 0) => Ok(Instruction::GetPubkey { display: false }),
            (0x03, chunk, 0x00) => Ok(Instruction::SignTx { chunk, more: false }),
            (0x03, chunk, 0x80) => Ok(Instruction::SignTx { chunk, more: true }),
            _ => Err(AppSW::InsNotSupported),
        }
    }
}
```

---

## The Parser Challenge

The biggest obstacle is the **2000-line async transaction parser** (`rust-app/src/parser/tx.rs`). You have three options:

### Option 1: Incremental Port (Recommended)

Start with just blind signing:

```rust
pub fn parse_sui_transaction(data: &[u8]) -> Result<SuiTransaction, AppSW> {
    // For V1: Just compute hash, require blind signing
    let hash = blake2b_hash(data);
    Ok(SuiTransaction::Unknown { hash })
}
```

Then add transaction types one by one:
1. ✅ Unknown (blind signing only)
2. → Simple transfers
3. → Token transfers
4. → Staking operations
5. → Complex programmable transactions

### Option 2: Use BCS Deserializer

Leverage Sui's existing BCS (Binary Canonical Serialization) library:

```rust
use bcs;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct TransactionData {
    pub kind: TransactionKind,
    pub sender: SuiAddress,
    pub gas_data: GasData,
    // ...
}

pub fn parse_sui_transaction(data: &[u8]) -> Result<SuiTransaction, AppSW> {
    let tx: TransactionData = bcs::from_bytes(data)
        .map_err(|_| AppSW::TxParsingFail)?;
    
    // Convert to SuiTransaction enum
    classify_transaction(tx)
}
```

**Pros**: Reuses official Sui types
**Cons**: Requires `std` features, may bloat binary size

### Option 3: Keep Alamgu Parser, Convert to Sync

Port the parser combinators to synchronous code:

```rust
// Before (async)
pub async fn tx_parser<BS: Readable>(...) -> impl AsyncParser<IntentMessage, BS> {
    let data = input.read().await;
    // ...
}

// After (sync)
pub fn tx_parser_sync(data: &[u8]) -> Result<KnownTx, AppSW> {
    let mut cursor = Cursor::new(data);
    let intent = parse_intent(&mut cursor)?;
    let tx_data = parse_transaction_data(&mut cursor)?;
    // ...
}
```

This requires careful transformation of all parser combinators.

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_transfer() {
        // BCS-encoded transfer transaction
        let tx_bytes = hex::decode("...").unwrap();
        
        let parsed = parse_sui_transaction(&tx_bytes).unwrap();
        
        match parsed {
            SuiTransaction::Transfer { amount, .. } => {
                assert_eq!(amount, 1_000_000_000); // 1 SUI
            }
            _ => panic!("Expected transfer"),
        }
    }
}
```

### Integration Tests (Ragger)

```python
# ragger-tests/test_sign_sui_transfer.py

def test_sign_simple_transfer(backend):
    client = SuiClient(backend)
    
    # Build transaction
    path = "m/44'/784'/0'/0'/0'"
    tx_data = build_transfer_tx(
        recipient="0x1234...",
        amount=1_000_000_000,  # 1 SUI
        gas_budget=10_000_000
    )
    
    # Sign in chunks (assuming 3 chunks)
    chunk1 = encode_path(path)  # Chunk 0: path
    chunk2 = tx_data[:200]      # Chunk 1: first part of tx
    chunk3 = tx_data[200:]      # Chunk 2: rest of tx (last chunk)
    
    signature = client.sign_transaction([chunk1, chunk2, chunk3])
    
    assert len(signature) == 64  # Ed25519 signature
    
    # Verify signature
    assert verify_sui_signature(tx_data, signature, path)
```

---

## Performance & Limitations

### Approach B (Fixed Chunking)

| Aspect | Limit | Notes |
|--------|-------|-------|
| **Max TX size** | 4KB | Good for 95% of transactions |
| **Chunks** | 4-5 | @~1KB each |
| **APDUs** | 4-6 | Path + TX chunks + response |
| **Latency** | ~200ms | Total signing time |
| **RAM usage** | ~5KB | TX buffer + signature |

**Rejected transactions**:
- Complex multi-sig (can be 10KB+)
- Bulk operations (100+ commands)
- Large programmable transactions

### Approach A (Block Protocol)

| Aspect | Limit | Notes |
|--------|-------|-------|
| **Max TX size** | Unlimited | Limited only by Ledger RAM |
| **Chunks** | 50+ | @180 bytes each |
| **APDUs** | 100+ | Many GET_CHUNK requests |
| **Latency** | ~500ms | Hash verification overhead |
| **RAM usage** | ~1KB | Streamed processing |

**No rejections** - handles all Sui transactions

---

## Migration Checklist

### Phase 1: Basic Infrastructure
- [ ] Create `SignContext` struct
- [ ] Implement chunking state machine
- [ ] Add P1/P2 parsing for chunk numbers
- [ ] Test with dummy transactions (always blind sign)

### Phase 2: Transaction Parser (ledger-parser-combinators Replacement)

**File**: `rust-app/src/parser/tx_sync.rs` (new)

**Current state**: 2000+ lines of async parser combinators in `parser/tx.rs`

**Challenge**: The transaction parser is tightly coupled with `ledger-parser-combinators`:
- All parsers are async (`async fn parse()`)
- Work with `ByteStream` (from `alamgu_async_block`)
- Use complex trait-based infrastructure
- BCS encoding (variable-length integers, nested structures)

**Migration strategy**:

- [ ] **Step 1**: Implement synchronous BCS utilities (see MIGRATION-GUIDE-BLOCK-PROTOCOL.md Step 6)
  - `read_uleb128()` - variable-length integers
  - `read_u64_le()`, `read_u32_le()` - fixed integers
  - `read_array()`, `read_vec()` - arrays and vectors
  - `read_bool()`, `read_option()` - boolean and optional
  
- [ ] **Step 2**: Start with blind signing
  - Skip transaction parsing initially
  - Just hash the transaction bytes
  - Display "Unknown Transaction" + hash
  - User must enable blind signing in settings
  
- [ ] **Step 3**: Port transaction parser incrementally
  ```rust
  pub fn parse_transaction(reader: &mut ChunkedReader) -> Result<KnownTx, Reply> {
      // Intent (version, scope, app_id)
      let version = read_uleb128(reader)?;
      let scope = read_uleb128(reader)?;
      let app_id = read_uleb128(reader)?;
      
      // Transaction kind (enum variant)
      let tx_kind = read_uleb128(reader)?;
      
      match tx_kind {
          0 => parse_programmable_tx(reader),
          _ => Err(Reply(0x6A80)), // Unsupported type
      }
  }
  
  fn parse_programmable_tx(reader: &mut ChunkedReader) -> Result<KnownTx, Reply> {
      // Inputs (Vec<CallArg>)
      let num_inputs = read_uleb128(reader)? as usize;
      let mut inputs = ArrayVec::new();
      
      for _ in 0..num_inputs {
          let input = parse_call_arg(reader)?;
          inputs.push(input);
      }
      
      // Commands (Vec<Command>)
      let num_commands = read_uleb128(reader)? as usize;
      
      // Analyze commands to identify transaction type
      identify_transaction_type(inputs, commands)
  }
  ```
  
- [ ] **Step 4**: Add transaction types one at a time
  - Transfer (most common)
  - Stake
  - Unstake
  - Token operations (with metadata)
  
- [ ] **Step 5**: Testing
  - Unit tests for each BCS primitive
  - Parser tests with known-good transactions
  - Cross-reference with Sui SDK parser behavior

**Time estimate**: 
- BCS utilities: 1-2 days
- Blind signing: 1 day
- Full parser: 5-7 days
- **Total: 7-10 days**

**Benefits**:
- ✅ No `ledger-parser-combinators` dependency
- ✅ No async/await in parsing
- ✅ Clearer, more direct code
- ✅ Better for embedded environment

### Phase 3: UI Integration
- [ ] Implement NBGL transaction review (Stax/Flex)
- [ ] Implement BAGL transaction review (Nano S+/X)
- [ ] Add device-specific formatting
- [ ] Test with real hardware

### Phase 4: Swap Mode
- [ ] Port swap parameter checking
- [ ] Test with Ledger Exchange integration
- [ ] Verify parameter mismatch detection

### Phase 5: Testing
- [ ] Unit test each transaction type parser
- [ ] Integration test with Speculos (all devices)
- [ ] Hardware test on real devices
- [ ] Fuzz test with malformed transactions

---

## Estimated Migration Time

**With Block Protocol + Parser Replacement**:
- Phase 1 (Infrastructure): 2-3 days
- Phase 2 (Parser): 7-10 days (BCS utilities + transaction parsing)
- Phase 3 (UI): 1-2 days  
- Phase 4 (Swap): 1 day
- Phase 5 (Testing): 2-3 days
- **Total: 13-19 days (~2-3 weeks)**

**Incremental Approach** (Recommended):
1. Week 1: Block protocol + blind signing + transfer parsing
2. Week 2: Add stake/unstake parsing + full UI
3. Week 3: Polish, testing, remove Alamgu dependencies

---

## Next Steps

After migrating Sign:

1. ✅ All core APDUs migrated (GetVersion, GetPubkey, Sign)
2. ✅ All Alamgu dependencies removed:
   - Remove `alamgu_async_block` from `Cargo.toml`
   - Remove `ledger-parser-combinators` from `Cargo.toml`
3. → Update all Ragger tests
4. → Benchmark to ensure no performance regression
5. → Test on all devices (NanoS+, NanoX, Flex, Stax, Apex P)
6. → Security audit of block protocol and parser reimplementation

The Sign command with transaction parser is the hardest part. Once this works, you've successfully achieved **complete independence from Alamgu**!
