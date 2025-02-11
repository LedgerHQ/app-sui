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
pub type ObjectDigestSchema = SHA3_256_HASH;

pub const SUI_ADDRESS_LENGTH: usize = 32;
pub type SuiAddress = Array<Byte, SUI_ADDRESS_LENGTH>;

pub type Recipient = SuiAddress;

pub type Amount = U64LE;

pub type U64LE = U64<{ Endianness::Little }>;
pub type U16LE = U16<{ Endianness::Little }>;

// TODO: confirm if 33 is indeed ok for all uses of SHA3_256_HASH
#[allow(non_camel_case_types)]
pub type SHA3_256_HASH = Array<Byte, 33>;

// Parsed data
pub type SuiAddressRaw = [u8; SUI_ADDRESS_LENGTH];
pub type ObjectDigest = <DefaultInterp as HasOutput<ObjectDigestSchema>>::Output;

pub type CoinData = ([u8; 32], u64);
pub type ObjectData = CoinData;

pub trait HasObjectData {
    fn get_object_data<'a: 'c, 'b: 'c, 'c>(&'b self, digest: &'a ObjectDigest) -> Self::State<'c>;

    type State<'c>: Future<Output = Option<ObjectData>>
    where
        Self: 'c;
}

impl HasObjectData for () {
    type State<'c> = impl Future<Output = Option<ObjectData>> + 'c;

    fn get_object_data<'a: 'c, 'b: 'c, 'c>(&'b self, _: &'a ObjectDigest) -> Self::State<'c> {
        async move { None }
    }
}
