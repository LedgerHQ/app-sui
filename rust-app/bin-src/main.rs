#![cfg_attr(target_family = "bolos", no_std)]
#![cfg_attr(target_family = "bolos", no_main)]

#[cfg(not(target_family = "bolos"))]
fn main() {}

use sui::app_main::*;

use sui::{
    ctx::RunCtx,
    swap::{
        lib_main,
        panic_handler::{is_swap_panic_armed, swap_panic},
    },
};

pub fn custom_panic(info: &PanicInfo) -> ! {
    use ledger_device_sdk::io;
    // A direct call to a compile-time-known function. The armed flag is only
    // compared, so caller-owned shared .bss can never select the target.
    if is_swap_panic_armed() {
        // No return.
        swap_panic(info);
    }
    ledger_device_sdk::log::error!("Panic happened! {:#?}", info);
    let mut comm = io::Comm::new();
    comm.reply(io::StatusWords::Panic);
    ledger_device_sdk::sys::exit_app(0);
}

ledger_device_sdk::set_panic!(custom_panic);

#[no_mangle]
extern "C" fn sample_main(arg0: u32) {
    if arg0 == 0 {
        app_main(&RunCtx::app());
    } else {
        lib_main(arg0);
    }
}
