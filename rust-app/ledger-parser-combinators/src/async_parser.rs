//! Parser implmementation using Futures.
//!
//! The main entry point of this module is [AsyncParser].
//!
//! Note: Currently, all of the parsers in this module are implemented using async blocks, but it is
//! worth remembering that if for some reason one of them is exhibiting poor stack / state
//! behavior, we _can_ manually construct a state type and directly implement Future for that
//! state.
//!
use crate::endianness::{Convert, Endianness};
use crate::interp::{Action, DefaultInterp, DropInterp, ObserveBytes, SubInterp};
use crate::schema::*;

use arrayvec::ArrayVec;
use core::convert::TryInto;
use core::future::Future;
use core::pin::Pin;
use core::task::Context;
#[cfg(feature = "logging")]
use ledger_log::*;
use pin_project::pin_project;

pub trait HasDefParser<BS: Readable>
where
    DefaultInterp: HasOutput<Self>,
{
    fn def_parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c>;

    /// Type synonym for the future returned by this parser.
    type State<'c>: Future<Output = <DefaultInterp as HasOutput<Self>>::Output>
    where
        BS: 'c,
        Self: 'c;
}

impl<T, BS: Readable> HasDefParser<BS> for T
where
    DefaultInterp: AsyncParser<T, BS>,
{
    fn def_parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        <DefaultInterp as AsyncParser<T, BS>>::parse(&DefaultInterp, input)
    }
    type State<'c> = impl Future<Output = <DefaultInterp as HasOutput<Self>>::Output> + 'c where BS: 'c, T: 'c;
}

pub static mut REJECTED_CODE: u16 = 0;

// This is an unused error code, see SyscallError in SDK
pub const PARSE_ERROR_CODE: u16 = 0x6850;

/// Reject the parse.
pub fn reject<T>(error_code: u16) -> impl Future<Output = T> {
    #[cfg(feature = "logging")]
    error!("Rejecting parse");
    // Do some out-of-band rejection thingie
    unsafe {
        REJECTED_CODE = error_code;
    }
    core::future::pending()
}

#[allow(unused_variables)]
pub fn reject_on<T>(file: &'static str, line: u32, error_code: u16) -> impl Future<Output = T> {
    #[cfg(feature = "logging")]
    error!("Rejecting, {}:{}", file, line);
    unsafe {
        REJECTED_CODE = error_code;
    }
    core::future::pending()
}

pub fn reset_rejected() {
    unsafe {
        REJECTED_CODE = 0;
    }
}

#[pin_project]
pub struct TryFuture<F: Future>(#[pin] pub F);
impl<F: Future> Future for TryFuture<F> {
    type Output = Option<F::Output>;
    fn poll(self: Pin<&mut Self>, ctxd: &mut Context<'_>) -> core::task::Poll<Self::Output> {
        use core::task::Poll;
        unsafe {
            REJECTED_CODE = 0;
        }
        match self.project().0.poll(ctxd) {
            Poll::Pending if unsafe { REJECTED_CODE != 0 } => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
            Poll::Ready(r) => Poll::Ready(Some(r)),
        }
    }
}

/// Readable defines an interface for input to a parser.
pub trait Readable {
    /// Type alias for the future type of read
    type OutFut<'a, const N: usize>: 'a + Future<Output = [u8; N]>
    where
        Self: 'a;
    /// read N bytes from this Readable; returns a future that will complete with a byte array of
    /// the result.
    fn read<'a: 'b, 'b, const N: usize>(&'a mut self) -> Self::OutFut<'b, N>;
}

pub trait ReadableLength {
    fn index(&self) -> usize;
}

#[derive(Clone)]
pub struct LengthTrack<R: Readable>(pub R, pub usize);

impl<BS: 'static + Readable> Readable for LengthTrack<BS> {
    type OutFut<'a, const N: usize> = impl Future<Output = [u8; N]> + 'a;
    fn read<'a: 'b, 'b, const N: usize>(&'a mut self) -> Self::OutFut<'b, N> {
        self.1 += N;
        self.0.read()
    }
}

impl<R: Readable> ReadableLength for LengthTrack<R> {
    fn index(&self) -> usize {
        self.1
    }
}

pub trait UnwrappableReadable: Readable {
    type Wrapped: Clone;
    fn unwrap_clone(&self) -> Self::Wrapped;
}

pub trait HasOutput<Schema: ?Sized> {
    type Output;
}

/// Core trait; parser that consumes a Readable according to the Schema and returns a future returning the result type.
///
///
pub trait AsyncParser<Schema, BS: Readable>: HasOutput<Schema> {
    /// Parse input, returning a future that returns this parser's return type for this schema.
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c>;

    /// Type synonym for the future returned by this parser.
    type State<'c>: Future<Output = Self::Output>
    where
        BS: 'c,
        Self: 'c;
}

impl<T, S: HasOutput<T>, const N: usize> HasOutput<Array<T, N>> for SubInterp<S> {
    type Output = [S::Output; N];
}

impl<S, T, BS: Readable, const N: usize> AsyncParser<Array<T, N>, BS> for SubInterp<S>
where
    T: DefaultArray,
    S: AsyncParser<T, BS>,
{
    type State<'c> = impl Future<Output = [<S as HasOutput<T>>::Output; N]> + 'c where BS: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let mut accumulator = ArrayVec::<<S as HasOutput<T>>::Output, N>::new();
            while !accumulator.is_full() {
                match accumulator.try_push(<S as AsyncParser<T, BS>>::parse(&self.0, input).await) {
                    Ok(rv) => rv,
                    _ => reject(PARSE_ERROR_CODE).await,
                };
            }
            match accumulator.take().into_inner() {
                Ok(rv) => rv,
                _ => reject(PARSE_ERROR_CODE).await,
            }
        }
    }
}

impl HasOutput<Byte> for DefaultInterp {
    type Output = u8;
}

impl<BS: Readable> AsyncParser<Byte, BS> for DefaultInterp {
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let [u] = input.read().await;
            u
        }
    }
}

/// DefaultArray marks array element types where there is no specialization for the array type.
///
/// This allows trait selection to operate with positive reasoning except for
/// where we implement !DefaultArray explicitly for particular types, such as Byte.
pub auto trait DefaultArray {}

impl !DefaultArray for Byte {}

/// We can implement DefaultInterp for [u8; N] becuase Byte has !DefaultArray.
impl<const N: usize> HasOutput<Array<Byte, N>> for DefaultInterp {
    type Output = [u8; N];
}

/// We can implement DefaultInterp for [u8; N] becuase Byte has !DefaultArray.
///
/// Overlap is not permitted, but this doesn't overlap because we've disabled the default
/// implementation.
impl<const N: usize, BS: Readable> AsyncParser<Array<Byte, N>, BS> for DefaultInterp {
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move { input.read().await }
    }
}

macro_rules! number_parser {
    ($p:ident, $size:expr, $t:ty) => {
        impl<const E: Endianness> HasOutput<$p<E>> for DefaultInterp {
            type Output = $t;
        }
        impl<const E: Endianness, BS: Readable> AsyncParser<$p<E>, BS> for DefaultInterp
        where
            $t: Convert<E>,
        {
            type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c;
            fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
                async move { Convert::<E>::deserialize(input.read::<$size>().await) }
            }
        }
    };
}
number_parser! { U16, 2, u16 }
number_parser! { U32, 4, u32 }
number_parser! { U64, 8, u64 }

impl<T, const N: usize> HasOutput<Array<T, N>> for DefaultInterp
where
    T: DefaultArray,
    DefaultInterp: HasOutput<T>,
{
    type Output = [<DefaultInterp as HasOutput<T>>::Output; N];
}

impl<T, const N: usize, BS: Readable> AsyncParser<Array<T, N>, BS> for DefaultInterp
where
    T: DefaultArray,
    DefaultInterp: AsyncParser<T, BS>,
{
    #[cfg(not(version("1.62")))] // This may need to be "fixed" based on testing on versions above 1.62
    type State<'c> = impl Future<Output = Self::Output> where BS: 'c;
    #[cfg(version("1.62"))]
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        <SubInterp<DefaultInterp> as AsyncParser<Array<T, N>, BS>>::parse(
            &SubInterp(DefaultInterp),
            input,
        )
    }
}

impl<N, I, S: HasOutput<I>, const M: usize> HasOutput<DArray<N, I, M>> for SubInterp<S> {
    type Output = ArrayVec<S::Output, M>;
}

impl<S, N, I, const M: usize, BS: Readable> AsyncParser<DArray<N, I, M>, BS> for SubInterp<S>
where
    S: AsyncParser<I, BS>,
    DefaultInterp: AsyncParser<N, BS>,
    <DefaultInterp as HasOutput<N>>::Output: TryInto<usize>,
{
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let length: usize = match DefaultInterp.parse(input).await.try_into() {
                Ok(a) => a,
                Err(_) => reject(PARSE_ERROR_CODE).await,
            };
            let mut accumulator = ArrayVec::new();
            for _ in 0..length {
                match accumulator.try_push(self.0.parse(input).await) {
                    Ok(rv) => rv,
                    _ => reject(PARSE_ERROR_CODE).await,
                };
            }
            accumulator
        }
    }
}

impl<Schema> HasOutput<Schema> for DropInterp {
    type Output = ();
}

impl<N, I, const M: usize, BS: Readable> AsyncParser<DArray<N, I, M>, BS> for DropInterp
where
    DefaultInterp: AsyncParser<I, BS>,
    DefaultInterp: AsyncParser<N, BS>,
    <DefaultInterp as HasOutput<N>>::Output: TryInto<usize>,
    DefaultInterp: AsyncParser<I, BS>,
{
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let length: usize =
                match <DefaultInterp as AsyncParser<N, BS>>::parse(&DefaultInterp, input)
                    .await
                    .try_into()
                {
                    Ok(a) => a,
                    Err(_) => reject(PARSE_ERROR_CODE).await,
                };
            for _ in 0..length {
                <DefaultInterp as AsyncParser<I, BS>>::parse(&DefaultInterp, input).await;
            }
        }
    }
}

impl<T, S: HasOutput<T>, R, F: Fn(<S as HasOutput<T>>::Output) -> Option<R>> HasOutput<T>
    for Action<S, F>
{
    type Output = R;
}

impl<
        T,
        S: AsyncParser<T, BS>,
        R,
        F: Fn(<S as HasOutput<T>>::Output) -> Option<R>,
        BS: Readable,
    > AsyncParser<T, BS> for Action<S, F>
{
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c, F: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            match self.1(self.0.parse(input).await) {
                Some(a) => a,
                None => reject(PARSE_ERROR_CODE).await,
            }
        }
    }
}

pub struct FutAction<S, F>(pub S, pub F);

impl<
        T,
        S: HasOutput<T>,
        R,
        F: Fn(<S as HasOutput<T>>::Output) -> Fut,
        Fut: Future<Output = Option<R>>,
    > HasOutput<T> for FutAction<S, F>
{
    type Output = R;
}

impl<
        T,
        S: AsyncParser<T, BS>,
        R,
        F: Fn(<S as HasOutput<T>>::Output) -> Fut,
        Fut: Future<Output = Option<R>>,
        BS: Readable,
    > AsyncParser<T, BS> for FutAction<S, F>
{
    type State<'c> = impl 'c + Future<Output = Self::Output> where BS: 'c, F: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            match self.1(self.0.parse(input).await).await {
                Some(a) => a,
                None => reject(PARSE_ERROR_CODE).await,
            }
        }
    }
}

impl<A, R, S, SR> HasOutput<A> for Action<S, fn(&SR, &mut Option<R>) -> Option<()>> {
    type Output = R;
}

impl<A, R, S: AsyncParser<A, BS>, BS: Readable> AsyncParser<A, BS>
    for Action<S, fn(&<S as HasOutput<A>>::Output, &mut Option<R>) -> Option<()>>
{
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c, R: 'c, <S as HasOutput<A>>::Output: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let mut destination = None;
            match self.1(&self.0.parse(input).await, &mut destination) {
                Some(()) => match destination {
                    Some(a) => a,
                    None => reject(PARSE_ERROR_CODE).await,
                },
                None => reject(PARSE_ERROR_CODE).await,
            }
        }
    }
}

/// Slightly different version of Action; present for reasons of impl selection.
pub struct FAction<S, O, R>(pub S, pub fn(O) -> Option<R>);

impl<T, S: HasOutput<T>, R> HasOutput<T> for FAction<S, S::Output, R> {
    type Output = R;
}

impl<T, S: AsyncParser<T, BS>, R, BS: Readable> AsyncParser<T, BS>
    for FAction<S, <S as HasOutput<T>>::Output, R>
{
    // FAction<S, fn(<S as HasOutput<T>>::Output) -> Option<R>> {
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c, R: 'c, <S as HasOutput<T>>::Output: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            match self.1(self.0.parse(input).await) {
                Some(a) => a,
                None => reject(PARSE_ERROR_CODE).await,
            }
        }
    }
}

impl<A, X, F, S: HasOutput<A>> HasOutput<A> for ObserveBytes<X, F, S> {
    type Output = (X, Option<<S as HasOutput<A>>::Output>);
}

/// HashIntercept wraps a Readable and updates a hash-like object with the data as it passes.
///
/// Used to support ObserveBytes for async parsers.
pub struct HashIntercept<BS, X, F>(pub BS, pub X, pub F);

impl<BS: 'static + Readable, X: 'static, F: Fn(&mut X, &[u8])> Readable
    for HashIntercept<BS, X, F>
{
    type OutFut<'a, const N: usize> = impl core::future::Future<Output = [u8; N]> + 'a where F: 'a;
    fn read<'a: 'b, 'b, const N: usize>(&'a mut self) -> Self::OutFut<'b, N> {
        async move {
            let d = self.0.read().await;
            self.2(&mut self.1, &d);
            d
        }
    }
}

/// ObserveBytes for AsyncParser operates by passing a HashIntercept to the sub-parser.
impl<
        X: 'static,
        F: Fn(&mut X, &[u8]) + Copy,
        S: AsyncParser<A, HashIntercept<BS, X, F>>,
        A,
        BS: 'static + Readable + Clone,
    > AsyncParser<A, BS> for ObserveBytes<X, F, S>
{
    type State<'c> = impl Future<Output = Self::Output> + 'c where S: 'c, F: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let mut hi = HashIntercept(input.clone(), (self.0)(), self.1);
            let rv = self.2.parse(&mut hi).await;
            *input = hi.0;
            (hi.1, Some(rv))
        }
    }
}

pub use paste::paste;

/// Pairs of parsers parse the sequence of their two schemas.
macro_rules! tuple_impls {
    // Stopping criteria (1-ary tuple)
    ($T:ident) => {
        tuple_impls!(@impl $T);
    };
    // Running criteria (n-ary tuple, with n >= 2)
    ($T:ident $( $U:ident )+) => {
        tuple_impls!($( $U )+);
        tuple_impls!(@impl $T $( $U )+);
    };
    // "Private" internal implementation
    (@impl $( $T:ident )+) => {
        $crate::async_parser::paste! {

            impl<$([<Schema $T>],)+ $([<Parser $T>]: HasOutput<[<Schema $T>]>,)+> HasOutput<($([<Schema $T>],)+)> for ($([<Parser $T>] ,)+) {
                type Output = ($([<Parser $T>]::Output,)+);
            }

            impl<$([<Schema $T>],)+ $([<Parser $T>]: AsyncParser<[<Schema $T>], BS>,)+ BS: Readable> AsyncParser<($([<Schema $T>],)+), BS>
                for ($([<Parser $T>],)+)
            {
                type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c, $([<Parser $T>] : 'c,)+;
                fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
                    async move {
                        $(let [<output_ $T:lower>] = self.${index()}.parse(input).await;)+
                        ($([<output_ $T:lower>],)+)
                    }
                }
            }

            impl<$([<Schema $T>],)+> HasOutput<($([<Schema $T>],)+)> for DefaultInterp
            where
                $(DefaultInterp: HasOutput<[<Schema $T>]>,)+
            {
                type Output = ($(<DefaultInterp as HasOutput<[<Schema $T>]>>::Output,)+);
            }

            impl<$([<Schema $T>],)+ BS: Readable> AsyncParser<($([<Schema $T>],)+), BS>
                for DefaultInterp
            where
                $(DefaultInterp: AsyncParser<[<Schema $T>], BS>,)+
            {
                type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c;
                fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
                    async move {
                        $(let [<output_ $T:lower>] = <DefaultInterp as AsyncParser<[<Schema $T>], BS>>::parse(&DefaultInterp, input).await;)+
                        ($([<output_ $T:lower>],)+)
                    }
                }
            }
        }
    }
}

tuple_impls!(E D C B A Z Y X W V U T);

pub trait LengthDelimitedParser<Schema, BS: Readable>: HasOutput<Schema> {
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS, length: usize) -> Self::State<'c>;
    type State<'c>: Future<Output = Self::Output>
    where
        BS: 'c,
        Self: 'c;
}

impl<
        T,
        S: LengthDelimitedParser<T, BS>,
        R,
        BS: Readable,
        F: Fn(<S as HasOutput<T>>::Output) -> Option<R>,
    > LengthDelimitedParser<T, BS> for Action<S, F>
{
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c, F: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS, length: usize) -> Self::State<'c> {
        async move {
            match self.1(self.0.parse(input, length).await) {
                Some(a) => a,
                None => reject_on(core::file!(), core::line!(), PARSE_ERROR_CODE).await,
            }
        }
    }
}

impl<
        T,
        S: LengthDelimitedParser<T, BS>,
        R,
        F: Fn(<S as HasOutput<T>>::Output) -> Fut,
        Fut: Future<Output = Option<R>>,
        BS: Readable,
    > LengthDelimitedParser<T, BS> for FutAction<S, F>
{
    type State<'c> = impl 'c + Future<Output = Self::Output> where BS: 'c, F: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS, length: usize) -> Self::State<'c> {
        async move {
            match self.1(self.0.parse(input, length).await).await {
                Some(a) => a,
                None => reject(PARSE_ERROR_CODE).await,
            }
        }
    }
}

struct Bind<S, F>(S, F);

impl<T, S: HasOutput<T>, R, Fut: Future<Output = R>, F: Fn(<S as HasOutput<T>>::Output) -> Fut>
    HasOutput<T> for Bind<S, F>
{
    type Output = Fut::Output;
}

impl<
        T,
        S: LengthDelimitedParser<T, BS>,
        R,
        BS: Readable,
        Fut: Future<Output = R>,
        F: Fn(<S as HasOutput<T>>::Output) -> Fut,
    > LengthDelimitedParser<T, BS> for Bind<S, F>
{
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c, F: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS, length: usize) -> Self::State<'c> {
        async move { self.1(self.0.parse(input, length).await).await }
    }
}
impl<
        T,
        S: AsyncParser<T, BS>,
        R,
        BS: Readable,
        Fut: Future<Output = R>,
        F: Fn(<S as HasOutput<T>>::Output) -> Fut,
    > AsyncParser<T, BS> for Bind<S, F>
{
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c, F: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move { self.1(self.0.parse(input).await).await }
    }
}

impl<Schema, BS: Readable> LengthDelimitedParser<Schema, BS> for DropInterp {
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS, length: usize) -> Self::State<'c> {
        async move {
            #[cfg(feature = "logging")]
            trace!("Dropping");
            for _ in 0..length {
                let [_]: [u8; 1] = input.read().await;
            }
            #[cfg(feature = "logging")]
            trace!("Dropped");
        }
    }
}
