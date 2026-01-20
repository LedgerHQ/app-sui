use crate::async_parser::*;
use crate::core_parsers::*;
use crate::interp::*;

use core::future::Future;

impl HasOutput<bool> for DefaultInterp {
    type Output = bool;
}

impl<BS: Readable> AsyncParser<bool, BS> for DefaultInterp {
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let [byte]: [u8; 1] = input.read().await;
            match byte {
                0 => false,
                1 => true,
                _ => reject_on(core::file!(), core::line!(), PARSE_ERROR_CODE).await,
            }
        }
    }
}

impl<T, S: HasOutput<T>> HasOutput<Option<T>> for SubInterp<S> {
    type Output = Option<S::Output>;
}

impl<T, S: HasOutput<T> + AsyncParser<T, BS>, BS: Readable> AsyncParser<Option<T>, BS>
    for SubInterp<S>
{
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let [byte]: [u8; 1] = input.read().await;
            match byte {
                0 => None,
                1 => Some(self.0.parse(input).await),
                _ => reject_on(core::file!(), core::line!(), PARSE_ERROR_CODE).await,
            }
        }
    }
}

/*
#### ULEB128-Encoded Integers

The BCS format also uses the [ULEB128 encoding](https://en.wikipedia.org/wiki/LEB128) internally
to represent unsigned 32-bit integers in two cases where small values are usually expected:
(1) lengths of variable-length sequences and (2) tags of enum values (see the corresponding
sections below).

|Type                       |Original data          |Hex representation |Serialized bytes   |
|---                        |---                    |---                |---                |
|ULEB128-encoded u32-integer|2^0 = 1                |0x00000001         |01                 |
|                           |2^7 = 128              |0x00000080         |80 01              |
|                           |2^14 = 16384           |0x00004000         |80 80 01           |
|                           |2^21 = 2097152         |0x00200000         |80 80 80 01        |
|                           |2^28 = 268435456       |0x10000000         |80 80 80 80 01     |
|                           |9487                   |0x0000250f         |8f 4a              |

In general, a ULEB128 encoding consists of a little-endian sequence of base-128 (7-bit)
digits. Each digit is completed into a byte by setting the highest bit to 1, except for the
last (highest-significance) digit whose highest bit is set to 0.

In BCS, the result of decoding ULEB128 bytes is required to fit into a 32-bit unsigned
integer and be in canonical form. For instance, the following values are rejected:
* 80 80 80 80 80 01 (2^36) is too large.
* 80 80 80 80 10 (2^33) is too large.
* 80 00 is not a minimal encoding of 0.
*/

pub struct ULEB128;

impl HasOutput<ULEB128> for DefaultInterp {
    type Output = u32;
}

// Parsing logic copied from bcs repo
impl<BS: Readable> AsyncParser<ULEB128, BS> for DefaultInterp {
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let mut value: u64 = 0;
            for shift in (0..32).step_by(7) {
                let [byte]: [u8; 1] = input.read().await;
                let digit = byte & 0x7f;
                value |= u64::from(digit) << shift;
                // If the highest bit of `byte` is 0, return the final value.
                if digit == byte {
                    if shift > 0 && digit == 0 {
                        // We only accept canonical ULEB128 encodings, therefore the
                        // heaviest (and last) base-128 digit must be non-zero.
                        reject_on(core::file!(), core::line!(), PARSE_ERROR_CODE).await
                    }
                    break;
                }
            }
            // Decoded integer must not overflow.
            use core::convert::TryFrom;
            match u32::try_from(value) {
                Ok(v) => v,
                Err(_) => reject_on::<u32>(core::file!(), core::line!(), PARSE_ERROR_CODE).await,
            }
        }
    }
}

pub type Vec<T, const N: usize> = DArray<ULEB128, T, N>;

#[cfg(test)]
mod test {
    use super::*;
    use core::future::Future;
    pub use num_traits::FromPrimitive;
    // trace_macros!(true);
    // trace_macros!(false);

    #[cfg(target_family = "bolos")]
    #[allow(unused_imports)]
    use ledger_device_sdk::testing::TestType;
    #[cfg(target_family = "bolos")]
    use testmacro::test_item as test;

    #[derive(Clone)]
    struct TestReadable<const N: usize>([u8; N], usize);
    impl<const M: usize> Readable for TestReadable<M> {
        type OutFut<'a, const N: usize> = impl 'a + Future<Output = [u8; N]>;
        fn read<'a: 'b, 'b, const N: usize>(&'a mut self) -> Self::OutFut<'b, N> {
            if self.1 + N <= self.0.len() {
                let offset = self.1;
                self.1 += N;
                use core::convert::TryInto;
                core::future::ready(self.0[offset..self.1].try_into().unwrap())
            } else {
                panic!("Read past end of input");
            }
        }
    }

    impl<const N: usize> ReadableLength for TestReadable<N> {
        fn index(&self) -> usize {
            self.1
        }
    }

    use core::task::*;

    static RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
        |a| RawWaker::new(a, &RAW_WAKER_VTABLE),
        |_| {},
        |_| {},
        |_| {},
    );

    fn poll_once<F: Future>(mut input: F) -> core::task::Poll<F::Output> {
        let waker = unsafe { Waker::from_raw(RawWaker::new(&(), &RAW_WAKER_VTABLE)) };
        let mut ctxd = Context::from_waker(&waker);
        let mut pinned = unsafe { core::pin::Pin::new_unchecked(&mut input) };
        pinned.as_mut().poll(&mut ctxd)
    }

    #[test]
    fn test_varint() {
        let mut input = TestReadable([0, 0, 0], 0);
        assert_eq!(poll_once(ULEB128.def_parse(&mut input)), Poll::Ready(0));
    }
    #[test]
    fn test_varint2() {
        let mut input = TestReadable([255, 0, 0], 0);
        assert_eq!(poll_once(ULEB128.def_parse(&mut input)), Poll::Pending);
    }
    #[test]
    fn test_varint3() {
        let mut input = TestReadable([254, 0, 0], 0);
        assert_eq!(poll_once(ULEB128.def_parse(&mut input)), Poll::Pending);
    }
    #[test]
    fn test_varint4() {
        let mut input = TestReadable([255, 0, 0], 0);
        assert_eq!(poll_once(ULEB128.def_parse(&mut input)), Poll::Pending);
    }
    #[test]
    fn test_varint5() {
        let mut input = TestReadable([0, 0, 0], 0);
        assert_eq!(poll_once(ULEB128.def_parse(&mut input)), Poll::Ready(0));
    }
    #[test]
    fn test_varint6() {
        let mut input = TestReadable([1, 0, 0], 0);
        assert_eq!(poll_once(ULEB128.def_parse(&mut input)), Poll::Ready(1));
    }
    #[test]
    fn test_varint7() {
        let mut input = TestReadable([2, 0, 0], 0);
        assert_eq!(poll_once(ULEB128.def_parse(&mut input)), Poll::Ready(2));
    }
    #[test]
    fn test_varint8() {
        let mut input = TestReadable([128, 128, 128, 128, 2, 0, 0], 0);
        assert_eq!(
            poll_once(ULEB128.def_parse(&mut input)),
            Poll::Ready(1 << (7 * 4 + 1))
        );
    }
    #[test]
    fn test_varint9() {
        let mut input = TestReadable([128, 128, 128, 128, 2, 0, 0], 0);
        assert_eq!(
            poll_once(ULEB128.def_parse(&mut input)),
            Poll::Ready(1 << (7 * 4 + 1))
        );
    }
    #[test]
    fn test_varint_2_pow_0() {
        let mut input = TestReadable([0x01], 0);
        assert_eq!(
            poll_once(ULEB128.def_parse(&mut input)),
            Poll::Ready(2_u32.pow(0))
        );
    }
    #[test]
    fn test_varint_2_pow_7() {
        let mut input = TestReadable([0x80, 0x01], 0);
        assert_eq!(
            poll_once(ULEB128.def_parse(&mut input)),
            Poll::Ready(2_u32.pow(7))
        );
    }
    #[test]
    fn test_varint_2_pow_14() {
        let mut input = TestReadable([0x80, 0x80, 0x01], 0);
        assert_eq!(
            poll_once(ULEB128.def_parse(&mut input)),
            Poll::Ready(2_u32.pow(14))
        );
    }
    #[test]
    fn test_varint_2_pow_21() {
        let mut input = TestReadable([0x80, 0x80, 0x80, 0x01], 0);
        assert_eq!(
            poll_once(ULEB128.def_parse(&mut input)),
            Poll::Ready(2_u32.pow(21))
        );
    }
    #[test]
    fn test_varint_2_pow_28() {
        let mut input = TestReadable([0x80, 0x80, 0x80, 0x80, 0x01], 0);
        assert_eq!(
            poll_once(ULEB128.def_parse(&mut input)),
            Poll::Ready(2_u32.pow(28))
        );
    }
    #[test]
    fn test_varint_9487() {
        let mut input = TestReadable([0x8f, 0x4a], 0);
        assert_eq!(poll_once(ULEB128.def_parse(&mut input)), Poll::Ready(9487));
    }

    #[test]
    fn test_pair_varint_9487() {
        let mut input = TestReadable([0x8f, 0x4a, 0x8f, 0x4a], 0);
        assert_eq!(
            poll_once((ULEB128, ULEB128).def_parse(&mut input)),
            Poll::Ready((9487, 9487))
        );
    }

    #[test]
    fn test_triple() {
        let mut input = TestReadable([0x8f, 0x4a, 0x8f, 0x4a, 0x80, 0x80, 0x80, 0x80, 0x01], 0);
        assert_eq!(
            poll_once((ULEB128, ULEB128, ULEB128).def_parse(&mut input)),
            Poll::Ready((9487, 9487, 2_u32.pow(28)))
        );
    }

    #[test]
    fn test_nested() {
        let mut input = TestReadable([0x8f, 0x4a, 0x8f, 0x4a, 0x80, 0x80, 0x80, 0x80, 0x01], 0);
        assert_eq!(
            poll_once((ULEB128, (ULEB128, ULEB128)).def_parse(&mut input)),
            Poll::Ready((9487, (9487, 2_u32.pow(28))))
        );
    }
}
