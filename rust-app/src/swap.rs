use core::{mem, slice};

use ledger_crypto_helpers::{
    common::{Address, CryptographyError},
    eddsa::with_public_keys,
};
use ledger_device_sdk::libcall::string::CustomString;
use ledger_device_sdk::libcall::{
    string::uint256_to_float,
    swap::{CheckAddressParams, CreateTxParams, PrintableAmountParams},
};

use crate::implementation::SuiPubKeyAddress;

#[derive(Debug)]
pub enum Error {}

pub fn check_address(params: &CheckAddressParams) -> Result<bool, CryptographyError> {
    let dpath = unsafe {
        slice::from_raw_parts(
            params.dpath.as_ptr() as *const _,
            params.dpath_len / mem::size_of::<u32>(),
        )
    };
    let ref_addr = &params.ref_address[..params.ref_address_len];

    with_public_keys(
        dpath,
        true,
        |_, address: &SuiPubKeyAddress| -> Result<_, CryptographyError> {
            let bin_addr = address.get_binary_address();
            Ok(ref_addr == bin_addr)
        },
    )
}

pub fn get_printable_amount(params: &mut PrintableAmountParams) -> Result<CustomString<79>, Error> {
    let mut amount_256 = [0u8; 32];
    let amount = &params.amount[..params.amount_len];
    amount_256[..amount.len()].copy_from_slice(amount);

    Ok(uint256_to_float(&amount_256, 9))
}

pub fn sign_transaction(_params: &mut CreateTxParams) -> Result<u8, Error> {
    todo!()
}
