use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::endianness::*;

// Schema
pub type ObjectRef = (ObjectID, SequenceNumber, ObjectDigest);

pub type AccountAddress = SuiAddress;
pub type ObjectID = AccountAddress;
pub type SequenceNumber = U64LE;
pub type ObjectDigest = SHA3_256_HASH;

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
