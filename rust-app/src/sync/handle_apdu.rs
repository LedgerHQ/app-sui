use crate::interface::*;
use crate::sync::ctx::block_protocol::{LedgerToHostCmd, Output, State};
use crate::sync::ctx::RunCtx;
use crate::sync::implementation::get_address;

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
                Output::WaitingChunk { requested_hash } => {
                    trace!("Requesting chunk");
                    ctx.comm.append(&[LedgerToHostCmd::GetChunk as u8]);
                    ctx.comm.append(requested_hash);
                }
                Output::WholeChunkReceived { data } => {
                    trace!("Getting address");
                    match get_address(&mut ctx.ui, &data, ins == Ins::VerifyAddress) {
                        Ok(address) => {
                            ctx.block_protocol_handler.reset();
                            ctx.comm.append(&[LedgerToHostCmd::ResultFinal as u8]);
                            ctx.comm.append(&address);
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
            //sign_apdu(io, ctx, settings, ui);
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
