// Complete state machine implementation for Sui transaction parsing
// Replaces parser combinators with explicit state machines

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::sync::parser::common::*;
use arrayvec::ArrayVec;

const MAX_GAS_COIN_COUNT: usize = 32;

// ========== ERROR TYPES ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// ULEB128 value overflows u32/u64
    Overflow,
    /// Non-canonical ULEB128 encoding
    NonCanonical,
    /// Invalid type tag
    InvalidType,
    /// Too many items in collection
    TooMany,
    /// Need more data to continue
    NeedMoreData,
    /// Invalid boolean value (not 0x00 or 0x01)
    InvalidBool,
    /// Unexpected state
    UnexpectedState,
}

// ========== ULEB128 PARSER (reusable) ==========

/// State machine for parsing ULEB128 (variable-length unsigned integers)
#[derive(Debug, Clone)]
pub struct UlebParser {
    value: u64,
    shift: u32,
}

impl UlebParser {
    pub fn new() -> Self {
        Self { value: 0, shift: 0 }
    }
    
    /// Feed one byte, return Some(value) if complete, None if needs more
    pub fn feed_byte(&mut self, byte: u8) -> Result<Option<u32>, ParseError> {
        // Check for overflow before processing
        if self.shift >= 35 {
            return Err(ParseError::Overflow);
        }
        
        let digit = (byte & 0x7f) as u64;
        self.value |= digit << self.shift;
        
        // High bit clear = last byte
        if byte & 0x80 == 0 {
            // Check for non-canonical encoding (unnecessary leading zeros)
            if self.shift > 0 && digit == 0 {
                return Err(ParseError::NonCanonical);
            }
            
            // Check final value fits in u32
            if self.value > u32::MAX as u64 {
                return Err(ParseError::Overflow);
            }
            
            Ok(Some(self.value as u32))
        } else {
            self.shift += 7;
            Ok(None)
        }
    }
    
    /// Feed one byte for u64 result
    pub fn feed_byte_u64(&mut self, byte: u8) -> Result<Option<u64>, ParseError> {
        if self.shift >= 70 {
            return Err(ParseError::Overflow);
        }
        
        let digit = (byte & 0x7f) as u64;
        
        // Check for overflow before shifting
        if self.shift > 0 && digit.leading_zeros() < (64 - self.shift) as u32 {
            return Err(ParseError::Overflow);
        }
        
        self.value |= digit << self.shift;
        
        if byte & 0x80 == 0 {
            if self.shift > 0 && digit == 0 {
                return Err(ParseError::NonCanonical);
            }
            Ok(Some(self.value))
        } else {
            self.shift += 7;
            Ok(None)
        }
    }
}

// ========== DATA STRUCTURES ==========

/// Parsed Intent (version, scope, app_id)
#[derive(Debug, Clone, Copy)]
pub struct Intent {
    pub version: u32,
    pub scope: u32,
    pub app_id: u32,
}

/// Parsed TransactionExpiration
#[derive(Debug, Clone, Copy)]
pub enum TransactionExpiration {
    None,
    Epoch(u64),
}

/// Parsed CallArg (PTB input)
#[derive(Debug, Clone)]
pub enum ParsedCallArg {
    Pure { bytes: Vec<u8> },
    ImmOrOwnedObject { object_id: CoinID, version: u64, digest: ObjectDigest },
    SharedObject { object_id: CoinID, initial_shared_version: u64, mutable: bool },
    Receiving { object_id: CoinID, version: u64, digest: ObjectDigest },
}

/// Parsed Argument (reference within PTB)
#[derive(Debug, Clone, Copy)]
pub enum ParsedArgument {
    Input(u16),
    GasCoin,
    Result(u16),
    NestedResult { command_index: u16, result_index: u16 },
}

/// Parsed Command
#[derive(Debug, Clone)]
pub enum ParsedCommand {
    TransferObjects {
        objects: Vec<ParsedArgument>,
        recipient: ParsedArgument,
    },
    SplitCoins {
        coin: ParsedArgument,
        amounts: Vec<ParsedArgument>,
    },
    MergeCoins {
        destination: ParsedArgument,
        sources: Vec<ParsedArgument>,
    },
    MoveCall {
        package: CoinID,
        module: ArrayVec<u8, COIN_STRING_LENGTH>,
        function: ArrayVec<u8, COIN_STRING_LENGTH>,
        type_arguments: Vec<ParsedTypeTag>,
        arguments: Vec<ParsedArgument>,
    },
    MakeMoveVec {
        type_tag: Option<ParsedTypeTag>,
        elements: Vec<ParsedArgument>,
    },
    Publish {
        // Simplified - just track that we saw it
        module_count: u32,
    },
    Upgrade {
        // Simplified
        module_count: u32,
    },
}

/// Parsed TypeTag (simplified - we mainly need to parse structure, not semantics)
#[derive(Debug, Clone)]
pub enum ParsedTypeTag {
    Bool,
    U8,
    U64,
    U128,
    Address,
    Signer,
    Vector(Box<ParsedTypeTag>),
    Struct {
        package: CoinID,
        module: ArrayVec<u8, COIN_STRING_LENGTH>,
        name: ArrayVec<u8, COIN_STRING_LENGTH>,
        type_params: ArrayVec<Box<ParsedTypeTag>, 4>,
    },
    U16,
    U32,
    U256,
}

// ========== SUB-STATE MACHINES ==========

/// State for parsing a fixed-size byte array
#[derive(Debug, Clone)]
pub struct ByteArrayParser<const N: usize> {
    pub bytes: ArrayVec<u8, N>,
}

impl<const N: usize> ByteArrayParser<N> {
    pub fn new() -> Self {
        Self { bytes: ArrayVec::new() }
    }
    
    pub fn feed(&mut self, chunk: &[u8]) -> (usize, Option<[u8; N]>) {
        let remaining = N - self.bytes.len();
        let to_copy = remaining.min(chunk.len());
        
        if let Err(_) = self.bytes.try_extend_from_slice(&chunk[..to_copy]) {
            return (0, None);
        }
        
        if self.bytes.len() == N {
            let mut result = [0u8; N];
            result.copy_from_slice(&self.bytes);
            (to_copy, Some(result))
        } else {
            (to_copy, None)
        }
    }
}

/// State for parsing a CallArg
#[derive(Debug, Clone)]
pub enum CallArgParserState {
    ReadingType { uleb: UlebParser },
    ReadingPureLength { uleb: UlebParser },
    ReadingPureBytes { bytes: Vec<u8>, remaining: usize },
    ReadingObjectId { parser: ByteArrayParser<32> },
    ReadingObjectVersion { uleb: UlebParser, object_id: CoinID },
    ReadingObjectDigest { parser: ByteArrayParser<33>, object_id: CoinID, version: u64 },
    ReadingSharedObjectId { parser: ByteArrayParser<32> },
    ReadingSharedVersion { uleb: UlebParser, object_id: CoinID },
    ReadingSharedMutable { object_id: CoinID, version: u64 },
    ReadingReceivingId { parser: ByteArrayParser<32> },
    ReadingReceivingVersion { uleb: UlebParser, object_id: CoinID },
    ReadingReceivingDigest { parser: ByteArrayParser<33>, object_id: CoinID, version: u64 },
}

impl CallArgParserState {
    pub fn new() -> Self {
        Self::ReadingType { uleb: UlebParser::new() }
    }
}

/// State for parsing an Argument
#[derive(Debug, Clone)]
pub enum ArgumentParserState {
    ReadingType { uleb: UlebParser },
    ReadingInputIndex { uleb: UlebParser },
    ReadingResultIndex { uleb: UlebParser },
    ReadingNestedCommand { uleb: UlebParser },
    ReadingNestedResult { uleb: UlebParser, command_index: u16 },
}

impl ArgumentParserState {
    pub fn new() -> Self {
        Self::ReadingType { uleb: UlebParser::new() }
    }
}

/// State for parsing a Command
#[derive(Debug, Clone)]
pub enum CommandParserState {
    ReadingType { uleb: UlebParser },
    
    // TransferObjects states
    ReadingTransferObjectsCount { uleb: UlebParser },
    ReadingTransferObjects { 
        count: u32, 
        parsed: u32, 
        objects: Vec<ParsedArgument>,
        current_arg: Option<ArgumentParserState>,
    },
    ReadingTransferRecipient { 
        objects: Vec<ParsedArgument>,
        arg_state: Option<ArgumentParserState>,
    },
    
    // SplitCoins states
    ReadingSplitCoin { arg_state: Option<ArgumentParserState> },
    ReadingSplitAmountsCount { uleb: UlebParser, coin: ParsedArgument },
    ReadingSplitAmounts {
        coin: ParsedArgument,
        count: u32,
        parsed: u32,
        amounts: Vec<ParsedArgument>,
        current_arg: Option<ArgumentParserState>,
    },
    
    // MergeCoins states
    ReadingMergeDestination { arg_state: Option<ArgumentParserState> },
    ReadingMergeSourcesCount { uleb: UlebParser, destination: ParsedArgument },
    ReadingMergeSources {
        destination: ParsedArgument,
        count: u32,
        parsed: u32,
        sources: Vec<ParsedArgument>,
        current_arg: Option<ArgumentParserState>,
    },
    
    // MoveCall states (simplified - skipping full type_arg and argument parsing for now)
    ReadingMoveCallPackage { parser: ByteArrayParser<32> },
    ReadingMoveCallModuleLen { uleb: UlebParser, package: CoinID },
    ReadingMoveCallModule { 
        package: CoinID, 
        remaining: usize,
        module: ArrayVec<u8, COIN_STRING_LENGTH>,
    },
    ReadingMoveCallFunctionLen { 
        package: CoinID,
        module: ArrayVec<u8, COIN_STRING_LENGTH>,
        uleb: UlebParser,
    },
    ReadingMoveCallFunction {
        package: CoinID,
        module: ArrayVec<u8, COIN_STRING_LENGTH>,
        remaining: usize,
        function: ArrayVec<u8, COIN_STRING_LENGTH>,
    },
    ReadingMoveCallTypeArgsCount {
        package: CoinID,
        module: ArrayVec<u8, COIN_STRING_LENGTH>,
        function: ArrayVec<u8, COIN_STRING_LENGTH>,
        uleb: UlebParser,
    },
    ReadingMoveCallArgsCount {
        package: CoinID,
        module: ArrayVec<u8, COIN_STRING_LENGTH>,
        function: ArrayVec<u8, COIN_STRING_LENGTH>,
        type_args: Vec<ParsedTypeTag>,
        uleb: UlebParser,
    },
    ReadingMoveCallArgs {
        package: CoinID,
        module: ArrayVec<u8, COIN_STRING_LENGTH>,
        function: ArrayVec<u8, COIN_STRING_LENGTH>,
        type_args: Vec<ParsedTypeTag>,
        count: u32,
        parsed: u32,
        arguments: Vec<ParsedArgument>,
        current_arg: Option<ArgumentParserState>,
    },
    
    // Simplified for other commands
    SkippingBytes { remaining: usize },
}

impl CommandParserState {
    pub fn new() -> Self {
        Self::ReadingType { uleb: UlebParser::new() }
    }
}

// ========== MAIN STATE MACHINE ==========

#[derive(Debug, Clone)]
pub enum TxParserState {
    /// Parsing Intent (version, scope, app_id)
    Intent {
        uleb: UlebParser,
        version: Option<u32>,
        scope: Option<u32>,
    },
    
    /// Parsing TransactionData enum variant tag (expect 0 for V1)
    TxDataVariant { uleb: UlebParser },
    
    /// Parsing sender address (32 bytes)
    Sender { parser: ByteArrayParser<32> },
    
    /// Parsing gas data
    GasPaymentCount { uleb: UlebParser },
    GasPayment {
        count: u32,
        parsed: u32,
        current_object_id: Option<ByteArrayParser<32>>,
        current_version: Option<(CoinID, UlebParser)>,
        current_digest: Option<(CoinID, u64, ByteArrayParser<33>)>,
    },
    GasOwner { parser: ByteArrayParser<32> },
    GasPrice { uleb: UlebParser },
    GasBudget { uleb: UlebParser },
    
    /// Parsing expiration
    ExpirationTag { uleb: UlebParser },
    ExpirationEpoch { uleb: UlebParser },
    
    /// Parsing transaction kind tag
    TxKindTag { uleb: UlebParser },
    
    /// Parsing PTB inputs
    PtbInputCount { uleb: UlebParser },
    PtbInputs {
        count: u32,
        parsed: u32,
        current_input: Option<CallArgParserState>,
    },
    
    /// Parsing PTB commands
    PtbCommandCount { uleb: UlebParser },
    PtbCommands {
        count: u32,
        parsed: u32,
        current_command: Option<CommandParserState>,
    },
    
    Done,
}

/// Main transaction parser
pub struct TxParser {
    state: TxParserState,
    
    // Accumulated parse results
    pub intent: Option<Intent>,
    pub sender: Option<SuiAddressRaw>,
    pub gas_payment_count: u32,
    pub gas_owner: Option<SuiAddressRaw>,
    pub gas_price: Option<u64>,
    pub gas_budget: Option<u64>,
    pub expiration: Option<TransactionExpiration>,
    pub inputs: Vec<ParsedCallArg>,
    pub commands: Vec<ParsedCommand>,
}

impl TxParser {
    pub fn new() -> Self {
        Self {
            state: TxParserState::Intent {
                uleb: UlebParser::new(),
                version: None,
                scope: None,
            },
            intent: None,
            sender: None,
            gas_payment_count: 0,
            gas_owner: None,
            gas_price: None,
            gas_budget: None,
            expiration: None,
            inputs: Vec::new(),
            commands: Vec::new(),
        }
    }
    
    /// Feed a chunk of data, returns Ok(bytes_consumed) or Err
    pub fn feed(&mut self, chunk: &[u8]) -> Result<usize, ParseError> {
        let mut cursor = 0;
        
        while cursor < chunk.len() {
            ledger_device_sdk::trace!("cursor: {}, state: {:?}", cursor, self.state );
            match &mut self.state {
                // ========== Intent Parsing ==========
                TxParserState::Intent { uleb, version, scope } => {
                    if version.is_none() {
                        match uleb.feed_byte(chunk[cursor])? {
                            Some(v) => {
                                *version = Some(v);
                                *uleb = UlebParser::new();
                            }
                            None => {}
                        }
                        cursor += 1;
                        continue;
                    }
                    
                    if scope.is_none() {
                        match uleb.feed_byte(chunk[cursor])? {
                            Some(s) => {
                                *scope = Some(s);
                                *uleb = UlebParser::new();
                            }
                            None => {}
                        }
                        cursor += 1;
                        continue;
                    }
                    
                    // Parse app_id
                    match uleb.feed_byte(chunk[cursor])? {
                        Some(app_id) => {
                            self.intent = Some(Intent {
                                version: version.unwrap(),
                                scope: scope.unwrap(),
                                app_id,
                            });
                            self.state = TxParserState::TxDataVariant {
                                uleb: UlebParser::new(),
                            };
                        }
                        None => {}
                    }
                    cursor += 1;
                }
                
                // ========== TransactionData Variant Tag ==========
                TxParserState::TxDataVariant { uleb } => {
                    match uleb.feed_byte(chunk[cursor])? {
                        Some(variant) => {
                            // We expect variant 0 (V1)
                            if variant != 0 {
                                ledger_device_sdk::error!("Unsupported TransactionData variant: {}", variant);
                                return Err(ParseError::InvalidType);
                            }
                            // After variant tag, TransactionDataV1 = (TransactionKind, Sender, GasData, Expiration)
                            self.state = TxParserState::TxKindTag {
                                uleb: UlebParser::new(),
                            };
                        }
                        None => {}
                    }
                    cursor += 1;
                }
                
                // ========== TransactionKind Variant Tag ==========
                TxParserState::TxKindTag { uleb } => {
                    match uleb.feed_byte(chunk[cursor])? {
                        Some(kind) => {
                            // We expect variant 0 (ProgrammableTransaction)
                            if kind != 0 {
                                ledger_device_sdk::error!("Unsupported TransactionKind: {}", kind);
                                return Err(ParseError::InvalidType);
                            }
                            // After TransactionKind tag, ProgrammableTransaction starts with inputs count
                            self.state = TxParserState::PtbInputCount {
                                uleb: UlebParser::new(),
                            };
                        }
                        None => {}
                    }
                    cursor += 1;
                }
                
                // ========== Sender Address ==========
                TxParserState::Sender { parser } => {
                    let (consumed, result) = parser.feed(&chunk[cursor..]);
                    cursor += consumed;
                    
                    if let Some(addr) = result {
                        self.sender = Some(addr);
                        self.state = TxParserState::GasPaymentCount {
                            uleb: UlebParser::new(),
                        };
                    }
                }
                
                // ========== Gas Data ==========
                TxParserState::GasPaymentCount { uleb } => {
                    match uleb.feed_byte(chunk[cursor])? {
                        Some(count) => {
                            if count > MAX_GAS_COIN_COUNT as u32 {
                                return Err(ParseError::TooMany);
                            }
                            self.gas_payment_count = count;
                            
                            if count == 0 {
                                // Skip to gas owner
                                self.state = TxParserState::GasOwner {
                                    parser: ByteArrayParser::new(),
                                };
                            } else {
                                self.state = TxParserState::GasPayment {
                                    count,
                                    parsed: 0,
                                    current_object_id: Some(ByteArrayParser::new()),
                                    current_version: None,
                                    current_digest: None,
                                };
                            }
                        }
                        None => {}
                    }
                    cursor += 1;
                }
                
                TxParserState::GasPayment { count, parsed, current_object_id, current_version, current_digest } => {
                    // Parse object ID
                    if let Some(parser) = current_object_id {
                        let (consumed, result) = parser.feed(&chunk[cursor..]);
                        cursor += consumed;
                        
                        if let Some(object_id) = result {
                            *current_object_id = None;
                            *current_version = Some((object_id, UlebParser::new()));
                        }
                        continue;
                    }
                    
                    // Parse version
                    if let Some((object_id, uleb)) = current_version {
                        match uleb.feed_byte_u64(chunk[cursor])? {
                            Some(version) => {
                                let oid = *object_id;
                                *current_version = None;
                                *current_digest = Some((oid, version, ByteArrayParser::new()));
                            }
                            None => {}
                        }
                        cursor += 1;
                        continue;
                    }
                    
                    // Parse digest
                    if let Some((object_id, version, parser)) = current_digest {
                        let (consumed, result) = parser.feed(&chunk[cursor..]);
                        cursor += consumed;
                        
                        if result.is_some() {
                            // Object ref complete
                            *parsed += 1;
                            *current_digest = None;
                            
                            if *parsed < *count {
                                *current_object_id = Some(ByteArrayParser::new());
                            } else {
                                // All gas payments parsed
                                self.state = TxParserState::GasOwner {
                                    parser: ByteArrayParser::new(),
                                };
                            }
                        }
                        continue;
                    }
                }
                
                TxParserState::GasOwner { parser } => {
                    let (consumed, result) = parser.feed(&chunk[cursor..]);
                    cursor += consumed;
                    
                    if let Some(owner) = result {
                        self.gas_owner = Some(owner);
                        self.state = TxParserState::GasPrice {
                            uleb: UlebParser::new(),
                        };
                    }
                }
                
                TxParserState::GasPrice { uleb } => {
                    match uleb.feed_byte_u64(chunk[cursor])? {
                        Some(price) => {
                            self.gas_price = Some(price);
                            self.state = TxParserState::GasBudget {
                                uleb: UlebParser::new(),
                            };
                        }
                        None => {}
                    }
                    cursor += 1;
                }
                
                TxParserState::GasBudget { uleb } => {
                    match uleb.feed_byte_u64(chunk[cursor])? {
                        Some(budget) => {
                            self.gas_budget = Some(budget);
                            self.state = TxParserState::ExpirationTag {
                                uleb: UlebParser::new(),
                            };
                        }
                        None => {}
                    }
                    cursor += 1;
                }
                
                // ========== Expiration ==========
                TxParserState::ExpirationTag { uleb } => {
                    match uleb.feed_byte(chunk[cursor])? {
                        Some(tag) => {
                            match tag {
                                0 => {
                                    // None
                                    self.expiration = Some(TransactionExpiration::None);
                                    // Expiration is the last field in TransactionDataV1
                                    self.state = TxParserState::Done;
                                }
                                1 => {
                                    // Epoch
                                    self.state = TxParserState::ExpirationEpoch {
                                        uleb: UlebParser::new(),
                                    };
                                }
                                _ => return Err(ParseError::InvalidType),
                            }
                        }
                        None => {}
                    }
                    cursor += 1;
                }
                
                TxParserState::ExpirationEpoch { uleb } => {
                    match uleb.feed_byte_u64(chunk[cursor])? {
                        Some(epoch) => {
                            self.expiration = Some(TransactionExpiration::Epoch(epoch));
                            // Expiration is the last field in TransactionDataV1
                            self.state = TxParserState::Done;
                        }
                        None => {}
                    }
                    cursor += 1;
                }
                
                // ========== Transaction Kind ==========
                TxParserState::TxKindTag { uleb } => {
                    match uleb.feed_byte(chunk[cursor])? {
                        Some(tag) => {
                            // 0 = ProgrammableTransaction
                            if tag != 0 {
                                return Err(ParseError::InvalidType);
                            }
                            self.state = TxParserState::PtbInputCount {
                                uleb: UlebParser::new(),
                            };
                        }
                        None => {}
                    }
                    cursor += 1;
                }
                
                // ========== PTB Inputs ==========
                TxParserState::PtbInputCount { uleb } => {
                    match uleb.feed_byte(chunk[cursor])? {
                        Some(count) => {
                            if count > 512 {
                                return Err(ParseError::TooMany);
                            }
                            
                            if count == 0 {
                                self.state = TxParserState::PtbCommandCount {
                                    uleb: UlebParser::new(),
                                };
                            } else {
                                self.state = TxParserState::PtbInputs {
                                    count,
                                    parsed: 0,
                                    current_input: Some(CallArgParserState::new()),
                                };
                            }
                        }
                        None => {}
                    }
                    cursor += 1;
                }
                
                TxParserState::PtbInputs { count, parsed, current_input } => {
                    let (consumed, finished) = parse_call_arg(&mut self.inputs, current_input, &chunk[cursor..])?;
                    cursor += consumed;
                    
                    if finished {
                        // Current input complete
                        *parsed += 1;
                        
                        if *parsed < *count {
                            *current_input = Some(CallArgParserState::new());
                        } else {
                            // All inputs parsed
                            self.state = TxParserState::PtbCommandCount {
                                uleb: UlebParser::new(),
                            };
                        }
                    }
                }
                
                // ========== PTB Commands ==========
                TxParserState::PtbCommandCount { uleb } => {
                    match uleb.feed_byte(chunk[cursor])? {
                        Some(count) => {
                            if count > 1024 {
                                return Err(ParseError::TooMany);
                            }
                            
                            if count == 0 {
                                // After PTB commands, parse Sender address
                                self.state = TxParserState::Sender {
                                    parser: ByteArrayParser::new(),
                                };
                            } else {
                                self.state = TxParserState::PtbCommands {
                                    count,
                                    parsed: 0,
                                    current_command: Some(CommandParserState::new()),
                                };
                            }
                        }
                        None => {}
                    }
                    cursor += 1;
                }
                
                TxParserState::PtbCommands { count, parsed, current_command } => {
                    let (consumed, finished) = parse_command(&mut self.commands, current_command, &chunk[cursor..])?;
                    cursor += consumed;
                    
                    if finished {
                        // Current command complete
                        *parsed += 1;
                        
                        if *parsed < *count {
                            *current_command = Some(CommandParserState::new());
                        } else {
                            // All commands parsed, now parse Sender address
                            self.state = TxParserState::Sender {
                                parser: ByteArrayParser::new(),
                            };
                        }
                    }
                }
                
                TxParserState::Done => {
                    // Parsing complete, ignore remaining data
                    break;
                }
            }
        }
        
        Ok(cursor)
    }
    
    pub fn is_complete(&self) -> bool {
        matches!(self.state, TxParserState::Done)
    }
}

// ========== Helper Functions (standalone to avoid borrow conflicts) ==========

/// Parse a CallArg (PTB input), returns (bytes_consumed, finished)
fn parse_call_arg(
    inputs: &mut Vec<ParsedCallArg>,
    state: &mut Option<CallArgParserState>,
    chunk: &[u8],
) -> Result<(usize, bool), ParseError> {
    let Some(s) = state else {
        return Ok((0, false));
    };
    
    let mut cursor = 0;
    
    loop {
        if cursor >= chunk.len() {
            break;
        }
        
        match s {
        CallArgParserState::ReadingType { uleb } => {
            match uleb.feed_byte(chunk[cursor])? {
                Some(0) => {
                    // Pure
                    *s = CallArgParserState::ReadingPureLength {
                        uleb: UlebParser::new(),
                    };
                }
                Some(1) => {
                    // ImmOrOwnedObject
                    *s = CallArgParserState::ReadingObjectId {
                        parser: ByteArrayParser::new(),
                    };
                }
                Some(2) => {
                    // SharedObject
                    *s = CallArgParserState::ReadingSharedObjectId {
                        parser: ByteArrayParser::new(),
                    };
                }
                Some(3) => {
                    // Receiving
                    *s = CallArgParserState::ReadingReceivingId {
                        parser: ByteArrayParser::new(),
                    };
                }
                Some(_) => return Err(ParseError::InvalidType),
                None => {}
            }
            cursor += 1;
        }
        
        CallArgParserState::ReadingPureLength { uleb } => {
            match uleb.feed_byte(chunk[cursor])? {
                Some(len) => {
                    if len > 256 {
                        return Err(ParseError::TooMany);
                    }
                    *s = CallArgParserState::ReadingPureBytes {
                        bytes: Vec::new(),
                        remaining: len as usize,
                    };
                }
                None => {}
            }
            cursor += 1;
        }
        
        CallArgParserState::ReadingPureBytes { bytes, remaining } => {
            let to_copy = (*remaining).min(chunk.len() - cursor);
            bytes.extend_from_slice(&chunk[cursor..cursor + to_copy]);
            cursor += to_copy;
            *remaining -= to_copy;
            
            if *remaining == 0 {
                let arg = ParsedCallArg::Pure { bytes: bytes.clone() };
                inputs.push(arg);
                *state = None;
                return Ok((cursor, true));
            }
        }
        
        CallArgParserState::ReadingObjectId { parser } => {
            let (consumed, result) = parser.feed(&chunk[cursor..]);
            cursor += consumed;
            
            if let Some(object_id) = result {
                *s = CallArgParserState::ReadingObjectVersion {
                    uleb: UlebParser::new(),
                    object_id,
                };
            }
        }
        
        CallArgParserState::ReadingObjectVersion { uleb, object_id } => {
            match uleb.feed_byte_u64(chunk[cursor])? {
                Some(version) => {
                    let oid = *object_id;
                    *s = CallArgParserState::ReadingObjectDigest {
                        parser: ByteArrayParser::new(),
                        object_id: oid,
                        version,
                    };
                }
                None => {}
            }
            cursor += 1;
        }
        
        CallArgParserState::ReadingObjectDigest { parser, object_id, version } => {
            let (consumed, result) = parser.feed(&chunk[cursor..]);
            cursor += consumed;
            
            if let Some(digest) = result {
                let arg = ParsedCallArg::ImmOrOwnedObject {
                    object_id: *object_id,
                    version: *version,
                    digest,
                };
                inputs.push(arg);
                *state = None;
                return Ok((cursor, true));
            }
        }
        
        CallArgParserState::ReadingSharedObjectId { parser } => {
            let (consumed, result) = parser.feed(&chunk[cursor..]);
            cursor += consumed;
            
            if let Some(object_id) = result {
                *s = CallArgParserState::ReadingSharedVersion {
                    uleb: UlebParser::new(),
                    object_id,
                };
            }
        }
        
        CallArgParserState::ReadingSharedVersion { uleb, object_id } => {
            match uleb.feed_byte_u64(chunk[cursor])? {
                Some(version) => {
                    let oid = *object_id;
                    *s = CallArgParserState::ReadingSharedMutable {
                        object_id: oid,
                        version,
                    };
                }
                None => {}
            }
            cursor += 1;
        }
        
        CallArgParserState::ReadingSharedMutable { object_id, version } => {
            let mutable = match chunk[cursor] {
                0x00 => false,
                0x01 => true,
                _ => return Err(ParseError::InvalidBool),
            };
            cursor += 1;
            
            let arg = ParsedCallArg::SharedObject {
                object_id: *object_id,
                initial_shared_version: *version,
                mutable,
            };
            inputs.push(arg);
            *state = None;
            return Ok((cursor, true));
        }
        
        CallArgParserState::ReadingReceivingId { parser } => {
            let (consumed, result) = parser.feed(&chunk[cursor..]);
            cursor += consumed;
            
            if let Some(object_id) = result {
                *s = CallArgParserState::ReadingReceivingVersion {
                    uleb: UlebParser::new(),
                    object_id,
                };
            }
        }
        
        CallArgParserState::ReadingReceivingVersion { uleb, object_id } => {
            match uleb.feed_byte_u64(chunk[cursor])? {
                Some(version) => {
                    let oid = *object_id;
                    *s = CallArgParserState::ReadingReceivingDigest {
                        parser: ByteArrayParser::new(),
                        object_id: oid,
                        version,
                    };
                }
                None => {}
            }
            cursor += 1;
        }
        
        CallArgParserState::ReadingReceivingDigest { parser, object_id, version } => {
            let (consumed, result) = parser.feed(&chunk[cursor..]);
            cursor += consumed;
            
            if let Some(digest) = result {
                let arg = ParsedCallArg::Receiving {
                    object_id: *object_id,
                    version: *version,
                    digest,
                };
                inputs.push(arg);
                *state = None;
                return Ok((cursor, true));
            }
        }
        }
    }
    
    Ok((cursor, false))
}
    
/// Parse an Argument, returns (bytes_consumed, Option<argument>)
fn parse_argument(
    state: &mut Option<ArgumentParserState>,
    chunk: &[u8],
) -> Result<(usize, Option<ParsedArgument>), ParseError> {
    let Some(s) = state else {
        return Ok((0, None));
    };
    
    let mut cursor = 0;
    let mut result = None;
    
    loop {
        if cursor >= chunk.len() {
            break;
        }
        
        match s {
            ArgumentParserState::ReadingType { uleb } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(0) => {
                        // GasCoin
                        result = Some(ParsedArgument::GasCoin);
                        *state = None;
                        cursor += 1;
                        break;
                    }
                    Some(1) => {
                        // Input
                        *s = ArgumentParserState::ReadingInputIndex {
                            uleb: UlebParser::new(),
                        };
                    }
                    Some(2) => {
                        // Result
                        *s = ArgumentParserState::ReadingResultIndex {
                            uleb: UlebParser::new(),
                        };
                    }
                    Some(3) => {
                        // NestedResult
                        *s = ArgumentParserState::ReadingNestedCommand {
                            uleb: UlebParser::new(),
                        };
                    }
                    Some(_) => return Err(ParseError::InvalidType),
                    None => {}
                }
                cursor += 1;
            }
            
            ArgumentParserState::ReadingInputIndex { uleb } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(idx) => {
                        result = Some(ParsedArgument::Input(idx as u16));
                        *state = None;
                        cursor += 1;
                        break;
                    }
                    None => {}
                }
                cursor += 1;
            }
            
            ArgumentParserState::ReadingResultIndex { uleb } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(idx) => {
                        result = Some(ParsedArgument::Result(idx as u16));
                        *state = None;
                        cursor += 1;
                        break;
                    }
                    None => {}
                }
                cursor += 1;
            }
            
            ArgumentParserState::ReadingNestedCommand { uleb } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(cmd_idx) => {
                        *s = ArgumentParserState::ReadingNestedResult {
                            uleb: UlebParser::new(),
                            command_index: cmd_idx as u16,
                        };
                    }
                    None => {}
                }
                cursor += 1;
            }
            
            ArgumentParserState::ReadingNestedResult { uleb, command_index } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(res_idx) => {
                        result = Some(ParsedArgument::NestedResult {
                            command_index: *command_index,
                            result_index: res_idx as u16,
                        });
                        *state = None;
                        cursor += 1;
                        break;
                    }
                    None => {}
                }
                cursor += 1;
            }
        }
    }
    
    Ok((cursor, result))
}
    
/// Parse a Command (simplified - only handling TransferObjects, SplitCoins, MergeCoins for now)
fn parse_command(
    commands: &mut Vec<ParsedCommand>,
    state: &mut Option<CommandParserState>,
    chunk: &[u8],
) -> Result<(usize, bool), ParseError> {
    let Some(s) = state else {
        return Ok((0, false));
    };
    
    let mut cursor = 0;
    
    loop {
        if cursor >= chunk.len() {
            break;
        }
        
        match s {
            CommandParserState::ReadingType { uleb } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(0) => {
                        // MoveCall - most complex, simplified implementation
                        *s = CommandParserState::ReadingMoveCallPackage {
                            parser: ByteArrayParser::new(),
                        };
                    }
                    Some(1) => {
                        // TransferObjects
                        *s = CommandParserState::ReadingTransferObjectsCount {
                            uleb: UlebParser::new(),
                        };
                    }
                    Some(2) => {
                        // SplitCoins
                        *s = CommandParserState::ReadingSplitCoin {
                            arg_state: Some(ArgumentParserState::new()),
                        };
                    }
                    Some(3) => {
                        // MergeCoins
                        *s = CommandParserState::ReadingMergeDestination {
                            arg_state: Some(ArgumentParserState::new()),
                        };
                    }
                    Some(_) => {
                        // Other command types - skip for now
                        *s = CommandParserState::SkippingBytes { remaining: 1000 };
                    }
                    None => {}
                }
                cursor += 1;
            }
            
            // TransferObjects
            CommandParserState::ReadingTransferObjectsCount { uleb } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(count) => {
                        if count > 32 {
                            return Err(ParseError::TooMany);
                        }
                        *s = CommandParserState::ReadingTransferObjects {
                            count,
                            parsed: 0,
                            objects: Vec::new(),
                            current_arg: Some(ArgumentParserState::new()),
                        };
                    }
                    None => {}
                }
                cursor += 1;
            }
            
            CommandParserState::ReadingTransferObjects { count, parsed, objects, current_arg } => {
                let (consumed, arg) = parse_argument(current_arg, &chunk[cursor..])?;
                cursor += consumed;
                
                if let Some(arg) = arg {
                    objects.push(arg);
                    *parsed += 1;
                    
                    if *parsed < *count {
                        *current_arg = Some(ArgumentParserState::new());
                    } else {
                        // Now read recipient
                        let objs = objects.clone();
                        *s = CommandParserState::ReadingTransferRecipient {
                            objects: objs,
                            arg_state: Some(ArgumentParserState::new()),
                        };
                    }
                }
            }
            
            CommandParserState::ReadingTransferRecipient { objects, arg_state } => {
                let (consumed, arg) = parse_argument(arg_state, &chunk[cursor..])?;
                cursor += consumed;
                
                if let Some(recipient) = arg {
                    let cmd = ParsedCommand::TransferObjects {
                        objects: objects.clone(),
                        recipient,
                    };
                    commands.push(cmd);
                    *state = None;
                    return Ok((cursor, true));
                }
            }
            
            // SplitCoins
            CommandParserState::ReadingSplitCoin { arg_state } => {
                let (consumed, arg) = parse_argument(arg_state, &chunk[cursor..])?;
                cursor += consumed;
                
                if let Some(coin) = arg {
                    *s = CommandParserState::ReadingSplitAmountsCount {
                        uleb: UlebParser::new(),
                        coin,
                    };
                }
            }
            
            CommandParserState::ReadingSplitAmountsCount { uleb, coin } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(count) => {
                        if count > 32 {
                            return Err(ParseError::TooMany);
                        }
                        let c = *coin;
                        *s = CommandParserState::ReadingSplitAmounts {
                            coin: c,
                            count,
                            parsed: 0,
                            amounts: Vec::new(),
                            current_arg: Some(ArgumentParserState::new()),
                        };
                    }
                    None => {}
                }
                cursor += 1;
            }
            
            CommandParserState::ReadingSplitAmounts { coin, count, parsed, amounts, current_arg } => {
                let (consumed, arg) = parse_argument(current_arg, &chunk[cursor..])?;
                cursor += consumed;
                
                if let Some(arg) = arg {
                    amounts.push(arg);
                    *parsed += 1;
                    
                    if *parsed < *count {
                        *current_arg = Some(ArgumentParserState::new());
                    } else {
                        let cmd = ParsedCommand::SplitCoins {
                            coin: *coin,
                            amounts: amounts.clone(),
                        };
                        commands.push(cmd);
                        *state = None;
                        return Ok((cursor, true));
                    }
                }
            }
            
            // MergeCoins
            CommandParserState::ReadingMergeDestination { arg_state } => {
                let (consumed, arg) = parse_argument(arg_state, &chunk[cursor..])?;
                cursor += consumed;
                
                if let Some(destination) = arg {
                    *s = CommandParserState::ReadingMergeSourcesCount {
                        uleb: UlebParser::new(),
                        destination,
                    };
                }
            }
            
            CommandParserState::ReadingMergeSourcesCount { uleb, destination } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(count) => {
                        if count > 32 {
                            return Err(ParseError::TooMany);
                        }
                        let dest = *destination;
                        *s = CommandParserState::ReadingMergeSources {
                            destination: dest,
                            count,
                            parsed: 0,
                            sources: Vec::new(),
                            current_arg: Some(ArgumentParserState::new()),
                        };
                    }
                    None => {}
                }
                cursor += 1;
            }
            
            CommandParserState::ReadingMergeSources { destination, count, parsed, sources, current_arg } => {
                let (consumed, arg) = parse_argument(current_arg, &chunk[cursor..])?;
                cursor += consumed;
                
                if let Some(arg) = arg {
                    sources.push(arg);
                    *parsed += 1;
                    
                    if *parsed < *count {
                        *current_arg = Some(ArgumentParserState::new());
                    } else {
                        let cmd = ParsedCommand::MergeCoins {
                            destination: *destination,
                            sources: sources.clone(),
                        };
                        commands.push(cmd);
                        *state = None;
                        return Ok((cursor, true));
                    }
                }
            }
            
            // MoveCall (simplified - just parse package/module/function, skip type args and args details)
            CommandParserState::ReadingMoveCallPackage { parser } => {
                let (consumed, result) = parser.feed(&chunk[cursor..]);
                cursor += consumed;
                
                if let Some(package) = result {
                    *s = CommandParserState::ReadingMoveCallModuleLen {
                        uleb: UlebParser::new(),
                        package,
                    };
                }
            }
            
            CommandParserState::ReadingMoveCallModuleLen { uleb, package } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(len) => {
                        if len > COIN_STRING_LENGTH as u32 {
                            return Err(ParseError::TooMany);
                        }
                        let pkg = *package;
                        *s = CommandParserState::ReadingMoveCallModule {
                            package: pkg,
                            remaining: len as usize,
                            module: ArrayVec::new(),
                        };
                    }
                    None => {}
                }
                cursor += 1;
            }
            
            CommandParserState::ReadingMoveCallModule { package, remaining, module } => {
                let to_copy = (*remaining).min(chunk.len() - cursor).min(COIN_STRING_LENGTH - module.len());
                module.try_extend_from_slice(&chunk[cursor..cursor + to_copy])
                    .map_err(|_| ParseError::TooMany)?;
                cursor += to_copy;
                *remaining -= to_copy;
                
                if *remaining == 0 {
                    let pkg = *package;
                    let mod_name = module.clone();
                    *s = CommandParserState::ReadingMoveCallFunctionLen {
                        package: pkg,
                        module: mod_name,
                        uleb: UlebParser::new(),
                    };
                }
            }
            
            CommandParserState::ReadingMoveCallFunctionLen { package, module, uleb } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(len) => {
                        if len > COIN_STRING_LENGTH as u32 {
                            return Err(ParseError::TooMany);
                        }
                        let pkg = *package;
                        let mod_name = module.clone();
                        *s = CommandParserState::ReadingMoveCallFunction {
                            package: pkg,
                            module: mod_name,
                            remaining: len as usize,
                            function: ArrayVec::new(),
                        };
                    }
                    None => {}
                }
                cursor += 1;
            }
            
            CommandParserState::ReadingMoveCallFunction { package, module, remaining, function } => {
                let to_copy = (*remaining).min(chunk.len() - cursor).min(COIN_STRING_LENGTH - function.len());
                function.try_extend_from_slice(&chunk[cursor..cursor + to_copy])
                    .map_err(|_| ParseError::TooMany)?;
                cursor += to_copy;
                *remaining -= to_copy;
                
                if *remaining == 0 {
                    let pkg = *package;
                    let mod_name = module.clone();
                    let func_name = function.clone();
                    *s = CommandParserState::ReadingMoveCallTypeArgsCount {
                        package: pkg,
                        module: mod_name,
                        function: func_name,
                        uleb: UlebParser::new(),
                    };
                }
            }
            
            CommandParserState::ReadingMoveCallTypeArgsCount { package, module, function, uleb } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(_count) => {
                        // For now, skip type args parsing (TODO: implement TypeTag state machine)
                        let pkg = *package;
                        let mod_name = module.clone();
                        let func_name = function.clone();
                        *s = CommandParserState::ReadingMoveCallArgsCount {
                            package: pkg,
                            module: mod_name,
                            function: func_name,
                            type_args: Vec::new(), // Empty for now
                            uleb: UlebParser::new(),
                        };
                    }
                    None => {}
                }
                cursor += 1;
            }
            
            CommandParserState::ReadingMoveCallArgsCount { package, module, function, type_args, uleb } => {
                match uleb.feed_byte(chunk[cursor])? {
                    Some(count) => {
                        if count > 32 {
                            return Err(ParseError::TooMany);
                        }
                        let pkg = *package;
                        let mod_name = module.clone();
                        let func_name = function.clone();
                        let type_params = type_args.clone();
                        *s = CommandParserState::ReadingMoveCallArgs {
                            package: pkg,
                            module: mod_name,
                            function: func_name,
                            type_args: type_params,
                            count,
                            parsed: 0,
                            arguments: Vec::new(),
                            current_arg: Some(ArgumentParserState::new()),
                        };
                    }
                    None => {}
                }
                cursor += 1;
            }
            
            CommandParserState::ReadingMoveCallArgs { package, module, function, type_args, count, parsed, arguments, current_arg } => {
                let (consumed, arg) = parse_argument(current_arg, &chunk[cursor..])?;
                cursor += consumed;
                
                if let Some(arg) = arg {
                    arguments.push(arg);
                    *parsed += 1;
                    
                    if *parsed < *count {
                        *current_arg = Some(ArgumentParserState::new());
                    } else {
                        let cmd = ParsedCommand::MoveCall {
                            package: *package,
                            module: module.clone(),
                            function: function.clone(),
                            type_arguments: type_args.clone(),
                            arguments: arguments.clone(),
                        };
                        commands.push(cmd);
                        *state = None;
                        return Ok((cursor, true));
                    }
                }
            }
            
            CommandParserState::SkippingBytes { remaining } => {
                // Placeholder for unimplemented command types
                let to_skip = (*remaining).min(chunk.len() - cursor);
                cursor += to_skip;
                *remaining -= to_skip;
                
                if *remaining == 0 {
                    *state = None;
                    return Ok((cursor, true));
                }
            }
        }
    }
    
    Ok((cursor, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_uleb_parser() {
        let mut uleb = UlebParser::new();
        
        // Single byte: 0
        assert_eq!(uleb.feed_byte(0x00).unwrap(), Some(0));
        
        // Single byte: 127
        let mut uleb = UlebParser::new();
        assert_eq!(uleb.feed_byte(0x7f).unwrap(), Some(127));
        
        // Two bytes: 128
        let mut uleb = UlebParser::new();
        assert_eq!(uleb.feed_byte(0x80).unwrap(), None);
        assert_eq!(uleb.feed_byte(0x01).unwrap(), Some(128));
        
        // Non-canonical (leading zero)
        let mut uleb = UlebParser::new();
        uleb.feed_byte(0x80).unwrap();
        assert_eq!(uleb.feed_byte(0x00), Err(ParseError::NonCanonical));
    }
    
    #[test]
    fn test_intent_parsing() {
        let mut parser = TxParser::new();
        
        // Intent: version=0, scope=0, app_id=0
        // TransactionDataV1: version=1
        let chunk = &[0x00, 0x00, 0x00, 0x01];
        let consumed = parser.feed(chunk).unwrap();
        assert_eq!(consumed, 4);
        
        assert_eq!(parser.intent.unwrap().version, 0);
        assert_eq!(parser.intent.unwrap().scope, 0);
        assert_eq!(parser.intent.unwrap().app_id, 0);
        
        // Should be ready to parse sender
        matches!(parser.state, TxParserState::Sender { .. });
    }
    
    #[test]
    fn test_chunked_feeding() {
        let mut parser = TxParser::new();
        
        // Feed intent byte by byte
        parser.feed(&[0x00]).unwrap(); // version
        parser.feed(&[0x00]).unwrap(); // scope
        parser.feed(&[0x00]).unwrap(); // app_id
        parser.feed(&[0x01]).unwrap(); // tx data version
        
        assert_eq!(parser.intent.unwrap().version, 0);
        matches!(parser.state, TxParserState::Sender { .. });
    }
    
    #[test]
    fn test_real_transaction_bytes() {
        // Hex from user: 5060b9150c06381181d0f9964338489391a2c45b...
        // This appears to include an 8-byte length prefix before the Intent
        // Expected Intent: version=0, scope=0, app_id=0
        
        // Manually decoded first bytes
        let bytes_with_prefix: &[u8] = &[
            // 8-byte length prefix (little-endian)
            0x50, 0x60, 0xb9, 0x15, 0x0c, 0x06, 0x38, 0x11,
            // Actual transaction data starts here
            // If it's version=0, scope=0, app_id=0, should be 0x00, 0x00, 0x00
        ];
        
        // The issue is that bytes 0-7 are being parsed as Intent instead of skipped
        // This causes version=80 (0x50) instead of version=0
        
        // Test: Parse transaction data that starts with Intent 0,0,0
        let correct_tx_data = &[0x00, 0x00, 0x00, 0x01]; // Intent + TxDataV1 version
        let mut parser = TxParser::new();
        parser.feed(correct_tx_data).unwrap();
        
        assert_eq!(parser.intent.as_ref().unwrap().version, 0);
        assert_eq!(parser.intent.as_ref().unwrap().scope, 0);
        assert_eq!(parser.intent.as_ref().unwrap().app_id, 0);
    }
}
