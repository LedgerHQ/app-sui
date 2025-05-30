use arrayvec::ArrayVec;
use core::future::Future;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::endianness::*;
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

pub type Sha3_256Hash = Array<Byte, 33>;

// Parsed data
pub type SuiAddressRaw = [u8; SUI_ADDRESS_LENGTH];
pub type ObjectDigest = <DefaultInterp as HasOutput<ObjectDigestSchema>>::Output;

pub type CoinID = [u8; 32];

// Max string length which will be shown to the user
// For parsing longer length is also handled, but it will be truncated to this
pub const COIN_STRING_LENGTH: usize = 16;

pub type CoinModuleName = ArrayVec<u8, COIN_STRING_LENGTH>;
pub type CoinFunctionName = ArrayVec<u8, COIN_STRING_LENGTH>;

pub type CoinType = (CoinID, CoinModuleName, CoinFunctionName);

pub type CoinData = (CoinType, u64);

pub const SUI_COIN_ID: CoinID = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
];

pub const SUI_SYSTEM_ID: CoinID = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3,
];

pub const SUI_SYSTEM_STATE_ID: CoinID = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5,
];

// This does not contain the correct module and function names, as we don't have a way to create const ArrayVec with them
pub const SUI_COIN_TYPE: CoinType = (SUI_COIN_ID, ArrayVec::new_const(), ArrayVec::new_const());

pub const SUI_COIN_DECIMALS: u8 = 9;

pub type ObjectData = CoinData;

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
