#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![reexport_test_harness_main = "test_main"]
#![test_runner(ledger_device_sdk::testing::sdk_test_runner)]
#![feature(generic_const_exprs)]
#![cfg_attr(test, feature(asm_const))]
#![feature(cfg_version)]

pub mod common;
pub mod hasher;
#[macro_use]
mod internal;
pub mod ed25519;
pub mod eddsa;

#[cfg(test)]
#[no_mangle]
fn sample_main() {
    test_main();
}

#[cfg(all(target_family = "bolos", test))]
mod test {
    use core::panic::PanicInfo;

    #[cfg_attr(not(version("1.64")), allow(unused))]
    const RELOC_SIZE: usize = 3500;

    ::core::arch::global_asm! {
        ".global _reloc_size",
        ".set _reloc_size, {reloc_size}",
        reloc_size = const RELOC_SIZE,
    }

    #[panic_handler]
    pub fn test_panic(info: &PanicInfo) -> ! {
        ledger_device_sdk::testing::test_panic(info)
    }
}
