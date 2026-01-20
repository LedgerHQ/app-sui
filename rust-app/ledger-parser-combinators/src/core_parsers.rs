use crate::endianness::Endianness;

pub use crate::schema::*;

// use generic_array::{ArrayLength, GenericArray};
pub trait RV {
    type R;
}

impl RV for Byte {
    type R = u8;
}

/// Fixed-length array.
impl<I: RV, const N: usize> RV for Array<I, N> {
    type R = [I::R; N];
}

use arrayvec::ArrayVec;
use core::convert::TryInto;
impl<N: RV, I: RV, const M: usize> RV for DArray<N, I, M>
where
    <N as RV>::R: TryInto<usize>,
{
    type R = ArrayVec<I::R, M>;
}

macro_rules! number_parser {
    ($p:ident, $t:ty) => {
        impl<const E: Endianness> RV for $p<E> {
            type R = $t;
        }
    };
}

number_parser! { U16, u16 }
number_parser! { U32, u32 }
number_parser! { U64, u64 }

//pub enum OutOfBand {
//    Prompt('a mut dyn Fn() -> usize),
//}

pub struct NOf<I, N>(pub N, pub I);

impl<I: RV, N: RV> RV for NOf<I, N>
where
    <I as RV>::R: TryInto<()>,
    <N as RV>::R: TryInto<usize>,
{
    type R = ();
}

//pub struct DArray<I, N>;
//pub struct Table;

/// LengthFallback; frame a subparser with a length byte.
pub struct LengthFallback<N, S>(pub N, pub S);

/// Alternative; schema is either A or B.
pub struct Alt<A, B>(pub A, pub B);
