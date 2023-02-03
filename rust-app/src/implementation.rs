use crate::interface::*;
use crate::test_parsers::*;
use crate::utils::*;
use alamgu_async_block::*;
use arrayvec::ArrayVec;
use core::fmt::Write;
use ledger_crypto_helpers::common::{try_option, Address, HexSlice};
use ledger_crypto_helpers::ed25519::Ed25519;
use ledger_crypto_helpers::eddsa::{ed25519_public_key_bytes, with_public_keys};
use ledger_crypto_helpers::hasher::{Hasher, SHA3_256};
use ledger_log::trace;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::bcs::async_parser::*;
use ledger_parser_combinators::interp::*;
use ledger_prompts_ui::final_accept_prompt;
use nanos_sdk::io::SyscallError;

use core::convert::TryFrom;
use core::future::Future;

type SuiAddressRaw = [u8; 20];

pub struct SuiPubKeyAddress(nanos_sdk::ecc::ECPublicKey<65, 'E'>, SuiAddressRaw);

impl Address<SuiPubKeyAddress, nanos_sdk::ecc::ECPublicKey<65, 'E'>> for SuiPubKeyAddress {
    fn get_address(key: &nanos_sdk::ecc::ECPublicKey<65, 'E'>) -> Result<Self, SyscallError> {
        let key_bytes = ed25519_public_key_bytes(key);
        let mut tmp = ArrayVec::<u8, 33>::new();
        let _ = tmp.try_push(0); // SIGNATURE_SCHEME_TO_FLAG['ED25519']
        let _ = tmp.try_extend_from_slice(key_bytes);
        let mut hasher: SHA3_256 = Hasher::new();
        hasher.update(&tmp);
        let hash: [u8; 32] = hasher.finalize();
        let mut address: SuiAddressRaw = [0; 20];
        address.clone_from_slice(&hash[0..20]);
        Ok(SuiPubKeyAddress(key.clone(), address))
    }
    fn get_binary_address(&self) -> &[u8] {
        &self.1
    }
}

impl core::fmt::Display for SuiPubKeyAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{}", HexSlice(&self.1))
    }
}

pub type BipParserImplT =
    impl AsyncParser<Bip32Key, ByteStream> + HasOutput<Bip32Key, Output = ArrayVec<u32, 10>>;
pub const BIP_PATH_PARSER: BipParserImplT = SubInterp(DefaultInterp);

pub async fn get_address_apdu(io: HostIO) {
    let input = io.get_params::<1>().unwrap();

    let path = BIP_PATH_PARSER.parse(&mut input[0].clone()).await;

    let mut rv = ArrayVec::<u8, 220>::new();

    if with_public_keys(&path, |key, address: &SuiPubKeyAddress| {
        try_option(|| -> Option<()> {
            let key_bytes = ed25519_public_key_bytes(key);

            scroller("Provide Public Key", |w| {
                Ok(write!(w, "For Address {address}")?)
            })?;

            final_accept_prompt(&[])?;

            rv.try_push(u8::try_from(key_bytes.len()).ok()?).ok()?;
            rv.try_extend_from_slice(key_bytes).ok()?;

            // And we'll send the address along;
            let binary_address = address.get_binary_address();
            rv.try_push(u8::try_from(binary_address.len()).ok()?).ok()?;
            rv.try_extend_from_slice(binary_address).ok()?;
            Some(())
        }())
    })
    .ok()
    .is_none()
    {
        reject::<()>().await;
    }

    io.result_final(&rv).await;
}

impl HasOutput<SingleTransactionKind> for SingleTransactionKind {
    type Output = ();
}

impl<BS: Clone + Readable> AsyncParser<SingleTransactionKind, BS> for SingleTransactionKind {
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let enum_variant =
                <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
            match enum_variant {
                // PaySui
                5 => {
                    trace!("SingleTransactionKind: PaySui");
                    pay_sui_parser().parse(input).await;
                }
                _ => reject_on(core::file!(), core::line!()).await,
            }
        }
    }
}

impl HasOutput<TransactionKind> for TransactionKind {
    type Output = ();
}

impl<BS: Clone + Readable> AsyncParser<TransactionKind, BS> for TransactionKind {
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let enum_variant =
                <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
            match enum_variant {
                0 => {
                    trace!("TransactionKind: Single");
                    <SingleTransactionKind as AsyncParser<SingleTransactionKind, BS>>::parse(
                        &SingleTransactionKind,
                        input,
                    )
                    .await;
                }
                _ => reject_on(core::file!(), core::line!()).await,
            }
        }
    }
}

const fn pay_sui_parser<BS: Clone + Readable>(
) -> impl AsyncParser<PaySui, BS> + HasOutput<PaySui, Output = ()> {
    Action(
        (SubInterp(coin_parser()), RecipientsAndAmounts),
        |(_, _): (_, _)| {
            trace!("PaySui Ok");
            Some(())
        },
    )
}

impl HasOutput<RecipientsAndAmounts> for RecipientsAndAmounts {
    type Output = ();
}

impl<BS: Clone + Readable> AsyncParser<RecipientsAndAmounts, BS> for RecipientsAndAmounts {
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS) -> Self::State<'c> {
        async move {
            let length =
                <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, input).await;
            trace!("RecipientsAndAmounts length: {}", length);
            let mut amt_bs = input.clone();

            for _ in 0..length {
                <DefaultInterp as AsyncParser<Recipient, BS>>::parse(&DefaultInterp, &mut amt_bs)
                    .await;
            }

            let length_amt =
                <DefaultInterp as AsyncParser<ULEB128, BS>>::parse(&DefaultInterp, &mut amt_bs)
                    .await;
            if length != length_amt {
                trace!(
                    "RecipientsAndAmounts length != length_amt: {}, {}",
                    length,
                    length_amt
                );
                reject::<()>().await;
            }
            for _ in 0..length {
                let recipient =
                    <DefaultInterp as AsyncParser<Recipient, BS>>::parse(&DefaultInterp, input)
                        .await;
                let amount =
                    <DefaultInterp as AsyncParser<Amount, BS>>::parse(&DefaultInterp, &mut amt_bs)
                        .await;
                if scroller("Transfer", |w| {
                    Ok(write!(w, "{amount} to 0x{}", HexSlice(&recipient))?)
                })
                .is_none()
                {
                    reject::<()>().await;
                }
            }
            *input = amt_bs;
        }
    }
}

const fn coin_parser<BS: Readable>(
) -> impl AsyncParser<ObjectRef, BS> + HasOutput<ObjectRef, Output = ()> {
    Action(
        (DefaultInterp, DefaultInterp, DefaultInterp),
        |(_obj_id, _seq, _obj_dig): (SuiAddressRaw, u64, [u8; 33])| {
            trace!(
                "Coin Ok {}, {}, {}",
                HexSlice(_obj_id.as_ref()),
                _seq,
                ledger_crypto_helpers::hasher::Base64Hash(_obj_dig)
            );
            Some(())
        },
    )
}

const fn object_ref_parser<BS: Readable>(
) -> impl AsyncParser<ObjectRef, BS> + HasOutput<ObjectRef, Output = ()> {
    Action((DefaultInterp, DefaultInterp, DefaultInterp), |_| Some(()))
}

const fn intent_parser<BS: Readable>(
) -> impl AsyncParser<Intent, BS> + HasOutput<Intent, Output = ()> {
    Action((DefaultInterp, DefaultInterp, DefaultInterp), |_| {
        trace!("Intent Ok");
        Some(())
    })
}

const fn transaction_data_parser<BS: Clone + Readable>(
) -> impl AsyncParser<TransactionData, BS> + HasOutput<TransactionData, Output = ()> {
    Action(
        (
            TransactionKind,
            DefaultInterp,
            object_ref_parser(),
            DefaultInterp,
            DefaultInterp,
        ),
        |(_, _sender, _, gas_price, gas_budget): (_, _, _, u64, u64)| {
            scroller("Gas", |w| {
                Ok(write!(w, "Price: {}, Budget: {}", gas_price, gas_budget)?)
            })
        },
    )
}

const fn tx_parser<BS: Clone + Readable>(
) -> impl AsyncParser<IntentMessage, BS> + HasOutput<IntentMessage, Output = ()> {
    Action((intent_parser(), transaction_data_parser()), |_| Some(()))
}

pub async fn sign_apdu(io: HostIO) {
    let mut input = io.get_params::<2>().unwrap();

    // Read length, and move input[0] by one byte
    let length = usize::from_le_bytes(input[0].read().await);

    NoinlineFut((|mut txn: ByteStream| async move {
        {
            trace!("Beginning parse");
            tx_parser().parse(&mut txn).await;
        }
    })(input[0].clone()))
    .await;

    NoinlineFut((|mut bs: ByteStream| async move {
        let path = BIP_PATH_PARSER.parse(&mut bs).await;
        if with_public_keys(&path, |_, address: &SuiPubKeyAddress| {
            try_option(|| -> Option<()> {
                scroller("Sign for Address", |w| Ok(write!(w, "{address}")?))?;
                final_accept_prompt(&["Sign Transaction?"])?;
                Some(())
            }())
        })
        .ok()
        .is_none()
        {
            reject::<()>().await;
        }
    })(input[1].clone()))
    .await;

    // By the time we get here, we've approved and just need to do the signature.
    NoinlineFut((|input: ArrayVec<ByteStream, 2>| async move {
        let mut ed = {
            let path = BIP_PATH_PARSER.parse(&mut input[1].clone()).await;
            match Ed25519::new(path).ok() {
                Some(ed) => ed,
                _ => reject().await,
            }
        };

        trace!("doing final");
        {
            let mut txn = input[0].clone();
            for _ in 0..length {
                let b: [u8; 1] = txn.read().await;
                ed.update(&b);
            }
        }
        if ed.done_with_r().ok().is_none() {
            reject::<()>().await;
        }
        {
            let mut txn = input[0].clone();
            for _ in 0..length {
                let b: [u8; 1] = txn.read().await;
                ed.update(&b);
            }
        }
        if let Some(sig) = { ed.finalize().ok() } {
            io.result_final(&sig.0[0..]).await;
        } else {
            reject::<()>().await;
        }
    })(input))
    .await
}

pub type APDUsFuture = impl Future<Output = ()>;

#[inline(never)]
pub fn handle_apdu_async(io: HostIO, ins: Ins) -> APDUsFuture {
    trace!("Constructing future");
    async move {
        trace!("Dispatching");
        match ins {
            Ins::GetVersion => {
                const APP_NAME: &str = "sui";
                let mut rv = ArrayVec::<u8, 220>::new();
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap());
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_MINOR").parse().unwrap());
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_PATCH").parse().unwrap());
                let _ = rv.try_extend_from_slice(APP_NAME.as_bytes());
                io.result_final(&rv).await;
            }
            Ins::GetPubkey => {
                NoinlineFut(get_address_apdu(io)).await;
            }
            Ins::Sign => {
                trace!("Handling sign");
                NoinlineFut(sign_apdu(io)).await;
            }
            Ins::TestParsers => {
                NoinlineFut(test_parsers(io)).await;
            }
            Ins::GetVersionStr => {}
            Ins::Exit => nanos_sdk::exit_app(0),
        }
    }
}
