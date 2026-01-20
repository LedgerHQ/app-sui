// use crate::core_parsers::*;
// use crate::json::*;
use crate::core_parsers::Alt;
use crate::interp_parser::*;
use crate::json::*;
pub use bstringify::bstringify;
#[allow(unused_imports)]
use core::fmt::Write;
#[cfg(feature = "logging")]
use ledger_log::trace;
pub use paste::paste;

#[cfg(all(target_family = "bolos", test))]
#[allow(unused_imports)]
use ledger_device_sdk::testing::TestType;
#[cfg(all(target_family = "bolos", test))]
use testmacro::test_item as test; // , Pic};

// Monoid when A=Self, otherwise conceptually obeys
// self.add_and_set(a: A) = self.addAndSet(pure(a) : Self)
pub trait Summable<A> {
    // Implementing a mutating summation here as an optimization for stack use.
    fn add_and_set(&mut self, other: &A);
    fn zero() -> Self;
}

impl<A> Summable<A> for () {
    fn add_and_set(&mut self, _other: &A) {}
    fn zero() -> Self {}
}

impl<A: Summable<C>, B: Summable<D>, C, D> Summable<(C, D)> for (A, B) {
    fn add_and_set(&mut self, other: &(C, D)) {
        self.0.add_and_set(&other.0);
        self.1.add_and_set(&other.1);
    }
    fn zero() -> Self {
        (Summable::<C>::zero(), Summable::<D>::zero())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Count(pub u64);

impl<A> Summable<A> for Count {
    fn add_and_set(&mut self, _other: &A) {
        self.0 += 1;
    }
    fn zero() -> Self {
        Count(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct All(pub bool);

impl Summable<bool> for All {
    fn add_and_set(&mut self, other: &bool) {
        self.0 &= *other;
    }
    fn zero() -> Self {
        All(true)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JsonToken<'a> {
    BeginArray,
    EndArray,
    BeginObject,
    EndObject,
    NameSeparator,
    ValueSeparator,
    BeginString,
    StringChunk(&'a [u8]),
    EndString,
    BeginNumber,
    NumberChunk(&'a [u8]),
    EndNumber,
    False,
    Null,
    True,
}

pub trait JsonInterp<S>: ParserCommon<S> {
    fn parse<'a>(
        &self,
        state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>>;
}

#[derive(Debug)]
pub enum JsonStringEscapeState {
    NotEscaped,
    FirstEscape,
    InSequence(u8),
}

#[derive(Debug)]
pub enum JsonTokenizerState {
    Value,
    InNumber,
    InString(JsonStringEscapeState),
    InTrue(u8),
    InNull(u8),
    InFalse(u8),
}

// JSON lexer
fn get_json_token<'a>(
    state: &mut JsonTokenizerState,
    chunk: &'a [u8],
) -> Result<(JsonToken<'a>, RemainingSlice<'a>), (PResult<OOB>, RemainingSlice<'a>)> {
    let mut cursor = chunk;
    loop {
        match state {
            JsonTokenizerState::Value => {
                break match cursor.split_first() {
                    Some((a, tail)) => {
                        let c = match char::from_u32(*a as u32) {
                            Some(c) => c,
                            None => {
                                break reject(cursor);
                            }
                        };
                        if matches!(c, '\t' | '\n' | '\r' | ' ') {
                            cursor = tail;
                            continue;
                        }
                        if c.is_digit(10) || c == '-' {
                            *state = JsonTokenizerState::InNumber;
                            break Ok((JsonToken::BeginNumber, cursor));
                        }
                        match c {
                            '"' => {
                                *state =
                                    JsonTokenizerState::InString(JsonStringEscapeState::NotEscaped);
                                Ok(JsonToken::BeginString)
                            }
                            '[' => Ok(JsonToken::BeginArray),
                            '{' => Ok(JsonToken::BeginObject),
                            ']' => Ok(JsonToken::EndArray),
                            '}' => Ok(JsonToken::EndObject),
                            ':' => Ok(JsonToken::NameSeparator),
                            ',' => Ok(JsonToken::ValueSeparator),
                            // Handle False, Null, True
                            't' => {
                                *state = JsonTokenizerState::InTrue(1);
                                cursor = tail;
                                continue;
                            }
                            'n' => {
                                *state = JsonTokenizerState::InNull(1);
                                cursor = tail;
                                continue;
                            }
                            'f' => {
                                *state = JsonTokenizerState::InFalse(1);
                                cursor = tail;
                                continue;
                            }
                            _ => reject(cursor),
                        }
                        .map(|a| (a, tail))
                    }
                    None => need_more(cursor),
                };
            }
            JsonTokenizerState::InString(escape_state) => {
                break match cursor.split_first() {
                    None => need_more(cursor),
                    Some((a, tail)) => {
                        let next_char = match char::from_u32(*a as u32) {
                            Some(c) => c,
                            None => {
                                break reject(cursor);
                            }
                        };
                        match escape_state {
                            JsonStringEscapeState::NotEscaped => {
                                // If first char is ", EndString.
                                //
                                // Scan until quotation, reverse solidus, control character, or
                                // end-of-chunk.
                                //
                                // Switch on next character:
                                match next_char {
                                    '"' => {
                                        // Return chunk.
                                        *state = JsonTokenizerState::Value;
                                        Ok(JsonToken::EndString)
                                    }
                                    '\\' => {
                                        *escape_state = JsonStringEscapeState::FirstEscape;
                                        Ok(JsonToken::StringChunk(&cursor[0..0]))
                                    }
                                    /*'\0'..='\u{001f}' => {
                                        reject(cursor)
                                    }*/
                                    _ => Ok(JsonToken::StringChunk(&cursor[0..1])),
                                }
                            }
                            JsonStringEscapeState::FirstEscape => {
                                // Expedient. Incorrect.
                                // TODO: fix to validate escape sequences properly.
                                *escape_state = JsonStringEscapeState::NotEscaped;
                                match next_char {
                                    '"' | '\\' | '/' => Ok(JsonToken::StringChunk(&cursor[0..1])),
                                    'b' => Ok(JsonToken::StringChunk(b"\x08")),
                                    'f' => Ok(JsonToken::StringChunk(b"\x0c")),
                                    'n' => Ok(JsonToken::StringChunk(b"\n")),
                                    'r' => Ok(JsonToken::StringChunk(b"\r")),
                                    't' => Ok(JsonToken::StringChunk(b"\t")),
                                    'u' => {
                                        *escape_state = JsonStringEscapeState::InSequence(0);
                                        Ok(JsonToken::StringChunk(&cursor[0..0]))
                                    }
                                    _ => reject(cursor),
                                }
                            }
                            JsonStringEscapeState::InSequence(ref mut _n) => {
                                // No unicode support yet, sorry.
                                reject(cursor)
                            }
                        }
                        .map(|a| (a, tail))
                    }
                };
            }
            JsonTokenizerState::InNumber => {
                match cursor.split_first() {
                    Some((b',' | b'}' | b']', _)) => {
                        *state = JsonTokenizerState::Value;
                        // Returning cursor instead of r; we need to emit both EndNumber and
                        // ValueSeparator, EndObject, or EndArray, so we change state and return
                        // the cursor _including_ the end symbol.
                        break Ok((JsonToken::EndNumber, cursor));
                    }
                    Some((_, r)) => break Ok((JsonToken::NumberChunk(&cursor[0..1]), r)),
                    _ => break Err((None, cursor)),
                }
            }

            JsonTokenizerState::InTrue(1) => match cursor.split_first() {
                Some((b'r', r)) => {
                    *state = JsonTokenizerState::InTrue(2);
                    cursor = r;
                }
                _ => {
                    break reject(cursor);
                }
            },
            JsonTokenizerState::InTrue(2) => match cursor.split_first() {
                Some((b'u', r)) => {
                    *state = JsonTokenizerState::InTrue(3);
                    cursor = r;
                }
                _ => {
                    break reject(cursor);
                }
            },
            JsonTokenizerState::InTrue(3) => match cursor.split_first() {
                Some((b'e', r)) => {
                    *state = JsonTokenizerState::Value;
                    break Ok((JsonToken::True, r));
                }
                _ => {
                    break reject(cursor);
                }
            },
            JsonTokenizerState::InNull(1) => match cursor.split_first() {
                Some((b'u', r)) => {
                    *state = JsonTokenizerState::InNull(2);
                    cursor = r;
                }
                _ => {
                    break reject(cursor);
                }
            },
            JsonTokenizerState::InNull(2) => match cursor.split_first() {
                Some((b'l', r)) => {
                    *state = JsonTokenizerState::InNull(3);
                    cursor = r;
                }
                _ => {
                    break reject(cursor);
                }
            },
            JsonTokenizerState::InNull(3) => match cursor.split_first() {
                Some((b'l', r)) => {
                    *state = JsonTokenizerState::Value;
                    break Ok((JsonToken::Null, r));
                }
                _ => {
                    break reject(cursor);
                }
            },
            JsonTokenizerState::InFalse(1) => match cursor.split_first() {
                Some((b'a', r)) => {
                    *state = JsonTokenizerState::InFalse(2);
                    cursor = r;
                }
                _ => {
                    break reject(cursor);
                }
            },
            JsonTokenizerState::InFalse(2) => match cursor.split_first() {
                Some((b'l', r)) => {
                    *state = JsonTokenizerState::InFalse(3);
                    cursor = r;
                }
                _ => {
                    break reject(cursor);
                }
            },
            JsonTokenizerState::InFalse(3) => match cursor.split_first() {
                Some((b's', r)) => {
                    *state = JsonTokenizerState::InFalse(4);
                    cursor = r;
                }
                _ => {
                    break reject(cursor);
                }
            },
            JsonTokenizerState::InFalse(4) => match cursor.split_first() {
                Some((b'e', r)) => {
                    *state = JsonTokenizerState::Value;
                    break Ok((JsonToken::False, r));
                }
                _ => {
                    break reject(cursor);
                }
            },
            _ => {
                break reject(cursor); // TODO: finish
            }
        }
    }
}

// Tokenize json and pass to a stream.
impl<T, S: ParserCommon<T>> ParserCommon<Json<T>> for Json<S> {
    type State = (JsonTokenizerState, <S as ParserCommon<T>>::State);
    type Returning = <S as ParserCommon<T>>::Returning;

    fn init(&self) -> Self::State {
        (
            JsonTokenizerState::Value,
            <S as ParserCommon<T>>::init(&self.0),
        )
    }
    fn init_in_place(&self, state: *mut core::mem::MaybeUninit<Self::State>) {
        unsafe {
            (core::ptr::addr_of_mut!((*(*state).as_mut_ptr()).0) as *mut JsonTokenizerState)
                .write(JsonTokenizerState::Value);
        }
        self.0.init_in_place(unsafe {
            core::ptr::addr_of_mut!((*(*state).as_mut_ptr()).0)
                as *mut core::mem::MaybeUninit<<S as ParserCommon<T>>::State>
        });
    }
}

impl<T, S: JsonInterp<T>> InterpParser<Json<T>> for Json<S> {
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        let mut cursor: &[u8] = chunk;
        loop {
            let tok_r = get_json_token(&mut state.0, cursor);
            let (token, new_cursor) = tok_r?;

            break match <S as JsonInterp<T>>::parse(&self.0, &mut state.1, token, destination) {
                Ok(()) => Ok(new_cursor),
                Err(None) => {
                    cursor = new_cursor;
                    continue;
                }
                Err(Some(a)) => {
                    #[cfg(feature = "logging")]
                    trace!(
                        "Json parser rejected on token: {:?} after: {}",
                        token,
                        core::str::from_utf8(&chunk[0..chunk.len() - cursor.len()])
                            .unwrap_or("UTF8FAILED")
                    );
                    Err((Some(a), new_cursor))
                }
            };
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DropInterpStateEnum {
    Start,
    InName,
    InString,
    InNumber,
    ObjectNamePosition,
    ObjectNameSeparator,
    AfterValue,
}

use arrayvec::ArrayVec;

#[derive(Debug)]
pub struct DropInterpJsonState {
    stack: ArrayVec<bool, 32>, // True = Object, False = Array
    seen_item: bool,
    state: DropInterpStateEnum,
}

impl ParserCommon<JsonAny> for DropInterp {
    type State = DropInterpJsonState;
    type Returning = ();
    fn init(&self) -> Self::State {
        DropInterpJsonState {
            stack: ArrayVec::new(),
            state: DropInterpStateEnum::Start,
            seen_item: false,
        }
    }
}

impl JsonInterp<JsonAny> for DropInterp {
    #[inline(never)]
    fn parse<'a>(
        &self,
        full_state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        let DropInterpJsonState {
            ref mut stack,
            ref mut state,
            ref mut seen_item,
        } = full_state;
        *state = match (stack.last(), &state, token) {
            // Strings
            (_, DropInterpStateEnum::Start, JsonToken::BeginString) => {
                DropInterpStateEnum::InString
            }
            (_, DropInterpStateEnum::InString, JsonToken::StringChunk(_)) => {
                DropInterpStateEnum::InString
            }
            (_, DropInterpStateEnum::InString, JsonToken::EndString) => {
                DropInterpStateEnum::AfterValue
            }
            (_, DropInterpStateEnum::InString, _) => {
                return Err(Some(OOB::Reject)); // Broken invariant from lexer.
            }

            // Numbers
            (_, DropInterpStateEnum::Start, JsonToken::BeginNumber) => {
                DropInterpStateEnum::InNumber
            }
            (_, DropInterpStateEnum::InNumber, JsonToken::NumberChunk(_)) => {
                DropInterpStateEnum::InNumber
            }
            (_, DropInterpStateEnum::InNumber, JsonToken::EndNumber) => {
                DropInterpStateEnum::AfterValue
            }
            (_, DropInterpStateEnum::InNumber, _) => {
                return Err(Some(OOB::Reject)); // Broken invariant from lexer.
            }

            // Named terms
            (
                _,
                DropInterpStateEnum::Start,
                JsonToken::False | JsonToken::Null | JsonToken::True,
            ) => DropInterpStateEnum::AfterValue,

            // Array handling
            (_, DropInterpStateEnum::Start, JsonToken::BeginArray) => {
                stack.push(false);
                *seen_item = false;
                DropInterpStateEnum::Start
            }
            (Some(false), DropInterpStateEnum::Start, JsonToken::EndArray) => {
                if *seen_item {
                    return Err(Some(OOB::Reject)); // Trailing comma is not allowed in json.
                } else {
                    stack.pop();
                    DropInterpStateEnum::AfterValue
                }
            }
            (Some(false), DropInterpStateEnum::AfterValue, JsonToken::ValueSeparator) => {
                *seen_item = true;
                DropInterpStateEnum::Start
            }
            (Some(false), DropInterpStateEnum::AfterValue, JsonToken::EndArray) => {
                stack.pop();
                DropInterpStateEnum::AfterValue
            }

            // Object handling
            (_, DropInterpStateEnum::Start, JsonToken::BeginObject) => {
                stack.push(true);
                *seen_item = false;
                DropInterpStateEnum::ObjectNamePosition
            }
            (Some(true), DropInterpStateEnum::ObjectNamePosition, JsonToken::EndObject) => {
                if *seen_item {
                    return Err(Some(OOB::Reject)); // Trailing comma is not allowed in json.
                } else {
                    stack.pop();
                    DropInterpStateEnum::AfterValue
                }
            }
            //   Names (special strings)
            (_, DropInterpStateEnum::ObjectNamePosition, JsonToken::BeginString) => {
                DropInterpStateEnum::InName
            }
            (_, DropInterpStateEnum::InName, JsonToken::BeginString) => DropInterpStateEnum::InName,
            (_, DropInterpStateEnum::InName, JsonToken::StringChunk(_)) => {
                DropInterpStateEnum::InName
            }
            (_, DropInterpStateEnum::InName, JsonToken::EndString) => {
                DropInterpStateEnum::ObjectNameSeparator
            }
            //   Field values
            (_, DropInterpStateEnum::ObjectNameSeparator, JsonToken::NameSeparator) => {
                // Recursively parse a value.
                DropInterpStateEnum::Start
            }
            (Some(true), DropInterpStateEnum::AfterValue, JsonToken::ValueSeparator) => {
                *seen_item = true;
                DropInterpStateEnum::ObjectNamePosition
            }
            (Some(true), DropInterpStateEnum::AfterValue, JsonToken::EndObject) => {
                stack.pop();
                DropInterpStateEnum::AfterValue
            }

            _ => return Err(Some(OOB::Reject)), // Invalid json structure.
        };
        match (stack.is_empty(), state) {
            (true, DropInterpStateEnum::AfterValue) => {
                *destination = Some(());
                Ok(())
            }
            _ => Err(None),
        }
    }
}

impl<const N: usize> ParserCommon<JsonAny> for JsonStringAccumulate<N> {
    type State = (DropInterpJsonState, u8);
    type Returning = ArrayVec<u8, N>;
    fn init(&self) -> Self::State {
        (
            DropInterpJsonState {
                stack: ArrayVec::new(),
                state: DropInterpStateEnum::Start,
                seen_item: false,
            },
            0,
        )
    }
}

impl<const N: usize> JsonInterp<JsonAny> for JsonStringAccumulate<N> {
    #[inline(never)]
    fn parse<'a>(
        &self,
        full_state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        match destination {
            None => set_from_thunk(destination, || Some(ArrayVec::new())),
            Some(_) => {}
        }
        let mut extend_dest = |c: &[u8]| -> Result<(), Option<OOB>> {
            destination
                .as_mut()
                .ok_or(Some(OOB::Reject))?
                .try_extend_from_slice(c)
                .map_err(|_| Some(OOB::Reject))?;
            Ok(())
        };
        let DropInterpJsonState {
            ref mut stack,
            ref mut state,
            ref mut seen_item,
        } = full_state.0;
        *state = match (stack.last(), &state, token) {
            // Strings
            (_, DropInterpStateEnum::Start, JsonToken::BeginString) => {
                extend_dest(b"\"")?;
                DropInterpStateEnum::InString
            }
            (_, DropInterpStateEnum::InString, JsonToken::StringChunk(c)) => {
                extend_dest(c)?;
                DropInterpStateEnum::InString
            }
            (_, DropInterpStateEnum::InString, JsonToken::EndString) => {
                extend_dest(b"\"")?;
                DropInterpStateEnum::AfterValue
            }
            (_, DropInterpStateEnum::InString, _) => {
                return Err(Some(OOB::Reject)); // Broken invariant from lexer.
            }

            // Numbers
            (_, DropInterpStateEnum::Start, JsonToken::BeginNumber) => {
                DropInterpStateEnum::InNumber
            }
            (_, DropInterpStateEnum::InNumber, JsonToken::NumberChunk(c)) => {
                extend_dest(c)?;
                DropInterpStateEnum::InNumber
            }
            (_, DropInterpStateEnum::InNumber, JsonToken::EndNumber) => {
                DropInterpStateEnum::AfterValue
            }
            (_, DropInterpStateEnum::InNumber, _) => {
                return Err(Some(OOB::Reject)); // Broken invariant from lexer.
            }

            // Named terms
            (_, DropInterpStateEnum::Start, JsonToken::False) => {
                extend_dest(b"false")?;
                DropInterpStateEnum::AfterValue
            }
            (_, DropInterpStateEnum::Start, JsonToken::Null) => {
                extend_dest(b"null")?;
                DropInterpStateEnum::AfterValue
            }
            (_, DropInterpStateEnum::Start, JsonToken::True) => {
                extend_dest(b"true")?;
                DropInterpStateEnum::AfterValue
            }

            // Array handling
            (_, DropInterpStateEnum::Start, JsonToken::BeginArray) => {
                stack.push(false);
                *seen_item = false;
                extend_dest(b"[")?;
                DropInterpStateEnum::Start
            }
            (Some(false), DropInterpStateEnum::Start, JsonToken::EndArray) => {
                if *seen_item {
                    return Err(Some(OOB::Reject)); // Trailing comma is not allowed in json.
                } else {
                    stack.pop();
                    extend_dest(b"]")?;
                    DropInterpStateEnum::AfterValue
                }
            }
            (Some(false), DropInterpStateEnum::AfterValue, JsonToken::ValueSeparator) => {
                *seen_item = true;
                extend_dest(b",")?;
                DropInterpStateEnum::Start
            }
            (Some(false), DropInterpStateEnum::AfterValue, JsonToken::EndArray) => {
                stack.pop();
                extend_dest(b"]")?;
                DropInterpStateEnum::AfterValue
            }

            // Object handling
            (_, DropInterpStateEnum::Start, JsonToken::BeginObject) => {
                stack.push(true);
                *seen_item = false;
                extend_dest(b"{")?;
                DropInterpStateEnum::ObjectNamePosition
            }
            (Some(true), DropInterpStateEnum::ObjectNamePosition, JsonToken::EndObject) => {
                if *seen_item {
                    return Err(Some(OOB::Reject)); // Trailing comma is not allowed in json.
                } else {
                    stack.pop();
                    extend_dest(b"}")?;
                    DropInterpStateEnum::AfterValue
                }
            }
            //   Names (special strings)
            (_, DropInterpStateEnum::ObjectNamePosition, JsonToken::BeginString) => {
                extend_dest(b"\"")?;
                DropInterpStateEnum::InName
            }
            (_, DropInterpStateEnum::InName, JsonToken::BeginString) => {
                extend_dest(b"\"")?;
                DropInterpStateEnum::InName
            }
            (_, DropInterpStateEnum::InName, JsonToken::StringChunk(c)) => {
                extend_dest(c)?;
                DropInterpStateEnum::InName
            }
            (_, DropInterpStateEnum::InName, JsonToken::EndString) => {
                extend_dest(b"\"")?;
                DropInterpStateEnum::ObjectNameSeparator
            }
            //   Field values
            (_, DropInterpStateEnum::ObjectNameSeparator, JsonToken::NameSeparator) => {
                // Recursively parse a value.
                extend_dest(b":")?;
                DropInterpStateEnum::Start
            }
            (Some(true), DropInterpStateEnum::AfterValue, JsonToken::ValueSeparator) => {
                *seen_item = true;
                extend_dest(b",")?;
                DropInterpStateEnum::ObjectNamePosition
            }
            (Some(true), DropInterpStateEnum::AfterValue, JsonToken::EndObject) => {
                stack.pop();
                extend_dest(b"}")?;
                DropInterpStateEnum::AfterValue
            }

            _ => return Err(Some(OOB::Reject)), // Invalid json structure.
        };
        match (stack.is_empty(), state) {
            (true, DropInterpStateEnum::AfterValue) => Ok(()),
            _ => Err(None),
        }
    }
}

use core::fmt::Debug;
#[cfg(test)]
fn test_json_interp<T: JsonInterp<A>, A>(
    p: &T,
    pairs: &[(JsonToken, Result<T::Returning, Option<OOB>>)],
) where
    <T as ParserCommon<A>>::Returning: Debug + PartialEq + Clone,
{
    let mut state = T::init(p);
    for (token, expected) in pairs {
        let mut destination = None;
        let rv = T::parse(p, &mut state, *token, &mut destination);
        assert_eq!(rv.map(|_| destination.unwrap()), *expected);
    }
}

#[cfg(test)]
fn test_json_interp_parser<T: InterpParser<A>, A>(
    p: &T,
    chunk: &[u8],
    expected: Result<(T::Returning, &[u8]), (Option<OOB>, &[u8])>,
) where
    <T as ParserCommon<A>>::Returning: Debug + PartialEq + Clone,
{
    let mut state = T::init(p);
    let mut destination = None;
    let rv = T::parse(p, &mut state, chunk, &mut destination);
    assert_eq!(rv.map(|c| (destination.unwrap(), c)), expected);
}
/*
#[cfg(test)]
fn test_json_interp_parser_p<T: InterpParser<A>, A, B>(p: &T, chunk: &[u8], expected: Result<(B, &[u8]), (Option<OOB>, &[u8])>) where <T as InterpParser<A>>::Returning: Debug + PartialEq<B> + Clone, B: PartialEq<<T as InterpParser<A>>::Returning> {
    let mut state = T::init(p);
    let rv = T::parse(p, &mut state, chunk);
    assert_eq!(expected, rv);
}
*/

#[cfg(test)]
#[test]
fn test_json_any_drop() {
    test_json_interp::<DropInterp, JsonAny>(
        &DropInterp,
        &[
            (JsonToken::BeginString, Err(None)),
            (JsonToken::StringChunk(b"foo"), Err(None)),
            (JsonToken::EndString, Ok(())),
        ],
    );

    test_json_interp::<DropInterp, JsonAny>(
        &DropInterp,
        &[
            (JsonToken::BeginString, Err(None)),
            (JsonToken::StringChunk(b"foo"), Err(None)),
            (JsonToken::EndString, Ok(())),
            (JsonToken::BeginString, Err(Some(OOB::Reject))),
        ],
    );
    test_json_interp::<DropInterp, JsonAny>(
        &DropInterp,
        &[
            (JsonToken::BeginArray, Err(None)),
            (JsonToken::BeginString, Err(None)),
            (JsonToken::StringChunk(b"foo"), Err(None)),
            (JsonToken::EndString, Err(None)),
            (JsonToken::ValueSeparator, Err(None)),
            (JsonToken::BeginString, Err(None)),
            (JsonToken::EndString, Err(None)),
            (JsonToken::ValueSeparator, Err(None)),
            (JsonToken::BeginNumber, Err(None)),
            (JsonToken::NumberChunk(b"4"), Err(None)),
            (JsonToken::NumberChunk(b"2"), Err(None)),
            (JsonToken::EndNumber, Err(None)),
            (JsonToken::EndArray, Ok(())),
            (JsonToken::BeginString, Err(Some(OOB::Reject))),
        ],
    );
    test_json_interp::<DropInterp, JsonAny>(
        &DropInterp,
        &[
            (JsonToken::BeginArray, Err(None)),
            (JsonToken::EndArray, Ok(())),
        ],
    );

    /*

    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"[]", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"[[],[]]", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"[[]]", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"{}", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"[{}]", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"[{},[],[[[{}]]]]", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"[}", Err((Some(OOB::Reject), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"{]", Err((Some(OOB::Reject), b"")));

    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"\"\"", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"\"foo bar\"", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"\"foo\nbar\"", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"\"foo\nb\\\"ar\"", Ok(((), b"")));

    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"{\"five\": {}}", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"{\"five\": {}, \"six\": [[[[[[[]]]]]]]}", Ok(((), b"")));

    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"null", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"true", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"false", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"1", Ok(((), b"")));
    test_json_interp_parser::<Json<DropInterp>, Json<JsonAny>>(&Json(DropInterp), b"{\"five\": 5, \"six\", [[[[[[[]]]]]]]}", Ok(((), b"")));
    */
}

impl ParserCommon<JsonBool> for DropInterp {
    type State = ();
    type Returning = ();
    fn init(&self) -> Self::State {}
}

impl JsonInterp<JsonBool> for DropInterp {
    #[inline(never)]
    fn parse<'a>(
        &self,
        _state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        match token {
            JsonToken::True | JsonToken::False => {
                *destination = Some(());
                Ok(())
            }
            _ => Err(Some(OOB::Reject)),
        }
    }
}

impl ParserCommon<JsonNull> for DropInterp {
    type State = ();
    type Returning = ();
    fn init(&self) -> Self::State {}
}

impl JsonInterp<JsonNull> for DropInterp {
    #[inline(never)]
    fn parse<'a>(
        &self,
        _state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        match token {
            JsonToken::Null => {
                *destination = Some(());
                Ok(())
            }
            _ => Err(Some(OOB::Reject)),
        }
    }
}

impl ParserCommon<JsonString> for DropInterp {
    type State = DropInterpJsonState;
    type Returning = ();
    fn init(&self) -> Self::State {
        DropInterpJsonState {
            stack: ArrayVec::new(),
            state: DropInterpStateEnum::Start,
            seen_item: false,
        }
    }
}

impl JsonInterp<JsonString> for DropInterp {
    #[inline(never)]
    fn parse<'a>(
        &self,
        full_state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        let DropInterpJsonState {
            ref mut stack,
            ref mut state,
            seen_item: _,
        } = full_state;
        *state = match (stack.last(), &state, token) {
            // Strings
            (_, DropInterpStateEnum::Start, JsonToken::BeginString) => {
                DropInterpStateEnum::InString
            }
            (_, DropInterpStateEnum::InString, JsonToken::StringChunk(_)) => {
                DropInterpStateEnum::InString
            }
            (_, DropInterpStateEnum::InString, JsonToken::EndString) => {
                DropInterpStateEnum::AfterValue
            }
            (_, DropInterpStateEnum::InString, _) => {
                return Err(Some(OOB::Reject)); // Broken invariant from lexer.
            }

            _ => return Err(Some(OOB::Reject)), // Invalid json structure.
        };
        match (stack.is_empty(), state) {
            (true, DropInterpStateEnum::AfterValue) => {
                *destination = Some(());
                Ok(())
            }
            _ => Err(None),
        }
    }
}

#[cfg(test)]
#[test]
fn test_json_string_drop() {
    test_json_interp_parser::<Json<DropInterp>, Json<JsonString>>(
        &Json(DropInterp),
        b"\"foo\nbar\"",
        Ok(((), b"")),
    );
    test_json_interp_parser::<Json<DropInterp>, Json<JsonString>>(
        &Json(DropInterp),
        b"{\"foo\nbar\": \"\"}",
        Err((Some(OOB::Reject), b"\"foo\nbar\": \"\"}")),
    );
    test_json_interp_parser::<Json<DropInterp>, Json<JsonString>>(
        &Json(DropInterp),
        b"[\"foo\nbar\"]",
        Err((Some(OOB::Reject), b"\"foo\nbar\"]")),
    );
    test_json_interp_parser::<Json<DropInterp>, Json<JsonString>>(
        &Json(DropInterp),
        b"{}",
        Err((Some(OOB::Reject), b"}")),
    );
    test_json_interp_parser::<Json<DropInterp>, Json<JsonString>>(
        &Json(DropInterp),
        b"[]",
        Err((Some(OOB::Reject), b"]")),
    );
}

impl ParserCommon<JsonNumber> for DropInterp {
    type State = DropInterpJsonState;
    type Returning = ();
    fn init(&self) -> Self::State {
        DropInterpJsonState {
            stack: ArrayVec::new(),
            state: DropInterpStateEnum::Start,
            seen_item: false,
        }
    }
}

impl JsonInterp<JsonNumber> for DropInterp {
    #[inline(never)]
    fn parse<'a>(
        &self,
        full_state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        let DropInterpJsonState {
            ref mut stack,
            ref mut state,
            seen_item: _,
        } = full_state;
        *state = match (stack.last(), &state, token) {
            // Numbers
            (_, DropInterpStateEnum::Start, JsonToken::BeginNumber) => {
                DropInterpStateEnum::InNumber
            }
            (_, DropInterpStateEnum::InNumber, JsonToken::NumberChunk(_)) => {
                DropInterpStateEnum::InNumber
            }
            (_, DropInterpStateEnum::InNumber, JsonToken::EndNumber) => {
                DropInterpStateEnum::AfterValue
            }
            (_, DropInterpStateEnum::InNumber, _) => {
                return Err(Some(OOB::Reject)); // Broken invariant from lexer.
            }

            _ => return Err(Some(OOB::Reject)), // Invalid json structure.
        };
        match (stack.is_empty(), state) {
            (true, DropInterpStateEnum::AfterValue) => {
                *destination = Some(());
                Ok(())
            }
            _ => Err(None),
        }
    }
}

#[derive(Debug)]
pub enum JsonArrayDropState<S, D> {
    Start,
    First,
    Item(S, Option<D>),
    AfterValue,
}

impl<T> ParserCommon<JsonArray<T>> for DropInterp
where
    DropInterp: ParserCommon<T>,
{
    type State = JsonArrayDropState<
        <DropInterp as ParserCommon<T>>::State,
        <DropInterp as ParserCommon<T>>::Returning,
    >;
    type Returning = ();
    fn init(&self) -> Self::State {
        JsonArrayDropState::Start
    }
}

impl<T> JsonInterp<JsonArray<T>> for DropInterp
where
    DropInterp: JsonInterp<T>,
{
    #[inline(never)]
    fn parse<'a>(
        &self,
        state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        use JsonArrayDropState::*;
        use JsonToken::*;
        let st = (state, token);
        loop {
            match st {
                (Start, BeginArray) => {
                    set_from_thunk(st.0, || First);
                }
                (First, EndArray) => {
                    *destination = Some(());
                    return Ok(());
                }
                (First, _) => {
                    set_from_thunk(st.0, || {
                        Item(<DropInterp as ParserCommon<T>>::init(&DropInterp), None)
                    });
                    continue;
                }
                (Item(ref mut s, ref mut sub_destination), tok) => {
                    <DropInterp as JsonInterp<T>>::parse(&DropInterp, s, tok, sub_destination)?;
                    set_from_thunk(st.0, || AfterValue);
                }
                (AfterValue, ValueSeparator) => set_from_thunk(st.0, || {
                    Item(<DropInterp as ParserCommon<T>>::init(&DropInterp), None)
                }),
                (AfterValue, EndArray) => {
                    *destination = Some(());
                    return Ok(());
                }
                _ => return Err(Some(OOB::Reject)),
            };
            return Err(None);
        }
    }
}

impl<A, B> ParserCommon<Alt<A, B>> for DropInterp
where
    DropInterp: ParserCommon<A> + ParserCommon<B>,
{
    type State = (
        Option<<DropInterp as ParserCommon<A>>::State>,
        Option<<DropInterp as ParserCommon<A>>::Returning>,
        Option<<DropInterp as ParserCommon<B>>::State>,
        Option<<DropInterp as ParserCommon<B>>::Returning>,
    );
    type Returning = ();
    fn init(&self) -> Self::State {
        (
            Some(<DropInterp as ParserCommon<A>>::init(&DropInterp)),
            None,
            Some(<DropInterp as ParserCommon<B>>::init(&DropInterp)),
            None,
        )
    }
}
impl<A, B> JsonInterp<Alt<A, B>> for DropInterp
where
    DropInterp: JsonInterp<A> + JsonInterp<B>,
{
    #[inline(never)]
    fn parse<'a>(
        &self,
        (ref mut state1, ref mut rv1, ref mut state2, ref mut rv2): &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        match (
            state1
                .as_mut()
                .map(|s| <DropInterp as JsonInterp<A>>::parse(&DropInterp, s, token, rv1))
                .transpose(),
            state2
                .as_mut()
                .map(|s| <DropInterp as JsonInterp<B>>::parse(&DropInterp, s, token, rv2))
                .transpose(),
        ) {
            (Err(None), Err(None)) => Err(None),
            (Err(None), Ok(None)) => Err(None),
            (Ok(None), Err(None)) => Err(None),
            // Left-preference. This will complicate things a bit if we ever try to do host-side hinting.
            (Ok(Some(())), _) => {
                set_from_thunk(destination, || Some(()));
                Ok(())
            }
            (Err(Some(OOB::Reject)), Ok(Some(()))) | (Ok(None), Ok(Some(()))) => {
                set_from_thunk(destination, || Some(()));
                Ok(())
            }
            (Err(Some(OOB::Reject)), Err(None)) => {
                set_from_thunk(state1, || None);
                Err(None)
            }
            (Err(None), Err(Some(OOB::Reject))) => {
                set_from_thunk(state2, || None);
                Err(None)
            }
            _ => Err(Some(OOB::Reject)),
        }
    }
}

/* Important: DROPS it's results */
impl<T, S: ParserCommon<T>> ParserCommon<JsonArray<T>> for SubInterp<S> {
    type State =
        JsonArrayDropState<<S as ParserCommon<T>>::State, <S as ParserCommon<T>>::Returning>;
    type Returning = ();
    fn init(&self) -> Self::State {
        JsonArrayDropState::Start
    }
}

impl<T, S: JsonInterp<T>> JsonInterp<JsonArray<T>> for SubInterp<S> {
    #[inline(never)]
    fn parse<'a>(
        &self,
        state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        use JsonArrayDropState::*;
        use JsonToken::*;
        let st = (state, token);
        loop {
            match st {
                (Start, BeginArray) => {
                    *destination = Some(());
                    set_from_thunk(st.0, || First);
                }
                (First, EndArray) => return Ok(()),
                (First, _) => {
                    set_from_thunk(st.0, || Item(<S as ParserCommon<T>>::init(&self.0), None));
                    continue;
                }
                (Item(ref mut s, ref mut sub_destination), tok) => {
                    <S as JsonInterp<T>>::parse(&self.0, s, tok, sub_destination)?;
                    // destination.as_mut().ok_or(Some(OOB::Reject))?.add_and_set(sub_destination.as_ref().ok_or(Some(OOB::Reject))?);
                    set_from_thunk(st.0, || AfterValue);
                }
                (AfterValue, ValueSeparator) => {
                    set_from_thunk(st.0, || Item(<S as ParserCommon<T>>::init(&self.0), None));
                }
                (AfterValue, EndArray) => return Ok(()),
                _ => return Err(Some(OOB::Reject)),
            };
            return Err(None);
        }
    }
}

// I'd love to unify this with the above, but I can't quite see a way to default RV to (). Could
// also allow the Summable instance to reject and this would also cover AccumulateArray.
pub struct SubInterpM<S, RV = ()>(S, core::marker::PhantomData<RV>);

impl<S, RV> SubInterpM<S, RV> {
    pub const fn new(s: S) -> Self {
        SubInterpM(s, core::marker::PhantomData)
    }
}

impl<T, S: ParserCommon<T>, RV: Debug + Summable<<S as ParserCommon<T>>::Returning>>
    ParserCommon<JsonArray<T>> for SubInterpM<S, RV>
{
    type State =
        JsonArrayDropState<<S as ParserCommon<T>>::State, <S as ParserCommon<T>>::Returning>;
    type Returning = RV;
    fn init(&self) -> Self::State {
        JsonArrayDropState::Start
    }
}

impl<T, S: JsonInterp<T>, RV: Debug + Summable<<S as ParserCommon<T>>::Returning>>
    JsonInterp<JsonArray<T>> for SubInterpM<S, RV>
{
    #[inline(never)]
    fn parse<'a>(
        &self,
        state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        use JsonArrayDropState::*;
        use JsonToken::*;
        let st = (state, token);
        loop {
            match st {
                (Start, BeginArray) => {
                    *destination = Some(Summable::zero());
                    set_from_thunk(st.0, || First);
                }
                (First, EndArray) => return Ok(()),
                (First, _) => {
                    set_from_thunk(st.0, || Item(<S as ParserCommon<T>>::init(&self.0), None));
                    continue;
                }
                (Item(ref mut s, ref mut sub_destination), tok) => {
                    <S as JsonInterp<T>>::parse(&self.0, s, tok, sub_destination)?;
                    #[cfg(feature = "logging")]
                    trace!("destination {:?}", destination);
                    destination
                        .as_mut()
                        .ok_or(Some(OOB::Reject))?
                        .add_and_set(sub_destination.as_ref().ok_or(Some(OOB::Reject))?);
                    set_from_thunk(st.0, || AfterValue);
                }
                (AfterValue, ValueSeparator) => {
                    set_from_thunk(st.0, || Item(<S as ParserCommon<T>>::init(&self.0), None));
                }
                (AfterValue, EndArray) => return Ok(()),
                _ => return Err(Some(OOB::Reject)),
            };
            return Err(None);
        }
    }
}

// This achieves foldr' by feeding back the RV to the subparser as the Parameter
pub struct SubInterpMFold<S, RV = ()>(S, core::marker::PhantomData<RV>);

impl<S, RV> SubInterpMFold<S, RV> {
    pub const fn new(s: S) -> Self {
        SubInterpMFold(s, core::marker::PhantomData)
    }
}

impl<T, S: ParserCommon<T>, RV: Debug + Summable<<S as ParserCommon<T>>::Returning>>
    ParserCommon<JsonArray<T>> for SubInterpMFold<S, RV>
{
    type State =
        JsonArrayDropState<<S as ParserCommon<T>>::State, <S as ParserCommon<T>>::Returning>;
    type Returning = RV;
    fn init(&self) -> Self::State {
        JsonArrayDropState::Start
    }
}

impl<
        T,
        S: JsonInterp<T> + DynParser<T, Parameter = RV>,
        RV: Clone + Debug + Summable<<S as ParserCommon<T>>::Returning>,
    > JsonInterp<JsonArray<T>> for SubInterpMFold<S, RV>
{
    #[inline(never)]
    fn parse<'a>(
        &self,
        state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        use JsonArrayDropState::*;
        use JsonToken::*;
        let st = (state, token);
        loop {
            match st {
                (Start, BeginArray) => {
                    *destination = Some(Summable::zero());
                    set_from_thunk(st.0, || First);
                }
                (First, EndArray) => return Ok(()),
                (First, _) => {
                    set_from_thunk(st.0, || Item(<S as ParserCommon<T>>::init(&self.0), None));
                    #[allow(clippy::single_match)]
                    match st.0 {
                        Item(ref mut s, ref mut sub_destination) => {
                            <S as DynParser<T>>::init_param(
                                &self.0,
                                destination.as_ref().ok_or(Some(OOB::Reject))?.clone(),
                                s,
                                sub_destination,
                            );
                        }
                        _ => {}
                    }
                    continue;
                }
                (Item(ref mut s, ref mut sub_destination), tok) => {
                    <S as JsonInterp<T>>::parse(&self.0, s, tok, sub_destination)?;
                    #[cfg(feature = "logging")]
                    trace!("destination {:?}", destination);
                    destination
                        .as_mut()
                        .ok_or(Some(OOB::Reject))?
                        .add_and_set(sub_destination.as_ref().ok_or(Some(OOB::Reject))?);
                    set_from_thunk(st.0, || AfterValue);
                }
                (AfterValue, ValueSeparator) => {
                    set_from_thunk(st.0, || Item(<S as ParserCommon<T>>::init(&self.0), None));
                    #[allow(clippy::single_match)]
                    match st.0 {
                        Item(ref mut s, ref mut sub_destination) => {
                            <S as DynParser<T>>::init_param(
                                &self.0,
                                destination.as_ref().ok_or(Some(OOB::Reject))?.clone(),
                                s,
                                sub_destination,
                            );
                        }
                        _ => {}
                    }
                }
                (AfterValue, EndArray) => return Ok(()),
                _ => return Err(Some(OOB::Reject)),
            };
            return Err(None);
        }
    }
}

pub struct AccumulateArray<ItemInterp, const N: usize>(pub ItemInterp);

impl<T, S: ParserCommon<T>, const N: usize> ParserCommon<JsonArray<T>> for AccumulateArray<S, N>
where
    <S as ParserCommon<T>>::Returning: Clone,
{
    type State =
        JsonArrayDropState<<S as ParserCommon<T>>::State, <S as ParserCommon<T>>::Returning>;
    type Returning = ArrayVec<<S as ParserCommon<T>>::Returning, N>;
    fn init(&self) -> Self::State {
        JsonArrayDropState::Start
    }
}

impl<T, S: JsonInterp<T>, const N: usize> JsonInterp<JsonArray<T>> for AccumulateArray<S, N>
where
    <S as ParserCommon<T>>::Returning: Clone,
{
    #[inline(never)]
    fn parse<'a>(
        &self,
        state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        use JsonArrayDropState::*;
        use JsonToken::*;
        loop {
            match state {
                Start if token == BeginArray => {
                    set_from_thunk(destination, || Some(ArrayVec::<_, N>::new()));
                    set_from_thunk(state, || First);
                }
                First if token == EndArray => return Ok(()),
                First => {
                    set_from_thunk(state, || Item(<S as ParserCommon<T>>::init(&self.0), None));
                    continue;
                }
                Item(ref mut s, ref mut sub_destination) => {
                    <S as JsonInterp<T>>::parse(&self.0, s, token, sub_destination)?;
                    destination
                        .as_mut()
                        .ok_or(Some(OOB::Reject))?
                        .try_push(sub_destination.as_ref().ok_or(Some(OOB::Reject))?.clone())
                        .or(Err(Some(OOB::Reject)))?;
                    set_from_thunk(state, || AfterValue);
                }
                AfterValue if token == ValueSeparator => {
                    set_from_thunk(state, || Item(<S as ParserCommon<T>>::init(&self.0), None));
                }
                AfterValue if token == EndArray => return Ok(()),
                _ => return Err(Some(OOB::Reject)),
            };
            return Err(None);
        }
    }
}

impl<A, R, S: JsonInterp<A>> JsonInterp<A>
    for Action<S, fn(&<S as ParserCommon<A>>::Returning, &mut Option<R>) -> Option<()>>
{
    #[inline(never)]
    fn parse<'a>(
        &self,
        state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        self.0.parse(&mut state.0, token, &mut state.1)?;
        match (self.1)(state.1.as_ref().ok_or(Some(OOB::Reject))?, destination) {
            None => Err(Some(OOB::Reject)),
            Some(()) => Ok(()),
        }
    }
}

impl<A, R, S: JsonInterp<A>, C> JsonInterp<A>
    for Action<S, fn(&<S as ParserCommon<A>>::Returning, &mut Option<R>, C) -> Option<()>>
{
    #[inline(never)]
    fn parse<'a>(
        &self,
        state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        self.0.parse(&mut state.0, token, &mut state.1)?;
        match (self.1)(
            state.1.as_ref().ok_or(Some(OOB::Reject))?,
            destination,
            core::mem::take(&mut state.2).ok_or(Some(OOB::Reject))?,
        ) {
            None => Err(Some(OOB::Reject)),
            Some(()) => Ok(()),
        }
    }
}

pub struct JsonStringAccumulate<const N: usize>;

#[derive(Debug)]
pub enum JsonStringAccumulateState<const N: usize> {
    Start,
    Accumulating,
    End,
}
impl<const N: usize> ParserCommon<JsonString> for JsonStringAccumulate<N> {
    type State = JsonStringAccumulateState<N>;
    type Returning = ArrayVec<u8, N>;
    fn init(&self) -> Self::State {
        JsonStringAccumulateState::Start
    }
}
impl<const N: usize> JsonInterp<JsonString> for JsonStringAccumulate<N> {
    #[inline(never)]
    fn parse<'a>(
        &self,
        state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        match (state, token) {
            (state @ JsonStringAccumulateState::Start, JsonToken::BeginString) => {
                match destination {
                    // This is intentional, it allows caller to append to an ArrayVec if necessary
                    None => set_from_thunk(destination, || Some(ArrayVec::new())),
                    Some(_) => {}
                }
                *state = JsonStringAccumulateState::Accumulating;
                Err(None)
            }
            (JsonStringAccumulateState::Accumulating, JsonToken::StringChunk(c)) => {
                destination
                    .as_mut()
                    .ok_or(Some(OOB::Reject))?
                    .try_extend_from_slice(c)
                    .map_err(|_| Some(OOB::Reject))?;
                Err(None)
            }
            (state @ JsonStringAccumulateState::Accumulating, JsonToken::EndString) => {
                *state = JsonStringAccumulateState::End;
                Ok(())
            }
            _ => Err(Some(OOB::Reject)),
        }
    }
}

impl<const N: usize> ParserCommon<JsonNumber> for JsonStringAccumulate<N> {
    type State = JsonStringAccumulateState<N>;
    type Returning = ArrayVec<u8, N>;
    fn init(&self) -> Self::State {
        JsonStringAccumulateState::Start
    }
}
impl<const N: usize> JsonInterp<JsonNumber> for JsonStringAccumulate<N> {
    #[inline(never)]
    fn parse<'a>(
        &self,
        state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        match (state, token) {
            (state @ JsonStringAccumulateState::Start, JsonToken::BeginNumber) => {
                set_from_thunk(destination, || Some(ArrayVec::new()));
                *state = JsonStringAccumulateState::Accumulating;
                Err(None)
            }
            (JsonStringAccumulateState::Accumulating, JsonToken::NumberChunk(c)) => {
                destination
                    .as_mut()
                    .ok_or(Some(OOB::Reject))?
                    .try_extend_from_slice(c)
                    .map_err(|_| Some(OOB::Reject))?;
                Err(None)
            }
            (state @ JsonStringAccumulateState::Accumulating, JsonToken::EndNumber) => {
                *state = JsonStringAccumulateState::End;
                Ok(())
            }
            _ => Err(Some(OOB::Reject)),
        }
    }
}

#[cfg(test)]
#[test]
fn test_json_string_accum() {
    use core::convert::TryInto;
    test_json_interp_parser::<Json<JsonStringAccumulate<10>>, Json<JsonString>>(
        &Json(JsonStringAccumulate),
        b"\"foo\nbar\"",
        Ok(((&b"foo\nbar"[..]).try_into().unwrap(), b"")),
    );
}

// These are a bit over-specific, but the more general Alt costs potentially significant amounts
// more memory.

pub struct OrDropAny<A>(pub A);

impl<A, I: ParserCommon<A>> ParserCommon<Alt<A, JsonAny>> for OrDropAny<I> {
    type State = (
        Option<I::State>,
        Option<<DropInterp as ParserCommon<JsonAny>>::State>,
    );
    type Returning = Option<<I as ParserCommon<A>>::Returning>;
    fn init(&self) -> Self::State {
        (
            Some(self.0.init()),
            Some(<DropInterp as ParserCommon<JsonAny>>::init(&DropInterp)),
        )
    }
}
impl<A, I: JsonInterp<A>> JsonInterp<Alt<A, JsonAny>> for OrDropAny<I> {
    #[inline(never)]
    fn parse<'a>(
        &self,
        (ref mut state1, ref mut state2): &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        let mut rv2 = None;
        match destination {
            None => set_from_thunk(destination, || Some(None)),
            Some(_) => (),
        }
        match (
            state1
                .as_mut()
                .map(|s| {
                    self.0
                        .parse(s, token, destination.as_mut().ok_or(Some(OOB::Reject))?)
                })
                .transpose(),
            state2
                .as_mut()
                .map(|s| {
                    <DropInterp as JsonInterp<JsonAny>>::parse(&DropInterp, s, token, &mut rv2)
                })
                .transpose(),
        ) {
            (Err(None), Err(None)) => Err(None),
            (Err(None), Ok(None)) => Err(None),
            (Ok(None), Err(None)) => Err(None),
            // Left-preference. This will complicate things a bit if we ever try to do host-side hinting.
            (Ok(Some(())), _) => Ok(()), // set_from_thunk(destination, || core::mem::take(rv1).map(AltResult::First)); Ok(()) }
            (Err(Some(OOB::Reject)), Ok(Some(()))) | (Ok(None), Ok(Some(()))) => {
                set_from_thunk(destination, || Some(None));
                Ok(())
            }
            (Err(Some(OOB::Reject)), Err(None)) => {
                set_from_thunk(state1, || None);
                Err(None)
            }
            (Err(None), Err(Some(OOB::Reject))) => {
                set_from_thunk(state2, || None);
                Err(None)
            }
            _ => {
                set_from_thunk(destination, || None);
                Err(Some(OOB::Reject))
            }
        }
    }
}

pub struct OrDrop<A>(pub A);

impl<A, I: ParserCommon<A>> ParserCommon<A> for OrDrop<I>
where
    DropInterp: ParserCommon<A>,
{
    type State = (
        Option<I::State>,
        Option<<DropInterp as ParserCommon<A>>::State>,
    );
    type Returning = Option<<I as ParserCommon<A>>::Returning>;
    fn init(&self) -> Self::State {
        (
            Some(self.0.init()),
            Some(<DropInterp as ParserCommon<A>>::init(&DropInterp)),
        )
    }
}
impl<A, I: JsonInterp<A>> JsonInterp<A> for OrDrop<I>
where
    DropInterp: JsonInterp<A>,
{
    #[inline(never)]
    fn parse<'a>(
        &self,
        (ref mut state1, ref mut state2): &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        let mut rv2 = None;
        match destination {
            None => set_from_thunk(destination, || Some(None)),
            Some(_) => (),
        }
        match (
            state1
                .as_mut()
                .map(|s| {
                    self.0
                        .parse(s, token, destination.as_mut().ok_or(Some(OOB::Reject))?)
                })
                .transpose(),
            state2
                .as_mut()
                .map(|s| <DropInterp as JsonInterp<A>>::parse(&DropInterp, s, token, &mut rv2))
                .transpose(),
        ) {
            (Err(None), Err(None)) => Err(None),
            (Err(None), Ok(None)) => Err(None),
            (Ok(None), Err(None)) => Err(None),
            // Left-preference. This will complicate things a bit if we ever try to do host-side hinting.
            (Ok(Some(())), _) => Ok(()), // set_from_thunk(destination, || core::mem::take(rv1).map(AltResult::First)); Ok(()) }
            (Err(Some(OOB::Reject)), Ok(Some(()))) | (Ok(None), Ok(Some(()))) => {
                set_from_thunk(destination, || Some(None));
                Ok(())
            }
            (Err(Some(OOB::Reject)), Err(None)) => {
                set_from_thunk(state1, || None);
                Err(None)
            }
            (Err(None), Err(Some(OOB::Reject))) => {
                set_from_thunk(state2, || None);
                Err(None)
            }
            _ => {
                set_from_thunk(destination, || None);
                Err(Some(OOB::Reject))
            }
        }
    }
}

#[derive(Debug)]
pub enum AltResult<A, B> {
    First(A),
    Second(B),
}

impl<A, B, I: ParserCommon<A>, J: ParserCommon<B>> ParserCommon<Alt<A, B>> for Alt<I, J> {
    type State = (
        Option<I::State>,
        Option<I::Returning>,
        Option<J::State>,
        Option<J::Returning>,
    );
    type Returning =
        AltResult<<I as ParserCommon<A>>::Returning, <J as ParserCommon<B>>::Returning>;
    fn init(&self) -> Self::State {
        (Some(self.0.init()), None, Some(self.1.init()), None)
    }
}
impl<A, B, I: JsonInterp<A>, J: JsonInterp<B>> JsonInterp<Alt<A, B>> for Alt<I, J> {
    #[inline(never)]
    fn parse<'a>(
        &self,
        (ref mut state1, ref mut rv1, ref mut state2, ref mut rv2): &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        match (
            state1
                .as_mut()
                .map(|s| self.0.parse(s, token, rv1))
                .transpose(),
            state2
                .as_mut()
                .map(|s| self.1.parse(s, token, rv2))
                .transpose(),
        ) {
            (Err(None), Err(None)) => Err(None),
            (Err(None), Ok(None)) => Err(None),
            (Ok(None), Err(None)) => Err(None),
            // Left-preference. This will complicate things a bit if we ever try to do host-side hinting.
            (Ok(Some(())), _) => {
                set_from_thunk(destination, || core::mem::take(rv1).map(AltResult::First));
                Ok(())
            }
            (Err(Some(OOB::Reject)), Ok(Some(()))) | (Ok(None), Ok(Some(()))) => {
                set_from_thunk(destination, || core::mem::take(rv2).map(AltResult::Second));
                Ok(())
            }
            (Err(Some(OOB::Reject)), Err(None)) => {
                set_from_thunk(state1, || None);
                Err(None)
            }
            (Err(None), Err(Some(OOB::Reject))) => {
                set_from_thunk(state2, || None);
                Err(None)
            }
            _ => Err(Some(OOB::Reject)),
        }
    }
}

/*
// Not currently used, questionable usefulness.
//
impl<const MAX : usize, const STRS : &'static StringList> JsonInterp<JsonStringEnum<MAX, STRS>> for DefaultInterp {
    type State = <JsonStringAccumulate<MAX> as JsonInterp<JsonString>>::State;
    type Returning = usize;
    fn init(&self) -> Self::State { <JsonStringAccumulate<MAX> as JsonInterp<JsonString>>::init(&JsonStringAccumulate) }
    #[inline(never)]
    fn parse<'a>(&self, state: &mut Self::State, token: JsonToken<'a>, destination: &mut Option<Self::Returning>) -> Result<(), Option<OOB>> {
        let a = <JsonStringAccumulate<MAX> as JsonInterp<JsonString>>::parse(&JsonStringAccumulate, state, token)?;
        let mut cursor = STRS;
        let mut n = 0;
        loop {
            match cursor {
                StringList::Cons(s, r) => {
                    if *s == &a[..] {
                        break Ok(n);
                    } else {
                        n = n + 1;
                        cursor = r;
                    }
                }
                StringList::Nil => { break Err(Some(OOB::Reject)); }

            }
        }
    }
}

#[cfg(test)]
#[test]
fn test_json_string_enum() {
    type TestEnum = JsonStringEnum<10, { &StringList::Cons(b"one", &StringList::Cons(b"five", &StringList::Nil)) }>;
    test_json_interp_parser::<Json<DefaultInterp>, Json<TestEnum> >(&Json(DefaultInterp), b"\"five\"", Ok((1, b"")));
    test_json_interp_parser::<Json<DefaultInterp>, Json<TestEnum> >(&Json(DefaultInterp), b"\"one\"", Ok((0, b"")));
    test_json_interp_parser::<Json<DefaultInterp>, Json<TestEnum> >(&Json(DefaultInterp), b"\"two\"", Err((Some(OOB::Reject), b"")));
}
*/

#[macro_export]
macro_rules! define_json_struct_interp {
    { $name:ident $n:literal { $($field:ident : $schemaType:ty),* } } => {
        $crate::json::paste! {

            impl<$([<Field $field:camel>]: $crate::json_interp::Summable<[<Field $field:camel>]>),*> $crate::json_interp::Summable<Self> for $name<$([<Field $field:camel>]),*> {
                fn add_and_set(&mut self, other: &Self) {
                    $(self.[<field_ $field:snake>].add_and_set(&other.[<field_ $field:snake>]));*
                }
                fn zero() -> Self {
                    $name { $([<field_ $field:snake>]: $crate::json_interp::Summable::<[<Field $field:camel>]>::zero() ),* }
                }
            }

            pub struct [<$name Interp>]<$([<Field $field:camel>]: ParserCommon<$schemaType>),*> {
                $(pub [<field_ $field:snake>] : [<Field $field:camel>] ),*
            }

            #[derive(Debug)]
            pub enum [<$name State>]<$([<Field $field:camel>]),*> {
                Start,
                Key(<JsonStringAccumulate<$n> as ParserCommon<JsonString>>::State, Option<<JsonStringAccumulate<$n> as ParserCommon<JsonString>>::Returning>),
                KeySep(<JsonStringAccumulate<$n> as ParserCommon<JsonString>>::Returning),
                ValueSep,
                End,
                $([<Field $field:camel>]([<Field $field:camel>])),*
            }

            //impl<A, AA : JsonInterp<A>, B, BB : JsonInterp<B>, C, CC : JsonInterp<C>> JsonInterp<SomeStruct<A,B,C>> for SomeStruct<AA, BB, CC>

            impl<$([<Field $field:camel Interp>] : ParserCommon<$schemaType>),*> ParserCommon<[<$name:camel Schema>]> for [<$name:camel Interp>]<$([<Field $field:camel Interp>]),*> {
                type State = [<$name State>]<
                    $(<[<Field $field:camel Interp>] as ParserCommon<$schemaType>>::State),*// ,
                    // $(Option<<[<Field $field:camel Interp>] as ParserCommon<$schemaType>>::Returning>),*
                    >;
                type Returning = $name <
                    $(Option<<[<Field $field:camel Interp>] as ParserCommon<$schemaType>>::Returning>),*
                    >;
                fn init(&self) -> Self::State { [<$name State>]::Start }
            }
            impl<$([<Field $field:camel Interp>] : JsonInterp<$schemaType>),*> JsonInterp<[<$name:camel Schema>]> for [<$name:camel Interp>]<$([<Field $field:camel Interp>]),*> {
    #[inline(never)]
                fn parse<'a>(&self, state: &mut Self::State, token: JsonToken<'a>, destination: &mut Option<Self::Returning>) -> Result<(), Option<$crate::interp_parser::OOB>> {
                    match state {
                        // Object handling
                        [<$name State>]::Start if token == JsonToken::BeginObject => {
                            $crate::interp_parser::set_from_thunk(destination, || Some($name {$( [<field_ $field:snake>] : None ),* } ));
                            $crate::interp_parser::set_from_thunk(state, || [<$name State>]::Key(<JsonStringAccumulate<$n> as ParserCommon<JsonString>>::init(&JsonStringAccumulate), None));
                        }
                        [<$name State>]::Key(ref mut key_state, ref mut key_destination) => {
                            <JsonStringAccumulate<$n> as JsonInterp<JsonString>>::parse(&JsonStringAccumulate, key_state, token, key_destination)?;
                            // Note: if we can figure out how, making key_val into a local that
                            // reserves stack at less than the function scope will make this parse
                            // cheaper.
                            let key_val = core::mem::take(key_destination).ok_or(Some($crate::interp_parser::OOB::Reject))?;
                            $crate::interp_parser::set_from_thunk(state, || [<$name State>]::KeySep(key_val));
                        }

                        [<$name State>]::KeySep(ref key) if token == JsonToken::NameSeparator => {
                            match &key[..] {
                                $(
                                    $crate::json_interp::bstringify!($field) => {
                                        #[cfg(feature = "logging")]
                                        trace!("json-struct-interp parser: checking key {:?}\n", core::str::from_utf8(key));
                                        $crate::interp_parser::set_from_thunk(state, || [<$name State>]::[<Field $field:camel>](<[<Field $field:camel Interp>] as ParserCommon<$schemaType>>::init(&self.[<field_ $field:snake>])));
                                    }
                                )*
                                ,
                                _ => {
                                    #[cfg(feature = "logging")]
                                    error!("json-struct-interp parser: Got unexpected key {:?}\n", core::str::from_utf8(key));
                                    return Err(Some($crate::interp_parser::OOB::Reject)) }
                            }
                        }
                        $(
                        [<$name State>]::[<Field $field:camel>](ref mut sub) => {
                                let rv_temp=<[<Field $field:camel Interp>] as JsonInterp<$schemaType>>::parse(&self.[<field_ $field:snake>], sub, token, &mut destination.as_mut().ok_or(Some($crate::interp_parser::OOB::Reject))?.[<field_ $field:snake>]);//);
                                rv_temp?;
                            $crate::interp_parser::set_from_thunk(state, || [<$name State>]::ValueSep);
                        })*

                        [<$name State>]::ValueSep if token == JsonToken::ValueSeparator => {
                            $crate::interp_parser::set_from_thunk(state, || [<$name State>]::Key(<JsonStringAccumulate<$n> as ParserCommon<JsonString>>::init(&JsonStringAccumulate), None));
                        }
                        [<$name State>]::ValueSep if token == JsonToken::EndObject => {
                            $crate::interp_parser::set_from_thunk(state, || [<$name State>]::End);
                            return Ok(());
                        }
                        _ => return Err(Some($crate::interp_parser::OOB::Reject)),
                    };
                    match state {
                        [<$name State>]::End => Ok(()),
                        _ => Err(None)
                    }
                }
            }


            type [<$name DropInterp>] = [<$name:camel Interp>]<$(define_json_struct_interp!{ DROP $field DropInterp } ),*>;
            const [<$name:upper _DROP_INTERP>] : [<$name DropInterp>] = [<$name DropInterp>] { $( [<field_ $field:snake>]: DropInterp ),* };
            impl ParserCommon<[<$name Schema>] > for DropInterp where
                {
                    type State = < [<$name DropInterp>] as ParserCommon<[<$name Schema>] >>::State;
                    type Returning = < [<$name DropInterp>] as ParserCommon<[<$name Schema>] >>::Returning;
                    fn init(&self) -> Self::State { <[<$name DropInterp>] as ParserCommon<[<$name Schema>]>>::init(&[<$name:upper _DROP_INTERP>]) }
            }
            impl JsonInterp<[<$name Schema>] > for DropInterp where {
                #[inline(never)]
                    fn parse<'a>(&self, full_state: &mut Self::State, token: JsonToken<'a>, destination: &mut Option<Self::Returning>) -> Result<(), Option<$crate::interp_parser::OOB>> {
                        <[<$name DropInterp>] as JsonInterp<[<$name Schema>]>>::parse(&[<$name:upper _DROP_INTERP>], full_state, token, destination)
                }
            }
        }
    };
    { DROP $a:ident $b: ident } => { $b }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define_json_struct;
    define_json_struct! {
        SomeStruct 10 {
            FooString : JsonString,
            bar_noodle : JsonString
        }
    }
    some_struct_definition!();
    /*    define_json_struct_interp!{
        SomeStruct 10 {
            FooString : JsonString,
            bar_noodle : JsonString
        }
    } */

    fn mk_astr<const N: usize>(c: &[u8]) -> Option<ArrayVec<u8, N>> {
        let mut v = ArrayVec::new();
        v.try_extend_from_slice(c).ok()?;
        Some(v)
    }
    #[cfg(all(target_family = "bolos", test))]
    use testmacro::test_item as test;

    #[test]
    fn test_somestruct_macro() {
        let _ = SomeStructInterp {
            field_foo_string: DropInterp,
            field_bar_noodle: DropInterp,
        };

        test_json_interp_parser::<
            Json<SomeStructInterp<JsonStringAccumulate<10>, JsonStringAccumulate<10>>>,
            Json<SomeStructSchema>,
        >(
            &Json(SomeStructInterp {
                field_foo_string: JsonStringAccumulate,
                field_bar_noodle: JsonStringAccumulate,
            }),
            b"{\"FooString\": \"one\", \"bar_noodle\": \"two\"}",
            Ok((
                SomeStruct {
                    field_foo_string: mk_astr(b"one"),
                    field_bar_noodle: mk_astr(b"two"),
                },
                b"",
            )),
        );

        test_json_interp_parser::<
            Json<SomeStructInterp<JsonStringAccumulate<10>, JsonStringAccumulate<10>>>,
            Json<SomeStructSchema>,
        >(
            &Json(SomeStructInterp {
                field_foo_string: JsonStringAccumulate,
                field_bar_noodle: JsonStringAccumulate,
            }),
            b"{\"NotFooStr\": \"one\", \"bar_noodle\": \"two\"}",
            Err((Some(OOB::Reject), b" \"one\", \"bar_noodle\": \"two\"}")),
        );
    }
}

impl<A, S: JsonInterp<A>> JsonInterp<A> for Preaction<S> {
    #[inline(never)]
    fn parse<'a>(
        &self,
        state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        loop {
            break match state {
                None => {
                    (self.0)().ok_or(Some(OOB::Reject))?;
                    set_from_thunk(state, || Some(<S as ParserCommon<A>>::init(&self.1)));
                    continue;
                }
                Some(ref mut s) => <S as JsonInterp<A>>::parse(&self.1, s, token, destination),
            };
        }
    }
}

impl<A, R, S: JsonInterp<A>> JsonInterp<A>
    for MoveAction<S, fn(<S as ParserCommon<A>>::Returning, &mut Option<R>) -> Option<()>>
{
    #[inline(never)]
    fn parse<'a>(
        &self,
        state: &mut Self::State,
        token: JsonToken<'a>,
        destination: &mut Option<Self::Returning>,
    ) -> Result<(), Option<OOB>> {
        self.0.parse(&mut state.0, token, &mut state.1)?;
        match (self.1)(
            core::mem::take(&mut state.1).ok_or(Some(OOB::Reject))?,
            destination,
        ) {
            None => Err(Some(OOB::Reject)),
            Some(()) => Ok(()),
        }
    }
}

/*
impl JsonInterp<JsonStringEnum<STRS>> for DefaultInterp {
    type State = DropInterpJsonState;
    type Returning = u8;
    fn init(&self) -> Self::State { DropInterpJsonState { stack: ArrayVec::new(), state: DropInterpStateEnum::Start, seen_item: false } }
    #[inline(never)]
    fn parse<'a>(&self, full_state: &mut Self::State, token: JsonToken<'a>, destination: &mut Option<Self::Returning>) -> Result<(), Option<OOB>> {
        let DropInterpJsonState { ref mut stack, ref mut state, ref mut seen_item } = full_state;
        *state = match (stack.last(), &state, token) {

            // Strings
            (_, DropInterpStateEnum::Start, JsonToken::BeginString) => {
                DropInterpStateEnum::InString
            }
            (_, DropInterpStateEnum::InString, JsonToken::StringChunk(_)) => {
                DropInterpStateEnum::InString
            }
            (_, DropInterpStateEnum::InString, JsonToken::EndString) => {
                DropInterpStateEnum::AfterValue
            }
            (_, DropInterpStateEnum::InString, _) => {
                return Err(Some(OOB::Reject)); // Broken invariant from lexer.
            }

            _ => { return Err(Some(OOB::Reject)) } // Invalid json structure.
        };
        match (stack.is_empty(), state) {
            (true, DropInterpStateEnum::AfterValue) => { Ok(()) }
            _ => { Err(None) }
        }
    }
}
*/

/*
struct Z<const Q : [(&'static [u8], usize); 5]>;

#[cfg(test)]
#[test]
fn test_constant_params() {
  let q = Z::<{
      [
          (b"one", 1),
          (b"two", 2),
          (b"three", 3),
          (b"four", 4),
          (b"five", 5)
      ]
  }>;
}

*/
