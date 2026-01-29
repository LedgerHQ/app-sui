use crate::interface::*;
use crate::sync::ctx::block_protocol::{LedgerToHostCmd, State};
use crate::sync::ctx::RunCtx;

use arrayvec::ArrayVec;
use ledger_device_sdk::log::trace;

pub enum IOError {
    APDUError,
}

pub fn handle_apdu(ctx: &mut RunCtx, ins: Ins) -> Result<(), IOError> {
    trace!("Dispatching");
    let data = match ctx.comm.get_data() {
        Ok(d) => d,
        Err(_e) => return Err(IOError::APDUError),
    };
    let state = ctx
        .block_protocol_handler
        .process_data(&data)
        .map_err(|_| IOError::APDUError)?;
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
            match state {
                State::WaitingChunk { requested_hash } => {
                    ctx.comm.append(&[LedgerToHostCmd::GetChunk as u8]);
                    ctx.comm.append(&requested_hash);
                }
                State::WholeChunkReceived { data: _ } => {}
                _ => {}
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
