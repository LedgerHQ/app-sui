use arrayvec::ArrayVec;
use ledger_device_sdk::{hash::sha2::Sha2_256, hash::HashInit};

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
    type Error = BlockProtocolError;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Start),
            1 => Ok(Self::GetChunkResponseSuccess),
            2 => Ok(Self::GetChunkResponseFailure),
            3 => Ok(Self::PutChunkResponse),
            4 => Ok(Self::ResultAccumulatingResponse),
            _ => Err(BlockProtocolError::InvalidCommand),
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

/// State of block protocol
#[derive(Debug)]
pub enum Output {
    NoAction,
    /// Waiting for GET_CHUNK_RESPONSE
    WaitingChunk,
    /// Waiting for PUT_CHUNK_RESPONSE
    WaitingPutResponse,
    /// Waiting for RESULT_ACCUMULATING_RESPONSE
    WaitingResultResponse,
    /// All chunks received, full data available
    WholeChunkReceived,
}

#[derive(Default, Debug, PartialEq)]
pub enum State {
    #[default]
    Idle,
    Processing,
}

#[derive(Debug, Default)]
pub struct BlockProtocolHandler {
    pub state: State,
    /// Input parameter hashes from START command
    /// 3 maximum hashes
    pub input_hashes: ArrayVec<Hash, 3>,
    /// Currently requested hash for chunk retrieval
    pub requested_hash: Hash,
    /// Accumulated result data
    pub result: Vec<u8>,
    /// Buffer for LedgerToHost responses
    pub response_buffer: ArrayVec<u8, 250>,
}

pub enum BlockProtocolError {
    EmptyData,
    InvalidCommand,
    HashMismatch,
}

impl BlockProtocolHandler {
    pub fn reset(&mut self) {
        self.state = State::Idle;
        self.input_hashes.clear();
        self.requested_hash = [0u8; HASH_LEN];
        self.result.clear();
        self.response_buffer.clear();
    }

    /// Process incoming data and return next action
    pub fn process_data(&mut self, data_in: &[u8]) -> Result<Output, BlockProtocolError> {
        if data_in.is_empty() {
            return Err(BlockProtocolError::EmptyData);
        }

        let in_cmd = HostToLedgerCmd::try_from(data_in[0])?;
        let in_payload = &data_in[1..];

        match in_cmd {
            HostToLedgerCmd::Start => {
                if self.state != State::Idle {
                    return Err(BlockProtocolError::InvalidCommand);
                }
                self.state = State::Processing;
                self.input_hashes.clear();
                self.result.clear();
                self.response_buffer.clear();

                if in_payload.len() % HASH_LEN != 0 {
                    return Err(BlockProtocolError::InvalidCommand);
                }

                for chunk in in_payload.chunks_exact(HASH_LEN) {
                    let hash: Hash = chunk.try_into().unwrap();
                    self.input_hashes
                        .try_push(hash)
                        .map_err(|_| BlockProtocolError::InvalidCommand)?;
                }
                self.requested_hash = self.input_hashes[0];
                Ok(Output::WaitingChunk)
            }
            HostToLedgerCmd::GetChunkResponseSuccess => {
                let received_hash = sha256(in_payload);
                if received_hash != self.requested_hash {
                    ledger_device_sdk::log::info!(
                        "Hash mismatch: expected {:x?}, got {:x?}",
                        self.requested_hash,
                        received_hash
                    );
                    return Err(BlockProtocolError::HashMismatch); // Invalid data / hash mismatch
                }
                self.requested_hash = in_payload[..HASH_LEN]
                    .try_into()
                    .map_err(|_| BlockProtocolError::InvalidCommand)?;
                self.result.extend_from_slice(&in_payload[HASH_LEN..]);
                if self.requested_hash == [0u8; HASH_LEN] {
                    Ok(Output::WholeChunkReceived)
                } else {
                    Ok(Output::WaitingChunk)
                }
            }
            HostToLedgerCmd::GetChunkResponseFailure => Ok(Output::NoAction),
            HostToLedgerCmd::PutChunkResponse => Ok(Output::NoAction),
            HostToLedgerCmd::ResultAccumulatingResponse => Ok(Output::NoAction),
        }
    }
}

//     /// Request a chunk from host (by hash)
//     // pub fn get_chunk(&mut self, hash: Hash) -> Result<(), Reply> {
//     //     self.state = BlockProtocolState::WaitingChunk {
//     //         requested_hash: hash,
//     //     };

//     //     self.comm.append(&[LedgerToHostCmd::GetChunk as u8]);
//     //     self.comm.append(&hash);
//     //     self.comm.reply_ok();

//     //     Ok(())
//     // }

//     /// Store chunk on host (returns its hash for later retrieval)
//     // pub fn put_chunk(&mut self, data: &[u8]) -> Result<Hash, Reply> {
//     //     let hash = sha256(data);

//     //     self.state = BlockProtocolState::WaitingPutResponse;

//     //     self.comm.append(&[LedgerToHostCmd::PutChunk as u8]);
//     //     self.comm.append(data);
//     //     self.comm.reply_ok();

//     //     Ok(hash)
//     // }

//     /// Send partial result (can be called multiple times)
//     // pub fn result_accumulating(&mut self, data: &[u8]) -> Result<(), Reply> {
//     //     self.state = BlockProtocolState::WaitingResultResponse;

//     //     // Store in buffer (optional - for tracking)
//     //     if self.result_buffer.remaining_capacity() >= data.len() {
//     //         let _ = self.result_buffer.try_extend_from_slice(data);
//     //     }

//     //     self.comm.append(&[LedgerToHostCmd::ResultAccumulating as u8]);
//     //     self.comm.append(data);
//     //     self.comm.reply_ok();

//     //     Ok(())
//     // }

//     /// Send final result (completes the transaction)
//     // pub fn result_final(&mut self, data: &[u8]) -> Result<(), Reply> {
//     //     self.state = BlockProtocolState::Idle;

//     //     // Append to result buffer (optional - for tracking)
//     //     if self.result_buffer.remaining_capacity() >= data.len() {
//     //         let _ = self.result_buffer.try_extend_from_slice(data);
//     //     }

//     //     self.comm.append(&[LedgerToHostCmd::ResultFinal as u8]);
//     //     self.comm.append(data);
//     //     self.comm.reply_ok();

//     //     Ok(())
//     // }

//     /// Get input parameter hashes from START command
//     // pub fn get_input_hashes(&self) -> &[Hash] {
//     //     &self.input_hashes
//     // }
// }

// /// Actions resulting from processing block protocol commands
// // #[derive(Debug)]
// // pub enum BlockAction {
// //     /// Instruction ready to process (START received)
// //     StartIns,
// //     /// Chunk data received (GET_CHUNK_RESPONSE_SUCCESS)
// //     ChunkReceived(Vec<u8>),
// //     /// PUT_CHUNK acknowledged
// //     PutAcknowledged,
// //     /// RESULT_ACCUMULATING acknowledged
// //     ResultAcknowledged,
// // }

fn sha256(data: &[u8]) -> Hash {
    let mut hasher = Sha2_256::new();
    let _ = hasher.update(data);
    let mut output: [u8; HASH_LEN] = [0; HASH_LEN];
    let _ = hasher.finalize(&mut output);
    output
}
