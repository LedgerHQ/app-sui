use crate::parser::common::*;
use arrayvec::ArrayVec;
use core::convert::TryInto;
use core::future::Future;
use ledger_crypto_helpers::common::HexSlice;
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
pub type ObjectContents = Vec<Byte, 40>;
pub type Coin = (UID, Amount);

pub struct ObjectDataSchema;
pub struct OwnerSchema;

pub type TransactionDigest = SHA3_256_HASH;
pub type UID = ObjectID;

pub type StorageRebate = U64LE;

pub const STRING_LENGTH: usize = 256;
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
    Coin(CoinID),
}

// Parsers

pub const fn object_parser<BS: Clone + Readable>(
) -> impl AsyncParser<ObjectInnerSchema, BS, Output = CoinData> {
    Action(
        (DefaultInterp, DefaultInterp, DefaultInterp, DefaultInterp),
        |(d, _, _, storage_rebate)| {
            info!("Object: StorageRebate {}", storage_rebate);
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
        |(object_type, _, sequence_number, d): (_, _, _, ArrayVec<u8, 40>)| {
            info!("SequenceNumber {}", sequence_number);
            match d.into_inner() {
                Ok(c) => {
                    let uid: [u8; 32] = match object_type {
                        MoveObjectType::GasCoin => SUI_COIN_ID,
                        MoveObjectType::StakedSui => SUI_COIN_ID,
                        MoveObjectType::Coin(coin_id) => coin_id,
                    };
                    let amount: u64 =
                        u64::from_le_bytes(c[32..].try_into().expect("amount slice wrong length"));
                    info!("CoinData 0x{}, {}", HexSlice(&uid), amount);
                    Some((uid, amount))
                }
                Err(_) => {
                    info!("ObjectContents not of len 40");
                    None
                }
            }
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
                    if let Some(coin_id) =
                        <DefaultInterp as AsyncParser<TypeTag, BS>>::parse(&DefaultInterp, input)
                            .await
                    {
                        MoveObjectType::Coin(coin_id)
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
) -> impl AsyncParser<StructTag, BS, Output = CoinID> {
    Action(
        (
            DefaultInterp,
            SubInterp(DefaultInterp),
            SubInterp(DefaultInterp),
            SubInterp(DefaultInterp),
        ),
        |(address, module, name, type_tags): (
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
            info!("StructTag TypeTag len {}", type_tags.len());
            Some(address)
        },
    )
}

impl HasOutput<TypeTag> for DefaultInterp {
    type Output = Option<CoinID>;
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
                    let owner = <DefaultInterp as AsyncParser<SuiAddress, BS>>::parse(
                        &DefaultInterp,
                        input,
                    )
                    .await;
                    info!("OwnerSchema: AddressOwner({})", HexSlice(&owner));
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
