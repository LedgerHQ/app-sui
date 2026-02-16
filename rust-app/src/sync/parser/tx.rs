use crate::sync::parser::common::*;
use ledger_parser_combinators::bcs::interp_parser::*;
use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::interp_parser::{
    rej, DefaultInterp, InterpParser, ParseResult, ParserCommon,
};

// ========== SCHEMA TYPES (describe wire format) ==========

pub type IntentMessageSchema = (IntentSchema, TransactionDataSchema);
pub struct TransactionDataSchema;

pub type TransactionDataV1Schema = (
    ULEB128,                     // version
    SuiAddressSchema,            // sender
    GasDataSchema,               // gas_data
    TransactionExpirationSchema, // expiration
);

pub type IntentSchema = (ULEB128, ULEB128, ULEB128);

pub struct TransactionKindSchema;
pub struct ProgrammableTransactionSchema;
pub struct CommandSchema;
pub struct ArgumentSchema;
pub struct CallArgSchema;

pub const MAX_GAS_COIN_COUNT: usize = 32;
pub type GasDataSchema = (
    Vec<ObjectRefSchema, MAX_GAS_COIN_COUNT>, // payment
    SuiAddressSchema,                         // owner
    AmountSchema,                             // price
    AmountSchema,                             // budget
);

pub struct TransactionExpirationSchema;
pub type EpochIdSchema = U64<{ Endianness::Little }>;

pub type SharedObjectSchema = (
    ObjectIDSchema,       // id
    SequenceNumberSchema, // initial_shared_version
    bool,                 // mutable
);

pub type Coins = Vec<ObjectRefSchema, { usize::MAX }>;

// ========== DATA TYPES (hold parsed values) ==========

pub struct Intent(pub u32, pub u32, pub u32);

pub struct TransactionKind(pub u8);

pub type GasData = (u64, Option<u64>);

pub enum CallArg {
    RecipientAddress(SuiAddressRaw),
    Amount(u64),
    OptionalAmount(Option<u64>),
    ObjectRef(ObjectDigest),
    SharedObject(CoinID),
    Other,
}

// Inputs which are referenced in computation of commands
pub enum InputValue {
    RecipientAddress(SuiAddressRaw),
    Amount(u64),
    OptionalAmount(Option<u64>),
    ObjectRef(ObjectDigest),
    SharedObject(CoinID),
    Object(CoinData),
    // ^ mutable via MergeCoins
}

// ========== PARSERS (convert schema → data) ==========

// Wrapper parser that parses IntentSchema and returns Intent data
pub struct IntentParser;

impl ParserCommon<IntentSchema> for IntentParser {
    type State = ();
    type Returning = Intent;

    fn init(&self) -> Self::State {
        ()
    }
}

impl InterpParser<IntentSchema> for IntentParser {
    fn parse<'a, 'b>(
        &self,
        _state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        let mut cursor = chunk;
        let mut uleb_state;
        let mut uleb_dest;

        // Parse IntentVersion (ULEB128)
        uleb_state = <DefaultInterp as ParserCommon<ULEB128>>::init(&DefaultInterp);
        uleb_dest = None;
        cursor = <DefaultInterp as InterpParser<ULEB128>>::parse(
            &DefaultInterp,
            &mut uleb_state,
            cursor,
            &mut uleb_dest,
        )?;
        let version = uleb_dest.ok_or_else(|| rej(cursor))?;

        // Parse IntentScope (ULEB128)
        uleb_state = <DefaultInterp as ParserCommon<ULEB128>>::init(&DefaultInterp);
        uleb_dest = None;
        cursor = <DefaultInterp as InterpParser<ULEB128>>::parse(
            &DefaultInterp,
            &mut uleb_state,
            cursor,
            &mut uleb_dest,
        )?;
        let scope = uleb_dest.ok_or_else(|| rej(cursor))?;

        // Parse AppId (ULEB128)
        uleb_state = <DefaultInterp as ParserCommon<ULEB128>>::init(&DefaultInterp);
        uleb_dest = None;
        cursor = <DefaultInterp as InterpParser<ULEB128>>::parse(
            &DefaultInterp,
            &mut uleb_state,
            cursor,
            &mut uleb_dest,
        )?;
        let app_id = uleb_dest.ok_or_else(|| rej(cursor))?;

        *destination = Some(Intent(version, scope, app_id));
        Ok(cursor)
    }
}

// pub struct GasDataParser;
// impl ParserCommon<GasDataSchema> for GasDataParser {
//     type State = ();
//     type Returning = GasData;

//     fn init(&self) -> Self::State {
//         ()
//     }
// }

// impl InterpParser<GasDataSchema> for GasDataParser {
//     fn parse<'a, 'b>(
//         &self,
//         _state: &'b mut Self::State,
//         chunk: &'a [u8],
//         destination: &mut Option<Self::Returning>,
//     ) -> ParseResult<'a> {
//         let mut cursor = chunk;
//         let mut uleb_state;
//         let mut uleb_dest;

//         // Parse price (ULEB128)
//         uleb_state = <DefaultInterp as ParserCommon<ULEB128>>::init(&DefaultInterp);
//         uleb_dest = None;
//         cursor = <DefaultInterp as InterpParser<ULEB128>>::parse(
//             &DefaultInterp,
//             &mut uleb_state,
//             cursor,
//             &mut uleb_dest,
//         )?;
//         let price = uleb_dest.ok_or_else(|| rej(cursor))?;

//         // Parse budget (ULEB128)
//         uleb_state = <DefaultInterp as ParserCommon<ULEB128>>::init(&DefaultInterp);
//         uleb_dest = None;
//         cursor = <DefaultInterp as InterpParser<ULEB128>>::parse(
//             &DefaultInterp,
//             &mut uleb_state,
//             cursor,
//             &mut uleb_dest,
//         )?;
//         let budget = uleb_dest.ok_or_else(|| rej(cursor))?;

//         *destination = Some((price, Some(budget)));
//         Ok(cursor)
//     }
// }
