use crate::{interface::Ins, parser::tx::KnownTx};
use ledger_device_sdk::io::{Comm, Reply};
use arrayvec::ArrayVec;

extern crate alloc;
use alloc::vec::Vec;

pub mod block_protocol;
use block_protocol::{BlockAction, BlockProtocolHandler};

/// Context for stateful instruction execution across multiple APDUs
pub struct Context {
    protocol: BlockProtocolHandler,
    state: State,
}

pub enum State {
    /// Ready to receive START instruction
    Idle,
    /// AppInfo
    AppInfo,
    /// Need to request first chunk
    NeedInputParam { param_index: usize },
    /// Parsing transaction data
    ParsingTransaction { buffer: Vec<u8>, bytes_needed: usize },
    /// Computing public key
    ComputingPublicKey { buffer: Vec<u8> },
    /// Ready to show clear signing UI
    ReadyForUI { tx: KnownTx, path: ArrayVec<u32, 10> },
    /// User approved, ready to sign
    ReadyToSign { tx_hash: [u8; 32], path: ArrayVec<u32, 10> },
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn new() -> Self {
        Context {
            protocol: BlockProtocolHandler::new(),
            state: State::Idle,
        }
    }

    /// Process incoming APDU and coordinate with block protocol
    pub fn handle_apdu(&mut self, comm: &mut Comm, ins: Ins) -> Result<(), Reply> {
        // Process block protocol command first
        let action = self.protocol.process_command(comm)?;
        
        ledger_log::info!("Block protocol action: {:x?}", action);
        match action {
            BlockAction::StartIns => {
                // START command received, begin processing
                ledger_log::info!("Starting instruction processing for {:?}", ins);
                self.start(ins)?;
            }
            BlockAction::ChunkReceived(chunk) => {
                // Continue processing with received chunk
                ledger_log::info!("Received chunk of size {}", chunk.len());
                self.process_chunk(ins, &chunk)?;
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
        // Handle current state
        self.handle_state(comm)
    }
    
    fn handle_state(&mut self, comm: &mut Comm) -> Result<(), Reply> {
        match &self.state {

            State::Idle => {
                // Waiting for START
                Ok(())
            }

            State::AppInfo => {
                let mut rv = ArrayVec::<u8, 220>::new();
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap());
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_MINOR").parse().unwrap());
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_PATCH").parse().unwrap());
                const APP_NAME: &str = "sui";
                let _ = rv.try_extend_from_slice(APP_NAME.as_bytes());
                self.protocol.result_final(comm, &rv)?;
                Ok(())
            }

            State::NeedInputParam { param_index } => {
                let hash = self.protocol.get_input_hashes()[*param_index];
                self.protocol.get_chunk(comm, hash)?;
                // Return OK, wait for GET_CHUNK_RESPONSE
                Ok(())
            }

            State::ComputingPublicKey { buffer } => {
                crate::implementation::get_address_apdu_sync(&mut self.protocol, comm, buffer, false);
                Ok(())
            }
            
            State::ParsingTransaction { buffer, bytes_needed } => {
                if buffer.len() >= *bytes_needed {
                    // Have enough data, parse it
                    let _tx = KnownTx::TransferTx {
                        recipient: [0u8; 32],
                        coin_type: (crate::parser::common::SUI_COIN_ID, ArrayVec::new(), ArrayVec::new()),
                        total_amount: 0,
                        gas_budget: 0,
                    };//parse_transaction(&buffer)?;
                    self.state = State::ReadyForUI { 
                        tx: KnownTx::TransferTx { recipient: [0u8; 32], coin_type: (crate::parser::common::SUI_COIN_ID, ArrayVec::new(), ArrayVec::new()), total_amount: 0, gas_budget: 0 }, path: ArrayVec::new() };
                    self.handle_state(comm)
                } else {
                    // Need more data, request next chunk
                    // ... (similar to above)
                    Ok(())
                }
            }
            
            State::ReadyToSign { tx_hash: _, path: _ } => {
                // Sign and return
                let signature = [0u8; 32];//sign_ed25519(path, tx_hash)?;
                self.protocol.result_final(comm, &signature)?;
                Ok(())
            }
            
            _ => Ok(()),
        }
    }
    
    fn start(&mut self, ins: Ins) -> Result<(), Reply> {
        // Initialize state based on instruction
        match ins {
            Ins::GetVersion => {
                self.state = State::AppInfo;
            }
            Ins::GetPubkey => {
                // Need to fetch BIP32 path (parameter 0)
                self.state = State::NeedInputParam { param_index: 0 };
            }
            Ins::Sign => {
                // Need to fetch transaction data (parameter 0)
                self.state = State::NeedInputParam { param_index: 0 };
            }
            _ => return Err(Reply(0x6D00)), // Instruction not supported
        }
        Ok(())
    }
    
    fn process_chunk(&mut self, ins: Ins, data: &[u8]) -> Result<(), Reply> {
        match (ins, &self.state) {
            (Ins::GetPubkey, State::NeedInputParam { .. }) => {
                // Received BIP32 path
                ledger_log::info!("Received BIP32 path chunk");

                self.state = State::ComputingPublicKey{ buffer: data[32..].to_vec() };
            }
            // (Ins::Sign, CommandState::NeedInputParam { param_index }) if *param_index == 0 => {
            //     // Received transaction data
            //     let bytes_needed = 256; // Example fixed size, should be parsed from header
            //     self.state = CommandState::ParsingTransaction { buffer: data.to_vec(), bytes_needed };
            // }
            _ => return Err(Reply(0x6A80)), // Unexpected state
        }

        // Process received chunk based on current state
        // match (ins, &mut self.state) {
        //     (Ins::Sign, CommandState::ParsingTransaction { buffer, .. }) => {
        //         buffer.extend_from_slice(data);
        //     }
        //     (Ins::GetPubkey, CommandState::ParsingPath { buffer }) => {
        //         buffer.extend_from_slice(data);
        //     }
        //     _ => {}
        // }
        Ok(())
    }
}