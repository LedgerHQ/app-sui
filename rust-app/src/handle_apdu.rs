use crate::ctx::RunCtx;
use crate::implementation::*;
use crate::interface::*;
use crate::settings::*;
use crate::ui::UserInterface;
use crate::utils::*;

use alamgu_async_block::*;
use arrayvec::ArrayVec;
use core::future::Future;
use ledger_log::trace;

pub type APDUsFuture<'ctx> = impl Future<Output = ()> + 'ctx;

#[inline(never)]
pub fn handle_apdu_async(
    io: HostIO,
    ins: Ins,
    ctx: &RunCtx,
    settings: Settings,
    ui: UserInterface,
) -> APDUsFuture {
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
                NoinlineFut(get_address_apdu(io, ui, true)).await;
            }
            Ins::GetPubkey => {
                NoinlineFut(get_address_apdu(io, ui, false)).await;
            }
            Ins::Sign => {
                trace!("Handling sign");
                NoinlineFut(sign_apdu(io, ctx, settings, ui)).await;
            }
            Ins::GetVersionStr => {}
            Ins::Exit if ctx.is_swap() => unsafe { ledger_secure_sdk_sys::os_lib_end() },
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
