use crate::parser::common::*;
use crate::utils::{estimate_btree_map_usage, NoinlineFut};

extern crate alloc;
use alloc::collections::BTreeMap;
use arrayvec::ArrayVec;
use core::convert::TryFrom;
use core::future::Future;
use either::*;
use ledger_device_sdk::io::SyscallError;
use ledger_log::info;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::bcs::async_parser::*;
use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::endianness::*;
use ledger_parser_combinators::interp::*;

// Tx Schema
pub type IntentMessage = (Intent, TransactionDataSchema);

pub struct TransactionDataSchema;

pub type TransactionDataV1 = (
    TransactionKindSchema,
    SuiAddress,            // sender
    GasDataSchema,         // gas_data
    TransactionExpiration, // expiration
);

pub struct TransactionKindSchema;

pub struct ProgrammableTransactionSchema;

pub struct CommandSchema;
pub struct ArgumentSchema;
pub struct CallArgSchema;

pub const MAX_GAS_COIN_COUNT: usize = 32;
pub type GasDataSchema = (
    Vec<ObjectRefSchema, MAX_GAS_COIN_COUNT>, // payment
    SuiAddress,                               // owner
    Amount,                                   // price
    Amount,                                   // budget
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

// Parsed data

// Gas Budget + total gas coin amount (if known)
pub type GasData = (u64, Option<u64>);

// Tx Parsers

pub enum CallArg {
    RecipientAddress(SuiAddressRaw),
    Amount(u64),
    OptionalAmount(Option<u64>),
    ObjectRef(ObjectDigest),
    SharedObject(CoinID),
    Other,
}

// Inputs which are referenced in computation of commands
pub enum InputValue {
    RecipientAddress(SuiAddressRaw),
    Amount(u64),
    OptionalAmount(Option<u64>),
    ObjectRef(ObjectDigest),
    SharedObject(CoinID),
    Object(CoinData),
    // ^ mutable via MergeCoins
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
                        1 | 9 => CallArg::OptionalAmount(
                            <SubInterp<DefaultInterp> as AsyncParser<Option<Amount>, BS>>::parse(
                                &SubInterp(DefaultInterp),
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
                            let (object_id, _, _) =
                                <(DefaultInterp, DefaultInterp, DefaultInterp) as AsyncParser<
                                    SharedObject,
                                    BS,
                                >>::parse(
                                    &(DefaultInterp, DefaultInterp, DefaultInterp), input
                                )
                                .await;
                            CallArg::SharedObject(object_id)
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

pub struct TypeInput;

impl HasOutput<TypeInput> for DefaultInterp {
    type Output = ();
}

impl<BS: Clone + Readable> AsyncParser<TypeInput, BS> for DefaultInterp {
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
                    info!("TypeInput: Bool");
                }
                1 => {
                    info!("TypeInput: U8");
                }
                2 => {
                    info!("TypeInput: U64");
                }
                3 => {
                    info!("TypeInput: U128");
                }
                4 => {
                    info!("TypeInput: Address");
                }
                5 => {
                    info!("TypeInput: Signer");
                }
                6 => {
                    info!("TypeInput: Vector(Box<TypeInput>)");
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
                7 => {
                    info!("TypeInput: Struct(Box<StructInput>)");
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
                8 => {
                    info!("TypeInput: U16");
                }
                9 => {
                    info!("TypeInput: U32");
                }
                10 => {
                    info!("TypeInput: U256");
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

pub const MOVE_CALL_ARGS_ARRAY_LENGTH: usize = 8;
pub const TRANSFER_OBJECT_ARRAY_LENGTH: usize = 8;
pub const SPLIT_COIN_ARRAY_LENGTH: usize = 8;
pub const MERGE_COIN_ARRAY_LENGTH: usize = 8;
pub const MAKE_MOVE_VEC_ARRAY_LENGTH: usize = 8;

pub const STRING_LENGTH: usize = 32;
pub type String = Vec<Byte, STRING_LENGTH>;

pub enum Command {
    MoveCall(
        CoinID,
        ArrayVec<u8, STRING_LENGTH>,
        ArrayVec<u8, STRING_LENGTH>,
        ArrayVec<Argument, MOVE_CALL_ARGS_ARRAY_LENGTH>,
    ),
    TransferObject(ArrayVec<Argument, TRANSFER_OBJECT_ARRAY_LENGTH>, Argument),
    SplitCoins(Argument, ArrayVec<Argument, SPLIT_COIN_ARRAY_LENGTH>),
    MergeCoins(Argument, ArrayVec<Argument, MERGE_COIN_ARRAY_LENGTH>),
    MakeMoveVec(ArrayVec<Argument, MAKE_MOVE_VEC_ARRAY_LENGTH>),
}

pub enum CommandResult {
    SplitCoinAmounts(CoinType, ArrayVec<u64, SPLIT_COIN_ARRAY_LENGTH>),
    MergedCoin(CoinData),
    MoveVecMergedCoin(CoinData),
    StakingPoolSplitCoin(u64),
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
                0 => {
                    info!("CommandSchema: MoveCall");
                    let package =
                        <DefaultInterp as AsyncParser<ObjectID, BS>>::parse(&DefaultInterp, input)
                            .await;
                    let module = <SubInterp<DefaultInterp> as AsyncParser<String, BS>>::parse(
                        &SubInterp(DefaultInterp),
                        input,
                    )
                    .await;
                    let function = <SubInterp<DefaultInterp> as AsyncParser<String, BS>>::parse(
                        &SubInterp(DefaultInterp),
                        input,
                    )
                    .await;
                    <SubInterp<DefaultInterp> as AsyncParser<Vec<TypeInput, 0>, BS>>::parse(
                        &SubInterp(DefaultInterp),
                        input,
                    )
                    .await;
                    let args = <SubInterp<DefaultInterp> as AsyncParser<
                        Vec<ArgumentSchema, MOVE_CALL_ARGS_ARRAY_LENGTH>,
                        BS,
                    >>::parse(&SubInterp(DefaultInterp), input)
                    .await;
                    Command::MoveCall(package, module, function, args)
                }
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
                3 => {
                    info!("CommandSchema: MergeCoins");
                    let v1 = <DefaultInterp as AsyncParser<ArgumentSchema, BS>>::parse(
                        &DefaultInterp,
                        input,
                    )
                    .await;
                    let v2 = <SubInterp<DefaultInterp> as AsyncParser<
                        Vec<ArgumentSchema, MERGE_COIN_ARRAY_LENGTH>,
                        BS,
                    >>::parse(&SubInterp(DefaultInterp), input)
                    .await;
                    Command::MergeCoins(v1, v2)
                }
                5 => {
                    info!("CommandSchema: MakeMoveVec");
                    <SubInterp<DefaultInterp> as AsyncParser<Vec<TypeInput, 0>, BS>>::parse(
                        &SubInterp(DefaultInterp),
                        input,
                    )
                    .await;
                    let v1 = <SubInterp<DefaultInterp> as AsyncParser<
                        Vec<ArgumentSchema, MAKE_MOVE_VEC_ARRAY_LENGTH>,
                        BS,
                    >>::parse(&SubInterp(DefaultInterp), input)
                    .await;
                    Command::MakeMoveVec(v1)
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

#[derive(Debug)]
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

pub struct ProgrammableTransactionParser<OD> {
    object_data_source: OD,
}

pub enum ProgrammableTransaction {
    TransferSuiTx {
        recipient: <DefaultInterp as HasOutput<Recipient>>::Output,
        amount: <DefaultInterp as HasOutput<Amount>>::Output,
        includes_gas_coin: bool,
    },
    TransferTokenTx {
        recipient: <DefaultInterp as HasOutput<Recipient>>::Output,
        amount: <DefaultInterp as HasOutput<Amount>>::Output,
        coin_type: CoinType,
    },
    StakeTx {
        recipient: <DefaultInterp as HasOutput<Recipient>>::Output,
        amount: <DefaultInterp as HasOutput<Amount>>::Output,
        includes_gas_coin: bool,
    },
    UnstakeTx {
        total_amount: u64,
    },
}

pub enum ProgrammableTransactionTypeState {
    UnknownTx,
    TransferTx,
    StakeTx,
    UnstakeTx,
}

impl<OD> HasOutput<ProgrammableTransactionSchema> for ProgrammableTransactionParser<OD> {
    type Output = ProgrammableTransaction;
}

impl<BS: Clone + Readable, OD: Clone + HasObjectData> AsyncParser<ProgrammableTransactionSchema, BS>
    for ProgrammableTransactionParser<OD>
{
    type State<'c>
        = impl Future<Output = Self::Output> + 'c
    where
        BS: 'c, OD: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let mut inputs: BTreeMap<u16, InputValue> = BTreeMap::new();
            let mut command_results: BTreeMap<u16, CommandResult> = BTreeMap::new();

            // By using heap we have the flexibility to handle transactions of various sizes
            // But if we exceed the heap usage it would crash the app while parsing the transaction.
            // It would be better to allow user to blind sign in such cases.
            // This ensures that we never hit heap memory limits during parse(8k)
            async fn check_heap_use(
                inputs: &BTreeMap<u16, InputValue>,
                command_results: &BTreeMap<u16, CommandResult>,
            ) {
                const MAX_HEAP_USAGE_ALLOWED: usize = 7 * 1024;

                let v1 = estimate_btree_map_usage(inputs);
                let v2 = estimate_btree_map_usage(command_results);
                if v1 + v2 > MAX_HEAP_USAGE_ALLOWED {
                    reject_on::<()>(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await;
                }
            }

            // Handle inputs
            {
                let length_u32 =
                    <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
                let length = u16::try_from(length_u32).expect("u16 expected");

                info!("ProgrammableTransaction: Inputs: {}", length);
                for i in 0..length {
                    check_heap_use(&inputs, &command_results).await;
                    let arg =
                        NoinlineFut(<DefaultInterp as AsyncParser<CallArgSchema, BS>>::parse(
                            &DefaultInterp,
                            input,
                        ))
                        .await;
                    match arg {
                        CallArg::Other => {}
                        CallArg::RecipientAddress(v) => {
                            inputs.insert(i, InputValue::RecipientAddress(v));
                        }
                        CallArg::Amount(v) => {
                            inputs.insert(i, InputValue::Amount(v));
                        }
                        CallArg::OptionalAmount(v) => {
                            inputs.insert(i, InputValue::OptionalAmount(v));
                        }
                        CallArg::ObjectRef(v) => {
                            inputs.insert(i, InputValue::ObjectRef(v));
                        }
                        CallArg::SharedObject(v) => {
                            inputs.insert(i, InputValue::SharedObject(v));
                        }
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

            let mut recipient_addr = None;

            // Total amount, that we know of, being transferred to recipient
            // Does not contain amount from GasCoin, in case the entire GasCoin is also being transferred
            let mut total_amount: u64 = 0;

            let mut coin_type: Option<CoinType> = None;

            // Are we transferring entire gas coin?
            let mut includes_gas_coin: bool = false;

            // Amount added to GasCoin via MergeCoins
            // as we don't know the GasCoin coin balance, we only track the any additions here
            let mut added_amount_to_gas_coin: u64 = 0;

            let mut tx_type: ProgrammableTransactionTypeState =
                ProgrammableTransactionTypeState::UnknownTx;

            // Handle commands
            {
                let length_u32 =
                    <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
                let length = u16::try_from(length_u32).expect("u16 expected");
                info!("ProgrammableTransaction: Commands: {}", length);
                for command_ix in 0..length {
                    check_heap_use(&inputs, &command_results).await;
                    let c = NoinlineFut(<DefaultInterp as AsyncParser<CommandSchema, BS>>::parse(
                        &DefaultInterp,
                        input,
                    ))
                    .await;
                    match c {
                        Command::MoveCall(package, module, function, args) => {
                            match tx_type {
                                ProgrammableTransactionTypeState::UnknownTx => {}
                                _ => {
                                    // We don't support more than one MoveCall operation per tx
                                    // (or MoveCall with TransferObject)
                                    info!("MoveCall operation not supported");
                                    reject_on(
                                        core::file!(),
                                        core::line!(),
                                        SyscallError::NotSupported as u16,
                                    )
                                    .await
                                }
                            }
                            let res = NoinlineFut(handle_move_call(
                                package,
                                module,
                                function,
                                args,
                                &inputs,
                                &mut recipient_addr,
                                &mut total_amount,
                                &mut coin_type,
                                &mut includes_gas_coin,
                                self.object_data_source.clone(),
                                &command_results,
                            ))
                            .await;
                            match res {
                                Left(tx_type_) => tx_type = tx_type_,
                                Right(v) => {
                                    command_results.insert(command_ix, v);
                                }
                            }
                        }
                        Command::TransferObject(coins, recipient_input) => {
                            match tx_type {
                                ProgrammableTransactionTypeState::UnknownTx => {
                                    tx_type = ProgrammableTransactionTypeState::TransferTx;
                                }
                                ProgrammableTransactionTypeState::TransferTx => {}
                                _ => {
                                    reject_on(
                                        core::file!(),
                                        core::line!(),
                                        SyscallError::NotSupported as u16,
                                    )
                                    .await
                                }
                            }
                            NoinlineFut(handle_transfer_object(
                                coins,
                                recipient_input,
                                &inputs,
                                &mut recipient_addr,
                                &mut total_amount,
                                &mut coin_type,
                                &mut includes_gas_coin,
                                self.object_data_source.clone(),
                                &command_results,
                            ))
                            .await;
                        }
                        Command::SplitCoins(coin, amounts) => {
                            let res = NoinlineFut(handle_split_coins(
                                coin,
                                amounts,
                                &inputs,
                                self.object_data_source.clone(),
                                &command_results,
                            ))
                            .await;
                            command_results.insert(command_ix, res);
                        }
                        Command::MergeCoins(dest_coin, coins) => {
                            NoinlineFut(handle_merge_coins(
                                dest_coin,
                                coins,
                                &mut inputs,
                                self.object_data_source.clone(),
                                &mut command_results,
                                &mut added_amount_to_gas_coin,
                            ))
                            .await;
                        }
                        Command::MakeMoveVec(coins) => {
                            let res = NoinlineFut(handle_make_move_vec(
                                coins,
                                &mut inputs,
                                &mut coin_type,
                                &mut includes_gas_coin,
                                self.object_data_source.clone(),
                                &command_results,
                            ))
                            .await;
                            command_results.insert(command_ix, res);
                        }
                    }
                }
            }

            let coin_type = match coin_type {
                Some(v) => v,
                _ => {
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
            };

            match tx_type {
                ProgrammableTransactionTypeState::TransferTx => {
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

                    if coin_type.0 != SUI_COIN_ID {
                        if includes_gas_coin {
                            reject_on(
                                core::file!(),
                                core::line!(),
                                SyscallError::NotSupported as u16,
                            )
                            .await
                        }

                        ProgrammableTransaction::TransferTokenTx {
                            recipient,
                            amount: total_amount,
                            coin_type,
                        }
                    } else {
                        if includes_gas_coin {
                            total_amount += added_amount_to_gas_coin;
                        }

                        ProgrammableTransaction::TransferSuiTx {
                            recipient,
                            amount: total_amount,
                            includes_gas_coin,
                        }
                    }
                }
                ProgrammableTransactionTypeState::StakeTx => {
                    if coin_type.0 != SUI_COIN_ID {
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
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

                    ProgrammableTransaction::StakeTx {
                        recipient,
                        amount: total_amount,
                        includes_gas_coin,
                    }
                }
                ProgrammableTransactionTypeState::UnstakeTx => {
                    if coin_type.0 != SUI_COIN_ID {
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                    ProgrammableTransaction::UnstakeTx { total_amount }
                }
                ProgrammableTransactionTypeState::UnknownTx => {
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

async fn handle_move_call<OD: HasObjectData>(
    package: CoinID,
    module: ArrayVec<u8, STRING_LENGTH>,
    function: ArrayVec<u8, STRING_LENGTH>,
    args: ArrayVec<Argument, MOVE_CALL_ARGS_ARRAY_LENGTH>,
    inputs: &BTreeMap<u16, InputValue>,
    recipient_addr: &mut Option<SuiAddressRaw>,
    total_amount: &mut u64,
    coin_type: &mut Option<CoinType>,
    includes_gas_coin: &mut bool,
    object_data_source: OD,
    command_results: &BTreeMap<u16, CommandResult>,
) -> Either<ProgrammableTransactionTypeState, CommandResult> {
    if package != SUI_SYSTEM_ID {
        reject_on(
            core::file!(),
            core::line!(),
            SyscallError::NotSupported as u16,
        )
        .await
    }
    fn is_sui_state(inp: &InputValue) -> Option<()> {
        match inp {
            InputValue::SharedObject(id_) => {
                if *id_ == SUI_SYSTEM_STATE_ID {
                    Some(())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    let get_arg_input = |arg_ix: usize| -> Option<&InputValue> {
        args.get(arg_ix).and_then(|arg| match arg {
            Argument::Input(ix) => inputs.get(ix),
            _ => None,
        })
    };

    return if core::str::from_utf8(module.as_slice()) == Ok("sui_system")
        && core::str::from_utf8(function.as_slice()) == Ok("request_add_stake")
    {
        info!("MoveCall 0x3::sui_system::request_add_stake");

        if let None = get_arg_input(0).and_then(is_sui_state) {
            reject_on(
                core::file!(),
                core::line!(),
                SyscallError::NotSupported as u16,
            )
            .await
        }

        match args.get(1) {
            None => {
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
            Some(arg) => {
                NoinlineFut(get_coin_amount(
                    arg,
                    inputs,
                    total_amount,
                    coin_type,
                    includes_gas_coin,
                    &object_data_source,
                    command_results,
                ))
                .await;
            }
        }

        match (get_arg_input(2), &recipient_addr) {
            (Some(InputValue::RecipientAddress(addr)), None) => {
                *recipient_addr = Some(addr.clone());
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
        Left(ProgrammableTransactionTypeState::StakeTx)
    } else if core::str::from_utf8(module.as_slice()) == Ok("sui_system")
        && core::str::from_utf8(function.as_slice()) == Ok("request_add_stake_mul_coin")
    {
        info!("MoveCall 0x3::sui_system::request_add_stake_mul_coin");

        if let None = get_arg_input(0).and_then(is_sui_state) {
            reject_on(
                core::file!(),
                core::line!(),
                SyscallError::NotSupported as u16,
            )
            .await
        }

        let mut total_amount_2: u64 = 0;
        if let Some(CommandResult::MoveVecMergedCoin((t, amt))) =
            args.get(1).and_then(|v| match v {
                Argument::Result(ix) => command_results.get(ix),
                _ => None,
            })
        {
            *coin_type = Some(t.clone());
            total_amount_2 = *amt;
        } else {
            reject_on(
                core::file!(),
                core::line!(),
                SyscallError::NotSupported as u16,
            )
            .await
        }

        match get_arg_input(2) {
            Some(InputValue::OptionalAmount(Some(amt))) => {
                *total_amount = *amt;
            }
            Some(InputValue::OptionalAmount(None)) => {
                *total_amount = total_amount_2;
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

        match (get_arg_input(3), &recipient_addr) {
            (Some(InputValue::RecipientAddress(addr)), None) => {
                *recipient_addr = Some(addr.clone());
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
        Left(ProgrammableTransactionTypeState::StakeTx)
    } else if core::str::from_utf8(module.as_slice()) == Ok("sui_system")
        && core::str::from_utf8(function.as_slice()) == Ok("request_withdraw_stake")
    {
        info!("MoveCall 0x3::sui_system::request_withdraw_stake");

        if let None = get_arg_input(0).and_then(is_sui_state) {
            reject_on(
                core::file!(),
                core::line!(),
                SyscallError::NotSupported as u16,
            )
            .await
        }

        match args.get(1) {
            None => {
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
            Some(arg) => {
                if let Some(amt) = match arg {
                    Argument::Result(command_ix) => match command_results.get(command_ix) {
                        Some(CommandResult::StakingPoolSplitCoin(amt)) => Some(amt),
                        _ => None,
                    },
                    _ => None,
                } {
                    *total_amount += amt;
                    *coin_type = Some(SUI_COIN_TYPE);
                } else {
                    NoinlineFut(get_coin_amount(
                        arg,
                        inputs,
                        total_amount,
                        coin_type,
                        includes_gas_coin,
                        &object_data_source,
                        command_results,
                    ))
                    .await;
                }
            }
        }

        Left(ProgrammableTransactionTypeState::UnstakeTx)
    } else if core::str::from_utf8(module.as_slice()) == Ok("staking_pool")
        && core::str::from_utf8(function.as_slice()) == Ok("split")
    {
        info!("MoveCall 0x3::staking_pool::split");

        // We do not need to check the balance or CoinID of coin
        // As incorrect values will be rejected on chain

        match get_arg_input(1) {
            Some(InputValue::Amount(amt)) => Right(CommandResult::StakingPoolSplitCoin(*amt)),
            _ => {
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
        }
    } else {
        info!(
            "MoveCall NYI: '0x{}::{}::{}'",
            HexSlice(&package),
            core::str::from_utf8(module.as_slice()).unwrap_or("invalid utf-8"),
            core::str::from_utf8(function.as_slice()).unwrap_or("invalid utf-8")
        );
        reject_on(
            core::file!(),
            core::line!(),
            SyscallError::NotSupported as u16,
        )
        .await
    };
}

async fn handle_transfer_object<OD: HasObjectData>(
    coins: ArrayVec<Argument, TRANSFER_OBJECT_ARRAY_LENGTH>,
    recipient_input: Argument,
    inputs: &BTreeMap<u16, InputValue>,
    recipient_addr: &mut Option<SuiAddressRaw>,
    total_amount: &mut u64,
    coin_type: &mut Option<CoinType>,
    includes_gas_coin: &mut bool,
    object_data_source: OD,
    command_results: &BTreeMap<u16, CommandResult>,
) {
    match recipient_input {
        Argument::Input(inp_index) => match inputs.get(&inp_index) {
            Some(InputValue::RecipientAddress(addr)) => match recipient_addr {
                Some(addr_) => {
                    if *addr != *addr_ {
                        info!("TransferObject multiple recipients");
                        reject_on::<()>(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await;
                    }
                }
                None => *recipient_addr = Some(addr.clone()),
            },
            _ => {
                info!("TransferObject invalid inp_index");
                reject_on::<()>(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await;
            }
        },
        _ => {
            reject_on(
                core::file!(),
                core::line!(),
                SyscallError::NotSupported as u16,
            )
            .await
        }
    }
    // set total_amount
    for coin in &coins {
        get_coin_amount(
            coin,
            inputs,
            total_amount,
            coin_type,
            includes_gas_coin,
            &object_data_source,
            command_results,
        )
        .await;
    }
}

async fn get_coin_amount<OD: HasObjectData>(
    coin: &Argument,
    inputs: &BTreeMap<u16, InputValue>,
    total_amount: &mut u64,
    coin_type: &mut Option<CoinType>,
    includes_gas_coin: &mut bool,
    object_data_source: &OD,
    command_results: &BTreeMap<u16, CommandResult>,
) {
    info!("get_coin_amount for {:?}", coin);
    match coin {
        Argument::GasCoin => {
            if let Some((id_, _, _)) = coin_type {
                if *id_ != SUI_COIN_ID {
                    info!("get_coin_amount mismatch in coin_id(s)");
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
            } else {
                *coin_type = Some(SUI_COIN_TYPE.clone());
            }
            *includes_gas_coin = true;
        }
        Argument::Input(input_ix) => match inputs.get(input_ix) {
            Some(InputValue::ObjectRef(digest)) => {
                info!("get_coin_amount trying object_data_source");
                let coin_data = object_data_source.get_object_data(&digest).await;
                match coin_data {
                    Some((coin_type_, amt)) => {
                        if let Some(v) = coin_type {
                            if *v != coin_type_ {
                                info!("get_coin_amount mismatch in coin_type(s)");
                                reject_on(
                                    core::file!(),
                                    core::line!(),
                                    SyscallError::NotSupported as u16,
                                )
                                .await
                            }
                        } else {
                            *coin_type = Some(coin_type_.clone());
                        }
                        *total_amount += amt
                    }
                    _ => {
                        info!("get_coin_amount Coin Object not found");
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                }
            }
            Some(InputValue::Object((coin_type_, amt))) => {
                if let Some(v) = coin_type {
                    if *v != *coin_type_ {
                        info!("get_coin_amount mismatch in coin_type(s)");
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                } else {
                    *coin_type = Some(coin_type_.clone());
                }
                *total_amount += amt;
            }
            Some(_) => {
                info!("get_coin_amount input refers to non ObjectRef");
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
            None => {
                info!("get_coin_amount input not found");
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
        },
        Argument::NestedResult(command_ix, coin_ix) => match command_results.get(command_ix) {
            Some(CommandResult::SplitCoinAmounts(coin_type_, coin_amounts)) => {
                if let Some(v) = coin_type {
                    if *v != *coin_type_ {
                        info!("get_coin_amount mismatch in coin_type(s)");
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                } else {
                    *coin_type = Some(coin_type_.clone());
                }
                if let Some(amt) = coin_amounts.get(*coin_ix as usize) {
                    *total_amount += amt;
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
        },
        Argument::Result(command_ix) => match command_results.get(command_ix) {
            Some(CommandResult::SplitCoinAmounts(coin_type_, coin_amounts)) => {
                if let Some(v) = coin_type {
                    if *v != *coin_type_ {
                        info!("get_coin_amount mismatch in coin_type(s)");
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                } else {
                    *coin_type = Some(coin_type_.clone());
                }
                if coin_amounts.len() == 1 {
                    *total_amount += coin_amounts[0];
                } else {
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
            }
            Some(CommandResult::MergedCoin((coin_type_, amt))) => {
                if let Some(v) = coin_type {
                    if *v != *coin_type_ {
                        info!("get_coin_amount mismatch in coin_type(s)");
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                } else {
                    *coin_type = Some(coin_type_.clone());
                }
                *total_amount += amt;
            }
            _ => {
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
        },
    }
}

async fn handle_split_coins<OD: HasObjectData>(
    coin: Argument,
    amounts: ArrayVec<Argument, SPLIT_COIN_ARRAY_LENGTH>,
    inputs: &BTreeMap<u16, InputValue>,
    object_data_source: OD,
    command_results: &BTreeMap<u16, CommandResult>,
) -> CommandResult {
    // We are not validating whether the coin balance is sufficient for the amounts specified
    // as the transaction would fail on the network with InsufficientCoinBalance error
    let coin_type = match coin {
        Argument::GasCoin => SUI_COIN_TYPE,
        Argument::Input(input_ix) => match inputs.get(&input_ix) {
            Some(InputValue::ObjectRef(digest)) => {
                info!("SplitCoins trying object_data_source");
                let coin_data = object_data_source.get_object_data(&digest).await;
                match coin_data {
                    Some((v, _)) => v,
                    _ => {
                        info!("SplitCoins Coin Object not found");
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                }
            }
            Some(InputValue::Object((v, _))) => v.clone(),
            _ => {
                info!("SplitCoins input refers to non ObjectRef");
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
        },
        Argument::NestedResult(command_ix, _) => {
            if let Some(v) = command_results
                .get(&command_ix)
                .and_then(|result| match result {
                    CommandResult::SplitCoinAmounts(id, _) => Some(id),
                    _ => None,
                })
            {
                v.clone()
            } else {
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
        }

        Argument::Result(command_ix) => match command_results.get(&command_ix) {
            Some(CommandResult::MergedCoin((v, _))) => v.clone(),
            _ => {
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
        },
    };
    let mut coin_amounts = ArrayVec::<u64, SPLIT_COIN_ARRAY_LENGTH>::new();
    for arg in &amounts {
        match arg {
            Argument::Input(inp_index) => match inputs.get(&inp_index) {
                Some(InputValue::Amount(amt)) => {
                    coin_amounts.push(*amt);
                }
                _ => {
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
            },
            _ => {
                info!("SplitCoins amount not fetched from inputs");
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
        }
    }
    CommandResult::SplitCoinAmounts(coin_type, coin_amounts)
}

async fn handle_merge_coins<OD: HasObjectData>(
    dest_coin: Argument,
    coins: ArrayVec<Argument, MERGE_COIN_ARRAY_LENGTH>,
    inputs: &mut BTreeMap<u16, InputValue>,
    object_data_source: OD,
    command_results: &mut BTreeMap<u16, CommandResult>,
    added_amount_to_gas_coin: &mut u64,
) {
    let mut total_amount_2: u64 = 0;
    let coin_type = match dest_coin {
        Argument::GasCoin => SUI_COIN_TYPE,
        Argument::Input(input_ix) => match inputs.get(&input_ix) {
            Some(InputValue::ObjectRef(digest)) => {
                info!("MergeCoins trying object_data_source");
                let coin_data = object_data_source.get_object_data(&digest).await;
                match coin_data {
                    Some((v, amt)) => {
                        total_amount_2 += amt;
                        v
                    }
                    _ => {
                        info!("MergeCoins Coin Object not found");
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                }
            }
            Some(InputValue::Object((coin_type, amt))) => {
                total_amount_2 += amt;
                coin_type.clone()
            }
            _ => {
                info!("MergeCoins input refers to non ObjectRef");
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
        },
        Argument::NestedResult(command_ix, coin_ix) => {
            if let Some((v, amt)) = command_results
                .get(&command_ix)
                .and_then(|result| match result {
                    CommandResult::SplitCoinAmounts(id, coin_amounts) => {
                        coin_amounts.get(coin_ix as usize).map(|amt| (id, amt))
                    }
                    _ => None,
                })
            {
                total_amount_2 += amt;
                v.clone()
            } else {
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
        }
        Argument::Result(_) => {
            info!("MergeCoins destination coin Result not supported");
            reject_on(
                core::file!(),
                core::line!(),
                SyscallError::NotSupported as u16,
            )
            .await
        }
    };
    for coin in &coins {
        match coin {
            Argument::GasCoin => {
                info!("MergeCoins cannot consume gas coin");
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
            Argument::Input(input_ix) => match inputs.get(input_ix) {
                Some(InputValue::ObjectRef(digest)) => {
                    info!("MergeCoins trying object_data_source");
                    let coin_data = object_data_source.get_object_data(&digest).await;
                    match coin_data {
                        Some((coin_type_, amt)) => {
                            if coin_type_ != coin_type {
                                info!("MergeCoins mismatch in coin_type(s)");
                                reject_on(
                                    core::file!(),
                                    core::line!(),
                                    SyscallError::NotSupported as u16,
                                )
                                .await
                            }
                            total_amount_2 += amt;
                        }
                        _ => {
                            info!("MergeCoins Coin Object not found");
                            reject_on(
                                core::file!(),
                                core::line!(),
                                SyscallError::NotSupported as u16,
                            )
                            .await
                        }
                    }
                }
                Some(InputValue::Object((coin_type_, amt))) => {
                    if *coin_type_ != coin_type {
                        info!("MergeCoins mismatch in coin_type(s)");
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                    total_amount_2 += amt;
                }
                _ => {
                    info!("MergeCoins input refers to non ObjectRef");
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
            },
            Argument::NestedResult(command_ix, coin_ix) => {
                if let Some(amt) = command_results
                    .get(command_ix)
                    .and_then(|result| match result {
                        CommandResult::SplitCoinAmounts(coin_type_, coin_amounts) => {
                            if *coin_type_ != coin_type {
                                None
                            } else {
                                coin_amounts.get(*coin_ix as usize)
                            }
                        }
                        _ => None,
                    })
                {
                    total_amount_2 += amt;
                } else {
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
            }
            Argument::Result(command_ix) => match command_results.get(command_ix) {
                Some(CommandResult::SplitCoinAmounts(coin_type_, coin_amounts)) => {
                    if *coin_type_ != coin_type {
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                    for amt in coin_amounts {
                        total_amount_2 += amt;
                    }
                }
                Some(CommandResult::MergedCoin((coin_type_, amt))) => {
                    if *coin_type_ != coin_type {
                        reject_on(
                            core::file!(),
                            core::line!(),
                            SyscallError::NotSupported as u16,
                        )
                        .await
                    }
                    total_amount_2 += amt;
                }
                _ => {
                    reject_on(
                        core::file!(),
                        core::line!(),
                        SyscallError::NotSupported as u16,
                    )
                    .await
                }
            },
        }
    }

    // MergeCoins does an overwrite of existing coins
    match dest_coin {
        Argument::GasCoin => {
            *added_amount_to_gas_coin += total_amount_2;
        }
        Argument::Input(input_ix) => {
            inputs.insert(input_ix, InputValue::Object((coin_type, total_amount_2)));
        }
        Argument::Result(command_ix) => {
            command_results.insert(
                command_ix,
                CommandResult::MergedCoin((coin_type, total_amount_2)),
            );
        }
        Argument::NestedResult(command_ix, coin_ix) => {
            command_results
                .get_mut(&command_ix)
                .map(|result| match result {
                    CommandResult::SplitCoinAmounts(_, coin_amounts) => {
                        coin_amounts[coin_ix as usize] = total_amount_2;
                    }
                    _ => {}
                });
        }
    }
}

async fn handle_make_move_vec<OD: HasObjectData>(
    coins: ArrayVec<Argument, MERGE_COIN_ARRAY_LENGTH>,
    inputs: &mut BTreeMap<u16, InputValue>,
    coin_type: &mut Option<CoinType>,
    includes_gas_coin: &mut bool,
    object_data_source: OD,
    command_results: &BTreeMap<u16, CommandResult>,
) -> CommandResult {
    let mut total_amount_2: u64 = 0;
    for coin in &coins {
        get_coin_amount(
            coin,
            inputs,
            &mut total_amount_2,
            coin_type,
            includes_gas_coin,
            &object_data_source,
            command_results,
        )
        .await;
    }
    CommandResult::MoveVecMergedCoin((
        match coin_type {
            Some(v) => v.clone(),
            _ => {
                reject_on(
                    core::file!(),
                    core::line!(),
                    SyscallError::NotSupported as u16,
                )
                .await
            }
        },
        total_amount_2,
    ))
}

pub struct TransactionKindParser<OD> {
    object_data_source: OD,
}

impl<OD> HasOutput<TransactionKindSchema> for TransactionKindParser<OD> {
    type Output =
        <ProgrammableTransactionParser<OD> as HasOutput<ProgrammableTransactionSchema>>::Output;
}

impl<BS: Clone + Readable, OD: Clone + HasObjectData> AsyncParser<TransactionKindSchema, BS>
    for TransactionKindParser<OD>
{
    type State<'c>
        = impl Future<Output = Self::Output> + 'c
    where
        BS: 'c, OD: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let enum_variant =
                <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
            match enum_variant {
                0 => {
                    info!("TransactionKind: ProgrammableTransaction");
                    <ProgrammableTransactionParser<OD> as AsyncParser<
                        ProgrammableTransactionSchema,
                        BS,
                    >>::parse(
                        &ProgrammableTransactionParser {
                            object_data_source: self.object_data_source.clone(),
                        },
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

pub type GasDataParserOutput = (ArrayVec<ObjectDigest, MAX_GAS_COIN_COUNT>, u64);

const fn gas_data_parser<BS: Clone + Readable>(
) -> impl AsyncParser<GasDataSchema, BS, Output = GasDataParserOutput> {
    Action(
        (
            SubInterp(object_ref_parser()),
            DefaultInterp,
            DefaultInterp,
            DefaultInterp,
        ),
        {
            |(coins, _sender, _gas_price, gas_budget): (_, _, u64, u64)| {
                // Gas price is per gas amount. Gas budget is total, reflecting the amount of gas *
                // gas price. We only care about the total, not the price or amount in isolation , so we
                // just ignore that field.
                //
                // C.F. https://github.com/MystenLabs/sui/pull/8676
                Some((coins, gas_budget))
            }
        },
    )
}

const fn object_ref_parser<BS: Readable>(
) -> impl AsyncParser<ObjectRefSchema, BS, Output = ObjectDigest> {
    Action(
        (DefaultInterp, DefaultInterp, DefaultInterp),
        |(_, _, d)| Some(d),
    )
}

const fn intent_parser<BS: Readable>() -> impl AsyncParser<Intent, BS, Output = ()> {
    Action((DefaultInterp, DefaultInterp, DefaultInterp), |_| {
        info!("Intent Ok");
        Some(())
    })
}

type TransactionDataV1Output<OD> = (
    <TransactionKindParser<OD> as HasOutput<TransactionKindSchema>>::Output,
    GasData,
);

pub struct TransactionDataParser<OD> {
    object_data_source: OD,
}

impl<OD> HasOutput<TransactionDataSchema> for TransactionDataParser<OD> {
    type Output = TransactionDataV1Output<OD>;
}

impl<BS: Clone + Readable, OD: Clone + HasObjectData> AsyncParser<TransactionDataSchema, BS>
    for TransactionDataParser<OD>
{
    type State<'c>
        = impl Future<Output = Self::Output> + 'c
    where
        BS: 'c, OD: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let enum_variant =
                <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
            match enum_variant {
                0 => {
                    info!("TransactionData: V1");
                    let v = (TransactionKindParser {
                        object_data_source: self.object_data_source.clone(),
                    })
                    .parse(input)
                    .await;

                    <DefaultInterp as AsyncParser<SuiAddress, BS>>::parse(&DefaultInterp, input)
                        .await;

                    let (gas_coins, gas_budget) = gas_data_parser().parse(input).await;

                    let mut total_gas_amount: Option<u64> = Some(0);
                    for digest in gas_coins {
                        if let Some(amt0) = total_gas_amount {
                            let coin_data = self.object_data_source.get_object_data(&digest).await;
                            match coin_data {
                                Some((_, amt)) => total_gas_amount = Some(amt0 + amt),
                                _ => total_gas_amount = None,
                            }
                        }
                    }

                    <DefaultInterp as AsyncParser<TransactionExpiration, BS>>::parse(
                        &DefaultInterp,
                        input,
                    )
                    .await;

                    (v, (gas_budget, total_gas_amount))
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

pub enum KnownTx {
    TransferTx {
        recipient: SuiAddressRaw,
        coin_type: CoinType,
        total_amount: u64,
        gas_budget: u64,
    },
    StakeTx {
        recipient: SuiAddressRaw,
        total_amount: u64,
        gas_budget: u64,
    },
    UnstakeTx {
        total_amount: u64,
        gas_budget: u64,
    },
}

use ledger_crypto_helpers::common::HexSlice;

pub const fn tx_parser<BS: Clone + Readable, OD: Clone + HasObjectData>(
    object_data_source: OD,
) -> impl AsyncParser<IntentMessage, BS, Output = KnownTx> {
    Action(
        (
            intent_parser(),
            TransactionDataParser { object_data_source },
        ),
        |(_, d): (
            _,
            <TransactionDataParser<OD> as HasOutput<TransactionDataSchema>>::Output,
        )| {
            match d.0 {
                ProgrammableTransaction::TransferSuiTx {
                    recipient,
                    amount,
                    includes_gas_coin,
                } => {
                    let (gas_budget, gas_coin_amount) = d.1;
                    let total_amount_ = if includes_gas_coin {
                        gas_coin_amount.map(|amt| amount + amt)
                    } else {
                        Some(amount)
                    };

                    total_amount_.map(|total_amount| KnownTx::TransferTx {
                        recipient,
                        coin_type: SUI_COIN_TYPE,
                        total_amount,
                        gas_budget,
                    })
                }
                ProgrammableTransaction::TransferTokenTx {
                    recipient,
                    amount,
                    coin_type,
                } => {
                    let (gas_budget, _) = d.1;
                    Some(KnownTx::TransferTx {
                        recipient,
                        coin_type,
                        total_amount: amount,
                        gas_budget,
                    })
                }
                ProgrammableTransaction::StakeTx {
                    recipient,
                    amount,
                    includes_gas_coin,
                } => {
                    let (gas_budget, gas_coin_amount) = d.1;
                    let total_amount_ = if includes_gas_coin {
                        gas_coin_amount.map(|amt| amount + amt)
                    } else {
                        Some(amount)
                    };

                    info!("StakeTx: {}, {}", HexSlice(&recipient), amount);
                    total_amount_.map(|total_amount| KnownTx::StakeTx {
                        recipient,
                        total_amount,
                        gas_budget,
                    })
                }
                ProgrammableTransaction::UnstakeTx { total_amount } => {
                    info!("UnstakeTx: {}", total_amount);
                    let (gas_budget, _) = d.1;
                    Some(KnownTx::UnstakeTx {
                        total_amount,
                        gas_budget,
                    })
                }
            }
        },
    )
}
