#![cfg_attr(target_family = "bolos", no_std)]
#![cfg_attr(target_family = "bolos", no_main)]

#[cfg(not(target_family = "bolos"))]
fn main() {}

use sui::main_nanos::*;

ledger_device_sdk::set_panic!(ledger_device_sdk::exiting_panic);

#[no_mangle]
extern "C" fn sample_main(arg0: u32) {
    if arg0 == 0 {
        app_main();
    } else {
        lib_main(arg0);
    }
}
