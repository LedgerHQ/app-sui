use core::future::Future;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::interp::*;

// Schema
pub type ObjectRefSchema = (ObjectID, SequenceNumber, ObjectDigestSchema);

pub type AccountAddress = SuiAddress;
pub type ObjectID = AccountAddress;
pub type SequenceNumber = U64LE;
pub type ObjectDigestSchema = Sha3_256Hash;

pub const SUI_ADDRESS_LENGTH: usize = 32;
pub type SuiAddress = Array<Byte, SUI_ADDRESS_LENGTH>;

pub type Recipient = SuiAddress;

pub type Amount = U64LE;

pub type U64LE = U64<{ Endianness::Little }>;
pub type U16LE = U16<{ Endianness::Little }>;
pub type U32LE = U32<{ Endianness::Little }>;

pub type Sha3_256Hash = Array<Byte, 33>;

// Parsed data
pub type SuiAddressRaw = [u8; SUI_ADDRESS_LENGTH];
pub type ObjectDigest = <DefaultInterp as HasOutput<ObjectDigestSchema>>::Output;

pub type CoinID = [u8; 32];

// Max string length which will be shown to the user
// For parsing longer length is also handled, but it will be truncated to this
pub const COIN_STRING_LENGTH: usize = 32;

/// Fixed-size, zero-padded module / function name bytes so `CoinType` is `Copy`.
pub type CoinNameBytes = [u8; COIN_STRING_LENGTH];

/// `(package_id, module, type_name)` — names are zero-padded to `COIN_STRING_LENGTH`.
pub type CoinType = (CoinID, CoinNameBytes, CoinNameBytes);

#[inline]
pub fn pad_coin_name_bytes(src: &[u8]) -> CoinNameBytes {
    let mut out = [0u8; COIN_STRING_LENGTH];
    if src.len() > COIN_STRING_LENGTH {
        panic!(
            "Overlong coin name {} ({} bytes) not supported",
            core::str::from_utf8(src).unwrap_or("<invalid utf-8>"),
            src.len()
        );
    }
    let n = src.len();
    out[..n].copy_from_slice(&src[..n]);
    out
}

/// Slice for UTF-8 / equality: trim trailing padding zeros.
#[inline]
pub fn coin_name_trimmed(s: &[u8]) -> &[u8] {
    let end = s.iter().rposition(|&b| b != 0).map(|i| i + 1).unwrap_or(0);
    &s[..end]
}

#[inline]
pub fn coin_type_from_short_str(id: CoinID, module: &str, function: &str) -> CoinType {
    (
        id,
        pad_coin_name_bytes(module.as_bytes()),
        pad_coin_name_bytes(function.as_bytes()),
    )
}

pub type CoinData = (CoinType, u64);

pub const SUI_COIN_ID: CoinID = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
];

pub const UNKNOWN_COIN_ID: CoinID = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub const SUI_SYSTEM_ID: CoinID = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3,
];

pub const SUI_SYSTEM_STATE_ID: CoinID = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5,
];

pub const SUI_COIN_TYPE: CoinType = (
    SUI_COIN_ID,
    [0u8; COIN_STRING_LENGTH],
    [0u8; COIN_STRING_LENGTH],
);

pub const UNKNOWN_COIN_TYPE: CoinType = (
    UNKNOWN_COIN_ID,
    [0u8; COIN_STRING_LENGTH],
    [0u8; COIN_STRING_LENGTH],
);

pub const SUI_COIN_DECIMALS: u8 = 9;

// Parsed on-ledger object data. The coin-vs-stake distinction must be preserved
// here: a StakedSui object is NOT liquid SUI, so it must never be classified as a
// plain coin by downstream transfer/split/merge validation.
#[derive(Clone, Copy)]
pub enum ObjectData {
    Coin { coin_type: CoinType, amount: u64 },
    StakedSui { amount: u64 },
}

pub trait HasObjectData {
    fn get_object_data<'a: 'c, 'b: 'c, 'c>(&'b self, digest: &'a ObjectDigest) -> Self::State<'c>;

    type State<'c>: Future<Output = Option<ObjectData>>
    where
        Self: 'c;
}

impl<T: HasObjectData> HasObjectData for Option<T> {
    type State<'c>
        = impl Future<Output = Option<ObjectData>> + 'c
    where
        T: 'c;

    fn get_object_data<'a: 'c, 'b: 'c, 'c>(&'b self, digest: &'a ObjectDigest) -> Self::State<'c> {
        async move {
            match self {
                Some(s) => s.get_object_data(digest).await,
                None => None,
            }
        }
    }
}

impl HasObjectData for () {
    type State<'c> = impl Future<Output = Option<ObjectData>> + 'c;

    fn get_object_data<'a: 'c, 'b: 'c, 'c>(&'b self, _: &'a ObjectDigest) -> Self::State<'c> {
        async move { None }
    }
}
