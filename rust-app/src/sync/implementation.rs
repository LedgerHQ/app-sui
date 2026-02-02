use crate::crypto_helpers::common::{try_option, Address};
use crate::crypto_helpers::eddsa::{ed25519_public_key_bytes, with_public_keys};
use crate::interface::*;
use crate::sync::ui::nbgl::UserInterface;

use arrayvec::ArrayVec;
use ledger_device_sdk::io::StatusWords;
use ledger_device_sdk::log::{error, info};

use core::convert::TryFrom;

pub const BIP32_PREFIX: [u32; 2] = ledger_device_sdk::ecc::make_bip32_path(b"m/44'/784'");

struct Bip32Path(ArrayVec<u32, 10>);

impl TryFrom<&[u8]> for Bip32Path {
    type Error = StatusWords;

    fn try_from(bs: &[u8]) -> Result<Self, Self::Error> {
        let mut arr = ArrayVec::<u32, 10>::new();
        // First byte is length
        let mut offset = 1;
        while offset + 4 <= bs.len() {
            let chunk: [u8; 4] = bs[offset..offset + 4]
                .try_into()
                .map_err(|_| StatusWords::BadLen)?;
            arr.try_push(u32::from_le_bytes(chunk))
                .map_err(|_| StatusWords::BadLen)?;
            offset += 4;
        }
        if offset != bs.len() {
            return Err(StatusWords::BadLen);
        }
        Ok(Bip32Path(arr))
    }
}

pub fn get_address(
    ui: &mut UserInterface,
    path: &[u8],
    prompt: bool,
) -> Result<ArrayVec<u8, 220>, StatusWords> {
    let bip32_path: Bip32Path = path.try_into()?;
    if !bip32_path.0.starts_with(&BIP32_PREFIX[0..2]) {
        error!("BIP32 path prefix mismatch");
        return Err(StatusWords::Unknown);
    }

    let mut rv = ArrayVec::<u8, 220>::new();

    if with_public_keys(&bip32_path.0, true, |key, address: &SuiPubKeyAddress| {
        try_option(|| -> Option<()> {
            if prompt {
                info!("Prompting for address confirmation");
                ui.confirm_address(address)?;
            }
            let key_bytes = ed25519_public_key_bytes(key);
            rv.try_push(u8::try_from(key_bytes.len()).ok()?).ok()?;
            rv.try_extend_from_slice(key_bytes).ok()?;

            // And we'll send the address along;
            let binary_address = address.get_binary_address();
            rv.try_push(u8::try_from(binary_address.len()).ok()?).ok()?;
            rv.try_extend_from_slice(binary_address).ok()?;
            Some(())
        }())
    })
    .is_err()
    {
        return Err(StatusWords::UserCancelled);
    }
    Ok(rv)
}
