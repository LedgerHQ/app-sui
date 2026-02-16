use arrayvec::ArrayVec;
use ledger_parser_combinators::interp_parser::*;

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
