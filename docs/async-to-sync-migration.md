# Async to Sync Parser Migration Guide

## Overview

This document describes the strategy for migrating the Sui Ledger app from asynchronous parsing (using `alamgu-async-block` and `AsyncParser`) to synchronous parsing (using `InterpParser`). The goal is to simplify the codebase by removing async abstractions that don't provide real async I/O benefits in the embedded Ledger environment.

**Status**: Migration in progress
**Start Date**: February 2026
**Estimated Duration**: 2-4 weeks
**Target Completion**: March 2026

---

## Table of Contents

1. [Rationale](#rationale)
2. [Prerequisites](#prerequisites)
3. [Migration Steps](#migration-steps)
4. [Testing Strategy](#testing-strategy)
5. [Common Pitfalls](#common-pitfalls)
6. [Rollback Plan](#rollback-plan)
7. [Reference](#reference)

---

## Rationale

### Why Remove Async?

#### Advantages of Synchronous Parsing

1. **Simpler execution model** - No async state machines or cooperative scheduling overhead
2. **More predictable memory usage** - Stack-based state management vs heap-allocated futures
3. **Better debuggability** - Synchronous stack traces are easier to follow than async chains
4. **Smaller binary size** - Eliminating async machinery reduces code size (est. 10-15% reduction)
5. **Fits the Ledger model** - APDU commands are inherently request-response, not truly async I/O
6. **Reduced dependencies** - Removes `alamgu-async-block` dependency

#### Why Current Async is "Fake Async"

The Ledger SDK uses "cooperative async" via `alamgu-async-block`, which is really just a way to write state machines with nicer syntax. It's not true async I/O:
- No concurrent execution
- No real async I/O operations
- Just syntactic sugar over state machines
- Adds complexity without corresponding benefits

### Trade-offs

#### What We Lose
- Elegant async/await syntax
- Composable async combinators
- Currently working implementation

#### What We Gain
- Simpler mental model
- More explicit control flow
- Better suited for embedded systems
- Easier debugging and profiling

---

## Prerequisites

### Required Knowledge
- Rust embedded development
- Parser combinator patterns
- Ledger APDU protocol
- Sui transaction structure
- BCS serialization format

### Feature Flag Infrastructure
✅ **Already Complete** - The codebase has feature flags in place:
- `sync` feature: Enables synchronous implementation
- Default (no feature): Uses async implementation
- Files organized in `src/sync/` directory

### Development Environment
- Rust nightly toolchain (see `rust-toolchain.toml`)
- Ledger emulator (Speculos)
- Test framework (Ragger)
- All 5 device targets: nanosplus, nanox, stax, flex, apex_p

---

## Migration Steps

### Step 1: Feature Flag Infrastructure ✅
**Status**: Complete  
**Duration**: N/A

The feature flag infrastructure is already in place:
```rust
// In lib.rs
#[cfg(all(target_family = "bolos", not(feature = "sync")))]
pub mod ctx;  // async

#[cfg(all(target_family = "bolos", feature = "sync"))]
pub mod sync;  // sync
```

Files are organized:
- `src/` - async implementation (default)
- `src/sync/` - sync implementation (feature gated)

---

### Step 2: Implement BCS Sync Parsers
**Status**: To Do  
**Duration**: 1-2 days  
**Dependencies**: None

#### 2.1 Create File Structure

```bash
touch rust-app/ledger-parser-combinators/src/bcs/interp_parser.rs
```

#### 2.2 Update Module Declaration

In `ledger-parser-combinators/src/bcs/mod.rs`:
```rust
pub mod async_parser;

#[cfg(feature = "sync")]
pub mod interp_parser;

#[cfg(feature = "sync")]
pub use interp_parser::*;
```

#### 2.3 Implement `bool` Parser

```rust
use crate::interp_parser::*;
use crate::core_parsers::*;

impl ParserCommon<bool> for DefaultInterp {
    type State = ();
    type Returning = bool;
    fn init(&self) -> Self::State { () }
}

impl InterpParser<bool> for DefaultInterp {
    fn parse<'a, 'b>(
        &self,
        _state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        match chunk.split_first() {
            None => Err((None, chunk)),
            Some((0, rest)) => {
                *destination = Some(false);
                Ok(rest)
            }
            Some((1, rest)) => {
                *destination = Some(true);
                Ok(rest)
            }
            Some((_, rest)) => Err(rej(rest)),
        }
    }
}
```

#### 2.4 Implement `Option<T>` Parser

```rust
pub enum OptionParserState<S> {
    ReadingDiscriminant,
    ReadingValue(S),
    Done,
}

impl<T, S: ParserCommon<T>> ParserCommon<Option<T>> for SubInterp<S> {
    type State = OptionParserState<S::State>;
    type Returning = Option<S::Returning>;
    
    fn init(&self) -> Self::State {
        OptionParserState::ReadingDiscriminant
    }
}

impl<T, S: InterpParser<T>> InterpParser<Option<T>> for SubInterp<S> {
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        use OptionParserState::*;
        let mut cursor = chunk;
        
        loop {
            match state {
                ReadingDiscriminant => {
                    match cursor.split_first() {
                        None => return Err((None, cursor)),
                        Some((0, rest)) => {
                            *destination = Some(None);
                            return Ok(rest);
                        }
                        Some((1, rest)) => {
                            cursor = rest;
                            set_from_thunk(state, || ReadingValue(self.0.init()));
                        }
                        Some((_, rest)) => return Err(rej(rest)),
                    }
                }
                ReadingValue(ref mut inner_state) => {
                    let mut inner_dest = None;
                    cursor = self.0.parse(inner_state, cursor, &mut inner_dest)?;
                    *destination = Some(inner_dest);
                    set_from_thunk(state, || Done);
                    return Ok(cursor);
                }
                Done => return Err(rej(cursor)),
            }
        }
    }
}
```

#### 2.5 Implement `ULEB128` Parser (Critical)

This is the most complex parser due to variable-length encoding.

```rust
pub struct ULEB128;

pub struct ULEB128State {
    value: u64,
    shift: u32,
    finished: bool,
}

impl ParserCommon<ULEB128> for DefaultInterp {
    type State = ULEB128State;
    type Returning = u32;
    
    fn init(&self) -> Self::State {
        ULEB128State {
            value: 0,
            shift: 0,
            finished: false,
        }
    }
}

impl InterpParser<ULEB128> for DefaultInterp {
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        let mut cursor = chunk;
        
        while state.shift < 32 && !state.finished {
            match cursor.split_first() {
                None => return Err((None, cursor)),
                Some((byte, rest)) => {
                    let digit = byte & 0x7f;
                    state.value |= u64::from(digit) << state.shift;
                    
                    // If high bit is 0, this is the last byte
                    if digit == *byte {
                        // Reject non-canonical encodings
                        if state.shift > 0 && digit == 0 {
                            return Err(rej(rest));
                        }
                        state.finished = true;
                        
                        // Check for overflow
                        use core::convert::TryFrom;
                        match u32::try_from(state.value) {
                            Ok(v) => {
                                *destination = Some(v);
                                return Ok(rest);
                            }
                            Err(_) => return Err(rej(rest)),
                        }
                    }
                    
                    state.shift += 7;
                    cursor = rest;
                }
            }
        }
        
        // Reached 32 bits without termination - overflow
        if !state.finished {
            return Err(rej(cursor));
        }
        
        Err((None, cursor))
    }
}

pub type Vec<T, const N: usize> = DArray<ULEB128, T, N>;
```

#### 2.6 Add Unit Tests

Create comprehensive tests for each parser:

```rust
#[cfg(test)]
mod test {
    use super::*;
    
    #[test]
    fn test_bool_true() {
        let parser = DefaultInterp;
        let mut state = parser.init();
        let mut dest = None;
        assert_eq!(parser.parse(&mut state, &[1], &mut dest), Ok(&[][..]));
        assert_eq!(dest, Some(true));
    }
    
    #[test]
    fn test_bool_false() {
        let parser = DefaultInterp;
        let mut state = parser.init();
        let mut dest = None;
        assert_eq!(parser.parse(&mut state, &[0], &mut dest), Ok(&[][..]));
        assert_eq!(dest, Some(false));
    }
    
    #[test]
    fn test_bool_invalid() {
        let parser = DefaultInterp;
        let mut state = parser.init();
        let mut dest = None;
        assert!(matches!(parser.parse(&mut state, &[2], &mut dest), Err(_)));
    }
    
    #[test]
    fn test_uleb128_single_byte() {
        let parser = DefaultInterp;
        let mut state = parser.init();
        let mut dest = None;
        assert_eq!(parser.parse(&mut state, &[0x01], &mut dest), Ok(&[][..]));
        assert_eq!(dest, Some(1));
    }
    
    #[test]
    fn test_uleb128_multi_byte() {
        let parser = DefaultInterp;
        let mut state = parser.init();
        let mut dest = None;
        // 9487 = 0x8f 0x4a
        assert_eq!(parser.parse(&mut state, &[0x8f, 0x4a], &mut dest), Ok(&[][..]));
        assert_eq!(dest, Some(9487));
    }
    
    #[test]
    fn test_uleb128_chunked() {
        let parser = DefaultInterp;
        let mut state = parser.init();
        let mut dest = None;
        
        // Feed first byte
        let result = parser.parse(&mut state, &[0x8f], &mut dest);
        assert!(matches!(result, Err((None, _))));
        assert_eq!(dest, None);
        
        // Feed second byte
        let result = parser.parse(&mut state, &[0x4a], &mut dest);
        assert_eq!(result, Ok(&[][..]));
        assert_eq!(dest, Some(9487));
    }
    
    #[test]
    fn test_uleb128_2_pow_28() {
        let parser = DefaultInterp;
        let mut state = parser.init();
        let mut dest = None;
        let data = [0x80, 0x80, 0x80, 0x80, 0x01];
        assert_eq!(parser.parse(&mut state, &data, &mut dest), Ok(&[][..]));
        assert_eq!(dest, Some(268435456));
    }
    
    #[test]
    fn test_uleb128_non_canonical() {
        let parser = DefaultInterp;
        let mut state = parser.init();
        let mut dest = None;
        // 0x80 0x00 is not canonical encoding of 0
        assert!(matches!(parser.parse(&mut state, &[0x80, 0x00], &mut dest), Err(_)));
    }
    
    #[test]
    fn test_uleb128_overflow() {
        let parser = DefaultInterp;
        let mut state = parser.init();
        let mut dest = None;
        // Too large for u32
        let data = [0x80, 0x80, 0x80, 0x80, 0x80, 0x01];
        assert!(matches!(parser.parse(&mut state, &data, &mut dest), Err(_)));
    }
}
```

#### 2.7 Validation

Run tests to ensure BCS parsers work correctly:
```bash
cd rust-app/ledger-parser-combinators
cargo test --features sync bcs::interp_parser
```

---

### Step 3: Port Simple Parser (BIP32 Path)
**Status**: To Do  
**Duration**: 2-3 days  
**Dependencies**: Step 2 complete

This step validates the approach on a simple parser before tackling complex transaction parsers.

#### 3.1 Create Parser File

Create `rust-app/src/parser/bip32_sync.rs`:

```rust
use ledger_parser_combinators::interp_parser::*;
use ledger_parser_combinators::core_parsers::*;
use arrayvec::ArrayVec;

pub struct Bip32Key;

pub struct Bip32ParserState {
    length: Option<u8>,
    elements_read: usize,
    buffer: ArrayVec<u32, 10>,
}

impl ParserCommon<Bip32Key> for DefaultInterp {
    type State = Bip32ParserState;
    type Returning = ArrayVec<u32, 10>;
    
    fn init(&self) -> Self::State {
        Bip32ParserState {
            length: None,
            elements_read: 0,
            buffer: ArrayVec::new(),
        }
    }
}

impl InterpParser<Bip32Key> for DefaultInterp {
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        let mut cursor = chunk;
        
        // Read length byte if we haven't yet
        if state.length.is_none() {
            match cursor.split_first() {
                None => return Err((None, cursor)),
                Some((len, rest)) => {
                    if *len > 10 {
                        return Err(rej(rest));
                    }
                    state.length = Some(*len);
                    cursor = rest;
                }
            }
        }
        
        let target_length = state.length.unwrap() as usize;
        
        // Read path elements (4 bytes each)
        while state.elements_read < target_length {
            if cursor.len() < 4 {
                return Err((None, cursor));
            }
            
            let element_bytes: [u8; 4] = [cursor[0], cursor[1], cursor[2], cursor[3]];
            let element = u32::from_le_bytes(element_bytes);
            
            if state.buffer.try_push(element).is_err() {
                return Err(rej(cursor));
            }
            state.elements_read += 1;
            cursor = &cursor[4..];
        }
        
        // Done!
        *destination = Some(state.buffer.clone());
        Ok(cursor)
    }
}
```

#### 3.2 Add Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bip32_parse_complete() {
        let parser = DefaultInterp;
        let mut state = ParserCommon::<Bip32Key>::init(&parser);
        let mut dest = None;
        
        // m/44'/784'/0'/0'/0'
        let data = [
            5,  // length
            0x2c, 0x00, 0x00, 0x80,  // 44' (0x8000002c)
            0x10, 0x03, 0x00, 0x80,  // 784' (0x80000310)
            0x00, 0x00, 0x00, 0x80,  // 0'
            0x00, 0x00, 0x00, 0x80,  // 0'
            0x00, 0x00, 0x00, 0x80,  // 0'
        ];
        
        let result = InterpParser::<Bip32Key>::parse(&parser, &mut state, &data, &mut dest);
        assert_eq!(result, Ok(&[][..]));
        let path = dest.unwrap();
        assert_eq!(path.len(), 5);
        assert_eq!(path[0], 0x8000002c);
        assert_eq!(path[1], 0x80000310);
    }
    
    #[test]
    fn test_bip32_parse_chunked() {
        let parser = DefaultInterp;
        let mut state = ParserCommon::<Bip32Key>::init(&parser);
        let mut dest = None;
        
        // Feed length + first element
        let chunk1 = [5, 0x2c, 0x00, 0x00, 0x80];
        let result = InterpParser::<Bip32Key>::parse(&parser, &mut state, &chunk1, &mut dest);
        assert!(matches!(result, Err((None, _))));
        
        // Feed remaining elements
        let chunk2 = [
            0x10, 0x03, 0x00, 0x80,
            0x00, 0x00, 0x00, 0x80,
            0x00, 0x00, 0x00, 0x80,
            0x00, 0x00, 0x00, 0x80,
        ];
        let result = InterpParser::<Bip32Key>::parse(&parser, &mut state, &chunk2, &mut dest);
        assert_eq!(result, Ok(&[][..]));
        assert_eq!(dest.unwrap().len(), 5);
    }
    
    #[test]
    fn test_bip32_invalid_length() {
        let parser = DefaultInterp;
        let mut state = ParserCommon::<Bip32Key>::init(&parser);
        let mut dest = None;
        
        let data = [11]; // > 10
        let result = InterpParser::<Bip32Key>::parse(&parser, &mut state, &data, &mut dest);
        assert!(matches!(result, Err(_)));
    }
}
```

#### 3.3 Integrate into sync/implementation.rs

Update `sync/implementation.rs` to use the new parser:

```rust
#[cfg(feature = "sync")]
use crate::parser::bip32_sync::*;

impl TryFrom<&[u8]> for Bip32Path {
    type Error = StatusWords;

    fn try_from(bs: &[u8]) -> Result<Self, Self::Error> {
        use ledger_parser_combinators::interp_parser::{ParserCommon, InterpParser};
        
        let parser = DefaultInterp;
        let mut state = ParserCommon::<Bip32Key>::init(&parser);
        let mut dest = None;
        
        match InterpParser::<Bip32Key>::parse(&parser, &mut state, bs, &mut dest) {
            Ok(remaining) if remaining.is_empty() => {
                Ok(Bip32Path(dest.ok_or(StatusWords::BadLen)?))
            }
            Ok(_) => Err(StatusWords::BadLen), // Extra data
            Err(_) => Err(StatusWords::BadLen),
        }
    }
}
```

#### 3.4 Test Integration

Test that address derivation still works:
```bash
cargo test --features sync --test test_pubkey_cmd
```

---

### Step 4: Port Transaction Parsers
**Status**: To Do  
**Duration**: 1-2 weeks  
**Dependencies**: Steps 2 & 3 complete

This is the bulk of the work. Each transaction type needs to be converted.

#### 4.1 General Pattern

**Async to Sync Conversion:**

```rust
// ASYNC VERSION
impl<BS: Readable> AsyncParser<Schema, BS> for DefaultInterp {
    fn parse(&self, input: &mut BS) -> impl Future<Output = Type> {
        async move {
            let field1 = parse_something(input).await;
            match field1 {
                Variant1 => { /* ... */ }
                Variant2 => { /* ... */ }
            }
        }
    }
}

// SYNC VERSION
pub enum SchemaParserState {
    ReadingField1(SubParserState),
    ProcessingVariant1(/* state */),
    ProcessingVariant2(/* state */),
    Done,
}

impl ParserCommon<Schema> for DefaultInterp {
    type State = SchemaParserState;
    type Returning = Type;
    fn init(&self) -> Self::State {
        SchemaParserState::ReadingField1(/* init */)
    }
}

impl InterpParser<Schema> for DefaultInterp {
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        use SchemaParserState::*;
        let mut cursor = chunk;
        
        loop {
            match state {
                ReadingField1(ref mut substate) => {
                    let mut subdest = None;
                    cursor = subparser.parse(substate, cursor, &mut subdest)?;
                    let field1 = subdest.ok_or_else(|| rej(cursor))?;
                    
                    match field1 {
                        Variant1 => {
                            set_from_thunk(state, || ProcessingVariant1(/* init */));
                        }
                        Variant2 => {
                            set_from_thunk(state, || ProcessingVariant2(/* init */));
                        }
                    }
                }
                ProcessingVariant1(ref mut substate) => {
                    // Parse variant 1
                    // ...
                    set_from_thunk(state, || Done);
                    return Ok(cursor);
                }
                ProcessingVariant2(ref mut substate) => {
                    // Parse variant 2
                    // ...
                    set_from_thunk(state, || Done);
                    return Ok(cursor);
                }
                Done => return Err(rej(cursor)),
            }
        }
    }
}
```

#### 4.2 File Organization

Create sync versions alongside async:
```
src/parser/
  tx.rs              # async (existing)
  tx_sync.rs         # sync (new)
  object.rs          # async (existing)
  object_sync.rs     # sync (new)
  tuid.rs            # async (existing)
  tuid_sync.rs       # sync (new)
  common.rs          # shared types (no changes needed)
```

#### 4.3 Priority Order

1. **Simple types first** (e.g., `ObjectDigest`, `Address`)
2. **CallArg** - Enum with multiple variants
3. **Argument** - Nested structure
4. **Command** - Complex enum with many variants
5. **Transaction** - Top-level structure
6. **Intent** - Wraps transaction

#### 4.4 CallArg Example (Detailed)

```rust
// In tx_sync.rs

pub enum CallArgParserState {
    ReadingTag(<DefaultInterp as ParserCommon<ULEB128>>::State),
    ReadingPure {
        length_state: <DefaultInterp as ParserCommon<ULEB128>>::State,
        length: Option<u32>,
        data: ArrayVec<u8, 16384>,
        bytes_read: usize,
    },
    ReadingObject(/* object reference state */),
    Done,
}

impl ParserCommon<CallArgSchema> for DefaultInterp {
    type State = CallArgParserState;
    type Returning = CallArg;
    
    fn init(&self) -> Self::State {
        CallArgParserState::ReadingTag(
            <DefaultInterp as ParserCommon<ULEB128>>::init(&DefaultInterp)
        )
    }
}

impl InterpParser<CallArgSchema> for DefaultInterp {
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        use CallArgParserState::*;
        let mut cursor = chunk;
        
        loop {
            match state {
                ReadingTag(ref mut tag_state) => {
                    let mut tag_dest = None;
                    cursor = <DefaultInterp as InterpParser<ULEB128>>::parse(
                        &DefaultInterp,
                        tag_state,
                        cursor,
                        &mut tag_dest
                    )?;
                    
                    let tag = tag_dest.ok_or_else(|| rej(cursor))?;
                    match tag {
                        0 => {
                            set_from_thunk(state, || ReadingPure {
                                length_state: <DefaultInterp as ParserCommon<ULEB128>>::init(&DefaultInterp),
                                length: None,
                                data: ArrayVec::new(),
                                bytes_read: 0,
                            });
                        }
                        1 => {
                            set_from_thunk(state, || ReadingObject(/* init */));
                        }
                        _ => return Err(rej(cursor)),
                    }
                }
                ReadingPure { ref mut length_state, ref mut length, ref mut data, ref mut bytes_read } => {
                    // First, read length if we haven't
                    if length.is_none() {
                        let mut len_dest = None;
                        cursor = <DefaultInterp as InterpParser<ULEB128>>::parse(
                            &DefaultInterp,
                            length_state,
                            cursor,
                            &mut len_dest
                        )?;
                        *length = len_dest;
                        if length.is_none() {
                            return Err((None, cursor));
                        }
                    }
                    
                    let target_len = length.unwrap() as usize;
                    
                    // Read data bytes
                    while *bytes_read < target_len {
                        match cursor.split_first() {
                            None => return Err((None, cursor)),
                            Some((byte, rest)) => {
                                if data.try_push(*byte).is_err() {
                                    return Err(rej(rest));
                                }
                                *bytes_read += 1;
                                cursor = rest;
                            }
                        }
                    }
                    
                    // Done reading Pure
                    *destination = Some(CallArg::Pure(data.clone()));
                    set_from_thunk(state, || Done);
                    return Ok(cursor);
                }
                ReadingObject(ref mut obj_state) => {
                    // Parse object reference
                    // ... similar pattern ...
                    set_from_thunk(state, || Done);
                    return Ok(cursor);
                }
                Done => return Err(rej(cursor)),
            }
        }
    }
}
```

#### 4.5 Testing Each Parser

Create comprehensive tests for each parser type:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_call_arg_pure() {
        let parser = DefaultInterp;
        let mut state = parser.init();
        let mut dest = None;
        
        // Tag 0 (Pure), length 5, data "hello"
        let data = vec![0, 5, b'h', b'e', b'l', b'l', b'o'];
        let result = parser.parse(&mut state, &data, &mut dest);
        
        assert_eq!(result, Ok(&[][..]));
        match dest.unwrap() {
            CallArg::Pure(bytes) => {
                assert_eq!(&bytes[..], b"hello");
            }
            _ => panic!("Expected Pure variant"),
        }
    }
    
    #[test]
    fn test_call_arg_pure_chunked() {
        let parser = DefaultInterp;
        let mut state = parser.init();
        let mut dest = None;
        
        // Feed tag and partial length
        let chunk1 = vec![0];
        let result = parser.parse(&mut state, &chunk1, &mut dest);
        assert!(matches!(result, Err((None, _))));
        
        // Feed rest of length and data
        let chunk2 = vec![5, b'h', b'e', b'l', b'l', b'o'];
        let result = parser.parse(&mut state, &chunk2, &mut dest);
        assert_eq!(result, Ok(&[][..]));
    }
    
    #[test]
    fn test_transfer_objects_command() {
        // Test complete command parsing
        // ...
    }
    
    #[test]
    fn test_full_transaction() {
        // Test parsing a complete transaction
        // ...
    }
}
```

#### 4.6 Progress Tracking

Create a checklist of parsers to port:

- [ ] `ObjectDigest`
- [ ] `ObjectRef`
- [ ] `CallArg`
- [ ] `Argument`
- [ ] `GasData`
- [ ] `TransferObjects` command
- [ ] `SplitCoins` command
- [ ] `MergeCoins` command
- [ ] `MakeMoveVec` command
- [ ] `MoveCall` command
- [ ] `Publish` command
- [ ] `Upgrade` command
- [ ] `Transaction`
- [ ] `Intent`
- [ ] `ObjectData`
- [ ] `TUID`

---

### Step 5: Complete Sync Module
**Status**: To Do  
**Duration**: 3-5 days  
**Dependencies**: Steps 2-4 complete

#### 5.1 Implement Chunk Reader

Create `src/sync/ctx/reader.rs`:

```rust
/// Helper for managing chunked input to parsers
pub struct ChunkReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> ChunkReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }
    
    pub fn remaining(&self) -> &'a [u8] {
        &self.data[self.offset..]
    }
    
    pub fn advance(&mut self, n: usize) {
        self.offset += n;
    }
    
    pub fn consumed(&self) -> usize {
        self.offset
    }
    
    pub fn is_complete(&self) -> bool {
        self.offset >= self.data.len()
    }
    
    pub fn reset(&mut self) {
        self.offset = 0;
    }
}
```

#### 5.2 Wire Up APDU Handler

In `src/sync/handle_apdu.rs`:

```rust
use crate::parser::tx_sync::{tx_parser_sync, TxSchema};
use crate::sync::ctx::reader::ChunkReader;
use ledger_parser_combinators::interp_parser::{ParserCommon, InterpParser};

pub fn sign_tx(
    comm: &mut Comm,
    ctx: &mut RunCtx,
    ui: &mut UserInterface,
) -> Result<(), StatusWords> {
    // Read transaction data from APDU
    let tx_data = /* get from comm */;
    let object_data = /* get from comm */;
    
    // Parse transaction
    let parser = DefaultInterp;
    let mut state = ParserCommon::<TxSchema>::init(&parser);
    let mut dest = None;
    let mut reader = ChunkReader::new(&tx_data);
    
    // Parse in loop to handle incremental parsing
    loop {
        let remaining = reader.remaining();
        match InterpParser::<TxSchema>::parse(&parser, &mut state, remaining, &mut dest) {
            Ok(unconsumed) => {
                let consumed = remaining.len() - unconsumed.len();
                reader.advance(consumed);
                
                if reader.is_complete() {
                    break;
                }
                // Continue parsing with next chunk
            }
            Err((None, unconsumed)) => {
                // Need more data
                let consumed = remaining.len() - unconsumed.len();
                reader.advance(consumed);
                
                if reader.is_complete() {
                    // No more data available - parse error
                    return Err(StatusWords::BadLen);
                }
                // In APDU context, we have all data, so this is an error
                return Err(StatusWords::BadLen);
            }
            Err((Some(OOB::Reject), _)) => {
                return Err(StatusWords::Unknown);
            }
        }
    }
    
    let known_tx = dest.ok_or(StatusWords::Unknown)?;
    
    // Continue with UI prompts and signing...
    match known_tx {
        KnownTx::TransferTx { params } => {
            ui.confirm_transfer(&params)?;
            // Sign and return
        }
        // Handle other transaction types
        _ => {
            return Err(StatusWords::Unknown);
        }
    }
    
    Ok(())
}
```

#### 5.3 Update Context Management

Ensure `src/sync/ctx.rs` properly manages parsing state if needed across multiple APDUs (though typically each APDU is self-contained).

#### 5.4 Test Integration

Test end-to-end signing flow:
```bash
cargo test --features sync --test test_sign_cmd
```

---

### Step 6: Switch Default & Comprehensive Testing
**Status**: To Do  
**Duration**: 3-5 days  
**Dependencies**: Step 5 complete

#### 6.1 Update Default Feature

In `Cargo.toml`:
```toml
[features]
default = ["sync"]  # Changed from async
sync = []
# Keep async available for comparison during transition
async = []
```

#### 6.2 Unit Testing

Run all parser unit tests:
```bash
cargo test --features sync
```

#### 6.3 Integration Testing

Run Ragger test suite on all devices:
```bash
./run-ragger-tests.sh
```

Expected tests:
- All address verification tests
- All signature tests
- All transaction type tests
- Error handling tests

#### 6.4 Signature Compatibility Testing

Critical: Ensure sync implementation produces identical signatures to async:

```python
# In ragger-tests/test_signature_compatibility.py

def test_transfer_signature_match(backend):
    """Verify sync produces same signature as async"""
    # Test transaction data
    tx = get_test_transaction()
    
    # Build and load async version
    async_sig = sign_with_build(tx, features=[])
    
    # Build and load sync version
    sync_sig = sign_with_build(tx, features=["sync"])
    
    # Signatures must match exactly
    assert async_sig == sync_sig, "Signature mismatch between async and sync!"
```

#### 6.5 Memory Profiling

Check stack usage hasn't increased significantly:
```bash
cargo stack-sizes --target nanosplus --features sync --release
```

Compare to async build:
```bash
cargo stack-sizes --target nanosplus --release
```

#### 6.6 Performance Benchmarking

While not critical for Ledger apps, verify no significant slowdown:

```rust
#[cfg(test)]
fn bench_transaction_parsing() {
    let test_txs = load_test_transactions();
    let start = /* get timestamp */;
    
    for tx in test_txs {
        parse_transaction(tx);
    }
    
    let elapsed = /* get timestamp */ - start;
    println!("Parsed {} txs in {}ms", test_txs.len(), elapsed);
}
```

#### 6.7 Manual Testing with Speculos

Test on all device types:
```bash
# Nano S+
cargo run --target nanosplus --features sync --release

# Nano X
cargo run --target nanox --features sync --release

# Stax
cargo run --target stax --features sync --release

# Flex
cargo run --target flex --features sync --release

# Apex P  
cargo run --target apex_p --features sync --release
```

Verify:
- Address display correct
- Transaction prompts correct
- Signatures match expected values
- UI navigation works
- Error handling appropriate

---

### Step 7: Remove Async Code
**Status**: To Do  
**Duration**: 1-2 days  
**Dependencies**: Step 6 complete, confidence high

⚠️ **Only proceed after extensive testing in Step 6**

#### 7.1 Create Cleanup Branch

```bash
git checkout -b remove-async-implementation
```

#### 7.2 Remove Async Files

```bash
# Parser combinators
rm rust-app/ledger-parser-combinators/src/async_parser.rs
rm rust-app/ledger-parser-combinators/src/bcs/async_parser.rs

# Async parsers
rm rust-app/src/parser/tx.rs
rm rust-app/src/parser/object.rs
rm rust-app/src/parser/tuid.rs

# Async implementation
rm -rf rust-app/src/ctx/
rm rust-app/src/ctx.rs
rm rust-app/src/handle_apdu.rs
rm rust-app/src/app_main.rs

# Rename sync versions to main
mv rust-app/src/parser/tx_sync.rs rust-app/src/parser/tx.rs
mv rust-app/src/parser/object_sync.rs rust-app/src/parser/object.rs
mv rust-app/src/parser/tuid_sync.rs rust-app/src/parser/tuid.rs

# Move sync module to main
mv rust-app/src/sync/* rust-app/src/
rm -rf rust-app/src/sync/
```

#### 7.3 Update Dependencies

In `rust-app/Cargo.toml`:
```toml
[dependencies]
# Remove:
# alamgu-async-block = "..."

# Keep:
ledger-parser-combinators = { path = "../ledger-parser-combinators" }
# ... other deps ...
```

In `ledger-parser-combinators/Cargo.toml`:
```toml
[dependencies]
# Remove async-related dependencies if any
```

#### 7.4 Clean Up Conditional Compilation

Search and remove all feature gate blocks:
```bash
# Find all feature gates
rg "#\[cfg\(.*feature.*sync.*\)\]" rust-app/src/

# Manually review and remove each
```

Update files:
- `rust-app/src/lib.rs` - remove feature gates
- `rust-app/bin-src/main.rs` - remove feature gates
- Clean up imports

#### 7.5 Update Module Structure

In `rust-app/src/lib.rs`:
```rust
// Remove conditionals
pub mod ctx;
pub mod handle_apdu;
pub mod app_main;
pub mod implementation;
pub mod ui;

// Rest of module declarations...
```

#### 7.6 Update Documentation

Update `README.md` to reflect sync-only implementation:
```markdown
## Architecture

This app uses synchronous parsing with the `InterpParser` trait from 
`ledger-parser-combinators`. Transaction data is parsed incrementally
using a state machine approach.
```

#### 7.7 Final Testing

Run complete test suite one more time:
```bash
# Unit tests
cargo test

# Integration tests
./run-ragger-tests.sh

# Manual testing on all devices
# ... (repeat 6.7)
```

#### 7.8 Update Feature Flags

In `Cargo.toml`:
```toml
[features]
default = []
# Remove sync and async features entirely
```

#### 7.9 Commit and Document

```bash
git add -A
git commit -m "Remove async implementation, use sync exclusively

- Removed alamgu-async-block dependency
- Removed all AsyncParser implementations
- Sync parsing is now the only implementation
- All tests passing on 5 device targets
- Signature compatibility verified"
```

---

## Testing Strategy

### Test Pyramid

```
                /\
               /  \
              /E2E \          - Manual testing with Speculos
             /------\
            /        \
           / Integr.  \       - Ragger tests with all devices
          /------------\
         /              \
        /  Unit Tests    \    - Parser unit tests
       /------------------\
```

### Unit Tests

**Location**: `rust-app/ledger-parser-combinators/src/bcs/interp_parser.rs` and `rust-app/src/parser/*_sync.rs`

**Coverage targets**:
- ✅ Each BCS primitive type
- ✅ Each transaction type
- ✅ Edge cases (empty, max size, invalid)
- ✅ Chunked input (byte-by-byte feeding)
- ✅ Error conditions

**Run**:
```bash
cargo test --features sync --lib
```

### Integration Tests

**Location**: `ragger-tests/`

**Test matrix**:
```
           | NanoS+ | NanoX | Stax | Flex | Apex P |
-----------|--------|-------|------|------|--------|
Address    |   ✓    |   ✓   |  ✓   |  ✓   |   ✓    |
Transfer   |   ✓    |   ✓   |  ✓   |  ✓   |   ✓    |
Stake      |   ✓    |   ✓   |  ✓   |  ✓   |   ✓    |
Split/Merge|   ✓    |   ✓   |  ✓   |  ✓   |   ✓    |
Token Tx   |   ✓    |   ✓   |  ✓   |  ✓   |   ✓    |
Invalid    |   ✓    |   ✓   |  ✓   |  ✓   |   ✓    |
```

**Run**:
```bash
./run-ragger-tests.sh
```

### Regression Tests

**Critical**: Signature compatibility

Create test suite that:
1. Loads known good transactions
2. Parses with sync implementation
3. Signs with device
4. Compares signature to known good signature

**Run**:
```bash
pytest ragger-tests/test_signature_regression.py
```

### Performance Tests

While not critical, track:
- Parse time per transaction type
- Stack usage
- Binary size

```bash
# Binary size
ls -lh target/nanosplus/release/rust-app

# Stack usage
cargo stack-sizes --target nanosplus --features sync --release | head -20
```

---

## Common Pitfalls

### 1. Off-By-One Errors in State Transitions

**Problem**: Parser consumes wrong number of bytes

**Example**:
```rust
// WRONG - doesn't advance cursor
cursor = parse_something(state, cursor, &mut dest)?;
// Use cursor again without checking

// RIGHT
let new_cursor = parse_something(state, cursor, &mut dest)?;
cursor = new_cursor;
```

**Prevention**: Always use returned cursor, never reuse input cursor

### 2. Not Handling "Need More Data" Correctly

**Problem**: Parser doesn't properly signal incomplete state

**Example**:
```rust
// WRONG - returns Ok with partial parse
if bytes_available < required {
    return Ok(cursor); // WRONG!
}

// RIGHT
if bytes_available < required {
    return Err((None, cursor)); // Need more data
}
```

**Prevention**: Always return `Err((None, cursor))` when data insufficient

### 3. Forgetting to Reset Subparser State

**Problem**: State from previous parse pollutes next parse

**Example**:
```rust
// WRONG - reuses old state
for i in 0..count {
    cursor = subparser.parse(&mut substate, cursor, &mut dest)?;
    // substate not reset!
}

// RIGHT
for i in 0..count {
    cursor = subparser.parse(&mut substate, cursor, &mut dest)?;
    substate = subparser.init(); // Reset for next iteration
}
```

**Prevention**: Always reinitialize subparser state after each use

### 4. Stack Overflow from Deep Nesting

**Problem**: State enums nest too deeply

**Example**:
```rust
// PROBLEMATIC - can overflow stack
pub enum BigState {
    Level1(Level2State),
}
pub enum Level2State {
    Level3(Level3State),
}
// ... many levels deep
```

**Prevention**: 
- Flatten state enums where possible
- Use `Box` for large states (if heap available)
- Monitor stack usage with `cargo stack-sizes`

### 5. Not Testing Chunked Input

**Problem**: Parser works with complete input but fails with chunks

**Prevention**: Always test byte-by-byte input:
```rust
#[test]
fn test_chunked_input() {
    let parser = DefaultInterp;
    let mut state = parser.init();
    let mut dest = None;
    
    let data = vec![/* test data */];
    
    // Feed one byte at a time
    for byte in data {
        match parser.parse(&mut state, &[byte], &mut dest) {
            Ok(_) => break,
            Err((None, _)) => continue, // Need more
            Err(_) => panic!("Parse error"),
        }
    }
    
    assert!(dest.is_some());
}
```

### 6. Incorrect Error Propagation

**Problem**: Converting `None` to error incorrectly

**Example**:
```rust
// WRONG - panics instead of propagating
let value = dest.unwrap();

// RIGHT - propagates error
let value = dest.ok_or_else(|| rej(cursor))?;
```

### 7. Not Handling ULEB128 Edge Cases

**Problem**: ULEB128 has tricky edge cases

**Critical tests**:
- Non-canonical encodings (e.g., `[0x80, 0x00]` for 0)
- Overflow (values > 2^32)
- Maximum length (5 bytes max)

### 8. State Machine Loops

**Problem**: Infinite loops in state machine

**Example**:
```rust
// PROBLEMATIC - might loop forever
loop {
    match state {
        State1 => {
            // Transition logic
            set_from_thunk(state, || State2);
            // Missing continue!
        }
        State2 => {
            // If this fails and sets back to State1...
        }
    }
}

// BETTER - explicit progress tracking
let mut iterations = 0;
loop {
    iterations += 1;
    if iterations > MAX_ITERATIONS {
        return Err(rej(cursor));
    }
    // State machine logic
}
```

---

## Rollback Plan

### If Issues Arise

#### During Development (Steps 2-5)
- Feature flag allows instant switch back to async
- Simply build without `--features sync`
- No deployment impact

#### During Testing (Step 6)
- If critical bugs found:
  1. Document bug
  2. Switch default back to async
  3. Fix sync implementation
  4. Retest

#### After Deployment (Step 7)
- Revert to pre-step-7 commit
- Restore async implementation from git history
- Deploy async build
- Debug sync issues offline

### Rollback Commands

```bash
# Revert to async implementation
git revert <step-7-commit-hash>

# Or checkout previous version
git checkout <pre-migration-commit>

# Rebuild without sync
cargo build --target nanosplus --release
```

---

## Timeline & Milestones

### Week 1
- ✅ Day 1-2: Implement BCS sync parsers (Step 2)
- ✅ Day 3-4: Port BIP32 parser (Step 3)
- ✅ Day 5: Initial testing and validation

### Week 2
- 🔄 Day 1-5: Port transaction parsers (Step 4)
  - Start with simple types
  - Progress to complex commands
  - Comprehensive unit tests

### Week 3
- 🔄 Day 1-2: Complete sync module integration (Step 5)
- 🔄 Day 3-5: Comprehensive testing (Step 6)
  - All Ragger tests
  - Manual testing on all devices
  - Performance profiling

### Week 4
- 🔄 Day 1-2: Final testing and validation
- 🔄 Day 3: Remove async code (Step 7)
- 🔄 Day 4-5: Final regression testing and documentation

**Total estimated duration: 2-4 weeks**

---

## Success Criteria

### Must Have
- ✅ All Ragger tests pass on all 5 devices
- ✅ Signatures match async implementation exactly
- ✅ No increase in binary size > 5%
- ✅ Stack usage within safe limits
- ✅ All transaction types supported

### Nice to Have
- ✅ Binary size reduction from removing async
- ✅ Improved parse performance
- ✅ Cleaner error messages
- ✅ Better debugging experience

### Quality Gates
- **Code Review**: All new parsers reviewed
- **Test Coverage**: >90% line coverage on parsers
- **Performance**: No regression > 10%
- **Memory**: Stack usage < device limits
- **Security**: No new vulnerabilities introduced

---

## Reference

### Key Files

**Parser Combinators**:
- `ledger-parser-combinators/src/interp_parser.rs` - Sync parser trait
- `ledger-parser-combinators/src/bcs/interp_parser.rs` - BCS sync implementations
- `ledger-parser-combinators/src/core_parsers.rs` - Schema definitions

**Sui Parsers**:
- `rust-app/src/parser/tx_sync.rs` - Transaction parsers
- `rust-app/src/parser/object_sync.rs` - Object parsers
- `rust-app/src/parser/bip32_sync.rs` - BIP32 path parser

**Integration**:
- `rust-app/src/sync/handle_apdu.rs` - APDU command handlers
- `rust-app/src/sync/implementation.rs` - Core implementation
- `rust-app/src/sync/ctx/reader.rs` - Chunk reader utility

**Tests**:
- `ragger-tests/test_*.py` - Integration tests
- `rust-app/src/parser/*_sync.rs` - Unit tests inline

### Helpful Resources

- **BCS Spec**: https://github.com/diem/bcs
- **ULEB128 Encoding**: https://en.wikipedia.org/wiki/LEB128
- **Sui Transaction Format**: https://docs.sui.io/
- **Ledger SDK Docs**: https://ledger.readthedocs.io/

### Contact

For questions or issues during migration:
- Review this document
- Check parser combinator examples in `interp_parser.rs`
- Compare with async implementation for reference
- Ask for help if stuck on complex parsers

---

## Appendix: Example Test Cases

### A.1 ULEB128 Test Vectors

```rust
#[test]
fn test_uleb128_vectors() {
    let vectors = vec![
        (vec![0x00], 0),
        (vec![0x01], 1),
        (vec![0x7F], 127),
        (vec![0x80, 0x01], 128),
        (vec![0x8f, 0x4a], 9487),
        (vec![0x80, 0x80, 0x01], 16384),
        (vec![0x80, 0x80, 0x80, 0x01], 2097152),
        (vec![0x80, 0x80, 0x80, 0x80, 0x01], 268435456),
    ];
    
    for (input, expected) in vectors {
        let parser = DefaultInterp;
        let mut state = parser.init();
        let mut dest = None;
        
        let result = parser.parse(&mut state, &input, &mut dest);
        assert_eq!(result, Ok(&[][..]));
        assert_eq!(dest, Some(expected));
    }
}
```

### A.2 Transaction Test Case

```rust
#[test]
fn test_transfer_transaction() {
    // Real transaction from Sui testnet
    let tx_bytes = base64::decode(
        "AAABACAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEBAQABAABv..."
    ).unwrap();
    
    let parser = DefaultInterp;
    let mut state = ParserCommon::<TxSchema>::init(&parser);
    let mut dest = None;
    
    let result = InterpParser::<TxSchema>::parse(
        &parser,
        &mut state,
        &tx_bytes,
        &mut dest
    );
    
    assert!(result.is_ok());
    
    let tx = dest.unwrap();
    match tx {
        KnownTx::TransferTx { params } => {
            assert_eq!(params.amount, /* expected amount */);
            // ... more assertions
        }
        _ => panic!("Expected TransferTx"),
    }
}
```

---

**Document Version**: 1.0  
**Last Updated**: February 2, 2026  
**Status**: Living document - update as migration progresses
