use crate::implementation::*;
use crate::interface::*;
use crate::settings::*;
#[cfg(not(any(target_os = "stax", target_os = "flex")))]
use crate::test_parsers::*;
use crate::utils::*;

use alamgu_async_block::*;
use arrayvec::ArrayVec;
use core::future::Future;
use ledger_log::trace;

pub type APDUsFuture = impl Future<Output = ()>;

#[inline(never)]
pub fn handle_apdu_async(io: HostIO, ins: Ins, settings: Settings) -> APDUsFuture {
    trace!("Constructing future");
    async move {
        trace!("Dispatching");
        match ins {
            Ins::GetVersion => {
                const APP_NAME: &str = "sui";
                let mut rv = ArrayVec::<u8, 220>::new();
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap());
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_MINOR").parse().unwrap());
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_PATCH").parse().unwrap());
                let _ = rv.try_extend_from_slice(APP_NAME.as_bytes());
                io.result_final(&rv).await;
            }
            Ins::VerifyAddress => {
                NoinlineFut(get_address_apdu(io, true)).await;
            }
            Ins::GetPubkey => {
                NoinlineFut(get_address_apdu(io, false)).await;
            }
            Ins::Sign => {
                trace!("Handling sign");
                NoinlineFut(sign_apdu(io, settings)).await;
            }
            Ins::GetVersionStr => {}
            Ins::Exit => ledger_device_sdk::exit_app(0),
        }
    }
}

// We are single-threaded in fact, albeit with nontrivial code flow. We don't need to worry about
// full atomicity of the below globals.
pub struct SingleThreaded<T>(pub T);
unsafe impl<T> Send for SingleThreaded<T> {}
unsafe impl<T> Sync for SingleThreaded<T> {}
impl<T> core::ops::Deref for SingleThreaded<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T> core::ops::DerefMut for SingleThreaded<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
