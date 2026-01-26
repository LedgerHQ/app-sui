use crate::core_parsers::*;
use crate::endianness::{Convert, Endianness};
use arrayvec::ArrayVec;

#[derive(PartialEq, Debug)]
pub enum OOB {
    // Prompt removed due to excessive memory use; we gain testability improvements if we can
    // reinstate an OOB for prompts and do the co-routine again, but we can't do that at this
    // memory use.
    //
    // Prompt([ArrayString<128>;2]),
    Reject,
}

// PResult stands for Partial Result
// None = Incomplete
pub type PResult<T> = Option<T>;

// This represents the part of the input that hasn't been yet consumed by the
// parser, represented as a slice of the input.
pub type RemainingSlice<'a> = &'a [u8];

// If the parser does its job correctly, we just need to return the remaining
// slice. If the parser still needs data, it will return a None (and in that
// case the remaining slice is empty because we consumed it all). If the parser
// encounters an error condition, it will signal it in the OOB type, and we'll
// return the remaining slice for further elaboration or resuming.
pub type ParseResult<'a> = Result<RemainingSlice<'a>, (PResult<OOB>, RemainingSlice<'a>)>;

pub fn reject<R>(chunk: &[u8]) -> Result<R, (PResult<OOB>, &[u8])> {
    Err(rej(chunk))
}

pub fn need_more<R>(chunk: &[u8]) -> Result<R, (PResult<OOB>, &[u8])> {
    Err((None, chunk))
}

// Core trait; describes an "interpretation" of a given datatype (specified with the types from
// core_parsers), which can have a variable return type and do stateful actions.

pub trait ParserCommon<P> {
    type State;
    type Returning;
    fn init(&self) -> Self::State;
    fn init_in_place(&self, state: *mut core::mem::MaybeUninit<Self::State>) {
        unsafe {
            (*state).as_mut_ptr().write(self.init());
        }
    }
}

pub trait InterpParser<P>: ParserCommon<P> {
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a>;
}

pub trait DynParser<P>: ParserCommon<P> {
    type Parameter;
    fn init_param(
        &self,
        params: Self::Parameter,
        state: &mut Self::State,
        destination: &mut Option<Self::Returning>,
    );
}

pub struct DefaultInterp;

pub struct SubInterp<S>(pub S);

// Structurally checks and skips the format, but consumes only the minimum memory required to do so
// and returns nothing.
pub struct DropInterp;

pub struct ByteState;

#[inline(never)]
pub fn init_with_default<X: Default>(x: &mut Option<X>) {
    *x = Some(X::default());
}

#[inline(never)]
pub fn set_from_thunk<X, F: FnOnce() -> X>(x: &mut X, f: F) {
    *x = f();
}

#[inline(never)]
pub fn call_me_maybe<F: FnOnce() -> Option<()>>(f: F) -> Option<()> {
    f()
}

impl ParserCommon<Byte> for DefaultInterp {
    type State = ByteState;
    type Returning = u8;
    fn init(&self) -> Self::State {
        Self::State {}
    }
}

impl InterpParser<Byte> for DefaultInterp {
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        _state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        match chunk.split_first() {
            None => Err((None, chunk)),
            Some((first, rest)) => {
                *destination = Some(*first);
                Ok(rest)
            }
        }
    }
}

impl ParserCommon<Byte> for DropInterp {
    type State = ();
    type Returning = ();
    fn init(&self) -> Self::State {}
}

impl InterpParser<Byte> for DropInterp {
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        _state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        match chunk.split_first() {
            None => Err((None, chunk)),
            Some((_, rest)) => {
                *destination = Some(());
                Ok(rest)
            }
        }
    }
}

pub struct ForwardArrayParserState<Item, SubparserState, const N: usize> {
    buffer: ArrayVec<Item, N>,
    // We want to let our subparser stream into it
    subparser_destination: Option<Item>,
    subparser_state: SubparserState,
}

/* Note: we use this for parsing numbers. Additional requirement: don't stream into destination,
 * because number parser will be recreating destination each time. */
impl<I, S: ParserCommon<I>, const N: usize> ParserCommon<Array<I, N>> for SubInterp<S> {
    type State = ForwardArrayParserState<
        <S as ParserCommon<I>>::Returning,
        <S as ParserCommon<I>>::State,
        N,
    >;
    type Returning = [<S as ParserCommon<I>>::Returning; N];
    fn init(&self) -> Self::State {
        Self::State {
            buffer: ArrayVec::<<S as ParserCommon<I>>::Returning, N>::new(),
            subparser_destination: None,
            subparser_state: <S as ParserCommon<I>>::init(&self.0),
        }
    }
}

impl<I, S: InterpParser<I>, const N: usize> InterpParser<Array<I, N>> for SubInterp<S> {
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        let mut remaining: &'a [u8] = chunk;
        while !state.buffer.is_full() {
            let new_chunk = self.0.parse(
                &mut state.subparser_state,
                remaining,
                &mut state.subparser_destination,
            )?;
            remaining = new_chunk;
            state.buffer.push(
                core::mem::take(&mut state.subparser_destination).ok_or_else(|| rej(remaining))?,
            );
            state.subparser_state = <S as ParserCommon<I>>::init(&self.0);
        }
        match state.buffer.take().into_inner() {
            Ok(rv) => {
                *destination = Some(rv);
                Ok(remaining)
            }
            Err(_) => Err(rej(remaining)), // Should be impossible, could just panic.
        }
    }
}

macro_rules! number_parser {
    ($p:ident, $size:expr) => {
        impl<const E: Endianness> ParserCommon<$p<E>> for DefaultInterp
        where
            <$p<E> as RV>::R: Convert<E>,
        {
            type State = <DefaultInterp as ParserCommon<Array<Byte, $size>>>::State;
            type Returning = <$p<E> as RV>::R;
            fn init(&self) -> Self::State {
                <DefaultInterp as ParserCommon<Array<Byte, $size>>>::init(&DefaultInterp)
            }
        }
        impl<const E: Endianness> InterpParser<$p<E>> for DefaultInterp
        where
            <$p<E> as RV>::R: Convert<E>,
        {
            #[inline(never)]
            fn parse<'a, 'b>(
                &self,
                state: &'b mut Self::State,
                chunk: &'a [u8],
                destination: &mut Option<Self::Returning>,
            ) -> ParseResult<'a> {
                let mut sub_destination: Option<[u8; $size]> = None;
                let remainder = <DefaultInterp as InterpParser<Array<Byte, $size>>>::parse(
                    &DefaultInterp,
                    state,
                    chunk,
                    &mut sub_destination,
                )?;
                *destination = Some(Convert::<E>::deserialize(
                    (sub_destination.ok_or_else(|| rej(remainder))?),
                ));
                Ok(remainder)
            }
        }
        impl<const E: Endianness> ParserCommon<$p<E>> for DropInterp {
            type State = <SubInterp<DropInterp> as ParserCommon<Array<Byte, $size>>>::State;
            type Returning = ();
            fn init(&self) -> Self::State {
                <SubInterp<DropInterp> as ParserCommon<Array<Byte, $size>>>::init(&SubInterp(
                    DropInterp,
                ))
            }
        }
        impl<const E: Endianness> InterpParser<$p<E>> for DropInterp {
            #[inline(never)]
            fn parse<'a, 'b>(
                &self,
                state: &'b mut Self::State,
                chunk: &'a [u8],
                destination: &mut Option<Self::Returning>,
            ) -> ParseResult<'a> {
                let mut sub_destination: Option<[(); $size]> = None;
                let remainder = <SubInterp<DropInterp> as InterpParser<Array<Byte, $size>>>::parse(
                    &SubInterp(DropInterp),
                    state,
                    chunk,
                    &mut sub_destination,
                )?;
                *destination = Some(());
                return Ok(remainder);
            }
        }
    };
}
number_parser! { U16, 2 }
number_parser! { U32, 4 }
number_parser! { U64, 8 }

pub enum ForwardDArrayParserState<N, IS, I, const M: usize> {
    Length(N),
    Elements(ArrayVec<I, M>, usize, IS, Option<I>),
    Done,
}

use core::convert::TryFrom;
impl<N, I, S: ParserCommon<I>, const M: usize> ParserCommon<DArray<N, I, M>> for SubInterp<S>
where
    DefaultInterp: ParserCommon<N>,
    usize: TryFrom<<DefaultInterp as ParserCommon<N>>::Returning>,
    <S as ParserCommon<I>>::Returning: Clone,
{
    type State = ForwardDArrayParserState<
        <DefaultInterp as ParserCommon<N>>::State,
        <S as ParserCommon<I>>::State,
        <S as ParserCommon<I>>::Returning,
        M,
    >;
    type Returning = ArrayVec<<S as ParserCommon<I>>::Returning, M>;
    fn init(&self) -> Self::State {
        Self::State::Length(<DefaultInterp as ParserCommon<N>>::init(&DefaultInterp))
    }
}

impl<N, I, S: InterpParser<I>, const M: usize> InterpParser<DArray<N, I, M>> for SubInterp<S>
where
    DefaultInterp: InterpParser<N>,
    usize: TryFrom<<DefaultInterp as ParserCommon<N>>::Returning>,
    <S as ParserCommon<I>>::Returning: Clone,
{
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        use ForwardDArrayParserState::*;
        let mut cursor: &'a [u8] = chunk;
        loop {
            match state {
                Length(ref mut nstate) => {
                    let mut sub_destination: Option<<DefaultInterp as ParserCommon<N>>::Returning> =
                        None;
                    let newcur: &'a [u8] = <DefaultInterp as InterpParser<N>>::parse(
                        &DefaultInterp,
                        nstate,
                        chunk,
                        &mut sub_destination,
                    )?;
                    let len_temp = sub_destination.ok_or_else(|| rej(newcur))?;
                    cursor = newcur;
                    let len = <usize as TryFrom<<DefaultInterp as ParserCommon<N>>::Returning>>::try_from(len_temp).map_err(|_| rej(newcur))?;
                    set_from_thunk(state, || {
                        Elements(
                            ArrayVec::new(),
                            len,
                            <S as ParserCommon<I>>::init(&self.0),
                            None,
                        )
                    });
                }
                Elements(ref mut vec, len, ref mut istate, ref mut sub_destination) => {
                    while vec.len() < *len {
                        cursor = self.0.parse(istate, cursor, sub_destination)?;
                        vec.try_push(core::mem::take(sub_destination).ok_or_else(|| rej(cursor))?)
                            .map_err(|_| rej(cursor))?;
                        *istate = <S as ParserCommon<I>>::init(&self.0);
                    }
                    *destination = match core::mem::replace(state, Done) {
                        Elements(vec, _, _, _) => Some(vec),
                        _ => break Err(rej(cursor)),
                    };
                    break Ok(cursor);
                }
                Done => {
                    break Err(rej(cursor));
                }
            }
        }
    }
}

impl<I, const N: usize> ParserCommon<Array<I, N>> for DefaultInterp
where
    DefaultInterp: ParserCommon<I>,
{
    type State = <SubInterp<DefaultInterp> as ParserCommon<Array<I, N>>>::State;
    type Returning = <SubInterp<DefaultInterp> as ParserCommon<Array<I, N>>>::Returning;
    fn init(&self) -> Self::State {
        <SubInterp<DefaultInterp> as ParserCommon<Array<I, N>>>::init(&SubInterp(DefaultInterp))
    }
}

impl<I, const N: usize> InterpParser<Array<I, N>> for DefaultInterp
where
    DefaultInterp: InterpParser<I>,
{
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        <SubInterp<DefaultInterp> as InterpParser<Array<I, N>>>::parse(
            &SubInterp(DefaultInterp),
            state,
            chunk,
            destination,
        )
    }
}

/* // TODO: determine why this doesn't work.
impl< N, I, const M : usize> InterpParser<DArray<N, I, M>> for DefaultInterp where
    DefaultInterp : InterpParser<I> + InterpParser<N>,
    usize: From<<DefaultInterp as InterpParser<N>>::Returning> {
    type State = <SubInterp<DefaultInterp> as InterpParser<DArray< N, I, M> > >::State;
    type Returning = <SubInterp<DefaultInterp> as InterpParser<DArray< N, I, M> > >::Returning;
    fn init(&self) -> Self::State {
        <SubInterp<DefaultInterp> as InterpParser<DArray<N, I, M>>>::init(&SubInterp(DefaultInterp))
    }
    #[inline(never)]
    fn parse<'a, 'b>(&self, state: &'b mut Self::State, chunk: &'a [u8]) -> ParseResult<'a> {
        <SubInterp<DefaultInterp> as InterpParser<DArray<N, I, M>>>::parse(&SubInterp(DefaultInterp), state, chunk)
    }
}
*/

// Action is essentailly an fmap that can fail.
// We _could_ constraint F to actually be an fn(..) -> Option<()> to improve error messages when
// functions do not have the correct shape, but that reduces our ability to write different
// instances later.
#[derive(Clone)]
pub struct Action<S, F>(pub S, pub F);

impl<A, R, S: ParserCommon<A>> ParserCommon<A>
    for Action<S, fn(&<S as ParserCommon<A>>::Returning, &mut Option<R>) -> Option<()>>
{
    type State = (
        <S as ParserCommon<A>>::State,
        Option<<S as ParserCommon<A>>::Returning>,
    );
    type Returning = R;

    // #[inline(never)] // Causes stack size increase
    fn init(&self) -> Self::State {
        (<S as ParserCommon<A>>::init(&self.0), None)
    }

    #[inline(never)]
    fn init_in_place(&self, state: *mut core::mem::MaybeUninit<Self::State>) {
        self.0.init_in_place(unsafe {
            core::ptr::addr_of_mut!((*(*state).as_mut_ptr()).0)
                as *mut core::mem::MaybeUninit<<S as ParserCommon<A>>::State>
        });
        call_fn(|| unsafe {
            (core::ptr::addr_of_mut!((*(*state).as_mut_ptr()).1)
                as *mut Option<<S as ParserCommon<A>>::Returning>)
                .write(None)
        });
    }
}

impl<A, R, S: InterpParser<A>> InterpParser<A>
    for Action<S, fn(&<S as ParserCommon<A>>::Returning, &mut Option<R>) -> Option<()>>
{
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        let new_chunk = self.0.parse(&mut state.0, chunk, &mut state.1)?;
        match (self.1)(state.1.as_ref().ok_or_else(|| rej(new_chunk))?, destination) {
            None => Err(rej(new_chunk)),
            Some(()) => Ok(new_chunk),
        }
    }
}

impl<A, R, S: DynParser<A>> DynParser<A>
    for Action<S, fn(&<S as ParserCommon<A>>::Returning, &mut Option<R>) -> Option<()>>
{
    type Parameter = S::Parameter;
    #[inline(never)]
    fn init_param(
        &self,
        param: Self::Parameter,
        state: &mut Self::State,
        _destination: &mut Option<Self::Returning>,
    ) {
        set_from_thunk(&mut state.0, || <S as ParserCommon<A>>::init(&self.0));
        set_from_thunk(&mut state.1, || None);
        self.0.init_param(param, &mut state.0, &mut state.1);
    }
}

/* This impl exists to allow the _function_ of an Action to be the target of the parameter for
 * DynParser, thus giving an escape hatch to thread a parameter past a non-parameterized
 * parser. Whether this should still be an Action as opposed to some other name is not immediately
 * clear. */
impl<A, R, S: ParserCommon<A>, C> ParserCommon<A>
    for Action<S, fn(&<S as ParserCommon<A>>::Returning, &mut Option<R>, C) -> Option<()>>
{
    type State = (
        <S as ParserCommon<A>>::State,
        Option<<S as ParserCommon<A>>::Returning>,
        Option<C>,
    );
    type Returning = R;

    #[inline(never)]
    fn init(&self) -> Self::State {
        (<S as ParserCommon<A>>::init(&self.0), None, None)
    }

    #[inline(never)]
    fn init_in_place(&self, state: *mut core::mem::MaybeUninit<Self::State>) {
        self.0.init_in_place(unsafe {
            core::ptr::addr_of_mut!((*(*state).as_mut_ptr()).0)
                as *mut core::mem::MaybeUninit<<S as ParserCommon<A>>::State>
        });
        call_fn(|| unsafe {
            (core::ptr::addr_of_mut!((*(*state).as_mut_ptr()).1)
                as *mut Option<<S as ParserCommon<A>>::Returning>)
                .write(None)
        });
        call_fn(|| unsafe {
            (core::ptr::addr_of_mut!((*(*state).as_mut_ptr()).2) as *mut Option<C>).write(None)
        });
    }
}

impl<A, R, S: InterpParser<A>, C> InterpParser<A>
    for Action<S, fn(&<S as ParserCommon<A>>::Returning, &mut Option<R>, C) -> Option<()>>
{
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        let new_chunk = self.0.parse(&mut state.0, chunk, &mut state.1)?;
        match (self.1)(
            state.1.as_ref().ok_or_else(|| rej(new_chunk))?,
            destination,
            core::mem::take(&mut state.2).ok_or_else(|| rej(new_chunk))?,
        ) {
            None => Err(rej(new_chunk)),
            Some(()) => Ok(new_chunk),
        }
    }
}

impl<A, R, S: ParserCommon<A>, C> DynParser<A>
    for Action<S, fn(&<S as ParserCommon<A>>::Returning, &mut Option<R>, C) -> Option<()>>
{
    type Parameter = C;
    #[inline(never)]
    fn init_param(
        &self,
        param: Self::Parameter,
        state: &mut Self::State,
        _destination: &mut Option<Self::Returning>,
    ) {
        set_from_thunk(&mut state.0, || <S as ParserCommon<A>>::init(&self.0));
        set_from_thunk(&mut state.1, || None);
        set_from_thunk(&mut state.2, || Some(param));
    }
}

/* A MoveAction is the same as an Action with the distinction that it takes it's argument via Move,
 * thus enabling it to work with types that do not have Copy or Clone and have nontrivial semantics
 * involving Drop. */
pub struct MoveAction<S, F>(pub S, pub F);
impl<A, R, S: ParserCommon<A>> ParserCommon<A>
    for MoveAction<S, fn(<S as ParserCommon<A>>::Returning, &mut Option<R>) -> Option<()>>
{
    type State = (
        <S as ParserCommon<A>>::State,
        Option<<S as ParserCommon<A>>::Returning>,
    );
    type Returning = R;

    #[inline(never)]
    fn init(&self) -> Self::State {
        (<S as ParserCommon<A>>::init(&self.0), None)
    }

    #[inline(never)]
    fn init_in_place(&self, state: *mut core::mem::MaybeUninit<Self::State>) {
        self.0.init_in_place(unsafe {
            core::ptr::addr_of_mut!((*(*state).as_mut_ptr()).0)
                as *mut core::mem::MaybeUninit<<S as ParserCommon<A>>::State>
        });
        call_fn(|| unsafe {
            (core::ptr::addr_of_mut!((*(*state).as_mut_ptr()).1)
                as *mut Option<<S as ParserCommon<A>>::Returning>)
                .write(None)
        });
    }
}

impl<A, R, S: InterpParser<A>> InterpParser<A>
    for MoveAction<S, fn(<S as ParserCommon<A>>::Returning, &mut Option<R>) -> Option<()>>
{
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        let new_chunk = self.0.parse(&mut state.0, chunk, &mut state.1)?;
        match (self.1)(
            core::mem::take(&mut state.1).ok_or_else(|| rej(new_chunk))?,
            destination,
        ) {
            None => Err(rej(new_chunk)),
            Some(()) => Ok(new_chunk),
        }
    }
}

impl<A, R, S: DynParser<A>> DynParser<A>
    for MoveAction<S, fn(<S as ParserCommon<A>>::Returning, &mut Option<R>) -> Option<()>>
{
    type Parameter = S::Parameter;
    #[inline(never)]
    fn init_param(
        &self,
        param: Self::Parameter,
        state: &mut Self::State,
        _destination: &mut Option<Self::Returning>,
    ) {
        set_from_thunk(&mut state.0, || <S as ParserCommon<A>>::init(&self.0));
        set_from_thunk(&mut state.1, || None);
        self.0.init_param(param, &mut state.0, &mut state.1);
    }
}

fn rej(cnk: &[u8]) -> (PResult<OOB>, RemainingSlice<'_>) {
    (Some(OOB::Reject), cnk)
}

pub struct Preaction<S>(pub fn() -> Option<()>, pub S);

impl<A, S: ParserCommon<A>> ParserCommon<A> for Preaction<S> {
    type State = Option<<S as ParserCommon<A>>::State>;
    type Returning = <S as ParserCommon<A>>::Returning;

    fn init(&self) -> Self::State {
        None
    }
}

impl<A, S: InterpParser<A>> InterpParser<A> for Preaction<S> {
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        loop {
            break match state {
                None => {
                    (self.0)().ok_or_else(|| rej(chunk))?;
                    set_from_thunk(state, || Some(<S as ParserCommon<A>>::init(&self.1)));
                    continue;
                }
                Some(ref mut s) => <S as InterpParser<A>>::parse(&self.1, s, chunk, destination),
            };
        }
    }
}

#[derive(Clone)]
// S is the first subparser to run
// F is the continuation parser to run, which can depend on the result of S
pub struct Bind<S, F>(pub S, pub F);

// Initially the state is the state of the first subparser, and its result location
// After the first subparser runs, if it failed, then the whole bind parser will fail
// but if it succeeds, then the parser state transitions to BindSecond.
#[derive(InPlaceInit)]
pub enum BindState<A, B, S: ParserCommon<A>, T: ParserCommon<B>> {
    BindFirst(S::State, Option<<S as ParserCommon<A>>::Returning>),
    BindSecond(T, <T as ParserCommon<B>>::State),
}

impl<A, B, S: ParserCommon<A>, T: ParserCommon<B>> ParserCommon<(A, B)>
    for Bind<S, fn(&<S as ParserCommon<A>>::Returning) -> Option<T>>
{
    type State = BindState<A, B, S, T>;
    type Returning = <T as ParserCommon<B>>::Returning;
    #[inline(never)]
    fn init(&self) -> Self::State {
        use BindState::*;
        BindFirst(<S as ParserCommon<A>>::init(&self.0), None)
    }
    fn init_in_place(&self, state: *mut core::mem::MaybeUninit<Self::State>) {
        Self::State::init_bind_first(
            state,
            |a| <S as ParserCommon<A>>::init_in_place(&self.0, a),
            |b| {
                call_fn(|| unsafe {
                    (*b).as_mut_ptr().write(None);
                })
            },
        );
    }
}

impl<A, B, S: InterpParser<A>, T: InterpParser<B>> InterpParser<(A, B)>
    for Bind<S, fn(&<S as ParserCommon<A>>::Returning) -> Option<T>>
{
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        use BindState::*;
        let mut cursor = chunk;
        loop {
            match state {
                BindFirst(ref mut s, ref mut r) => {
                    cursor = self.0.parse(s, cursor, r)?;
                    let r_temp = core::mem::take(r);
                    call_me_maybe(|| {
                        let next = self.1(r_temp.as_ref()?)?;
                        let next_state = next.init();
                        *state = BindSecond(next, next_state);
                        Some(())
                    })
                    .ok_or_else(|| rej(cursor))?;
                }
                BindSecond(t, ref mut s) => {
                    cursor = t.parse(s, cursor, destination)?;
                    return Ok(cursor);
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct DynBind<S, F>(pub S, pub F);

#[derive(InPlaceInit)]
#[repr(u8)]
pub enum DynBindState<A, B, S: ParserCommon<A>, T: ParserCommon<B>> {
    BindFirst(S::State, Option<<S as ParserCommon<A>>::Returning>),
    BindSecond(<T as ParserCommon<B>>::State),
}

#[inline(never)]
fn call_fn(f: impl FnOnce()) {
    f()
}

impl<A, B, S: ParserCommon<A>, T: DynParser<B, Parameter = S::Returning>> ParserCommon<(A, B)>
    for DynBind<S, T>
// fn(&<S as InterpParser<A>>::Returning) -> Option<T>>
{
    type State = DynBindState<A, B, S, T>;
    type Returning = <T as ParserCommon<B>>::Returning;
    #[inline(never)]
    fn init(&self) -> Self::State {
        use DynBindState::*;
        BindFirst(<S as ParserCommon<A>>::init(&self.0), None)
    }
    #[inline(never)]
    fn init_in_place(&self, state: *mut core::mem::MaybeUninit<Self::State>) {
        Self::State::init_bind_first(
            state,
            |a| call_fn(|| <S as ParserCommon<A>>::init_in_place(&self.0, a)),
            |b| {
                call_fn(|| unsafe {
                    (*b).as_mut_ptr().write(None);
                })
            },
        );
    }
}

impl<A, B, S: InterpParser<A>, T: DynParser<B, Parameter = S::Returning> + InterpParser<B>>
    InterpParser<(A, B)> for DynBind<S, T>
{
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        use DynBindState::*;
        let mut cursor = chunk;
        loop {
            match state {
                BindFirst(ref mut s, ref mut r) => {
                    cursor = self.0.parse(s, cursor, r)?;
                    call_me_maybe(|| {
                        let r_temp = if let BindFirst(_, ref mut r) = state {
                            core::mem::take(r)?
                        } else {
                            unreachable!()
                        };
                        Self::State::init_bind_second(
                            unsafe { core::mem::transmute(state as *mut Self::State) },
                            |a| call_fn(|| self.1.init_in_place(a)),
                        );
                        if let BindSecond(ref mut s) = state {
                            self.1.init_param(r_temp, s, destination);
                        } else {
                            unreachable!();
                        }
                        Some(())
                    })
                    .ok_or_else(|| rej(cursor))?;
                }
                BindSecond(ref mut s) => {
                    cursor = self.1.parse(s, cursor, destination)?;
                    return Ok(cursor);
                }
            }
        }
    }
}

impl<A, B, S: DynParser<A>, T: DynParser<B, Parameter = S::Returning>> DynParser<(A, B)>
    for DynBind<S, T>
{
    type Parameter = S::Parameter;
    #[inline(never)]
    fn init_param(
        &self,
        param: Self::Parameter,
        state: &mut Self::State,
        _destination: &mut Option<Self::Returning>,
    ) {
        self.init_in_place(unsafe { core::mem::transmute(state as *mut Self::State) });
        match state {
            DynBindState::BindFirst(ref mut s, ref mut sub_destination) => {
                self.0.init_param(param, s, sub_destination)
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Clone)]
pub struct ObserveBytes<X, F, S>(pub fn() -> X, pub F, pub S);

impl<A, X: Clone, F: Fn(&mut X, &[u8]), S: ParserCommon<A>> ParserCommon<A>
    for ObserveBytes<X, F, S>
{
    type State = Option<<S as ParserCommon<A>>::State>;
    // Making a compromise here; if we return our sub-parser's result still wrapped in Option, we
    // can avoid storing it in our own state and then copying.
    type Returning = (X, Option<<S as ParserCommon<A>>::Returning>);
    #[inline(never)]
    fn init(&self) -> Self::State {
        None
    }
}

impl<A, X: Clone, F: Fn(&mut X, &[u8]), S: InterpParser<A>> InterpParser<A>
    for ObserveBytes<X, F, S>
{
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        loop {
            break match state {
                None => {
                    *destination = Some(((self.0)(), None));
                    set_from_thunk(state, || Some(<S as ParserCommon<A>>::init(&self.2)));
                    continue;
                }
                Some(ref mut subparser_state) => {
                    let new_chunk = <S as InterpParser<A>>::parse(
                        &self.2,
                        subparser_state,
                        chunk,
                        &mut destination.as_mut().ok_or_else(|| rej(chunk))?.1,
                    )?;
                    self.1(
                        &mut destination.as_mut().ok_or_else(|| rej(new_chunk))?.0,
                        &chunk[0..chunk.len() - new_chunk.len()],
                    );
                    Ok(new_chunk)
                }
            };
        }
    }
}

impl<A, X: Clone, F: Fn(&mut X, &[u8]), S: InterpParser<A>> DynParser<A> for ObserveBytes<X, F, S> {
    type Parameter = X;
    #[inline(never)]
    fn init_param(
        &self,
        param: Self::Parameter,
        state: &mut Self::State,
        destination: &mut Option<Self::Returning>,
    ) {
        *destination = Some((param, None));
        *state = Some(<S as ParserCommon<A>>::init(&self.2));
    }
}

pub enum PairState<A, B> {
    Init,
    First(A),
    Second(B),
}

impl<A: ParserCommon<C>, B: ParserCommon<D>, C, D> ParserCommon<(C, D)> for (A, B) {
    type State = PairState<<A as ParserCommon<C>>::State, <B as ParserCommon<D>>::State>;
    type Returning = (Option<A::Returning>, Option<B::Returning>);
    // #[inline(never)] // Causes stack size increase
    fn init(&self) -> Self::State {
        PairState::Init
    }
}

impl<A: InterpParser<C>, B: InterpParser<D>, C, D> InterpParser<(C, D)> for (A, B) {
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        let mut cursor = chunk;
        loop {
            match state {
                PairState::Init => {
                    init_with_default(destination);
                    set_from_thunk(state, || {
                        PairState::First(<A as ParserCommon<C>>::init(&self.0))
                    });
                }
                PairState::First(ref mut sub) => {
                    cursor = <A as InterpParser<C>>::parse(
                        &self.0,
                        sub,
                        cursor,
                        &mut destination.as_mut().ok_or_else(|| rej(cursor))?.0,
                    )?;
                    set_from_thunk(state, || {
                        PairState::Second(<B as ParserCommon<D>>::init(&self.1))
                    });
                }
                PairState::Second(ref mut sub) => {
                    cursor = <B as InterpParser<D>>::parse(
                        &self.1,
                        sub,
                        cursor,
                        &mut destination.as_mut().ok_or_else(|| rej(cursor))?.1,
                    )?;
                    break Ok(cursor);
                }
            }
        }
    }
}

/*
 // TODO: handle struct-like data structures without using the pair parser above and with named
 // fields.
 //
#[macro_export]
macro_rules! def_table {
    {struct $name:ident { $($fieldName:ident : $type:ty),+ } } =>
    {
        struct $name<$($fieldName),+> {
            $($fieldName: $fieldName),+
        }
    }

    enum
    impl<$($fieldName : InterpParser<$type>),+> InterpParser<$name<$($fieldName),+>> for $name<$($fieldName),+> {

    }
}
*/

#[derive(InPlaceInit)]
pub enum LengthFallbackParserState<N, NO, IS> {
    Length(N, NO),
    Element(usize, usize, IS),
    Failed(usize, usize),
    Done,
}

// First step, sketch out the states of your parser, with your transitions in mind
pub struct LengthLimitedState<State> {
    bytes_seen: usize,
    child_state: State,
}

// Now define the parser type, which will resemble the mirror image of the state
#[derive(Clone)]
pub struct LengthLimited<S> {
    bytes_limit: usize,
    subparser: S,
}

// Implement InterpParser for the parser
impl<I, S: ParserCommon<I>> ParserCommon<I> for LengthLimited<S> {
    type State = LengthLimitedState<<S as ParserCommon<I>>::State>;
    type Returning = <S as ParserCommon<I>>::Returning;
    // Init is usually fairly straightforward
    fn init(&self) -> Self::State {
        LengthLimitedState {
            bytes_seen: 0,
            child_state: self.subparser.init(),
        }
    }
}

impl<I, S: InterpParser<I>> InterpParser<I> for LengthLimited<S> {
    // Start by typing out the type signature, copying the input slice into a mutable reference
    // and successfully return the cursor. Elaborate on the parser from there.
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        let feed_amount = core::cmp::min(chunk.len(), self.bytes_limit - state.bytes_seen);
        // If you're calling a subparser, you will probably want to match on its status
        // Note that we are trying to keep _our_ state in lockstep with the state of our child.
        // If the child consumes, we account for it, even if we end up in a bad state.
        match self
            .subparser
            .parse(&mut state.child_state, &chunk[0..feed_amount], destination)
        {
            Ok(new_cursor) => {
                let consumed = feed_amount - new_cursor.len();
                state.bytes_seen += consumed;
                // If our child has accepted, they better have eaten all their vegetables.
                if consumed < feed_amount || state.bytes_seen < self.bytes_limit {
                    return Err(rej(new_cursor));
                }
                Ok(&chunk[feed_amount..chunk.len()])
            }
            Err((None, new_cursor)) => {
                let consumed = feed_amount - new_cursor.len();
                state.bytes_seen += consumed;
                // How can you have any pudding if you don't eat your meat?
                if consumed < feed_amount || state.bytes_seen >= self.bytes_limit {
                    return Err(rej(new_cursor));
                }
                Err((None, new_cursor))
            }
            Err((w, new_cursor)) => {
                let consumed = feed_amount - new_cursor.len();
                state.bytes_seen += consumed;
                Err((w, new_cursor))
            }
        }
    }
}

// I is a closure to initialize the observer of the input, namely X, which is usually a hasher
// F is a method which does the observing for the observer.
// S is the parser for the input of the hasher from the raw input
// Note that ObserveLengthedBytes also consumes a length prefix from the raw input
// Confer: LengthFallback
#[derive(Clone)]
pub struct ObserveLengthedBytes<I: Fn() -> X, X, F, S>(pub I, pub F, pub S, pub bool);

impl<IFun: Fn() -> X, N, I, S: ParserCommon<I>, X, F: Fn(&mut X, &[u8])>
    ParserCommon<LengthFallback<N, I>> for ObserveLengthedBytes<IFun, X, F, S>
where
    DefaultInterp: ParserCommon<N>,
    usize: TryFrom<<DefaultInterp as ParserCommon<N>>::Returning>,
    <DefaultInterp as ParserCommon<N>>::Returning: Copy,
{
    type State = LengthFallbackParserState<
        <DefaultInterp as ParserCommon<N>>::State,
        Option<<DefaultInterp as ParserCommon<N>>::Returning>,
        <S as ParserCommon<I>>::State,
    >;
    type Returning = (Option<<S as ParserCommon<I>>::Returning>, X);
    // #[inline(never)] // Causes stack size increase
    fn init(&self) -> Self::State {
        LengthFallbackParserState::Length(
            <DefaultInterp as ParserCommon<N>>::init(&DefaultInterp),
            None,
        )
    }
    #[inline(never)]
    fn init_in_place(&self, state: *mut core::mem::MaybeUninit<Self::State>) {
        Self::State::init_length(
            state,
            |a| <DefaultInterp as ParserCommon<N>>::init_in_place(&DefaultInterp, a),
            |b| {
                call_fn(|| unsafe {
                    (*b).as_mut_ptr().write(None);
                })
            },
        );
    }
}

impl<IFun: Fn() -> X, N, I, S: InterpParser<I>, X, F: Fn(&mut X, &[u8])>
    InterpParser<LengthFallback<N, I>> for ObserveLengthedBytes<IFun, X, F, S>
where
    DefaultInterp: InterpParser<N>,
    usize: TryFrom<<DefaultInterp as ParserCommon<N>>::Returning>,
    <DefaultInterp as ParserCommon<N>>::Returning: Copy,
{
    #[inline(never)]
    fn parse<'a, 'b>(
        &self,
        state: &'b mut Self::State,
        chunk: &'a [u8],
        destination: &mut Option<Self::Returning>,
    ) -> ParseResult<'a> {
        use LengthFallbackParserState::*;
        let mut cursor: &'a [u8] = chunk;
        loop {
            break match state {
                Length(ref mut nstate, ref mut length_out) => {
                    cursor = <DefaultInterp as InterpParser<N>>::parse(
                        &DefaultInterp,
                        nstate,
                        cursor,
                        length_out,
                    )?;
                    let len = <usize as TryFrom<<DefaultInterp as ParserCommon<N>>::Returning>>::try_from(length_out.ok_or_else(|| rej(cursor))?)
                        .map_err(|_| rej(cursor))?;
                    match destination {
                        None => {
                            call_me_maybe(|| {
                                let result = self.0();
                                *destination = Some((None, result));
                                Some(())
                            })
                            .ok_or_else(|| rej(cursor))?;
                        }
                        Some(_) => {}
                    }
                    set_from_thunk(state, || {
                        Element(0, len, <S as ParserCommon<I>>::init(&self.2))
                    });
                    continue;
                }
                Element(ref mut consumed, len, ref mut istate) => {
                    let passed_cursor =
                        &cursor[0..core::cmp::min(cursor.len(), (*len) - (*consumed))];
                    match self.2.parse(
                        istate,
                        passed_cursor,
                        &mut destination.as_mut().ok_or_else(|| rej(cursor))?.0,
                    ) {
                        Ok(new_cursor) => {
                            let consumed_from_chunk = passed_cursor.len() - new_cursor.len();
                            *consumed += consumed_from_chunk;
                            self.1(
                                &mut destination.as_mut().ok_or_else(|| rej(cursor))?.1,
                                &cursor[0..passed_cursor.len() - new_cursor.len()],
                            );
                            if *consumed == *len {
                                Ok(&cursor[consumed_from_chunk..])
                            } else {
                                cursor = new_cursor;
                                destination.as_mut().ok_or_else(|| rej(cursor))?.0 = None;
                                let cv = *consumed;
                                let lv = *len;
                                set_from_thunk(state, || Failed(cv, lv));
                                continue;
                            }
                        }
                        Err((None, new_cursor)) => {
                            let consumed_from_chunk = passed_cursor.len() - new_cursor.len();
                            *consumed += consumed_from_chunk;
                            self.1(
                                &mut destination.as_mut().ok_or_else(|| rej(cursor))?.1,
                                &cursor[0..passed_cursor.len() - new_cursor.len()],
                            );
                            if *consumed == *len {
                                Ok(&cursor[consumed_from_chunk..])
                            } else {
                                Err((None, &cursor[consumed_from_chunk..]))
                            }
                        }
                        Err((Some(OOB::Reject), _)) => {
                            let cv = *consumed;
                            let lv = *len;
                            set_from_thunk(state, || Failed(cv, lv));
                            continue;
                        }
                    }
                }
                Failed(ref mut consumed, len) => {
                    if self.3 {
                        return Err(rej(cursor));
                    } else {
                        use core::cmp::min;
                        let new_cursor = &cursor[min((*len) - (*consumed), cursor.len())..];
                        self.1(
                            &mut destination.as_mut().ok_or_else(|| rej(cursor))?.1,
                            &cursor[0..cursor.len() - new_cursor.len()],
                        );
                        if cursor.len() >= ((*len) - (*consumed)) {
                            set_from_thunk(state, || Done);
                            set_from_thunk(
                                &mut destination.as_mut().ok_or_else(|| rej(cursor))?.0,
                                || None,
                            );
                            Ok(new_cursor)
                        } else {
                            let new_consumed = *consumed + cursor.len();
                            let new_len = *len;
                            set_from_thunk(state, || Failed(new_consumed, new_len));
                            Err((None, new_cursor))
                        }
                    }
                }
                Done => Err(rej(cursor)),
            };
        }
    }
}

impl<IFun: Fn() -> X, N, I, S: InterpParser<I>, X, F: Fn(&mut X, &[u8])>
    DynParser<LengthFallback<N, I>> for ObserveLengthedBytes<IFun, X, F, S>
where
    DefaultInterp: InterpParser<N>,
    usize: TryFrom<<DefaultInterp as ParserCommon<N>>::Returning>,
    <DefaultInterp as ParserCommon<N>>::Returning: Copy,
{
    type Parameter = X;
    #[inline(never)]
    fn init_param(
        &self,
        param: Self::Parameter,
        state: &mut Self::State,
        destination: &mut Option<Self::Returning>,
    ) {
        set_from_thunk(destination, || Some((None, param)));
        *state = LengthFallbackParserState::Length(
            <DefaultInterp as ParserCommon<N>>::init(&DefaultInterp),
            None,
        )
    }
}

/*
#[cfg(test)]
mod test {

#[cfg(all(target_family="bolos", test))]
    use testmacro::test_item as test;
#[cfg(all(target_family="bolos", test))]
#[allow(unused_imports)]
    use ledger_device_sdk::{TestType, debug_print}; // , Pic};
#[cfg(all(not(target_family="bolos"), test))]
    fn debug_print(_s: &str) {
    }

    struct DBG;
    use core;
    #[allow(unused_imports)]
    use core::fmt::Write;
    impl core::fmt::Write for DBG {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            use arrayvec::ArrayString;
            let mut qq = ArrayString::<128>::new();
            qq.push_str(s);
            debug_print(qq.as_str());
            Ok(())
        }
    }

    use core::fmt::Debug;
    use super::{InterpParser, DefaultInterp, SubInterp, Action, ObserveBytes, OOB};
    #[allow(unused_imports)]
    use crate::core_parsers::{Byte, Array, DArray, U16, U32 };
    #[allow(unused_imports)]
    use arrayvec::ArrayVec;

    fn parser_test_feed<P, T: InterpParser<P>, RT: Debug + ?Sized>(parser: T, chunks: &[&[u8]], result: &RT, oobs: &[OOB]) where T::Returning: PartialEq<RT> + Debug
    {
        let mut oob_iter = oobs.iter();
        let mut chunk_iter = chunks.iter();
        let mut cursor : &[u8] = chunk_iter.next().unwrap();
        let mut parser_state = T::init(&parser);
        let mut destination : Option<RT>;
        loop {
            match <T as InterpParser<P>>::parse(&parser, &mut parser_state, cursor, &mut destination) {
                Err((Some(o), _new_cursor)) => {
                    assert_eq!(Some(&o), oob_iter.next());
                    match o {
                        OOB::Reject => {
                            assert_eq!(oob_iter.next(), None);
                            assert_eq!(chunk_iter.next(), None);
                            break;
                        }
                        // If there are any non-Reject OOB options uncomment this.
                        // _ => {
                        //    cursor = new_cursor; // Not sure why rustc claims this is unused.
                        //}
                    }
                }
                Err((None, new_cursor)) => {
                    assert_eq!(new_cursor, &[][..]);
                    match chunk_iter.next() {
                        Some(new) => {
                            cursor = new;
                        }
                        None => {
                            panic!("Ran out of input chunks before parser accepted");
                        }
                    }
                }
                Ok(new_cursor) => {
                    assert_eq!(destination.as_ref().unwrap(), result);
                    assert_eq!(new_cursor, &[][..]);
                    assert_eq!(chunk_iter.next(), None);
                    assert_eq!(oob_iter.next(), None);
                    break;
                }
            }
        }
    }
/*
#[test]
fn byte_parser() {
    let mut state = <DefaultInterp as InterpParser<Byte>>::init(&DefaultInterp);
    assert_eq!(<DefaultInterp as InterpParser<Byte>>::parse(&DefaultInterp, &mut state, b"cheez"), Ok((b'c', &b"heez"[..])));
    assert_eq!(<DefaultInterp as InterpParser<Byte>>::parse(&DefaultInterp, &mut state, b""), Err((None, &b""[..])));

}
*/

    fn init_parser<A, P: InterpParser<A>>(p: &P) -> <P as InterpParser<A>>::State {
        <P as InterpParser<A>>::init(p)
    }
    fn run_parser<'a, 'b, A, P: InterpParser<A>>(p: &P, state: &'b mut <P as InterpParser<A>>::State, chunk: &'a [u8]) -> Result<(<P as InterpParser<A>>::Returning, super::RemainingSlice<'a>), (super::PResult<OOB>, super::RemainingSlice<'a>)> {
        let mut destination : Option<<P as InterpParser<A>>::Returning> = None;
        <P as InterpParser<A>>::parse(p, state, chunk).map(|_| destination.unwrap());
    }

/*
#[test]
fn interp_byte_parser() {
    let p = super::Action(DefaultInterp, |x: &u8, d: &mut Option<u8>| { *d = Some(*x); Some(()) });
    let mut state = init_parser::<Byte,_>(&p);
    assert_eq!(run_parser::<Byte,_>(&p, &mut state, b"cheez"), Ok((b'c', &b"heez"[..])));
}
*/

/*
#[test]
fn test_length_fallback() {
    type Format = super::LengthFallback<Byte, Array<Byte, 5>>;
    parser_test_feed::<Format, _, _>(super::ObserveLengthedBytes(|| { ArrayVec::<u8, 5>::new() }, |a: &mut ArrayVec<u8, 5> , b: &[u8]| { let _ = a.try_extend_from_slice(b); }, DefaultInterp), &[b"\x05fooba"], &(Ok(*(b"fooba")), (*(b"fooba")).into()), &[]);
    use crate::endianness::{Endianness};
    type Format2 = super::LengthFallback<U32< { Endianness::Little } >, Array<Byte, 5>>;
    parser_test_feed::<Format2, _, _>(super::ObserveLengthedBytes(|| { ArrayVec::<u8, 5>::new() }, |a: &mut ArrayVec<u8, 5> , b: &[u8]| { let _ = a.try_extend_from_slice(b); }, DefaultInterp), &[b"\x05\x00\x00\x00fooba"], &(Ok(*(b"fooba")), (*(b"fooba")).into()), &[]);
    parser_test_feed::<Format2, _, _>(super::ObserveLengthedBytes(|| { ArrayVec::<u8, 6>::new() }, |a: &mut ArrayVec<u8, 6> , b: &[u8]| { let _ = a.try_extend_from_slice(b); }, DefaultInterp), &[b"\x06\x00\x00\x00foobar"], &(Err(()), (*(b"foobar")).into()), &[]);
    parser_test_feed::<Format2, _, _>(super::ObserveLengthedBytes(|| { ArrayVec::<u8, 7>::new() }, |a: &mut ArrayVec<u8, 7> , b: &[u8]| { let _ = a.try_extend_from_slice(b); }, DefaultInterp), &[b"\x07\x00\x00\x00foobarb"], &(Err(()), (*(b"foobarb")).into()), &[]);
    parser_test_feed::<Format2, _, _>(super::ObserveLengthedBytes(|| { ArrayVec::<u8, 4>::new() }, |a: &mut ArrayVec<u8, 4> , b: &[u8]| { let _ = a.try_extend_from_slice(b); }, DefaultInterp), &[b"\x04\x00\x00\x00foob"], &(Err(()), (*(b"foob")).into()), &[]);
}

#[test]
fn test_array() {
    let p = super::DefaultInterp;
    parser_test_feed::<Array<Byte,5>, super::DefaultInterp, _>(p, &[b"foo",b"ba"], &b"fooba"[..], &[])
}

#[test]
    fn test_darray() {
        parser_test_feed::<DArray<Byte,Byte,5>, _, _>(SubInterp(DefaultInterp), &[b"\0"], &b""[..], &[]);
        parser_test_feed::<DArray<Byte,Byte,5>, _, _>(SubInterp(DefaultInterp), &[b"\x05abcde"], &b"abcde"[..], &[]);
        parser_test_feed::<DArray<Byte,Byte,5>, _, _>(SubInterp(Action(DefaultInterp, |_: &u8| Some(()))), &[b"\x05abcde"], &[(),(),(),(),()][..], &[]);
        let obs = ObserveBytes(
            || 0, |a : &mut usize,b : &[u8]| { *a += b.len(); },
            SubInterp(
                Action(DefaultInterp, |_: &u8| Some(()))));
        parser_test_feed::<DArray<Byte,Byte,5>, _, _>(obs, &[b"\x05abcde"], &(6, (ArrayVec::from([(),(),(),(),()]))), &[]);
    }
*/

}
*/
