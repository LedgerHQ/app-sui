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

pub fn get_amount_in_decimals(amount: u64) -> (u64, ArrayString<12>) {
    let factor_pow = 9;
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
