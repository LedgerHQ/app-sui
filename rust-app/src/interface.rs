use core::convert::TryFrom;
use core::marker::ConstParamTy;
use ledger_device_sdk::io::{ApduHeader, StatusWords};
use ledger_parser_combinators::bcs::async_parser::*;
use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::endianness::*;
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

pub type SignParameters = (IntentMessage, Bip32Key);

// Sui Types
pub type IntentMessage = (Intent, TransactionData);

pub struct TransactionData;

pub type TransactionDataV1 = (
    TransactionKind,
    SuiAddress,            // sender
    GasData,               // gas_data
    TransactionExpiration, // expiration
);

pub struct TransactionKind;

pub struct ProgrammableTransaction;

pub struct CommandSchema;
pub struct ArgumentSchema;
pub struct CallArgSchema;

pub type GasData = (
    Vec<ObjectRef, { usize::MAX }>, // payment
    SuiAddress,                     // owner
    Amount,                         // price
    Amount,                         // budget
);

pub struct TransactionExpiration;
pub type EpochId = U64<{ Endianness::Little }>;

pub type ObjectRef = (ObjectID, SequenceNumber, ObjectDigest);

pub type SharedObject = (
    ObjectID,       // id
    SequenceNumber, // initial_shared_version
    bool,           // mutable
);

pub type AccountAddress = SuiAddress;
pub type ObjectID = AccountAddress;
pub type SequenceNumber = U64LE;
pub type ObjectDigest = SHA3_256_HASH;

pub const SUI_ADDRESS_LENGTH: usize = 32;
pub type SuiAddress = Array<Byte, SUI_ADDRESS_LENGTH>;

pub type Coins = Vec<ObjectRef, { usize::MAX }>;

pub type Recipient = SuiAddress;

pub type Amount = U64LE;

pub type U64LE = U64<{ Endianness::Little }>;
pub type U16LE = U16<{ Endianness::Little }>;

pub type Intent = (IntentVersion, IntentScope, AppId);
pub type IntentVersion = ULEB128;
pub type IntentScope = ULEB128;
pub type AppId = ULEB128;

// TODO: confirm if 33 is indeed ok for all uses of SHA3_256_HASH
#[allow(non_camel_case_types)]
pub type SHA3_256_HASH = Array<Byte, 33>;

pub type SuiAddressRaw = [u8; SUI_ADDRESS_LENGTH];

#[allow(dead_code)]
pub struct SuiPubKeyAddress(ledger_device_sdk::ecc::ECPublicKey<65, 'E'>, SuiAddressRaw);

use arrayvec::ArrayVec;
use ledger_crypto_helpers::common::{Address, HexSlice};
use ledger_crypto_helpers::eddsa::ed25519_public_key_bytes;
use ledger_crypto_helpers::hasher::{Blake2b, Hasher};
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
        Ok(SuiPubKeyAddress(key.clone(), hash))
    }
    fn get_binary_address(&self) -> &[u8] {
        &self.1
    }
}

impl core::fmt::Display for SuiPubKeyAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{}", HexSlice(&self.1))
    }
}

#[repr(u8)]
#[derive(Debug, TryFromPrimitive)]
pub enum Ins {
    GetVersion = 0,
    VerifyAddress = 1,
    GetPubkey = 2,
    Sign = 3,
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
