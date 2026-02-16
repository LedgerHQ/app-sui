#![cfg_attr(target_family = "bolos", no_std)]
#![cfg_attr(target_family = "bolos", no_main)]

#[cfg(not(target_family = "bolos"))]
fn main() {}

#[cfg(all(target_family = "bolos", not(feature = "sync")))]
use sui::app_main::*;

#[cfg(all(target_family = "bolos", feature = "sync"))]
use sui::sync::app_main::*;

#[cfg(all(target_family = "bolos", not(feature = "sync")))]
use sui::{
    ctx::RunCtx,
    swap::{lib_main, panic_handler::get_swap_panic_handler},
};

#[cfg(all(target_family = "bolos", feature = "sync"))]
use sui::sync::ctx::RunCtx;

#[cfg(all(target_family = "bolos", not(feature = "sync")))]
pub fn custom_panic(info: &PanicInfo) -> ! {
    use ledger_device_sdk::io;
    if let Some(swap_panic_handler) = get_swap_panic_handler() {
        // This handler is no-return
        swap_panic_handler(info);
    }
    ledger_device_sdk::log::error!("Panic happened! {:#?}", info);
    let mut comm = io::Comm::new();
    comm.reply(io::StatusWords::Panic);
    ledger_device_sdk::sys::exit_app(0);
}

#[cfg(all(target_family = "bolos", feature = "sync"))]
pub fn custom_panic(info: &PanicInfo) -> ! {
    use ledger_device_sdk::io;
    ledger_device_sdk::log::error!("Panic happened! {:#?}", info);
    let mut comm = io::Comm::new();
    comm.reply(io::StatusWords::Panic);
    ledger_device_sdk::sys::exit_app(0);
}

ledger_device_sdk::set_panic!(custom_panic);

#[no_mangle]
#[cfg(all(target_family = "bolos", not(feature = "sync")))]
extern "C" fn sample_main(arg0: u32) {
    if arg0 == 0 {
        app_main(&RunCtx::app());
    } else {
        lib_main(arg0);
    }
}

#[no_mangle]
#[cfg(all(target_family = "bolos", feature = "sync"))]
extern "C" fn sample_main(_arg0: u32) {
    app_main();
}
