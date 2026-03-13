use crate::core_parsers::*;
use crate::interp_parser::*;

impl ParserCommon<bool> for DefaultInterp {
    type State = ();
    type Returning = bool;
    fn init(&self) -> Self::State {
        ()
    }
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
                ReadingDiscriminant => match cursor.split_first() {
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
                },
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
        assert_eq!(
            parser.parse(&mut state, &[0x8f, 0x4a], &mut dest),
            Ok(&[][..])
        );
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
        assert!(matches!(
            parser.parse(&mut state, &[0x80, 0x00], &mut dest),
            Err(_)
        ));
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
