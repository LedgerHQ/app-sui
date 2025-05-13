use core::panic::PanicInfo;

use ledger_device_sdk::io;

static mut SWAP_PANIC_HANDLER: Option<fn(&PanicInfo) -> !> = None;

pub fn get_swap_panic_handler() -> Option<fn(&PanicInfo) -> !> {
    unsafe { SWAP_PANIC_HANDLER }
}

// Set the panic handler for the swap app
// SAFETY: should be used only in lib swap call, after app is initialized
pub(crate) unsafe fn set_swap_panic_handler(handler: fn(&PanicInfo) -> !) {
    unsafe {
        SWAP_PANIC_HANDLER = Some(handler);
    }
}

pub(crate) fn swap_panic_handler(_info: &PanicInfo) -> ! {
    ledger_log::error!("Swap panic happened! {:#?}", _info);

    let mut comm = io::Comm::new();
    comm.swap_reply(io::StatusWords::Panic);

    unsafe { ledger_secure_sdk_sys::os_lib_end() }
}
