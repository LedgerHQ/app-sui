use arrayvec::ArrayVec;
use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::interp_parser::{
    rej, DefaultInterp, ForwardArrayParserState, InterpParser, ParseResult, ParserCommon,
};

// ========== SCHEMA TYPES (describe wire format) ==========

pub type ObjectRefSchema = (
    Array<Byte, SUI_ADDRESS_LENGTH>,
    U64<{ Endianness::Little }>,
    Array<Byte, 33>,
);

pub type AccountAddressSchema = SuiAddressSchema;
pub type ObjectIDSchema = AccountAddressSchema;
pub type SequenceNumberSchema = U64<{ Endianness::Little }>;
pub type ObjectDigestSchema = Array<Byte, 33>;

pub const SUI_ADDRESS_LENGTH: usize = 32;
pub type SuiAddressSchema = Array<Byte, SUI_ADDRESS_LENGTH>;

pub type RecipientSchema = Array<Byte, SUI_ADDRESS_LENGTH>;

pub type AmountSchema = U64<{ Endianness::Little }>;

pub type U64LESchema = U64<{ Endianness::Little }>;
pub type U16LESchema = U16<{ Endianness::Little }>;

pub type Sha3_256HashSchema = Array<Byte, 33>;

// ========== DATA TYPES (hold parsed values) ==========
pub type SuiAddressRaw = [u8; SUI_ADDRESS_LENGTH];
pub type ObjectDigest = [u8; 33];

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

// ========== PARSERS (convert schema → data) ==========

pub struct ObjectRefParser;

// pub struct ObjectRefParserState<I, S, const N: usize> {
//     uleb_state: ForwardArrayParserState<I, S, N>,
//     uleb_dest: Option<I>,
// }

impl ParserCommon<ObjectRefSchema> for ObjectRefParser {
    type State = ();
    type Returning = ObjectDigest;
    fn init(&self) -> Self::State {
        ()
    }
}
impl InterpParser<ObjectRefSchema> for ObjectRefParser {
    fn parse<'a, 'b>(
        &self,
        _state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        let mut cursor = chunk;

        // Parse SuiAddress Array<Byte, SUI_ADDRESS_LENGTH>
        let mut uleb_state =
            <DefaultInterp as ParserCommon<Array<Byte, SUI_ADDRESS_LENGTH>>>::init(&DefaultInterp);
        let mut uleb_dest = None;
        cursor = <DefaultInterp as InterpParser<Array<Byte, SUI_ADDRESS_LENGTH>>>::parse(
            &DefaultInterp,
            &mut uleb_state,
            cursor,
            &mut uleb_dest,
        )?;
        let _address_array = uleb_dest.ok_or_else(|| rej(cursor))?;

        // Parse U64<{ Endianness::Little }>
        let mut uleb_state =
            <DefaultInterp as ParserCommon<U64<{ Endianness::Little }>>>::init(&DefaultInterp);
        let mut uleb_dest = None;
        cursor = <DefaultInterp as InterpParser<U64<{ Endianness::Little }>>>::parse(
            &DefaultInterp,
            &mut uleb_state,
            cursor,
            &mut uleb_dest,
        )?;
        let _seq_number = uleb_dest.ok_or_else(|| rej(cursor))?;

        // Parse Array<Byte, 33>
        let mut uleb_state = <DefaultInterp as ParserCommon<Array<Byte, 33>>>::init(&DefaultInterp);
        let mut uleb_dest = None;
        cursor = <DefaultInterp as InterpParser<Array<Byte, 33>>>::parse(
            &DefaultInterp,
            &mut uleb_state,
            cursor,
            &mut uleb_dest,
        )?;
        let digest_array = uleb_dest.ok_or_else(|| rej(cursor))?;
        *destination = Some(digest_array);
        Ok(cursor)
    }
}
