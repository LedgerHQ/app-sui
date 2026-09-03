use core::panic::PanicInfo;

use ledger_device_sdk::io;

/// Marks that a swap signing libcall is in progress and panics should hand control
/// back to the caller.
///
/// This must never be a function pointer. Libcall startup deliberately does not
/// clear the app's `.bss` -- it is shared with, and holds live state from, the
/// calling Exchange app -- so anything read here during `SwapCheckAddress` or
/// `SwapGetPrintableAmount` is the caller's RAM, not ours. An `Option<fn>` made that
/// directly callable: its `None` is all-zero, so any nonzero bytes decoded as
/// `Some(pointer)` and were invoked, on a target with neither ASLR nor
/// control-flow integrity.
///
/// A plain word compared against a constant cannot become a branch target. Two
/// residual cases, both benign:
///
/// * caller RAM happens to equal `ARMED` (about one in 2^32): the swap panic path
///   runs during a libcall, which is the correct path there anyway.
/// * it does not, during a signing libcall whose arming write was itself clobbered:
///   the standalone path runs, which replies and exits rather than returning
///   through `os_lib_end`. Deterministic and fail-closed, if untidy for the caller.
///
/// Chosen to look nothing like zero, all-ones, ASCII, or an address.
const ARMED: u32 = 0xA5C3_7E19;

static mut SWAP_PANIC_ARMED: u32 = 0;

/// Whether the swap panic path should be taken.
///
/// Reading shared `.bss` is unavoidable: the panic handler takes no arguments, the
/// linker script requires `.data` to be empty so there is nowhere the loader would
/// initialise per invocation, and the SDK exposes no way to ask whether this
/// invocation is a library call. Reading is safe here only because the value is
/// compared, never dereferenced or called.
pub fn is_swap_panic_armed() -> bool {
    // Volatile, and not merely for tidiness: the flag is only ever written with
    // ARMED, so LLVM otherwise proves it holds one of two values and shrinks the
    // object to a single byte -- cutting the odds of caller RAM matching from one
    // in 2^32 to one in 256. Volatile also reflects the truth that another app can
    // write this address.
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(SWAP_PANIC_ARMED)) == ARMED }
}

/// Arm the swap panic path.
///
/// SAFETY: should be used only in a lib swap call, after the app is initialized.
/// This writes shared `.bss`, so it must not be called during the pre-sign
/// commands, whose `.bss` belongs to the caller.
pub(crate) unsafe fn arm_swap_panic() {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(SWAP_PANIC_ARMED), ARMED);
    }
}

/// The swap panic path: report to the calling app and hand control back to it.
pub fn swap_panic(_info: &PanicInfo) -> ! {
    ledger_device_sdk::log::error!("Swap panic happened! {:#?}", _info);

    let mut comm = io::Comm::new();
    comm.swap_reply(io::StatusWords::Panic);

    unsafe { ledger_device_sdk::sys::os_lib_end() }
}
