#![cfg_attr(target_family = "bolos", no_std)]
#![cfg_attr(target_family = "bolos", no_main)]

#[cfg(not(target_family = "bolos"))]
fn main() {}

#[cfg(not(any(target_os = "stax", target_os = "flex")))]
use sui::main_nanos::*;

#[cfg(any(target_os = "stax", target_os = "flex"))]
use sui::main_stax::*;

ledger_device_sdk::set_panic!(ledger_device_sdk::exiting_panic);

#[no_mangle]
extern "C" fn sample_main() {
    app_main()
}
