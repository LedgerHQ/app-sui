use crate::parser::common::*;
use core::convert::TryFrom;
use core::marker::ConstParamTy;
use ledger_device_sdk::io::{ApduHeader, StatusWords};
use ledger_parser_combinators::core_parsers::*;
use num_enum::TryFromPrimitive;

#[derive(ConstParamTy, PartialEq, Eq)]
#[repr(u8)]
pub enum ParseChecks {
    None,
    PromptUser,
    CheckSwapTx,
}

// Payload for a public key request
pub type Bip32Key = DArray<Byte, U32<{ Endianness::Little }>, 10>;

pub struct SuiPubKeyAddress(SuiAddressRaw);

use arrayvec::ArrayVec;
use crate::crypto_helpers::common::{Address, HexSlice};
use crate::crypto_helpers::eddsa::ed25519_public_key_bytes;
use crate::crypto_helpers::hasher::{Blake2b, Hasher};
use ledger_device_sdk::io::SyscallError;

impl Address<SuiPubKeyAddress, ledger_device_sdk::ecc::ECPublicKey<65, 'E'>> for SuiPubKeyAddress {
    fn get_address(
        key: &ledger_device_sdk::ecc::ECPublicKey<65, 'E'>,
    ) -> Result<Self, SyscallError> {
        let key_bytes = ed25519_public_key_bytes(key);
        let mut tmp = ArrayVec::<u8, 33>::new();
        let _ = tmp.try_push(0); // SIGNATURE_SCHEME_TO_FLAG['ED25519']
        let _ = tmp.try_extend_from_slice(key_bytes);
        let mut hasher: Blake2b = Hasher::new();
        hasher.update(&tmp);
        let hash: [u8; SUI_ADDRESS_LENGTH] = hasher.finalize();
        Ok(SuiPubKeyAddress(hash))
    }
    fn get_binary_address(&self) -> &[u8] {
        &self.0
    }
}

impl core::fmt::Display for SuiPubKeyAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{}", HexSlice(&self.0))
    }
}

#[repr(u8)]
#[derive(Debug, TryFromPrimitive)]
pub enum Ins {
    GetVersion = 0,
    VerifyAddress = 1,
    GetPubkey = 2,
    Sign = 3,
    ProvideTrustedDynamicDescriptor = 0x22,
    GetVersionStr = 0xfe,
    Exit = 0xff,
}

impl TryFrom<ApduHeader> for Ins {
    type Error = StatusWords;
    fn try_from(m: ApduHeader) -> Result<Ins, Self::Error> {
        match m {
            ApduHeader {
                cla: 0,
                ins,
                p1: 0,
                p2: 0,
            } => Self::try_from(ins).map_err(|_| StatusWords::BadIns),
            _ => Err(StatusWords::BadIns),
        }
    }
}

// Status word used when swap transaction parameters check failed
pub const SW_SWAP_TX_PARAM_MISMATCH: u16 = 0x6e05;
