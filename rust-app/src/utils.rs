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

pub fn get_amount_in_decimals(amount: u64, decimals: u8) -> (u64, ArrayString<12>) {
    let factor_pow = decimals as u32;
    let factor = u64::pow(10, factor_pow);
    let quotient = amount / factor;
    let remainder = amount % factor;
    let mut remainder_str: ArrayString<12> = ArrayString::new();
    {
        // Make a string for the remainder, containing at lease one zero
        // So 1 SUI will be displayed as "1.0"
        let mut rem = remainder;
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
