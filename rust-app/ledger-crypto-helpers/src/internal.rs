/*use arrayvec::{CapacityError};
use core::default::Default;
use core::ops::{Deref,DerefMut};
use ledger_log::*;
use nanos_sdk::bindings::*;
use nanos_sdk::io::SyscallError;
use zeroize::{DefaultIsZeroes, Zeroizing};
*/

macro_rules! call_c_api_function {
    ($($call:tt)*) => {
        {
            let err = unsafe {
                $($call)*
            };
            if err != 0 {
 //               error!("Syscall errored: {:?}", SyscallError::from(err));
                Err(SyscallError::from(err))
            } else {
                Ok(())
            }
        }
    }
}
