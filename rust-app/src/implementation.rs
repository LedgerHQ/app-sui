use crate::ctx::RunCtx;
use crate::interface::*;
use crate::parser::tx::{tx_parser, ProgrammableTransaction};
use crate::settings::*;
use crate::swap;
use crate::swap::params::TxParams;
use crate::ui::*;
use crate::utils::*;
use alamgu_async_block::*;
use arrayvec::ArrayVec;
use ledger_crypto_helpers::common::{try_option, Address};
use ledger_crypto_helpers::eddsa::{ed25519_public_key_bytes, eddsa_sign, with_public_keys};
use ledger_crypto_helpers::hasher::{Blake2b, Hasher, HexHash};
use ledger_device_sdk::io::{StatusWords, SyscallError};
use ledger_log::trace;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::interp::*;

use core::convert::TryFrom;

pub type BipParserImplT = impl AsyncParser<Bip32Key, ByteStream, Output = ArrayVec<u32, 10>>;
pub const BIP_PATH_PARSER: BipParserImplT = SubInterp(DefaultInterp);

// Need a path of length 5, as make_bip32_path panics with smaller paths
pub const BIP32_PREFIX: [u32; 5] =
    ledger_device_sdk::ecc::make_bip32_path(b"m/44'/784'/123'/0'/0'");

pub async fn get_address_apdu(io: HostIO, ui: UserInterface, prompt: bool) {
    let input = match io.get_params::<1>() {
        Some(v) => v,
        None => reject(SyscallError::InvalidParameter as u16).await,
    };

    let path = BIP_PATH_PARSER.parse(&mut input[0].clone()).await;

    if !path.starts_with(&BIP32_PREFIX[0..2]) {
        reject::<()>(SyscallError::InvalidParameter as u16).await;
    }

    let mut rv = ArrayVec::<u8, 220>::new();

    if with_public_keys(&path, true, |key, address: &SuiPubKeyAddress| {
        try_option(|| -> Option<()> {
            if prompt {
                ui.confirm_address(address)?;
            }

            let key_bytes = ed25519_public_key_bytes(key);

            rv.try_push(u8::try_from(key_bytes.len()).ok()?).ok()?;
            rv.try_extend_from_slice(key_bytes).ok()?;

            // And we'll send the address along;
            let binary_address = address.get_binary_address();
            rv.try_push(u8::try_from(binary_address.len()).ok()?).ok()?;
            rv.try_extend_from_slice(binary_address).ok()?;
            Some(())
        }())
    })
    .is_err()
    {
        reject::<()>(StatusWords::UserCancelled as u16).await;
    }

    io.result_final(&rv).await;
}

async fn prompt_tx_params(
    ui: &UserInterface,
    path: &[u32],
    TxParams {
        amount,
        fee,
        destination_address,
    }: TxParams,
) {
    if with_public_keys(path, true, |_, address: &SuiPubKeyAddress| {
        try_option(ui.confirm_sign_tx(address, destination_address, amount, fee))
    })
    .ok()
    .is_none()
    {
        reject::<()>(StatusWords::UserCancelled as u16).await;
    };
}
async fn check_tx_params(expected: &TxParams, received: &TxParams) {
    if !swap::check_tx_params(expected, received) {
        reject::<()>(SW_SWAP_TX_PARAM_MISMATCH).await;
    }
}

pub async fn sign_apdu(io: HostIO, ctx: &RunCtx, settings: Settings, ui: UserInterface) {
    let _on_failure = defer::defer(|| {
        // In case of a swap, we need to communicate that signing failed
        if ctx.is_swap() && !ctx.is_swap_sign_succeeded() {
            ctx.set_swap_sign_failure();
        }
    });

    let mut input = match io.get_params::<2>() {
        Some(v) => v,
        None => reject(SyscallError::InvalidParameter as u16).await,
    };

    // Read length, and move input[0] by one byte
    let length = usize::from_le_bytes(input[0].read().await);

    let known_txn = {
        let mut txn = input[0].clone();
        NoinlineFut(async move {
            trace!("Beginning check parse");
            TryFuture(tx_parser().parse(&mut txn)).await.is_some()
        })
        .await
    };

    if known_txn {
        let mut txn = input[0].clone();
        let (
            ProgrammableTransaction::TransferSuiTx {
                recipient,
                amount: total_amount,
            },
            gas_budget,
        ) = tx_parser().parse(&mut txn).await;

        let mut bs = input[1].clone();
        let path = BIP_PATH_PARSER.parse(&mut bs).await;
        if !path.starts_with(&BIP32_PREFIX[0..2]) {
            reject::<()>(SyscallError::InvalidParameter as u16).await;
        }

        let tx_params = TxParams {
            amount: total_amount,
            fee: gas_budget,
            destination_address: recipient,
        };

        if ctx.is_swap() {
            let expected = ctx.get_swap_tx_params();
            check_tx_params(expected, &tx_params).await;
        } else {
            // Show prompts after all inputs have been parsed
            prompt_tx_params(&ui, path.as_slice(), tx_params).await;
        }
    } else if !settings.get_blind_sign() || ctx.is_swap() {
        ui.warn_tx_not_recognized();
        reject::<()>(SyscallError::NotSupported as u16).await;
    }

    NoinlineFut(async move {
        let mut hasher: Blake2b = Hasher::new();
        {
            let mut txn = input[0].clone();
            const CHUNK_SIZE: usize = 128;
            let (chunks, rem) = (length / CHUNK_SIZE, length % CHUNK_SIZE);
            for _ in 0..chunks {
                let b: [u8; CHUNK_SIZE] = txn.read().await;
                hasher.update(&b);
            }
            for _ in 0..rem {
                let b: [u8; 1] = txn.read().await;
                hasher.update(&b);
            }
        }
        let hash: HexHash<32> = hasher.finalize();
        if !known_txn {
            // Show prompts after all inputs have been parsed
            if ui.confirm_blind_sign_tx(&hash).is_none() {
                reject::<()>(StatusWords::UserCancelled as u16).await;
            };
        }
        let path = BIP_PATH_PARSER.parse(&mut input[1].clone()).await;
        if !path.starts_with(&BIP32_PREFIX[0..2]) {
            reject::<()>(SyscallError::InvalidParameter as u16).await;
        }
        if let Some(sig) = { eddsa_sign(&path, true, &hash.0).ok() } {
            io.result_final(&sig.0[0..]).await;
        } else {
            reject::<()>(SyscallError::Unspecified as u16).await;
        }
    })
    .await;

    // Does nothing if not a swap mode
    ctx.set_swap_sign_success();
}
