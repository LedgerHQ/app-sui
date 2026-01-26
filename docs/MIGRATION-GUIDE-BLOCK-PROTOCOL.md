# Guide: Reimplementing the Block Protocol (Synchronous)

This guide shows how to reimplement the block protocol from scratch without `alamgu_async_block`, making it completely synchronous while maintaining client compatibility.

## Overview

**Goal**: Remove `alamgu_async_block` dependency entirely

**Approach**: Reimplement block protocol as synchronous state machine

**Estimated Time**: 2-3 weeks

**Difficulty**: High (security-critical code)

---

## Block Protocol Commands Reference

The block protocol defines 9 commands split between Host→Ledger and Ledger→Host:

### Host to Ledger (in APDU data)

| Command | Value | Description | Payload |
|---------|-------|-------------|---------|
| `START` | 0 | Initiates protocol, provides parameter hashes | Concatenated 32-byte hashes |
| `GET_CHUNK_RESPONSE_SUCCESS` | 1 | Returns requested data chunk | Data (verified by hash) |
| `GET_CHUNK_RESPONSE_FAILURE` | 2 | Host doesn't have requested chunk | Empty |
| `PUT_CHUNK_RESPONSE` | 3 | Acknowledges data storage | Empty |
| `RESULT_ACCUMULATING_RESPONSE` | 4 | Acknowledges partial result | Empty |

### Ledger to Host (in response)

| Command | Value | Description | Payload |
|---------|-------|-------------|---------|
| `RESULT_ACCUMULATING` | 0 | Sends partial result (can repeat) | Partial result data |
| `RESULT_FINAL` | 1 | Sends final result (completes) | Final result data |
| `GET_CHUNK` | 2 | Requests data chunk by hash | 32-byte hash |
| `PUT_CHUNK` | 3 | Stores data on host | Data to store |

### When Each Command Is Used

**`START`**: Every block protocol transaction begins with this. Host sends hashes of all input parameters.

**`GET_CHUNK` / `GET_CHUNK_RESPONSE_*`**: Core data transfer mechanism. Ledger requests chunks by hash, host responds with data. Used for all inputs (transaction data, BIP32 path, metadata).

**`PUT_CHUNK` / `PUT_CHUNK_RESPONSE`**: Optional. Ledger can store intermediate data on host and retrieve it later by hash. Useful for complex computations where Ledger RAM is limited.

**`RESULT_ACCUMULATING` / `RESULT_ACCUMULATING_RESPONSE`**: Optional. For large results that don't fit in one APDU response. Ledger sends result in multiple parts. (Note: Sui signatures are only 64 bytes, so typically not needed)

**`RESULT_FINAL`**: Every transaction ends with this. Sends final result and completes the protocol.

---

## Implementation Structure

The block protocol reimplementation consists of three main files:

```
rust-app/src/block_protocol/
  mod.rs       # BlockProtocolHandler - Core state machine for protocol commands
  reader.rs    # ChunkedReader - Incremental data reading from chunks
  context.rs   # CommandContext - Stateful command execution coordinator
```

**Module responsibilities**:
- **`mod.rs`**: Implements all 9 block protocol commands, hash verification, state transitions
- **`reader.rs`**: Provides high-level API for reading data incrementally from chunked inputs
- **`context.rs`**: Coordinates between block protocol and command logic across multiple APDUs

---

## Step 1: Block Protocol State Machine (mod.rs)

**File**: `rust-app/src/block_protocol/mod.rs` (new)

```rust
use ledger_device_sdk::io::{Comm, Reply, StatusWords};
use ledger_crypto_helpers::hasher::{Blake2b, Hasher};
use arrayvec::ArrayVec;

const HASH_LEN: usize = 32;
pub type Hash = [u8; HASH_LEN];

/// Block protocol commands from Host to Ledger (APDU data byte 0)
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum HostToLedgerCmd {
    Start = 0,
    GetChunkResponseSuccess = 1,
    GetChunkResponseFailure = 2,
    PutChunkResponse = 3,
    ResultAccumulatingResponse = 4,
}

impl TryFrom<u8> for HostToLedgerCmd {
    type Error = Reply;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Start),
            1 => Ok(Self::GetChunkResponseSuccess),
            2 => Ok(Self::GetChunkResponseFailure),
            3 => Ok(Self::PutChunkResponse),
            4 => Ok(Self::ResultAccumulatingResponse),
            _ => Err(StatusWords::BadIns.into()),
        }
    }
}

/// Block protocol commands from Ledger to Host (Response byte 0)
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum LedgerToHostCmd {
    ResultAccumulating = 0,
    ResultFinal = 1,
    GetChunk = 2,
    PutChunk = 3,
}

/// State of block protocol interaction
pub enum BlockProtocolState {
    /// Ready to receive START command
    Idle,
    /// Waiting for GET_CHUNK_RESPONSE
    WaitingChunk { requested_hash: Hash },
    /// Waiting for PUT_CHUNK_RESPONSE
    WaitingPutResponse,
    /// Waiting for RESULT_ACCUMULATING_RESPONSE
    WaitingResultResponse,
    /// Transaction completed
    Done,
}

pub struct BlockProtocolHandler {
    state: BlockProtocolState,
    /// Input parameter hashes from START command
    input_hashes: ArrayVec<Hash, 3>,
    /// Accumulated result data (for RESULT_ACCUMULATING)
    result_buffer: ArrayVec<u8, 256>,
}
```

---

## Step 2: Implement Block Protocol Handler

```rust
impl BlockProtocolHandler {
    pub fn new() -> Self {
        BlockProtocolHandler {
            state: BlockProtocolState::Idle,
            input_hashes: ArrayVec::new(),
            result_buffer: ArrayVec::new(),
        }
    }
    
    /// Process incoming APDU and return next action
    pub fn process_command(&mut self, comm: &mut Comm) -> Result<BlockAction, Reply> {
        let data = comm.get_data().map_err(|_| StatusWords::BadLen)?;
        
        if data.is_empty() {
            return Err(StatusWords::BadLen.into());
        }
        
        let cmd = HostToLedgerCmd::try_from(data[0])?;
        let payload = &data[1..];
        
        match (&self.state, cmd) {
            // START: Receive parameter hashes
            (BlockProtocolState::Idle, HostToLedgerCmd::Start) => {
                self.input_hashes.clear();
                
                // Parse hashes from payload (32 bytes each)
                if payload.len() % HASH_LEN != 0 {
                    return Err(StatusWords::BadLen.into());
                }
                
                for chunk in payload.chunks_exact(HASH_LEN) {
                    let hash: Hash = chunk.try_into()
                        .map_err(|_| StatusWords::BadLen)?;
                    self.input_hashes.try_push(hash)
                        .map_err(|_| StatusWords::BadLen)?;
                }
                
                // Ready to process with input hashes
                Ok(BlockAction::ProcessCommand)
            }
            
            // GET_CHUNK_RESPONSE_SUCCESS: Receive chunk data
            (BlockProtocolState::WaitingChunk { requested_hash }, 
             HostToLedgerCmd::GetChunkResponseSuccess) => {
                // Verify hash of received data
                let received_hash = sha256(payload);
                if &received_hash != requested_hash {
                    return Err(Reply(0x6A80)); // Invalid data / hash mismatch
                }
                
                self.state = BlockProtocolState::Idle;
                Ok(BlockAction::ChunkReceived(payload.to_vec()))
            }
            
            // GET_CHUNK_RESPONSE_FAILURE: Host doesn't have the chunk
            (BlockProtocolState::WaitingChunk { .. }, 
             HostToLedgerCmd::GetChunkResponseFailure) => {
                Err(Reply(0x6A88)) // Referenced data not found
            }
            
            // PUT_CHUNK_RESPONSE: Host acknowledged storing data
            (BlockProtocolState::WaitingPutResponse, 
             HostToLedgerCmd::PutChunkResponse) => {
                self.state = BlockProtocolState::Idle;
                Ok(BlockAction::PutAcknowledged)
            }
            
            // RESULT_ACCUMULATING_RESPONSE: Host acknowledged partial result
            (BlockProtocolState::WaitingResultResponse,
             HostToLedgerCmd::ResultAccumulatingResponse) => {
                self.state = BlockProtocolState::Idle;
                Ok(BlockAction::ResultAcknowledged)
            }
            
            _ => Err(StatusWords::BadIns.into()),
        }
    }
    
    /// Request a chunk from host (by hash)
    pub fn get_chunk(&mut self, comm: &mut Comm, hash: Hash) -> Result<(), Reply> {
        self.state = BlockProtocolState::WaitingChunk { requested_hash: hash };
        
        comm.append(&[LedgerToHostCmd::GetChunk as u8]);
        comm.append(&hash);
        comm.reply_ok();
        
        Ok(())
    }
    
    /// Store chunk on host (returns its hash for later retrieval)
    pub fn put_chunk(&mut self, comm: &mut Comm, data: &[u8]) -> Result<Hash, Reply> {
        let hash = sha256(data);
        
        self.state = BlockProtocolState::WaitingPutResponse;
        
        comm.append(&[LedgerToHostCmd::PutChunk as u8]);
        comm.append(data);
        comm.reply_ok();
        
        Ok(hash)
    }
    
    /// Send partial result (can be called multiple times)
    pub fn result_accumulating(&mut self, comm: &mut Comm, data: &[u8]) -> Result<(), Reply> {
        self.state = BlockProtocolState::WaitingResultResponse;
        
        // Store in buffer (optional - for tracking)
        if self.result_buffer.remaining_capacity() >= data.len() {
            let _ = self.result_buffer.try_extend_from_slice(data);
        }
        
        comm.append(&[LedgerToHostCmd::ResultAccumulating as u8]);
        comm.append(data);
        comm.reply_ok();
        
        Ok(())
    }
    
    /// Send final result (completes the transaction)
    pub fn result_final(&mut self, comm: &mut Comm, data: &[u8]) -> Result<(), Reply> {
        self.state = BlockProtocolState::Done;
        
        // Append to result buffer (optional - for tracking)
        if self.result_buffer.remaining_capacity() >= data.len() {
            let _ = self.result_buffer.try_extend_from_slice(data);
        }
        
        comm.append(&[LedgerToHostCmd::ResultFinal as u8]);
        comm.append(data);
        comm.reply_ok();
        
        Ok(())
    }
    
    /// Get input parameter hashes from START command
    pub fn get_input_hashes(&self) -> &[Hash] {
        &self.input_hashes
    }
}

/// Actions resulting from processing block protocol commands
pub enum BlockAction {
    /// Command ready to process (START received)
    ProcessCommand,
    /// Chunk data received (GET_CHUNK_RESPONSE_SUCCESS)
    ChunkReceived(Vec<u8>),
    /// PUT_CHUNK acknowledged
    PutAcknowledged,
    /// RESULT_ACCUMULATING acknowledged
    ResultAcknowledged,
}

fn sha256(data: &[u8]) -> Hash {
    let mut hasher = Blake2b::new();
    hasher.update(data);
    hasher.finalize()
}
```

---

## Step 3: Implement Chunked Data Reader

**File**: `rust-app/src/block_protocol/reader.rs`

```rust
use super::{BlockProtocolHandler, Hash, HASH_LEN};
use ledger_device_sdk::io::{Comm, Reply};
use arrayvec::ArrayVec;

/// Reads chunked data via block protocol
pub struct ChunkedReader<'a> {
    protocol: &'a mut BlockProtocolHandler,
    comm: &'a mut Comm,
    current_block: Vec<u8>,
    current_offset: usize,
    next_hash: Hash,
    finished: bool,
}

impl<'a> ChunkedReader<'a> {
    /// Create reader for a parameter hash
    pub fn new(
        protocol: &'a mut BlockProtocolHandler,
        comm: &'a mut Comm,
        initial_hash: Hash,
    ) -> Self {
        ChunkedReader {
            protocol,
            comm,
            current_block: Vec::new(),
            current_offset: 0,
            next_hash: initial_hash,
            finished: false,
        }
    }
    
    /// Read exactly N bytes
    pub fn read<const N: usize>(&mut self) -> Result<[u8; N], Reply> {
        let mut result = [0u8; N];
        
        for i in 0..N {
            result[i] = self.read_byte()?;
        }
        
        Ok(result)
    }
    
    /// Read a single byte
    fn read_byte(&mut self) -> Result<u8, Reply> {
        if self.finished {
            return Err(Reply(0x6A80)); // No more data
        }
        
        // Need to fetch next block?
        if self.current_offset >= self.current_block.len() {
            self.fetch_next_block()?;
        }
        
        let byte = self.current_block[self.current_offset];
        self.current_offset += 1;
        
        Ok(byte)
    }
    
    fn fetch_next_block(&mut self) -> Result<(), Reply> {
        // Check if this is the end marker
        if self.next_hash == [0u8; HASH_LEN] {
            self.finished = true;
            return Err(Reply(0x6A80)); // End of data
        }
        
        // Request chunk from host
        self.protocol.get_chunk(self.comm, self.next_hash)?;
        
        // Wait for response (next command will be GET_CHUNK_RESPONSE)
        // This returns control to main loop
        // Main loop will call process_command() again with the response
        
        // This is where the synchronous approach gets tricky:
        // We need to return from this function and wait for next APDU
        Err(Reply(0x9000)) // OK, but need more data
    }
}
```

---

## Step 4: Command Context for Stateful Execution

The synchronous block protocol requires stateful command execution across multiple APDUs.

**File**: `rust-app/src/block_protocol/context.rs` (new)

```rust
use super::{BlockProtocolHandler, BlockAction, Hash};
use crate::interface::Ins;
use ledger_device_sdk::io::{Comm, Reply};
use arrayvec::ArrayVec;

/// Context for stateful command execution across multiple APDUs
pub struct CommandContext {
    protocol: BlockProtocolHandler,
    state: CommandState,
}

pub enum CommandState {
    /// Ready to receive START command
    Idle,
    /// Need to request first chunk
    NeedInputParam { param_index: usize },
    /// Parsing transaction data
    ParsingTransaction { buffer: Vec<u8>, bytes_needed: usize },
    /// Parsing path
    ParsingPath { buffer: Vec<u8>, bytes_needed: usize },
    /// Ready to show UI
    ReadyForUI { tx: ParsedTransaction, path: Bip32Path },
    /// User approved, ready to sign
    ReadyToSign { tx_hash: [u8; 32], path: Bip32Path },
}

impl CommandContext {
    pub fn new() -> Self {
        CommandContext {
            protocol: BlockProtocolHandler::new(),
            state: CommandState::Idle,
        }
    }
    
    /// Process incoming APDU and coordinate with block protocol
    pub fn handle_apdu(&mut self, comm: &mut Comm, ins: Ins) -> Result<(), Reply> {
        // Process block protocol command first
        let action = self.protocol.process_command(comm)?;
        
        match action {
            BlockAction::ProcessCommand => {
                // START command received, begin processing
                self.start_command(ins)?;
            }
            BlockAction::ChunkReceived(data) => {
                // Continue processing with received chunk
                self.continue_with_chunk(&data)?;
            }
            BlockAction::PutAcknowledged => {
                // Host acknowledged PUT_CHUNK
                // Continue with next operation
            }
            BlockAction::ResultAcknowledged => {
                // Host acknowledged RESULT_ACCUMULATING
                // Can send more results or RESULT_FINAL
            }
        }
        
        // Execute current state
        self.execute_state(comm)
    }
    
    fn execute_state(&mut self, comm: &mut Comm) -> Result<(), Reply> {
        match &self.state {
            CommandState::Idle => {
                // Waiting for START
                Ok(())
            }
            
            CommandState::NeedInputParam { param_index } => {
                let hash = self.protocol.get_input_hashes()[*param_index];
                self.protocol.get_chunk(comm, hash)?;
                // Return OK, wait for GET_CHUNK_RESPONSE
                Ok(())
            }
            
            CommandState::ParsingTransaction { buffer, bytes_needed } => {
                if buffer.len() >= *bytes_needed {
                    // Have enough data, parse it
                    let tx = parse_transaction(&buffer)?;
                    self.state = CommandState::ReadyForUI { tx, path: todo!() };
                    self.execute_state(comm)
                } else {
                    // Need more data, request next chunk
                    // ... (similar to above)
                    Ok(())
                }
            }
            
            CommandState::ReadyToSign { tx_hash, path } => {
                // Sign and return
                let signature = sign_ed25519(path, tx_hash)?;
                self.protocol.result_final(comm, &signature)?;
                Ok(())
            }
            
            _ => Ok(()),
        }
    }
    
    fn start_command(&mut self, ins: Ins) -> Result<(), Reply> {
        // Initialize state based on instruction
        match ins {
            Ins::GetVersion => {
                // No inputs needed, ready to respond
                self.state = CommandState::ReadyForUI { /* ... */ };
            }
            Ins::GetPubkey => {
                // Need to fetch BIP32 path (parameter 0)
                self.state = CommandState::NeedInputParam { param_index: 0 };
            }
            Ins::Sign => {
                // Need to fetch transaction data (parameter 0)
                self.state = CommandState::NeedInputParam { param_index: 0 };
            }
            _ => return Err(Reply(0x6D00)), // Instruction not supported
        }
        Ok(())
    }
    
    fn continue_with_chunk(&mut self, data: &[u8]) -> Result<(), Reply> {
        // Process received chunk based on current state
        match &mut self.state {
            CommandState::ParsingTransaction { buffer, .. } => {
                buffer.extend_from_slice(data);
            }
            CommandState::ParsingPath { buffer, .. } => {
                buffer.extend_from_slice(data);
            }
            _ => {}
        }
        Ok(())
    }
}
```

**Key features**:
- Manages `BlockProtocolHandler` instance
- Tracks command execution state across APDUs
- Coordinates between block protocol and command logic
- Handles all `BlockAction` results

---

## Step 5: Update Main Loop

**File**: `rust-app/src/main_nanos.rs`

```rust
pub fn app_main(ctx: &RunCtx) {
    let mut comm = Comm::new().set_expected_cla(0x00);
    let mut cmd_ctx = CommandContext::new();
    
    loop {
        if ctx.is_swap_finished() {
            return;
        }
        
        let evt = comm.next_event::<Ins>();
        
        match evt {
            io::Event::Command(ins) => {
                match cmd_ctx.handle_apdu(&mut comm, ins) {
                    Ok(()) => {
                        // Command processed, response already sent
                    }
                    Err(reply) => {
                        comm.reply(reply);
                    }
                }
            }
            
            io::Event::Button(btn) => {
                // Handle button presses
                handle_button(&mut comm, &mut cmd_ctx, btn);
            }
            
            io::Event::Ticker => {
                // Handle ticker
            }
        }
    }
}
```

---

## Step 6: Synchronous BCS Parsing Utilities

The Sui app uses BCS (Binary Canonical Serialization) format. We need synchronous parsers to replace `ledger-parser-combinators`.

**File**: `rust-app/src/parser/bcs_sync.rs` (new)

### ULEB128 Variable-Length Encoding

BCS uses ULEB128 for lengths and enum variants:

```rust
use ledger_device_sdk::io::Reply;

/// Read ULEB128-encoded integer from ChunkedReader
pub fn read_uleb128(reader: &mut ChunkedReader) -> Result<u64, Reply> {
    let mut result: u64 = 0;
    let mut shift = 0;
    
    loop {
        if shift >= 64 {
            return Err(Reply(0x6A80)); // Overflow
        }
        
        let byte = reader.read_byte()?;
        result |= ((byte & 0x7F) as u64) << shift;
        
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        
        shift += 7;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_uleb128() {
        // 300 = 0xAC 0x02 in ULEB128
        let data = [0xAC, 0x02];
        let mut reader = MockReader::new(&data);
        assert_eq!(read_uleb128(&mut reader).unwrap(), 300);
    }
}
```

### Fixed-Size Integers

```rust
/// Read u64 little-endian
pub fn read_u64_le(reader: &mut ChunkedReader) -> Result<u64, Reply> {
    let bytes = reader.read::<8>()?;
    Ok(u64::from_le_bytes(bytes))
}

/// Read u32 little-endian
pub fn read_u32_le(reader: &mut ChunkedReader) -> Result<u32, Reply> {
    let bytes = reader.read::<4>()?;
    Ok(u32::from_le_bytes(bytes))
}

/// Read u16 little-endian
pub fn read_u16_le(reader: &mut ChunkedReader) -> Result<u16, Reply> {
    let bytes = reader.read::<2>()?;
    Ok(u16::from_le_bytes(bytes))
}
```

### Arrays and Vectors

```rust
/// Read fixed-size array (e.g., address = [u8; 32])
pub fn read_array<const N: usize>(reader: &mut ChunkedReader) -> Result<[u8; N], Reply> {
    reader.read::<N>()
}

/// Read variable-length vector with ULEB128 length prefix
pub fn read_vec<T, F>(
    reader: &mut ChunkedReader,
    max_len: usize,
    parse_element: F,
) -> Result<ArrayVec<T, MAX>, Reply>
where
    F: Fn(&mut ChunkedReader) -> Result<T, Reply>,
{
    let len = read_uleb128(reader)? as usize;
    
    if len > max_len {
        return Err(Reply(0x6A80)); // Too many elements
    }
    
    let mut result = ArrayVec::new();
    for _ in 0..len {
        let element = parse_element(reader)?;
        result.try_push(element)
            .map_err(|_| Reply(0x6A80))?;
    }
    
    Ok(result)
}
```

### BIP32 Path Parsing

**File**: `rust-app/src/parser/bip32.rs` (new)

Replace `ledger-parser-combinators` BIP32 parser:

```rust
use arrayvec::ArrayVec;
use super::bcs_sync::{ChunkedReader, read_u32_le};
use ledger_device_sdk::io::Reply;

const MAX_BIP32_PATH_LENGTH: usize = 10;

pub fn parse_bip32_path(reader: &mut ChunkedReader) -> Result<ArrayVec<u32, MAX_BIP32_PATH_LENGTH>, Reply> {
    // BIP32 path format: 1 byte length + n * 4 bytes (u32 little-endian)
    let length = reader.read_byte()? as usize;
    
    if length > MAX_BIP32_PATH_LENGTH {
        return Err(Reply(0x6A80)); // Path too long
    }
    
    let mut path = ArrayVec::new();
    for _ in 0..length {
        let component = read_u32_le(reader)?;
        path.try_push(component)
            .map_err(|_| Reply(0x6A80))?;
    }
    
    Ok(path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bip32_path() {
        // Path: m/44'/784'/0'/0'/0'
        // Format: [5, 0x8000002C, 0x80000310, 0x80000000, 0x80000000, 0x80000000]
        let data = [
            5, // length
            0x2C, 0x00, 0x00, 0x80, // 44 | 0x80000000
            0x10, 0x03, 0x00, 0x80, // 784 | 0x80000000
            0x00, 0x00, 0x00, 0x80, // 0 | 0x80000000
            0x00, 0x00, 0x00, 0x80, // 0 | 0x80000000
            0x00, 0x00, 0x00, 0x80, // 0 | 0x80000000
        ];
        
        let mut reader = MockReader::new(&data);
        let path = parse_bip32_path(&mut reader).unwrap();
        
        assert_eq!(path.len(), 5);
        assert_eq!(path[0], 0x8000002C); // 44'
        assert_eq!(path[1], 0x80000310); // 784'
    }
}
```

### Boolean and Option Types

```rust
/// Read boolean (BCS: 0 = false, 1 = true)
pub fn read_bool(reader: &mut ChunkedReader) -> Result<bool, Reply> {
    let byte = reader.read_byte()?;
    match byte {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Reply(0x6A80)), // Invalid boolean
    }
}

/// Read Option<T> (BCS: 0 = None, 1 = Some(T))
pub fn read_option<T, F>(
    reader: &mut ChunkedReader,
    parse_value: F,
) -> Result<Option<T>, Reply>
where
    F: Fn(&mut ChunkedReader) -> Result<T, Reply>,
{
    match read_bool(reader)? {
        false => Ok(None),
        true => Ok(Some(parse_value(reader)?)),
    }
}
```

### Example: Parsing Sui Address

```rust
use ledger_crypto_helpers::hasher::{Blake2b, Hasher};

pub const SUI_ADDRESS_LENGTH: usize = 32;
pub type SuiAddress = [u8; SUI_ADDRESS_LENGTH];

pub fn parse_sui_address(reader: &mut ChunkedReader) -> Result<SuiAddress, Reply> {
    read_array::<SUI_ADDRESS_LENGTH>(reader)
}

pub fn parse_object_ref(reader: &mut ChunkedReader) -> Result<(SuiAddress, u64, [u8; 33]), Reply> {
    let object_id = parse_sui_address(reader)?;
    let sequence_number = read_u64_le(reader)?;
    let digest = read_array::<33>(reader)?; // Sha3_256Hash
    
    Ok((object_id, sequence_number, digest))
}
```

---

## Step 7: Transaction Parser Architecture

For the Sign command, we need a synchronous transaction parser to replace the 2000+ line async parser.

### Strategy: Incremental Parsing

```rust
pub enum ParsingState {
    /// Reading transaction intent (version, scope, app_id)
    ReadingIntent { bytes_read: usize },
    
    /// Reading transaction kind enum variant
    ReadingTxKind,
    
    /// Parsing programmable transaction
    ParsingProgrammableTx { commands_parsed: usize, total_commands: usize },
    
    /// Parsing gas data
    ParsingGasData { coins_read: usize },
    
    /// Complete, ready to display
    Complete { tx: ParsedTransaction },
}

pub struct TransactionParser {
    state: ParsingState,
    buffer: ArrayVec<u8, 512>, // Temporary buffer for partial data
}

impl TransactionParser {
    pub fn parse_chunk(&mut self, reader: &mut ChunkedReader) -> Result<ParseProgress, Reply> {
        match &self.state {
            ParsingState::ReadingIntent { bytes_read } => {
                // Read intent: (version, scope, app_id) - all ULEB128
                let version = read_uleb128(reader)?;
                let scope = read_uleb128(reader)?;
                let app_id = read_uleb128(reader)?;
                
                self.state = ParsingState::ReadingTxKind;
                Ok(ParseProgress::Continue)
            }
            
            ParsingState::ReadingTxKind => {
                let kind = read_uleb128(reader)?;
                
                match kind {
                    0 => self.parse_programmable_tx(reader),
                    _ => Err(Reply(0x6A80)), // Unsupported tx kind
                }
            }
            
            // ... more states
            
            ParsingState::Complete { tx } => Ok(ParseProgress::Done(tx.clone())),
        }
    }
}

pub enum ParseProgress {
    /// Need more data, call parse_chunk again
    Continue,
    /// Parsing complete
    Done(ParsedTransaction),
}
```

### Fallback: Blind Signing

Start with blind signing while building the parser:

```rust
pub fn sign_blind(reader: &mut ChunkedReader) -> Result<[u8; 32], Reply> {
    let mut hasher = Blake2b::new();
    
    // Hash all transaction bytes
    loop {
        match reader.read_byte() {
            Ok(byte) => hasher.update(&[byte]),
            Err(_) => break, // End of stream
        }
    }
    
    Ok(hasher.finalize())
}
```

Then add transaction type parsing incrementally.

---

## Step 8: Update Main Loop

**File**: `rust-app/src/main_nanos.rs`

```rust
pub fn app_main(ctx: &RunCtx) {
    let mut comm = Comm::new().set_expected_cla(0x00);
    let mut cmd_ctx = CommandContext::new();
    
    loop {
        if ctx.is_swap_finished() {
            return;
        }
        
        let evt = comm.next_event::<Ins>();
        
        match evt {
            io::Event::Command(ins) => {
                match cmd_ctx.handle_apdu(&mut comm, ins) {
                    Ok(()) => {
                        // Command processed, response already sent
                    }
                    Err(reply) => {
                        comm.reply(reply);
                    }
                }
            }
            
            io::Event::Button(btn) => {
                // Handle button presses
                handle_button(&mut comm, &mut cmd_ctx, btn);
            }
            
            io::Event::Ticker => {
                // Handle ticker
            }
        }
    }
}
```

---

## Key Challenges and Solutions

### Challenge 1: Stateful Execution

**Problem**: Block protocol requires multiple APDU exchanges for one logical command.

**Solution**: Store state in `CommandContext` between APDUs:
- Which parameter we're reading
- How much data we've received
- What operation we're performing

### Challenge 2: Incremental Parsing

**Problem**: Can't parse until all data arrives, but data comes in chunks.

**Solution**: Buffer data as it arrives:
```rust
enum ParsingState {
    ReadingLength { bytes: ArrayVec<u8, 8> },
    ReadingData { length: usize, bytes: Vec<u8> },
    Complete { parsed: Transaction },
}
```

### Challenge 3: Hash Verification (Critical!)

**Problem**: Must verify every chunk matches requested hash to prevent tampering.

**Solution**: Calculate SHA256 on received data:
```rust
let received_hash = sha256(payload);
if received_hash != requested_hash {
    return Err(Reply(0x6A80)); // Invalid data / hash mismatch
}
```

**Security**: This prevents man-in-the-middle attacks where an attacker could modify transaction data.

### Challenge 4: Result Accumulation

**Problem**: Large results (e.g., long signatures, multiple outputs) may not fit in single APDU response.

**Solution**: Use `RESULT_ACCUMULATING` for partial results:
```rust
// Send partial result
handler.result_accumulating(comm, &partial_data)?;
// Wait for RESULT_ACCUMULATING_RESPONSE
// ... more partial results if needed ...
// Send final result
handler.result_final(comm, &final_data)?;
```

**Note**: For Sui, signatures are only 64 bytes, so `RESULT_FINAL` alone is sufficient. But the protocol supports larger responses.

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_block_protocol_start() {
        let mut handler = BlockProtocolHandler::new();
        let mut comm = MockComm::new();
        
        // Send START with two parameter hashes
        let hash1 = [0x01; 32];
        let hash2 = [0x02; 32];
        comm.set_data(&[&[0x00], &hash1[..], &hash2[..]].concat());
        
        let action = handler.process_command(&mut comm).unwrap();
        assert!(matches!(action, BlockAction::ProcessCommand));
        assert_eq!(handler.get_input_hashes(), &[hash1, hash2]);
    }
    
    #[test]
    fn test_chunk_verification_success() {
        let mut handler = BlockProtocolHandler::new();
        let mut comm = MockComm::new();
        
        let data = b"test data for chunking";
        let correct_hash = sha256(data);
        
        // Request chunk
        handler.get_chunk(&mut comm, correct_hash).unwrap();
        assert!(matches!(handler.state, BlockProtocolState::WaitingChunk { .. }));
        
        // Receive chunk with correct hash
        comm.set_data(&[&[0x01], data].concat());
        let action = handler.process_command(&mut comm).unwrap();
        
        match action {
            BlockAction::ChunkReceived(received) => {
                assert_eq!(received, data);
            }
            _ => panic!("Expected ChunkReceived"),
        }
    }
    
    #[test]
    fn test_chunk_verification_failure() {
        let mut handler = BlockProtocolHandler::new();
        let mut comm = MockComm::new();
        
        let wrong_hash = [0xFF; 32];
        handler.get_chunk(&mut comm, wrong_hash).unwrap();
        
        // Receive chunk with mismatched hash
        let data = b"tampered data";
        comm.set_data(&[&[0x01], data].concat());
        
        let result = handler.process_command(&mut comm);
        assert!(result.is_err()); // Should reject tampered data
    }
    
    #[test]
    fn test_put_chunk() {
        let mut handler = BlockProtocolHandler::new();
        let mut comm = MockComm::new();
        
        let data = b"data to store";
        let hash = handler.put_chunk(&mut comm, data).unwrap();
        
        // Verify hash is correct
        assert_eq!(hash, sha256(data));
        assert!(matches!(handler.state, BlockProtocolState::WaitingPutResponse));
        
        // Host acknowledges
        comm.set_data(&[0x03]); // PUT_CHUNK_RESPONSE
        let action = handler.process_command(&mut comm).unwrap();
        assert!(matches!(action, BlockAction::PutAcknowledged));
    }
    
    #[test]
    fn test_result_accumulating() {
        let mut handler = BlockProtocolHandler::new();
        let mut comm = MockComm::new();
        
        // Send partial result
        handler.result_accumulating(&mut comm, b"part1").unwrap();
        
        // Host acknowledges
        comm.set_data(&[0x04]); // RESULT_ACCUMULATING_RESPONSE
        handler.process_command(&mut comm).unwrap();
        
        // Send another partial
        handler.result_accumulating(&mut comm, b"part2").unwrap();
        comm.set_data(&[0x04]);
        handler.process_command(&mut comm).unwrap();
        
        // Send final
        handler.result_final(&mut comm, b"final").unwrap();
        
        // Verify buffer contains all parts (if tracked)
        assert_eq!(handler.result_buffer.as_slice(), b"part1part2final");
    }
    
    #[test]
    fn test_get_chunk_response_failure() {
        let mut handler = BlockProtocolHandler::new();
        let mut comm = MockComm::new();
        
        let hash = [0xAA; 32];
        handler.get_chunk(&mut comm, hash).unwrap();
        
        // Host responds with failure
        comm.set_data(&[0x02]); // GET_CHUNK_RESPONSE_FAILURE
        let result = handler.process_command(&mut comm);
        
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, 0x6A88); // Referenced data not found
    }
}
```

### Integration Tests

Test with real block protocol exchanges using Speculos.

---

## Migration Checklist

### Block Protocol Implementation
- [ ] Implement `BlockProtocolHandler` state machine
  - [ ] `START` command handler (parse parameter hashes)
  - [ ] `GET_CHUNK_RESPONSE_SUCCESS` handler (receive + verify data)
  - [ ] `GET_CHUNK_RESPONSE_FAILURE` handler (handle missing data)
  - [ ] `PUT_CHUNK_RESPONSE` handler (acknowledge storage)
  - [ ] `RESULT_ACCUMULATING_RESPONSE` handler (acknowledge partial result)
  - [ ] `GET_CHUNK` sender (request data by hash)
  - [ ] `PUT_CHUNK` sender (store data on host)
  - [ ] `RESULT_ACCUMULATING` sender (send partial result)
  - [ ] `RESULT_FINAL` sender (send final result)
  - [ ] State validation (reject invalid transitions)
  - [ ] SHA256 hash verification (security-critical!)

### Data Reading & Command Framework
- [ ] Implement `ChunkedReader` for incremental data reading
- [ ] Implement `CommandContext` for stateful execution
- [ ] Update main loop to use new block protocol

### Command Migration
- [ ] Port GetVersion command
- [ ] Port GetPubkey command
- [ ] Port Sign command (most complex)

### Testing
- [ ] Test all commands with existing clients
- [ ] Test all 9 block protocol commands individually
- [ ] Test hash verification (tampered data should be rejected)
- [ ] Test error cases (FAILURE responses, invalid states)
- [ ] Test PUT_CHUNK / RESULT_ACCUMULATING (if used)
- [ ] Performance testing
- [ ] Test on all device types (Nano S+, X, Stax, Flex, Apex P)

### Cleanup
- [ ] Remove `alamgu_async_block` from `Cargo.toml`
- [ ] Verify app still compiles and runs
- [ ] Final integration test with real Sui Wallet

---

## Estimated Timeline

- **Week 1**: Block protocol core implementation
  - Day 1-2: State machine + all 9 command handlers
  - Day 3-4: ChunkedReader + comprehensive unit tests
  - Day 5: Integration testing, hash verification validation
- **Week 2**: Port simple commands (GetVersion, GetPubkey)
- **Week 3**: Port Sign command + transaction parser
- **Week 4**: Testing, debugging, client compatibility verification

---

## Security Considerations

1. **Hash Verification**: Critical for preventing data tampering - verify EVERY chunk
2. **State Validation**: Ensure state transitions are valid - reject unexpected commands
3. **Buffer Limits**: Prevent DoS via excessive data - enforce maximum sizes
4. **Error Handling**: Don't leak sensitive info in errors - use generic error codes
5. **Chunk Tampering**: Test that modified chunks are detected and rejected

**Test Attack Scenarios**:
- Modified chunk data (hash mismatch should be detected)
- Out-of-order commands (invalid state transitions)
- Excessive data (buffer overflow attempts)
- Missing chunks (GET_CHUNK_RESPONSE_FAILURE handling)

---

## Success Criteria

✅ All 9 block protocol commands work identically to before
✅ Existing Sui Wallet can sign transactions
✅ Hash verification prevents tampered data (tested!)
✅ All state transitions are valid (no crashes on unexpected commands)
✅ All tests pass
✅ No `alamgu_async_block` dependency in `Cargo.toml`
