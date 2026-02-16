use crate::interface::*;
use crate::sync::ctx::block_protocol::{LedgerToHostCmd, Output};
use crate::sync::ctx::RunCtx;
use crate::sync::implementation::get_address;
use crate::sync::parser::tx_state_machine::TxParser;

use arrayvec::ArrayVec;
use ledger_device_sdk::{
    io::StatusWords,
    log::{error, trace},
};

pub fn handle_apdu(ctx: &mut RunCtx, ins: Ins) -> Result<(), StatusWords> {
    trace!("Dispatching");
    let data = match ctx.comm.get_data() {
        Ok(d) => d,
        Err(_e) => return Err(StatusWords::NothingReceived),
    };
    let output = ctx
        .block_protocol_handler
        .process_data(&data)
        .map_err(|_| StatusWords::Unknown)?;
    match ins {
        Ins::GetVersion => {
            trace!("Handling get version");
            const APP_NAME: &str = "sui";
            let mut rv = ArrayVec::<u8, 6>::new();
            let _ = rv.try_push(env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap());
            let _ = rv.try_push(env!("CARGO_PKG_VERSION_MINOR").parse().unwrap());
            let _ = rv.try_push(env!("CARGO_PKG_VERSION_PATCH").parse().unwrap());
            let _ = rv.try_extend_from_slice(APP_NAME.as_bytes());
            ctx.comm.append(&[LedgerToHostCmd::ResultFinal as u8]);
            ctx.comm.append(&rv);
            Ok(())
        }
        Ins::VerifyAddress | Ins::GetPubkey => {
            trace!("Handling verify address");
            match output {
                Output::WaitingChunk => {
                    trace!("Requesting chunk");
                    ctx.comm.append(&[LedgerToHostCmd::GetChunk as u8]);
                    ctx.comm
                        .append(ctx.block_protocol_handler.requested_hash.as_slice());
                }
                Output::WholeChunkReceived => {
                    trace!("Getting address");
                    match get_address(
                        &mut ctx.ui,
                        &ctx.block_protocol_handler.result,
                        ins == Ins::VerifyAddress,
                    ) {
                        Ok(address) => {
                            ctx.comm.append(&[LedgerToHostCmd::ResultFinal as u8]);
                            ctx.comm.append(&address);
                            ctx.block_protocol_handler.reset();
                        }
                        Err(e) => {
                            error!("Error getting address");
                            ctx.block_protocol_handler.reset();
                            return Err(e);
                        }
                    }
                }
                _ => {
                    trace!("Unhandled state");
                    // Handle other states if necessary
                }
            }
            Ok(())
        }
        Ins::Sign => {
            trace!("Handling sign");
            match output {
                Output::WaitingChunk => {
                    trace!(
                        "Requesting chunk, current data length {}",
                        ctx.block_protocol_handler.result.len()
                    );
                    ctx.comm.append(&[LedgerToHostCmd::GetChunk as u8]);
                    ctx.comm
                        .append(ctx.block_protocol_handler.requested_hash.as_slice());
                }
                Output::WholeChunkReceived => {
                    trace!(
                        "Signing Tx length {}",
                        ctx.block_protocol_handler.result.len()
                    );
                    
                    // Log first 16 bytes for debugging
                    if ctx.block_protocol_handler.result.len() >= 16 {
                        trace!("First 16 bytes: {:02x?}", &ctx.block_protocol_handler.result[..16]);
                    }
                    
                    // Check if there's a length prefix (usize little-endian)
                    // like in the async implementation
                    // On ARM32 (Ledger devices), usize is 4 bytes
                    const LEN_PREFIX_SIZE: usize = core::mem::size_of::<usize>();
                    
                    let tx_data = if ctx.block_protocol_handler.result.len() >= LEN_PREFIX_SIZE {
                        // Try reading length prefix
                        let declared_length = if LEN_PREFIX_SIZE == 4 {
                            let mut bytes = [0u8; 4];
                            bytes.copy_from_slice(&ctx.block_protocol_handler.result[..4]);
                            u32::from_le_bytes(bytes) as usize
                        } else {
                            let mut bytes = [0u8; 8];
                            bytes.copy_from_slice(&ctx.block_protocol_handler.result[..8]);
                            u64::from_le_bytes(bytes) as usize
                        };
                        
                        trace!("Potential length prefix: {}", declared_length);
                        
                        // If the declared length matches the remaining data, skip the prefix
                        if declared_length + LEN_PREFIX_SIZE == ctx.block_protocol_handler.result.len() {
                            trace!("Skipping {}-byte length prefix", LEN_PREFIX_SIZE);
                            &ctx.block_protocol_handler.result[LEN_PREFIX_SIZE..]
                        } else {
                            trace!("No length prefix detected, parsing from start");
                            &ctx.block_protocol_handler.result[..]
                        }
                    } else {
                        &ctx.block_protocol_handler.result[..]
                    };
                    
                    //Parse transaction with state machine
                    let mut parser = TxParser::new();
                    match parser.feed(tx_data) {
                        Ok(_consumed) => {
                            trace!("Parsed {} bytes", _consumed);
                            if parser.is_complete() {
                                trace!("Transaction parsing complete");
                                trace!("Intent: {:?}", parser.intent);
                                trace!("Sender: {:?}", parser.sender);
                                trace!("Gas price: {:?}, budget: {:?}", parser.gas_price, parser.gas_budget);
                                trace!("Inputs: {} commands: {}", parser.inputs.len(), parser.commands.len());
                                
                                // TODO: Display transaction details to user and sign
                                // For now, just acknowledge receipt
                                ctx.comm.append(&[LedgerToHostCmd::ResultFinal as u8]);
                                ctx.comm.append(&[0x90, 0x00]); // Success
                            } else {
                                error!("Transaction parsing incomplete");
                                return Err(StatusWords::BadLen);
                            }
                        }
                        Err(_e) => {
                            error!("Transaction parsing failed");
                            return Err(StatusWords::Unknown);
                        }
                    }
                    
                    ctx.block_protocol_handler.reset();
                }
                _ => {
                    trace!("Unhandled state");
                    // Handle other states if necessary
                }
            }
            Ok(())
        }
        Ins::ProvideTrustedDynamicDescriptor => {
            trace!("Handling provide trusted dynamic descriptor");
            //validate_tlv(io, ctx);
            Ok(())
        }
        Ins::GetVersionStr => Ok(()),
        //Ins::Exit if ctx.is_swap() => unsafe { ledger_device_sdk::sys::os_lib_end() },
        Ins::Exit => ledger_device_sdk::exit_app(0),
    }
}
