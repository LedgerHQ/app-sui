//! Generic "interpretations" for schema types.

/// Parse the schema using the "natural" interpretation.
///
/// usually this translates the input to an equivalent rust type or struct, for instance
/// `DefaultIntepr` on a `U32<{Endianness::Big}>` schema will produce a `u32` in the ledger's native
/// little-endian byte order, and an `Array<U16<{Endianness::Little}>,5>` would produce a `[u16;5]`
/// result; `DArray` becomes `ArrayVec`, etc.

pub struct DefaultInterp;

/// For array-like schemas; given a subparser for the item, produce a parser for the whole array.
pub struct SubInterp<S>(pub S);

/// Structurally checks and skips the schema in the input, but consumes only the minimum memory
/// required to do so and returns nothing.
pub struct DropInterp;

/// Action is essentailly an fmap that can fail.
///
/// We _could_ constraint F to actually be an fn(..) -> Option<()> to improve error messages when
/// functions do not have the correct shape, but that reduces our ability to write different
/// instances later.
#[derive(Clone)]
pub struct Action<S, F>(pub S, pub F);

/// A MoveAction is the same as an Action with the distinction that it takes it's argument via Move,
/// thus enabling it to work with types that do not have Copy or Clone and have nontrivial semantics
/// involving Drop.
pub struct MoveAction<S, F>(pub S, pub F);

#[derive(Clone)]
/// Monadic bind.
///
/// S is the first subparser to run
/// F is a function that returns the continuation parser to run, which can depend on the result of S
pub struct Bind<S, F>(pub S, pub F);

/// Bind, with reduced-copying parameter passing.
///
/// This is the main consumer for DynInterpParser; rather than constructing a fully new parser F,
/// this uses a static second parser that accepts a parameter via DynInterpParser to achieve a
/// similar result. This is possibly sufficiently less flexible that calling it monadic is
/// incorrect.
#[derive(Clone)]
pub struct DynBind<S, F>(pub S, pub F);

/// ObserveBytes runs a subparser and in parallel feeds the bytes consumed by the subparser into an
/// "observer", typically a hashing algorithm.
///
/// The first parameter is a function to build the observer, the second is the function to update
/// the observer, and S is the subparser.
#[derive(Clone)]
pub struct ObserveBytes<X, F, S>(pub fn() -> X, pub F, pub S);

/// Essentially SubInterp but for LengthFallback.
#[derive(Clone)]
pub struct LengthLimited<S> {
    pub bytes_limit: usize,
    pub subparser: S,
}

/// ObserveLengthedBytes is notionally the composition of a LengthFallback parser with
/// ObserveBytes.
///
/// This has the caveat that the length portion of the LengthFallback is _not_ fed to the
/// hashing function, which complicates separated implementation.
///
/// * I is a closure to initialize the observer of the input, namely X, which is usually a hasher
/// * F is a method which does the observing for the observer.
/// * S is the parser for the input of the hasher from the raw input
/// * the bool specifies whether to hard reject if the subparser fails.
///
/// Note that ObserveLengthedBytes also consumes a length prefix from the raw input
/// Confer: LengthFallback
#[derive(Clone)]
pub struct ObserveLengthedBytes<I: Fn() -> X, X, F, S>(pub I, pub F, pub S, pub bool);

pub struct Buffer<const N: usize>;
