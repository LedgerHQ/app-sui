// A couple type ascription functions to help the compiler along.
pub const fn mkfn<A, B, C>(q: fn(&A, &mut B) -> C) -> fn(&A, &mut B) -> C {
    q
}
pub const fn mkmvfn<A, B, C>(q: fn(A, &mut B) -> Option<C>) -> fn(A, &mut B) -> Option<C> {
    q
}
/*
const fn mkvfn<A>(q: fn(&A,&mut Option<()>)->Option<()>) -> fn(&A,&mut Option<()>)->Option<()> {
q
}
*/

use core::future::Future;
use core::pin::*;
use core::task::*;
use pin_project::pin_project;
#[pin_project]
pub struct NoinlineFut<F: Future>(#[pin] pub F);

impl<F: Future> Future for NoinlineFut<F> {
    type Output = F::Output;
    #[inline(never)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> core::task::Poll<Self::Output> {
        self.project().0.poll(cx)
    }
}

use arrayvec::ArrayString;

/// Largest `decimals` this can divide by: 10^19 is the biggest power of ten that
/// fits in a u64. Beyond it `u64::pow` wraps -- `overflow-checks` is off in both
/// profiles -- and reaches exactly zero from 10^64 on, which would make the division
/// below panic. Every source of a decimals value must be bounded by this before it
/// gets here.
pub const MAX_COIN_DECIMALS: u8 = 19;

/// One character per decimal place the app is willing to accept, so the fractional
/// part is always rendered in full. A narrower buffer silently dropped the digits
/// past its capacity, which for a token declaring 13 or more decimals could render
/// a small nonzero amount as a row of zeroes.
pub const AMOUNT_FRACTION_LEN: usize = MAX_COIN_DECIMALS as usize;

/// Longest `{quotient}.{fraction}` `get_amount_in_decimals` can produce, for sizing
/// the buffers its output is written into. The quotient loses a digit for every
/// decimal place the fraction gains -- `amount` is a u64, so the quotient stays
/// below 10^(20 - decimals) -- which caps the pair at 20 digits plus the separator
/// regardless of the coin's decimals.
pub const AMOUNT_TEXT_LEN: usize = 21;

// That trade-off, and so AMOUNT_TEXT_LEN, only holds while a u64 amount cannot have
// more decimal places than digits. It is the same bound `factor` needs to stay
// inside a u64, so asserting it here pins both rather than leaving them to the
// comment on MAX_COIN_DECIMALS.
const _: () = assert!(MAX_COIN_DECIMALS <= 19);

pub fn get_amount_in_decimals(
    amount: u64,
    decimals: u8,
) -> (u64, ArrayString<AMOUNT_FRACTION_LEN>) {
    debug_assert!(decimals <= MAX_COIN_DECIMALS);
    let factor_pow = decimals as u32;
    let factor = u64::pow(10, factor_pow);
    let quotient = amount / factor;
    let remainder = amount % factor;
    let mut remainder_str: ArrayString<AMOUNT_FRACTION_LEN> = ArrayString::new();
    {
        // Make a string for the remainder, containing at lease one zero
        // So 1 SUI will be displayed as "1.0"
        let mut rem = remainder;
        // At most `decimals` digits are pushed, and the buffer holds
        // MAX_COIN_DECIMALS of them, so no push can be dropped.
        for i in 0..factor_pow {
            let f = u64::pow(10, factor_pow - i - 1);
            let r = rem / f;
            let _ = remainder_str.try_push(char::from(b'0' + r as u8));
            rem %= f;
            if rem == 0 {
                break;
            }
        }
    }
    (quotient, remainder_str)
}

extern crate alloc;
use alloc::collections::BTreeMap;
use core::mem::size_of;

/// Estimates the memory usage of a BTreeMap
pub fn estimate_btree_map_usage<K, V>(map: &BTreeMap<K, V>) -> usize {
    let base_size = size_of::<BTreeMap<K, V>>();

    // Size of key and value types
    let key_size = size_of::<K>();
    let value_size = size_of::<V>();

    // Approximate overhead per node in the BTree
    // This is an estimation as the exact overhead depends on implementation details
    let node_overhead = 16; // Pointer overhead, metadata, etc.

    let entry_size = key_size + value_size + node_overhead;

    base_size + (entry_size * map.len())
}
