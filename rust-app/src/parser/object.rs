use crate::crypto_helpers::common::HexSlice;
use crate::crypto_helpers::hasher::HexHash;
use crate::parser::common::*;
use arrayvec::ArrayVec;
use core::convert::TryInto;
use core::future::Future;
use ledger_device_sdk::hash::HashInit;
use ledger_device_sdk::io::SyscallError;
use ledger_device_sdk::log::info;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::bcs::async_parser::*;
use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::interp::*;

// Object Schema
pub type ObjectInnerSchema = (
    ObjectDataSchema,
    OwnerSchema,
    TransactionDigest,
    StorageRebate,
);

pub type MoveObject = (MoveObjectType, bool, SequenceNumber, ObjectContents);

// The object content parsing is limited to either a simple Coin (40 bytes) or StakedSui (80 bytes)
// We will simply reject parsing objects with content size greater than OBJECT_CONTENTS_LEN
pub const OBJECT_CONTENTS_LEN: usize = 80;
pub type ObjectContents = Vec<Byte, OBJECT_CONTENTS_LEN>;
pub type Coin = (UID, Amount);

pub struct ObjectDataSchema;
pub struct OwnerSchema;

pub type TransactionDigest = Sha3_256Hash;
pub type UID = ObjectID;

pub type StorageRebate = U64LE;

pub const STRING_LENGTH: usize = 64;
pub type String = Vec<Byte, STRING_LENGTH>;

pub type StructTag = (SuiAddress, String, String, Vec<SkipTypeTag, 5>);

pub struct TypeTag;

/// Schema for skipping a type param. Iterative loop, no recursion, no heap.
pub struct SkipTypeTag;

const MAX_TYPE_PARAM_DEPTH: u8 = 2;
/// Max nesting for type params (e.g. `Vec<Struct<Vec<T>>>`). Exceeding rejects.
const SKIP_TYPE_TAG_STACK_CAP: usize = 16;

// Parsed data
pub enum MoveObjectType {
    /// A type that is not `0x2::coin::Coin<T>`
    // Other(StructTag),
    /// A SUI coin (i.e., `0x2::coin::Coin<0x2::sui::SUI>`)
    GasCoin,
    /// A record of a staked SUI coin (i.e., `0x3::staking_pool::StakedSui`)
    StakedSui,
    /// A non-SUI coin type (i.e., `0x2::coin::Coin<T> where T != 0x2::sui::SUI`)
    Coin(CoinType),
}

// Parsers

pub const fn object_parser<BS: Clone + Readable>(
) -> impl AsyncParser<ObjectInnerSchema, BS, Output = ObjectData> {
    Action(
        (DefaultInterp, DefaultInterp, DefaultInterp, DefaultInterp),
        |(d, _, _, _storage_rebate)| {
            info!("Object: StorageRebate {}", _storage_rebate);
            Some(d)
        },
    )
}

impl HasOutput<ObjectDataSchema> for DefaultInterp {
    type Output = ObjectData;
}

impl<BS: Clone + Readable> AsyncParser<ObjectDataSchema, BS> for DefaultInterp {
    type State<'c>
        = impl Future<Output = Self::Output> + 'c
    where
        BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let enum_variant =
                <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
            match enum_variant {
                0 => {
                    info!("ObjectDataSchema: Move(MoveObject)");
                    move_object_parser().parse(input).await
                }
                1 => {
                    info!("ObjectDataSchema: Package(MovePackage)");
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
                _ => {
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
            }
        }
    }
}

pub const fn move_object_parser<BS: Clone + Readable>(
) -> impl AsyncParser<MoveObject, BS, Output = ObjectData> {
    Action(
        (
            DefaultInterp,
            DefaultInterp,
            DefaultInterp,
            SubInterp(DefaultInterp),
        ),
        |(object_type, _, _sequence_number, d): (_, _, _, ArrayVec<u8, OBJECT_CONTENTS_LEN>)| {
            info!("SequenceNumber {}", _sequence_number);

            // A coin object is always of size 40, and StakedSui is 80.
            // The last 8 bytes contain the balance/principal amount in both layouts.
            // The stake-vs-coin distinction MUST be preserved in the returned
            // ObjectData so downstream transfer validation cannot mistake a
            // StakedSui position for liquid SUI.
            match object_type {
                MoveObjectType::GasCoin => coin_amount(&d).map(|amount| ObjectData::Coin {
                    coin_type: SUI_COIN_TYPE,
                    amount,
                }),
                MoveObjectType::Coin(coin_type) => {
                    coin_amount(&d).map(|amount| ObjectData::Coin { coin_type, amount })
                }
                MoveObjectType::StakedSui => {
                    staked_sui_amount(&d).map(|amount| ObjectData::StakedSui { amount })
                }
            }
        },
    )
}

// A Coin<T> object is 40 bytes: 32-byte UID followed by an 8-byte LE balance.
fn coin_amount(d: &ArrayVec<u8, OBJECT_CONTENTS_LEN>) -> Option<u64> {
    match d.len() {
        40 => Some(u64::from_le_bytes(
            d.as_slice()[32..]
                .try_into()
                .expect("amount slice wrong length"),
        )),
        _ => {
            info!("ObjectContents incorrect (expected Coin of size 40)");
            None
        }
    }
}

// A StakedSui object is 80 bytes with the 8-byte LE principal in the last bytes.
fn staked_sui_amount(d: &ArrayVec<u8, OBJECT_CONTENTS_LEN>) -> Option<u64> {
    match d.len() {
        80 => Some(u64::from_le_bytes(
            d.as_slice()[72..]
                .try_into()
                .expect("amount slice wrong length"),
        )),
        _ => {
            info!("ObjectContents incorrect (expected StakedSui of size 80)");
            None
        }
    }
}

impl HasOutput<MoveObjectType> for DefaultInterp {
    type Output = MoveObjectType;
}

impl<BS: Clone + Readable> AsyncParser<MoveObjectType, BS> for DefaultInterp {
    type State<'c>
        = impl Future<Output = Self::Output> + 'c
    where
        BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let enum_variant =
                <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
            match enum_variant {
                0 => {
                    info!("MoveObjectType: Other(StructTag)");
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
                1 => {
                    info!("MoveObjectType: GasCoin");
                    MoveObjectType::GasCoin
                }
                2 => {
                    info!("MoveObjectType: StakedSui");
                    MoveObjectType::StakedSui
                }
                3 => {
                    info!("MoveObjectType: Coin(TypeTag)");
                    if let Some(ct) =
                        <DefaultInterp as AsyncParser<TypeTag, BS>>::parse(&DefaultInterp, input)
                            .await
                    {
                        MoveObjectType::Coin(ct)
                    } else {
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                }
                _ => {
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
            }
        }
    }
}

pub const fn struct_tag_parser<BS: Clone + Readable>(
) -> impl AsyncParser<StructTag, BS, Output = CoinType> {
    Action(
        (
            DefaultInterp,
            SubInterp(DefaultInterp),
            SubInterp(DefaultInterp),
            SubInterp(DefaultInterp),
        ),
        |(address, mut module, mut name, _type_tags): (
            [u8; 32],
            ArrayVec<u8, STRING_LENGTH>,
            ArrayVec<u8, STRING_LENGTH>,
            ArrayVec<(), 5>,
        )| {
            info!("StructTag Address 0x{}", HexSlice(&address));
            info!(
                "StructTag Module {}",
                core::str::from_utf8(module.as_slice()).unwrap_or("invalid utf-8")
            );
            info!(
                "StructTag Name {}",
                core::str::from_utf8(name.as_slice()).unwrap_or("invalid utf-8")
            );
            info!("StructTag TypeTag len {}", _type_tags.len());
            let mod_short: ArrayVec<u8, COIN_STRING_LENGTH> = module
                .drain(..module.len().min(COIN_STRING_LENGTH))
                .collect();
            let name_short: ArrayVec<u8, COIN_STRING_LENGTH> =
                name.drain(..name.len().min(COIN_STRING_LENGTH)).collect();
            // Action requires `Fn(...) -> Option<R>` (see async_parser `Action` impl).
            let out: CoinType = (
                address,
                pad_coin_name_bytes(mod_short.as_slice()),
                pad_coin_name_bytes(name_short.as_slice()),
            );
            Some(out)
        },
    )
}

impl HasOutput<TypeTag> for DefaultInterp {
    type Output = Option<CoinType>;
}

impl<BS: Clone + Readable> AsyncParser<TypeTag, BS> for DefaultInterp {
    type State<'c>
        = impl Future<Output = Self::Output> + 'c
    where
        BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let enum_variant =
                <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
            match enum_variant {
                0 => {
                    info!("TypeTag: Bool");
                    None
                }
                1 => {
                    info!("TypeTag: U8");
                    None
                }
                2 => {
                    info!("TypeTag: U64");
                    None
                }
                3 => {
                    info!("TypeTag: U128");
                    None
                }
                4 => {
                    info!("TypeTag: Address");
                    None
                }
                5 => {
                    info!("TypeTag: Signer");
                    None
                }
                6 => {
                    info!("TypeTag: Vector(Box<TypeTag>)");
                    None
                }
                7 => {
                    info!("TypeTag: Struct(StructTag)");
                    Some(struct_tag_parser().parse(input).await)
                }
                _ => {
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
            }
        }
    }
}

impl HasOutput<SkipTypeTag> for DefaultInterp {
    type Output = ();
}

impl<BS: Clone + Readable> AsyncParser<SkipTypeTag, BS> for DefaultInterp {
    type State<'c>
        = impl Future<Output = Self::Output> + 'c
    where
        BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let mut stack: ArrayVec<(u32, u8), SKIP_TYPE_TAG_STACK_CAP> = ArrayVec::new();
            stack.push((1, MAX_TYPE_PARAM_DEPTH));

            while let Some((count, depth)) = stack.pop() {
                if count > 1 && stack.try_push((count - 1, depth)).is_err() {
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }

                let variant: u32 =
                    <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
                match variant {
                    0..=5 | 8..=10 => {}
                    6 => {
                        if stack.try_push((1, depth)).is_err() {
                            reject_on(
                                core::file!(),
                                core::line!(),
                                SyscallError::NotSupported as u16,
                            )
                            .await
                        }
                    }
                    7 => {
                        if depth == 0 {
                            reject_on(
                                core::file!(),
                                core::line!(),
                                SyscallError::NotSupported as u16,
                            )
                            .await
                        }
                        let _addr: [u8; 32] = input.read().await;
                        // BCS length-prefixed strings (same as `Vec<Byte, STRING_LENGTH>` in StructTag).
                        <DropInterp as AsyncParser<Vec<Byte, STRING_LENGTH>, BS>>::parse(
                            &DropInterp,
                            input,
                        )
                        .await;
                        <DropInterp as AsyncParser<Vec<Byte, STRING_LENGTH>, BS>>::parse(
                            &DropInterp,
                            input,
                        )
                        .await;
                        let type_param_count: u32 =
                            <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(
                                &DefaultInterp,
                                input,
                            )
                            .await;
                        if type_param_count > 0
                            && stack.try_push((type_param_count, depth - 1)).is_err()
                        {
                            reject_on(
                                core::file!(),
                                core::line!(),
                                SyscallError::NotSupported as u16,
                            )
                            .await
                        }
                    }
                    _ => {
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                }
            }
        }
    }
}

impl HasOutput<OwnerSchema> for DefaultInterp {
    type Output = ();
}

impl<BS: Clone + Readable> AsyncParser<OwnerSchema, BS> for DefaultInterp {
    type State<'c>
        = impl Future<Output = Self::Output> + 'c
    where
        BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let enum_variant =
                <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
            match enum_variant {
                0 => {
                    info!("OwnerSchema: AddressOwner(SuiAddress)");
                    let _owner = <DefaultInterp as AsyncParser<SuiAddress, BS>>::parse(
                        &DefaultInterp,
                        input,
                    )
                    .await;
                    info!("OwnerSchema: AddressOwner({})", HexSlice(&_owner));
                }
                1 => {
                    info!("OwnerSchema: ObjectOwner(SuiAddress)");
                    <DefaultInterp as AsyncParser<SuiAddress, BS>>::parse(&DefaultInterp, input)
                        .await;
                }
                2 => {
                    info!("OwnerSchema: Shared");
                    <DefaultInterp as AsyncParser<SequenceNumber, BS>>::parse(
                        &DefaultInterp,
                        input,
                    )
                    .await;
                }
                3 => {
                    info!("OwnerSchema: Immutable");
                }
                _ => {
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
            }
        }
    }
}

pub async fn compute_object_hash<BS: Clone + Readable>(bs: &mut BS, length: usize) -> HexHash<32> {
    let mut hasher = ledger_device_sdk::hash::blake2::Blake2b_256::new();
    let salt = b"Object::";
    let _ = hasher.update(salt);

    const CHUNK_SIZE: usize = 128;
    let (chunks, rem) = (length / CHUNK_SIZE, length % CHUNK_SIZE);
    for _ in 0..chunks {
        let b: [u8; CHUNK_SIZE] = bs.read().await;
        let _ = hasher.update(&b);
    }
    for _ in 0..rem {
        let b: [u8; 1] = bs.read().await;
        let _ = hasher.update(&b);
    }

    let mut hash: HexHash<32> = Default::default();
    let _ = hasher.finalize(&mut hash.0);
    hash
}
