pub mod common;
pub mod ed25519;
pub mod eddsa;
pub mod hasher;

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
