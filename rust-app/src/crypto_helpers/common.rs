use arrayvec::CapacityError;
use core::fmt;
use ledger_device_sdk::ecc::*;
use ledger_device_sdk::io::SyscallError;

pub fn try_option<A>(q: Option<A>) -> Result<A, CryptographyError> {
    q.ok_or(CryptographyError::NoneError)
}

// Target chain's notion of an address and how to format one.

pub trait Address<A, K>: fmt::Display {
    fn get_address(key: &K) -> Result<A, SyscallError>;
    fn get_binary_address(&self) -> &[u8];
}

pub struct HexSlice<'a>(pub &'a [u8]);

// You can choose to implement multiple traits, like Lower and UpperHex
impl fmt::Display for HexSlice<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            // Decide if you want to pad the value or have spaces inbetween, etc.
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum CryptographyError {
    NoneError,
    SyscallError(SyscallError),
    CxError(CxError),
    CapacityError(CapacityError),
}

impl From<SyscallError> for CryptographyError {
    fn from(e: SyscallError) -> Self {
        CryptographyError::SyscallError(e)
    }
}
impl From<CxError> for CryptographyError {
    fn from(e: CxError) -> Self {
        CryptographyError::CxError(e)
    }
}
impl From<CapacityError> for CryptographyError {
    fn from(e: CapacityError) -> Self {
        CryptographyError::CapacityError(e)
    }
}
