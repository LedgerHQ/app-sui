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
    ObjectRef(ObjectDigest),
    Other,
}

// Inputs which are referenced in computation of commands
pub enum InputValue {
    RecipientAddress(SuiAddressRaw),
    Amount(u64),
    ObjectRef(ObjectDigest),
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

pub const TRANSFER_OBJECT_ARRAY_LENGTH: usize = 8;
pub const SPLIT_COIN_ARRAY_LENGTH: usize = 8;
pub const MERGE_COIN_ARRAY_LENGTH: usize = 8;

pub enum Command {
    TransferObject(ArrayVec<Argument, TRANSFER_OBJECT_ARRAY_LENGTH>, Argument),
    SplitCoins(Argument, ArrayVec<Argument, SPLIT_COIN_ARRAY_LENGTH>),
    MergeCoins(Argument, ArrayVec<Argument, MERGE_COIN_ARRAY_LENGTH>),
}

pub enum CommandResult {
    SplitCoinAmounts(CoinID, ArrayVec<u64, SPLIT_COIN_ARRAY_LENGTH>),
    MergedCoin(CoinData),
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

pub struct ProgrammableTransactionParser<OD> {
    object_data_source: OD,
}

pub enum ProgrammableTransaction {
    TransferSuiTx {
        recipient: <DefaultInterp as HasOutput<Recipient>>::Output,
        amount: <DefaultInterp as HasOutput<Amount>>::Output,
        includes_gas_coin: bool,
    },
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
                        CallArg::Other => {}
                        CallArg::RecipientAddress(v) => {
                            inputs.insert(i, InputValue::RecipientAddress(v));
                        }
                        CallArg::Amount(v) => {
                            inputs.insert(i, InputValue::Amount(v));
                        }
                        CallArg::ObjectRef(v) => {
                            inputs.insert(i, InputValue::ObjectRef(v));
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

            let mut command_results: BTreeMap<u16, CommandResult> = BTreeMap::new();
            let mut recipient_addr = None;

            // Total amount, that we know of, being transferred to recipient
            // Does not contain amount from GasCoin, in case the entire GasCoin is also being transferred
            let mut total_amount: u64 = 0;

            // Are we transferring entire gas coin?
            let mut includes_gas_coin: bool = false;

            // Amount added to GasCoin via MergeCoins
            // as we don't know the GasCoin coin balance, we only track the any additions here
            let mut added_amount_to_gas_coin: u64 = 0;

            // Handle commands
            {
                let length_u32 =
                    <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
                let length = u16::try_from(length_u32).expect("u16 expected");
                info!("ProgrammableTransaction: Commands: {}", length);
                for command_ix in 0..length {
                    let c = <DefaultInterp as AsyncParser<CommandSchema, BS>>::parse(
                        &DefaultInterp,
                        input,
                    )
                    .await;
                    match c {
                        Command::TransferObject(coins, recipient_input) => {
                            match recipient_input {
                                Argument::Input(inp_index) => match inputs.get(&inp_index) {
                                    Some(InputValue::RecipientAddress(addr)) => {
                                        match recipient_addr {
                                            Some(addr_) => {
                                                if *addr != addr_ {
                                                    info!("TransferObject multiple recipients");
                                                    reject_on::<()>(
                                                        core::file!(),
                                                        core::line!(),
                                                        SyscallError::NotSupported as u16,
                                                    )
                                                    .await;
                                                }
                                            }
                                            None => recipient_addr = Some(addr.clone()),
                                        }
                                    }
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
                            let mut coin_id: Option<CoinID> = None;
                            // set total_amount
                            for coin in &coins {
                                match coin {
                                    Argument::GasCoin => {
                                        if let Some(id_) = coin_id {
                                            if id_ != SUI_COIN_ID {
                                                info!("TransferObject mismatch in coin_id(s)");
                                                reject_on(
                                                    core::file!(),
                                                    core::line!(),
                                                    SyscallError::NotSupported as u16,
                                                )
                                                .await
                                            } else {
                                                coin_id = Some(SUI_COIN_ID);
                                            }
                                        }
                                        includes_gas_coin = true;
                                    }
                                    Argument::Input(input_ix) => match inputs.get(input_ix) {
                                        Some(InputValue::ObjectRef(digest)) => {
                                            info!("TransferObject trying object_data_source");
                                            let coin_data = self
                                                .object_data_source
                                                .get_object_data(&digest)
                                                .await;
                                            match coin_data {
                                                Some((_, amt)) => total_amount += amt,
                                                _ => {
                                                    info!("TransferObject Coin Object not found");
                                                    reject_on(
                                                        core::file!(),
                                                        core::line!(),
                                                        SyscallError::NotSupported as u16,
                                                    )
                                                    .await
                                                }
                                            }
                                        }
                                        Some(InputValue::Object((id, amt))) => {
                                            if let Some(id_) = coin_id {
                                                if id_ != *id {
                                                    info!("TransferObject mismatch in coin_id(s)");
                                                    reject_on(
                                                        core::file!(),
                                                        core::line!(),
                                                        SyscallError::NotSupported as u16,
                                                    )
                                                    .await
                                                } else {
                                                    coin_id = Some(*id);
                                                }
                                            }
                                            total_amount += amt;
                                        }
                                        Some(InputValue::Amount(_)) => {
                                            info!("TransferObject input refers to non ObjectRef");
                                            reject_on(
                                                core::file!(),
                                                core::line!(),
                                                SyscallError::NotSupported as u16,
                                            )
                                            .await
                                        }
                                        Some(InputValue::RecipientAddress(_)) => {
                                            info!("TransferObject input refers to non ObjectRef");
                                            reject_on(
                                                core::file!(),
                                                core::line!(),
                                                SyscallError::NotSupported as u16,
                                            )
                                            .await
                                        }
                                        None => {
                                            info!("TransferObject input not found");
                                            reject_on(
                                                core::file!(),
                                                core::line!(),
                                                SyscallError::NotSupported as u16,
                                            )
                                            .await
                                        }
                                    },
                                    Argument::NestedResult(command_ix, coin_ix) => {
                                        match command_results.get(command_ix) {
                                            Some(CommandResult::SplitCoinAmounts(
                                                id,
                                                coin_amounts,
                                            )) => {
                                                if let Some(id_) = coin_id {
                                                    if id_ != *id {
                                                        info!(
                                                            "TransferObject mismatch in coin_id(s)"
                                                        );
                                                        reject_on(
                                                            core::file!(),
                                                            core::line!(),
                                                            SyscallError::NotSupported as u16,
                                                        )
                                                        .await
                                                    } else {
                                                        coin_id = Some(*id);
                                                    }
                                                }
                                                if let Some(amt) =
                                                    coin_amounts.get(*coin_ix as usize)
                                                {
                                                    total_amount += amt;
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
                                    Argument::Result(command_ix) => {
                                        match command_results.get(command_ix) {
                                            Some(CommandResult::MergedCoin((id, amt))) => {
                                                if let Some(id_) = coin_id {
                                                    if id_ != *id {
                                                        info!(
                                                            "TransferObject mismatch in coin_id(s)"
                                                        );
                                                        reject_on(
                                                            core::file!(),
                                                            core::line!(),
                                                            SyscallError::NotSupported as u16,
                                                        )
                                                        .await
                                                    } else {
                                                        coin_id = Some(*id);
                                                    }
                                                }
                                                total_amount += amt;
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
                        Command::SplitCoins(coin, amounts) => {
                            let coin_id = match coin {
                                Argument::GasCoin => SUI_COIN_ID,
                                _ => {
                                    reject_on(
                                        core::file!(),
                                        core::line!(),
                                        SyscallError::NotSupported as u16,
                                    )
                                    .await
                                }
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
                            command_results.insert(
                                command_ix,
                                CommandResult::SplitCoinAmounts(coin_id, coin_amounts),
                            );
                        }
                        Command::MergeCoins(dest_coin, coins) => {
                            let mut total_amount_2: u64 = 0;
                            let coin_id = match dest_coin {
                                Argument::GasCoin => SUI_COIN_ID,
                                Argument::Input(input_ix) => match inputs.get(&input_ix) {
                                    Some(InputValue::ObjectRef(digest)) => {
                                        info!("MergeCoins trying object_data_source");
                                        let coin_data =
                                            self.object_data_source.get_object_data(&digest).await;
                                        match coin_data {
                                            Some((id, amt)) => {
                                                total_amount_2 += amt;
                                                id
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
                                    Some(InputValue::Object((id, amt))) => {
                                        total_amount_2 += amt;
                                        *id
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
                                _ => {
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
                                            let coin_data = self
                                                .object_data_source
                                                .get_object_data(&digest)
                                                .await;
                                            match coin_data {
                                                Some((id, amt)) => {
                                                    if id != coin_id {
                                                        info!("MergeCoins mismatch in coin_id(s)");
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
                                        Some(InputValue::Object((id, amt))) => {
                                            if *id != coin_id {
                                                info!("MergeCoins mismatch in coin_id(s)");
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
                                        if let Some(amt) =
                                            command_results.get(command_ix).and_then(|result| {
                                                match result {
                                                    CommandResult::SplitCoinAmounts(
                                                        id,
                                                        coin_amounts,
                                                    ) => {
                                                        if *id != coin_id {
                                                            None
                                                        } else {
                                                            coin_amounts.get(*coin_ix as usize)
                                                        }
                                                    }
                                                    _ => None,
                                                }
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
                                    Argument::Result(command_ix) => {
                                        match command_results.get(command_ix) {
                                            Some(CommandResult::SplitCoinAmounts(
                                                id,
                                                coin_amounts,
                                            )) => {
                                                if *id != coin_id {
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
                                            Some(CommandResult::MergedCoin((id, amt))) => {
                                                if *id != coin_id {
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
                                        }
                                    }
                                }
                            }

                            // MergeCoins does an overwrite of existing coins
                            match dest_coin {
                                Argument::GasCoin => {
                                    added_amount_to_gas_coin += total_amount_2;
                                }
                                Argument::Input(input_ix) => {
                                    inputs.insert(
                                        input_ix,
                                        InputValue::Object((coin_id, total_amount_2)),
                                    );
                                }
                                Argument::Result(command_ix) => {
                                    command_results.insert(
                                        command_ix,
                                        CommandResult::MergedCoin((coin_id, total_amount_2)),
                                    );
                                }
                                Argument::NestedResult(command_ix, coin_ix) => {
                                    command_results.get_mut(&command_ix).map(
                                        |result| match result {
                                            CommandResult::SplitCoinAmounts(_, coin_amounts) => {
                                                coin_amounts[coin_ix as usize] = total_amount_2;
                                            }
                                            _ => {}
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
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

const fn gas_data_parser<BS: Clone + Readable, OD: Clone + HasObjectData>(
    object_data_source: OD,
) -> impl AsyncParser<GasDataSchema, BS, Output = GasData> {
    FutAction(
        (
            SubInterp(object_ref_parser()),
            DefaultInterp,
            DefaultInterp,
            DefaultInterp,
        ),
        {
            move |(coins, _sender, _gas_price, gas_budget): (_, _, u64, u64)| {
                let object_data_source = object_data_source.clone();
                async move {
                    let mut total_amount: Option<u64> = Some(0);
                    for digest in coins {
                        if let Some(amt0) = total_amount {
                            let coin_data = object_data_source.get_object_data(&digest).await;
                            match coin_data {
                                Some((_, amt)) => total_amount = Some(amt0 + amt),
                                _ => total_amount = None,
                            }
                        }
                    }
                    // Gas price is per gas amount. Gas budget is total, reflecting the amount of gas *
                    // gas price. We only care about the total, not the price or amount in isolation , so we
                    // just ignore that field.
                    //
                    // C.F. https://github.com/MystenLabs/sui/pull/8676
                    Some((gas_budget, total_amount))
                }
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

const fn transaction_data_v1_parser<BS: Clone + Readable, OD: Clone + HasObjectData>(
    object_data_source: OD,
    object_data_source2: OD,
) -> impl AsyncParser<TransactionDataV1, BS, Output = TransactionDataV1Output<OD>> {
    Action(
        (
            TransactionKindParser {
                object_data_source: object_data_source2,
            },
            DefaultInterp,
            gas_data_parser(object_data_source),
            DefaultInterp,
        ),
        |(v, _, gas_budget, _)| Some((v, gas_budget)),
    )
}

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
                    transaction_data_v1_parser(
                        self.object_data_source.clone(),
                        self.object_data_source.clone(),
                    )
                    .parse(input)
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

pub const fn tx_parser<BS: Clone + Readable, OD: Clone + HasObjectData>(
    object_data_source: OD,
) -> impl AsyncParser<
    IntentMessage,
    BS,
    Output = <TransactionDataParser<OD> as HasOutput<TransactionDataSchema>>::Output,
> {
    Action(
        (
            intent_parser(),
            TransactionDataParser { object_data_source },
        ),
        |(_, d)| Some(d),
    )
}
