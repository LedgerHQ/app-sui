use crate::parser::common::*;

extern crate alloc;
use alloc::collections::BTreeMap;
use arrayvec::ArrayVec;
use core::convert::TryFrom;
use core::future::Future;
use ledger_device_sdk::io::SyscallError;
use ledger_log::info;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::bcs::async_parser::*;
use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::endianness::*;
use ledger_parser_combinators::interp::*;

// Tx Schema
pub type IntentMessage = (Intent, TransactionData);

pub struct TransactionData;

pub type TransactionDataV1 = (
    TransactionKind,
    SuiAddress,            // sender
    GasData,               // gas_data
    TransactionExpiration, // expiration
);

pub struct TransactionKind;

pub struct ProgrammableTransaction;

pub struct CommandSchema;
pub struct ArgumentSchema;
pub struct CallArgSchema;

pub type GasData = (
    Vec<ObjectRefSchema, { usize::MAX }>, // payment
    SuiAddress,                     // owner
    Amount,                         // price
    Amount,                         // budget
);

pub struct TransactionExpiration;
pub type EpochId = U64<{ Endianness::Little }>;

pub type SharedObject = (
    ObjectID,       // id
    SequenceNumber, // initial_shared_version
    bool,           // mutable
);

pub type Coins = Vec<ObjectRefSchema, { usize::MAX }>;

pub type Intent = (IntentVersion, IntentScope, AppId);
pub type IntentVersion = ULEB128;
pub type IntentScope = ULEB128;
pub type AppId = ULEB128;

// Tx Parsers

pub enum CallArg {
    RecipientAddress(SuiAddressRaw),
    Amount(u64),
    ObjectRef(ObjectDigest),
    Other,
}

impl HasOutput<CallArgSchema> for DefaultInterp {
    type Output = CallArg;
}

impl<BS: Clone + Readable> AsyncParser<CallArgSchema, BS> for DefaultInterp {
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
                    let length =
                        <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input)
                            .await;
                    info!("CallArgSchema: Pure: length: {}", length);
                    match length {
                        8 => CallArg::Amount(
                            <DefaultInterp as AsyncParser<Amount, BS>>::parse(
                                &DefaultInterp,
                                input,
                            )
                            .await,
                        ),
                        32 => CallArg::RecipientAddress(
                            <DefaultInterp as AsyncParser<Recipient, BS>>::parse(
                                &DefaultInterp,
                                input,
                            )
                            .await,
                        ),
                        _ => {
                            for _ in 0..length {
                                let _: [u8; 1] = input.read().await;
                            }
                            CallArg::Other
                        }
                    }
                }
                1 => {
                    let enum_variant =
                        <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input)
                            .await;
                    match enum_variant {
                        0 => {
                            info!("CallArgSchema: ObjectRef: ImmOrOwnedObject");
                            CallArg::ObjectRef(object_ref_parser().parse(input).await)
                        }
                        1 => {
                            info!("CallArgSchema: ObjectRef: SharedObject");
                            <(DefaultInterp, DefaultInterp, DefaultInterp) as AsyncParser<
                                    SharedObject,
                                BS,
                                >>::parse(
                                &(DefaultInterp, DefaultInterp, DefaultInterp), input
                            )
                                .await;
                            CallArg::Other
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
                _ => {
                    info!("CallArgSchema: Unknown enum: {}", enum_variant);
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

pub const TRANSFER_OBJECT_ARRAY_LENGTH: usize = 1;
pub const SPLIT_COIN_ARRAY_LENGTH: usize = 8;

pub enum Command {
    TransferObject(ArrayVec<Argument, TRANSFER_OBJECT_ARRAY_LENGTH>, Argument),
    SplitCoins(Argument, ArrayVec<Argument, SPLIT_COIN_ARRAY_LENGTH>),
}

impl HasOutput<CommandSchema> for DefaultInterp {
    type Output = Command;
}

impl<BS: Clone + Readable> AsyncParser<CommandSchema, BS> for DefaultInterp {
    type State<'c>
        = impl Future<Output = Self::Output> + 'c
    where
        BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let enum_variant =
                <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
            match enum_variant {
                1 => {
                    info!("CommandSchema: TransferObject");
                    let v1 = <SubInterp<DefaultInterp> as AsyncParser<
                        Vec<ArgumentSchema, TRANSFER_OBJECT_ARRAY_LENGTH>,
                        BS,
                    >>::parse(&SubInterp(DefaultInterp), input)
                    .await;
                    let v2 = <DefaultInterp as AsyncParser<ArgumentSchema, BS>>::parse(
                        &DefaultInterp,
                        input,
                    )
                    .await;
                    Command::TransferObject(v1, v2)
                }
                2 => {
                    info!("CommandSchema: SplitCoins");
                    let v1 = <DefaultInterp as AsyncParser<ArgumentSchema, BS>>::parse(
                        &DefaultInterp,
                        input,
                    )
                    .await;
                    let v2 = <SubInterp<DefaultInterp> as AsyncParser<
                        Vec<ArgumentSchema, SPLIT_COIN_ARRAY_LENGTH>,
                        BS,
                    >>::parse(&SubInterp(DefaultInterp), input)
                    .await;
                    Command::SplitCoins(v1, v2)
                }
                _ => {
                    info!("CommandSchema: Unknown enum: {}", enum_variant);
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

pub enum Argument {
    GasCoin,
    Input(u16),
    Result(u16),
    NestedResult(u16, u16),
}

impl HasOutput<ArgumentSchema> for DefaultInterp {
    type Output = Argument;
}

impl<BS: Clone + Readable> AsyncParser<ArgumentSchema, BS> for DefaultInterp {
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
                    info!("ArgumentSchema: GasCoin");
                    Argument::GasCoin
                }
                1 => {
                    info!("ArgumentSchema: Input");
                    Argument::Input(
                        <DefaultInterp as AsyncParser<U16LE, BS>>::parse(&DefaultInterp, input)
                            .await,
                    )
                }
                2 => {
                    info!("ArgumentSchema: Result");
                    Argument::Result(
                        <DefaultInterp as AsyncParser<U16LE, BS>>::parse(&DefaultInterp, input)
                            .await,
                    )
                }
                3 => {
                    info!("ArgumentSchema: NestedResult");
                    Argument::NestedResult(
                        <DefaultInterp as AsyncParser<U16LE, BS>>::parse(&DefaultInterp, input)
                            .await,
                        <DefaultInterp as AsyncParser<U16LE, BS>>::parse(&DefaultInterp, input)
                            .await,
                    )
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

impl HasOutput<ProgrammableTransaction> for ProgrammableTransaction {
    type Output = (
        <DefaultInterp as HasOutput<Recipient>>::Output,
        <DefaultInterp as HasOutput<Amount>>::Output,
    );
}

impl<BS: Clone + Readable> AsyncParser<ProgrammableTransaction, BS> for ProgrammableTransaction {
    type State<'c>
        = impl Future<Output = Self::Output> + 'c
    where
        BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let mut recipient_addr = None;
            let mut recipient_index = None;
            let mut inputs:  BTreeMap <u16, CallArg> = BTreeMap::new();
            let mut amounts: ArrayVec<(u64, u32), SPLIT_COIN_ARRAY_LENGTH> = ArrayVec::new();

            // Handle inputs
            {
                let length_u32 =
                    <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
                let length = u16::try_from(length_u32).expect("u16 expected");

                info!("ProgrammableTransaction: Inputs: {}", length);
                for i in 0..length {
                    let arg = <DefaultInterp as AsyncParser<CallArgSchema, BS>>::parse(
                        &DefaultInterp,
                        input,
                    )
                        .await;
                    match arg {
                        CallArg::Other => {},
                        _ => {
                            inputs.insert(i, arg);
                        },
                    }
                }
            }

            if inputs.is_empty() {
                reject_on::<()>(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await;
            }

            let recipient = match recipient_addr {
                Some(addr) => addr,
                _ => {
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
            };

            let mut verified_recipient = false;
            let mut total_amount: u64 = 0;
            // Handle commands
            {
                let length =
                    <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
                info!("ProgrammableTransaction: Commands: {}", length);
                for _ in 0..length {
                    let c = <DefaultInterp as AsyncParser<CommandSchema, BS>>::parse(
                        &DefaultInterp,
                        input,
                    )
                    .await;
                    match c {
                        Command::TransferObject(_nested_results, recipient_input) => {
                            if verified_recipient {
                                // Reject more than one TransferObject(s)
                                reject_on::<()>(
                                    core::file!(),
                                    core::line!(),
                                    SyscallError::NotSupported as u16,
                                )
                                .await;
                            }
                            match recipient_input {
                                Argument::Input(inp_index) => {
                                    if Some(inp_index as u32) != recipient_index {
                                        info!("TransferObject recipient mismatch");
                                        reject_on::<()>(
                                            core::file!(),
                                            core::line!(),
                                            SyscallError::NotSupported as u16,
                                        )
                                        .await;
                                    }
                                    verified_recipient = true;
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
                        Command::SplitCoins(coin, input_indices) => {
                            match coin {
                                Argument::GasCoin => {}
                                _ => {
                                    reject_on(
                                        core::file!(),
                                        core::line!(),
                                        SyscallError::NotSupported as u16,
                                    )
                                    .await
                                }
                            }
                            for arg in &input_indices {
                                match arg {
                                    Argument::Input(inp_index) => {
                                        for (amt, ix) in &amounts {
                                            if *ix == (*inp_index as u32) {
                                                match total_amount.checked_add(*amt) {
                                                    Some(t) => total_amount = t,
                                                    None => {
                                                        reject_on(
                                                            core::file!(),
                                                            core::line!(),
                                                            SyscallError::InvalidParameter as u16,
                                                        )
                                                        .await
                                                    }
                                                }
                                            }
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
            }

            if !verified_recipient {
                reject_on::<()>(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await;
            }

            (recipient, total_amount)
        }
    }
}

impl HasOutput<TransactionKind> for TransactionKind {
    type Output = <ProgrammableTransaction as HasOutput<ProgrammableTransaction>>::Output;
}

impl<BS: Clone + Readable> AsyncParser<TransactionKind, BS> for TransactionKind {
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
                    info!("TransactionKind: ProgrammableTransaction");
                    <ProgrammableTransaction as AsyncParser<ProgrammableTransaction, BS>>::parse(
                        &ProgrammableTransaction,
                        input,
                    )
                    .await
                }
                _ => {
                    info!("TransactionKind: {}", enum_variant);
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

impl HasOutput<TransactionExpiration> for DefaultInterp {
    type Output = ();
}

impl<BS: Clone + Readable> AsyncParser<TransactionExpiration, BS> for DefaultInterp {
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
                    info!("TransactionExpiration: None");
                }
                1 => {
                    info!("TransactionExpiration: Epoch");
                    <DefaultInterp as AsyncParser<EpochId, BS>>::parse(&DefaultInterp, input).await;
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

const fn gas_data_parser<BS: Clone + Readable>() -> impl AsyncParser<GasData, BS, Output = u64> {
    Action(
        (
            SubInterp(Action(object_ref_parser(), |_| Some(()))),
            DefaultInterp,
            DefaultInterp,
            DefaultInterp,
        ),
        |(_, _sender, _gas_price, gas_budget): (_, _, u64, u64)| {
            // Gas price is per gas amount. Gas budget is total, reflecting the amount of gas *
            // gas price. We only care about the total, not the price or amount in isolation , so we
            // just ignore that field.
            //
            // C.F. https://github.com/MystenLabs/sui/pull/8676
            Some(gas_budget)
        },
    )
}

const fn object_ref_parser<BS: Readable>() -> impl AsyncParser<ObjectRefSchema, BS, Output = ObjectDigest> {
    Action((DefaultInterp, DefaultInterp, DefaultInterp), |(_,_,d)| Some(d))
}

const fn intent_parser<BS: Readable>() -> impl AsyncParser<Intent, BS, Output = ()> {
    Action((DefaultInterp, DefaultInterp, DefaultInterp), |_| {
        info!("Intent Ok");
        Some(())
    })
}

type TransactionDataV1Output = (<TransactionKind as HasOutput<TransactionKind>>::Output, u64);

const fn transaction_data_v1_parser<BS: Clone + Readable>(
) -> impl AsyncParser<TransactionDataV1, BS, Output = TransactionDataV1Output> {
    Action(
        (
            TransactionKind,
            DefaultInterp,
            gas_data_parser(),
            DefaultInterp,
        ),
        |(v, _, gas_budget, _)| Some((v, gas_budget)),
    )
}

impl HasOutput<TransactionData> for TransactionData {
    type Output = TransactionDataV1Output;
}

impl<BS: Clone + Readable> AsyncParser<TransactionData, BS> for TransactionData {
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
                    info!("TransactionData: V1");
                    transaction_data_v1_parser().parse(input).await
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

pub const fn tx_parser<BS: Clone + Readable>(
) -> impl AsyncParser<IntentMessage, BS, Output = <TransactionData as HasOutput<TransactionData>>::Output>
{
    Action((intent_parser(), TransactionData), |(_, d)| Some(d))
}
