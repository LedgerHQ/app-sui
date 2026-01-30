pub use crate::endianness::Endianness;

/// A schema for a single byte.
#[derive(Default)]
pub struct Byte;

/// A schema for a fixed `N`-item array of `I`, laid out sequentially in the input stream with no
/// prefix.
#[derive(Default)]
pub struct Array<I, const N: usize>(pub I);

/// A Dynamic length array, with a maximum length.
///
/// Memory layout is first the schema `N`, then `N` copies of schema `I`, with a maximum `N` of
/// `M`.
pub struct DArray<N, I, const M: usize>(pub N, pub I);

macro_rules! number_parser {
    ($p:ident, $n:expr) => {
        #[doc = "Parse an `E`-endian unsigned"]
        #[doc=$n]
        #[doc = "-bit integer"]
        #[derive(Default)]
        pub struct $p<const E: Endianness>;
    };
}

number_parser! { U16, "16" }
number_parser! { U32, "32" }
number_parser! { U64, "64" }
