use crate::parser::common::*;
use arrayvec::ArrayVec;
use core::convert::TryInto;
use core::future::Future;
#[cfg(feature = "speculos")]
use ledger_crypto_helpers::common::HexSlice;
use ledger_crypto_helpers::hasher::{Blake2b, Hasher, HexHash};
use ledger_device_sdk::io::SyscallError;
use ledger_log::info;
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

// Limited to parsing Coin
pub const OBJECT_CONTENTS_LEN: usize = 80;
pub type ObjectContents = Vec<Byte, OBJECT_CONTENTS_LEN>;
pub type Coin = (UID, Amount);

pub struct ObjectDataSchema;
pub struct OwnerSchema;

pub type TransactionDigest = SHA3_256_HASH;
pub type UID = ObjectID;

pub type StorageRebate = U64LE;

pub const STRING_LENGTH: usize = 64;
pub type String = Vec<Byte, STRING_LENGTH>;

pub type StructTag = (SuiAddress, String, String, TypeParams);
pub type TypeParams = Vec<TypeTag2, 5>;

pub struct TypeTag;

// This is to avoid recursion of async parsers
pub struct TypeTag2;

// Parsed data
pub enum MoveObjectType {
    /// A type that is not `0x2::coin::Coin<T>`
    // Other(StructTag),
    /// A SUI coin (i.e., `0x2::coin::Coin<0x2::sui::SUI>`)
    GasCoin,
    /// A record of a staked SUI coin (i.e., `0x3::staking_pool::StakedSui`)
    StakedSui,
    /// A non-SUI coin type (i.e., `0x2::coin::Coin<T> where T != 0x2::sui::SUI`)
    Coin((CoinID, CoinModuleName, CoinFunctionName)),
}

// Parsers

pub const fn object_parser<BS: Clone + Readable>(
) -> impl AsyncParser<ObjectInnerSchema, BS, Output = CoinData> {
    Action(
        (DefaultInterp, DefaultInterp, DefaultInterp, DefaultInterp),
        |(d, _, _, _storage_rebate)| {
            info!("Object: StorageRebate {}", _storage_rebate);
            Some(d)
        },
    )
}

impl HasOutput<ObjectDataSchema> for DefaultInterp {
    type Output = CoinData;
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
) -> impl AsyncParser<MoveObject, BS, Output = CoinData> {
    Action(
        (
            DefaultInterp,
            DefaultInterp,
            DefaultInterp,
            SubInterp(DefaultInterp),
        ),
        |(object_type, _, _sequence_number, d): (_, _, _, ArrayVec<u8, OBJECT_CONTENTS_LEN>)| {
            info!("SequenceNumber {}", _sequence_number);

            let (coin_type, is_stake) = match object_type {
                MoveObjectType::GasCoin => (SUI_COIN_TYPE, false),
                MoveObjectType::StakedSui => (SUI_COIN_TYPE, true),
                MoveObjectType::Coin(v) => (v, false),
            };
            let amount: Option<u64> = match (d.len(), is_stake) {
                (40, false) => Some(u64::from_le_bytes(
                    d.as_slice()[32..]
                        .try_into()
                        .expect("amount slice wrong length"),
                )),
                // StakedSui
                (80, true) => Some(u64::from_le_bytes(
                    d.as_slice()[72..]
                        .try_into()
                        .expect("amount slice wrong length"),
                )),
                _ => {
                    info!("ObjectContents incorrect");
                    None
                }
            };
            amount.map(|v| (coin_type, v))
        },
    )
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
                    if let Some((coin_id, module, name)) =
                        <DefaultInterp as AsyncParser<TypeTag, BS>>::parse(&DefaultInterp, input)
                            .await
                    {
                        MoveObjectType::Coin((coin_id, module, name))
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
) -> impl AsyncParser<StructTag, BS, Output = (CoinID, CoinModuleName, CoinFunctionName)> {
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
            ArrayVec<_, 5>,
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
            Some((
                address,
                module
                    .drain(..module.len().min(COIN_STRING_LENGTH))
                    .collect::<ArrayVec<_, COIN_STRING_LENGTH>>(),
                name.drain(..name.len().min(COIN_STRING_LENGTH))
                    .collect::<ArrayVec<_, COIN_STRING_LENGTH>>(),
            ))
        },
    )
}

impl HasOutput<TypeTag> for DefaultInterp {
    type Output = Option<(CoinID, CoinModuleName, CoinFunctionName)>;
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

impl HasOutput<TypeTag2> for DefaultInterp {
    type Output = ();
}

impl<BS: Clone + Readable> AsyncParser<TypeTag2, BS> for DefaultInterp {
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
                    info!("TypeTag2: Bool");
                }
                1 => {
                    info!("TypeTag2: U8");
                }
                2 => {
                    info!("TypeTag2: U64");
                }
                3 => {
                    info!("TypeTag2: U128");
                }
                4 => {
                    info!("TypeTag2: Address");
                }
                5 => {
                    info!("TypeTag2: Signer");
                }
                6 => {
                    info!("TypeTag2: Vector(Box<TypeTag2>)");
                }
                7 => {
                    info!("TypeTag2: Struct(StructTag)");
                    // Don't do recursion; ignore this object
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
    let mut hasher: Blake2b = Hasher::new();
    let salt = b"Object::";
    hasher.update(salt);

    const CHUNK_SIZE: usize = 128;
    let (chunks, rem) = (length / CHUNK_SIZE, length % CHUNK_SIZE);
    for _ in 0..chunks {
        let b: [u8; CHUNK_SIZE] = bs.read().await;
        hasher.update(&b);
    }
    for _ in 0..rem {
        let b: [u8; 1] = bs.read().await;
        hasher.update(&b);
    }

    hasher.finalize::<HexHash<32>>()
}
