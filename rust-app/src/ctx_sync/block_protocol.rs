use ledger_device_sdk::io::{Comm, Reply, StatusWords};
use ledger_crypto_helpers::hasher::{SHA256, Hasher};
use arrayvec::ArrayVec;

extern crate alloc;
use alloc::vec::Vec;

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
}

pub struct BlockProtocolHandler {
    state: BlockProtocolState,
    /// Input parameter hashes from START command
    input_hashes: ArrayVec<Hash, 3>,
    /// Accumulated result data (for RESULT_ACCUMULATING)
    result_buffer: ArrayVec<u8, 256>,
}

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
                Ok(BlockAction::StartIns)
            }
            
            // GET_CHUNK_RESPONSE_SUCCESS: Receive chunk data
            (BlockProtocolState::WaitingChunk { requested_hash }, 
             HostToLedgerCmd::GetChunkResponseSuccess) => {
                // Verify hash of received data
                let received_hash = sha256(payload);
                if &received_hash != requested_hash {
                    ledger_log::info!("Hash mismatch: expected {:x?}, got {:x?}", requested_hash, received_hash);
                    return Err(Reply(0x6A80)); // Invalid data / hash mismatch
                }
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
        self.state = BlockProtocolState::Idle;
        
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
#[derive(Debug)]
pub enum BlockAction {
    /// Instruction ready to process (START received)
    StartIns,
    /// Chunk data received (GET_CHUNK_RESPONSE_SUCCESS)
    ChunkReceived(Vec<u8>),
    /// PUT_CHUNK acknowledged
    PutAcknowledged,
    /// RESULT_ACCUMULATING acknowledged
    ResultAcknowledged,
}

fn sha256(data: &[u8]) -> Hash {
    let mut hasher = SHA256::new();
    hasher.update(data);
    hasher.finalize()
}