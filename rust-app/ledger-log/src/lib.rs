#![cfg_attr(target_family = "bolos", no_std)]

use core::fmt::Write;
#[cfg(all(target_family = "bolos", feature = "speculos"))]
use ledger_device_sdk::testing::debug_print;

pub struct DBG;

#[cfg(not(target_family = "bolos"))]
impl Write for DBG {
    fn write_str(&mut self, _s: &str) -> core::fmt::Result {
        print!("{}", _s);
        Ok(())
    }
}

#[cfg(all(target_family = "bolos", feature = "speculos"))]
impl Write for DBG {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        debug_print(s);
        Ok(())
    }
}

#[cfg(all(target_family = "bolos", not(feature = "speculos")))]
impl Write for DBG {
    fn write_str(&mut self, _s: &str) -> core::fmt::Result {
        Ok(()) // No-op for production builds
    }
}

#[cfg(any(not(target_family = "bolos"), feature = "speculos"))]
#[macro_export]
macro_rules! log {
    (target: $target:expr, $lvl:expr, $fmt:literal $($arg:tt)*) => ({
        use core::fmt::Write;
        let _ = core::write!($crate::DBG, concat!("[{}] {}:{}: ", $fmt, "\r\n"), $lvl, core::file!(), core::line!() $($arg)*);
    });
    ($lvl:expr, $fmt:literal $($arg:tt)*) => (log!(target: __log_module_path!(), $lvl, $fmt $($arg)*))
}

#[cfg(all(target_family = "bolos", not(feature = "speculos")))]
#[macro_export]
macro_rules! log {
    (target: $target:expr, $lvl:expr, $fmt:literal $($arg:tt)*) => ({ });
    ($lvl:expr, $fmt:literal $($arg:tt)*) => (log!(target: __log_module_path!(), $lvl, $fmt $($arg)*))
}

#[cfg(feature = "log_error")]
#[macro_export]
macro_rules! error {
    ($fmt:literal $($arg:tt)*) => ({use $crate::log; log!("ERROR", $fmt $($arg)*)})
}
#[cfg(not(feature = "log_error"))]
#[macro_export]
macro_rules! error {
    ($fmt:literal $($arg:tt)*) => {{}};
}
#[cfg(feature = "log_warn")]
#[macro_export]
macro_rules! warn {
    ($fmt:literal $($arg:tt)*) => ({use $crate::log; log!("WARN", $fmt $($arg)*)})
}
#[cfg(not(feature = "log_warn"))]
#[macro_export]
macro_rules! warn {
    ($fmt:literal $($arg:tt)*) => {{}};
}
#[cfg(feature = "log_info")]
#[macro_export]
macro_rules! info {
    ($fmt:literal $($arg:tt)*) => ({use $crate::log; log!("INFO", $fmt $($arg)*)})
}
#[cfg(not(feature = "log_info"))]
#[macro_export]
macro_rules! info {
    ($fmt:literal $($arg:tt)*) => {{}};
}
#[cfg(feature = "log_debug")]
#[macro_export]
macro_rules! debug {
    ($fmt:literal $($arg:tt)*) => ({use $crate::log; log!("DEBUG", $fmt $($arg)*)})
}
#[cfg(not(feature = "log_debug"))]
#[macro_export]
macro_rules! debug {
    ($fmt:literal $($arg:tt)*) => {{}};
}
#[cfg(feature = "log_trace")]
#[macro_export]
macro_rules! trace {
    ($fmt:literal $($arg:tt)*) => ({use $crate::log; log!("TRACE", $fmt $($arg)*)})
}
#[cfg(not(feature = "log_trace"))]
#[macro_export]
macro_rules! trace {
    ($fmt:literal $($arg:tt)*) => {{}};
}

#[test]
fn test_debug() {
    debug!("FOO FOO FOO\n");
    assert_eq!(true, false);
}
