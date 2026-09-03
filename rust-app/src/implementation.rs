use crate::crypto_helpers::common::{try_option, Address};
use crate::crypto_helpers::eddsa::{ed25519_public_key_bytes, eddsa_sign, with_public_keys};
use crate::crypto_helpers::hasher::HexHash;
use crate::ctx::{RunCtx, TICKER_LENGTH};
use crate::interface::*;
use crate::parser::common::{
    HasObjectData, ObjectData, ObjectDigest, SuiAddressRaw, COIN_STRING_LENGTH,
};
use crate::parser::object::{compute_object_hash, object_parser};
use crate::parser::tuid::{parse_tuid, Tuid};
use crate::parser::tx::{tx_parser, KnownTx, TransactionExpirationVariant, TxPrincipals};
use crate::settings::*;
use crate::swap;
use crate::swap::params::TxParams;
use crate::ui::common::{ReplayDomain, StakeParams};
use crate::ui::*;
use crate::utils::*;
use alamgu_async_block::*;
use arrayvec::{ArrayString, ArrayVec};
use ledger_device_sdk::hash::HashInit;
use ledger_device_sdk::io::{StatusWords, SyscallError};
use ledger_device_sdk::log::{info, trace};
use ledger_device_sdk::tlv::tlv_dynamic_token::{parse_dynamic_token_tlv, DynamicTokenOut};
use ledger_device_sdk::tlv::TlvError;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::interp::*;
use ledger_parser_combinators::schema::*;

use crate::crypto_helpers::common::HexSlice;

use core::convert::TryFrom;
use core::future::Future;

// Payload for a public key request
pub type Bip32Key = DArray<Byte, U32<{ Endianness::Little }>, 10>;

pub type BipParserImplT = impl AsyncParser<Bip32Key, ByteStream, Output = ArrayVec<u32, 10>>;
#[define_opaque(BipParserImplT)]
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

/// The sender to disclose in the review, or `None` for an ordinary transaction.
///
/// Sui accepts a gas-owner signature as authorization in its own right, so this
/// device can be made to fund a transaction it did not send. `Argument::GasCoin`
/// then resolves to *this* device's coin while the transaction's effects accrue
/// to the sender, so the review has to name that sender rather than rendering
/// "From" off the signing path.
fn sponsored_sender(
    address: &SuiPubKeyAddress,
    principals: &TxPrincipals,
) -> Option<SuiAddressRaw> {
    let signer: SuiAddressRaw = address.get_binary_address().try_into().ok()?;
    principals.sponsored_sender_for(&signer)
}

/// The SIP-58 replay domain to show, when the transaction carries one.
fn replay_domain(expiration: TransactionExpirationVariant) -> Option<ReplayDomain> {
    match expiration {
        TransactionExpirationVariant::ValidDuring { chain, nonce } => {
            Some(ReplayDomain { chain, nonce })
        }
        _ => None,
    }
}

async fn prompt_tx_params(
    ui: &UserInterface,
    path: &[u32],
    tx_params: TxParams,
    principals: TxPrincipals,
    replay: Option<ReplayDomain>,
    ctx: &RunCtx,
) {
    if with_public_keys(path, true, |_, address: &SuiPubKeyAddress| {
        try_option(ui.confirm_sign_tx(
            address,
            &tx_params,
            sponsored_sender(address, &principals),
            replay,
            ctx,
        ))
    })
    .ok()
    .is_none()
    {
        reject::<()>(StatusWords::UserCancelled as u16).await;
    };
}
async fn check_tx_params(expected: &TxParams, received: &TxParams, ctx: &RunCtx) {
    if !swap::check_tx_params(expected, received, ctx) {
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

    let mut input = match io.get_params::<3>() {
        Some(v) => v,
        None => reject(SyscallError::InvalidParameter as u16).await,
    };

    info!("input length {}", input.len());

    // Read length, and move input[0] by one byte
    let length = usize::from_le_bytes(input[0].read().await);

    info!("apdu sign tx length: {}\n", length);

    let mut txn = LengthTrack(input[0].clone(), 0);
    let parsed = {
        let object_data_source = input.get(2).map(|bs| WithObjectData { bs: bs.clone() });
        let txn = &mut txn;
        NoinlineFut(async move {
            info!("Beginning tx_parse");
            TryFuture(tx_parser(object_data_source).parse(txn)).await
        })
        .await
    };
    // The reviewed parse must consume exactly the host-declared signed byte range,
    // or the display and the signed bytes can disagree (B2CA-2793 finding 2).
    // Treat any mismatch as an unrecognized tx, the same fail-safe already used
    // for parse ambiguity elsewhere. This check is deliberately kept outside the
    // async block above: folding it into that block's tail expression (rather
    // than leaving `TryFuture(...).await` itself as the tail) changes the
    // generated state machine's drop timing for the (heap-backed) parser state
    // nested within it, which was observed to trip embedded-alloc's reentrancy
    // guard ("RefCell already borrowed") specifically on nanox.
    let known_txn = if txn.index() == length {
        parsed
    } else {
        info!(
            "sign_apdu: reviewed parse consumed {} bytes, signed length is {}",
            txn.index(),
            length
        );
        None
    };

    info!("End of tx_parse");

    let is_unknown_txn = known_txn.is_none();

    // Kept alongside the recognized tx so every review branch can disclose a
    // sponsorship, and so swap can refuse one outright.
    let principals = known_txn.as_ref().map(|p| p.principals);
    let replay = known_txn.as_ref().and_then(|p| replay_domain(p.expiration));
    let known_txn = known_txn.map(|p| p.tx);

    match known_txn {
        Some(KnownTx::TransferTx {
            recipient,
            total_amount,
            coin_type,
            gas_budget,
            gas_from_address_balance,
            includes_gas_coin,
        }) => {
            info!("Known transfer tx\n");
            let mut bs = input[1].clone();
            let path = BIP_PATH_PARSER.parse(&mut bs).await;
            if !path.starts_with(&BIP32_PREFIX[0..2]) {
                reject::<()>(SyscallError::InvalidParameter as u16).await;
            }

            let tx_params = TxParams {
                amount: total_amount,
                fee: gas_budget,
                destination_address: recipient,
                coin_type,
                gas_from_address_balance,
                includes_gas_coin,
                ..Default::default()
            };

            let principals = match principals {
                Some(p) => p,
                None => reject(SyscallError::InvalidState as u16).await,
            };

            if ctx.is_swap() {
                // Swap signs with no review at all, so a sponsorship could never be
                // disclosed. The Exchange quote describes the user's own transfer,
                // never one they merely fund, so refuse rather than sign blind.
                if principals.is_sponsored() {
                    reject::<()>(SyscallError::NotSupported as u16).await;
                }
                let expected = ctx.get_swap_tx_params();
                check_tx_params(expected, &tx_params, ctx).await;
            } else {
                // Show prompts after all inputs have been parsed
                NoinlineFut(prompt_tx_params(
                    &ui,
                    path.as_slice(),
                    tx_params,
                    principals,
                    replay,
                    ctx,
                ))
                .await;
            }
        }
        Some(KnownTx::StakeTx {
            recipient,
            total_amount,
            gas_budget,
            gas_from_address_balance,
            includes_gas_coin,
        }) => {
            info!("Known stake tx\n");
            if ctx.is_swap() {
                reject::<()>(SyscallError::NotSupported as u16).await;
            }
            let mut bs = input[1].clone();
            let path = BIP_PATH_PARSER.parse(&mut bs).await;
            if !path.starts_with(&BIP32_PREFIX[0..2]) {
                reject::<()>(SyscallError::InvalidParameter as u16).await;
            }

            if with_public_keys(&path, true, |_, address: &SuiPubKeyAddress| {
                try_option(ui.confirm_stake_tx(
                    address,
                    &StakeParams {
                        recipient,
                        total_amount,
                        gas_budget,
                        gas_from_address_balance,
                        includes_gas_coin,
                    },
                    principals.and_then(|p| sponsored_sender(address, &p)),
                    replay,
                ))
            })
            .ok()
            .is_none()
            {
                reject::<()>(StatusWords::UserCancelled as u16).await;
            };
        }
        Some(KnownTx::UnstakeTx {
            total_amount,
            gas_budget,
            gas_from_address_balance,
        }) => {
            info!("Known unstake tx\n");
            if ctx.is_swap() {
                reject::<()>(SyscallError::NotSupported as u16).await;
            }
            let mut bs = input[1].clone();
            let path = BIP_PATH_PARSER.parse(&mut bs).await;
            if !path.starts_with(&BIP32_PREFIX[0..2]) {
                reject::<()>(SyscallError::InvalidParameter as u16).await;
            }

            if with_public_keys(&path, true, |_, address: &SuiPubKeyAddress| {
                try_option(ui.confirm_unstake_tx(
                    address,
                    total_amount,
                    gas_budget,
                    gas_from_address_balance,
                    principals.and_then(|p| sponsored_sender(address, &p)),
                    replay,
                ))
            })
            .ok()
            .is_none()
            {
                reject::<()>(StatusWords::UserCancelled as u16).await;
            };
        }
        None => {
            info!("Unknown tx\n");
            if ctx.is_swap() {
                // Reject unknown transactions in swap mode
                reject::<()>(SyscallError::NotSupported as u16).await;
            } else if !settings.get_blind_sign() {
                ui.warn_tx_not_recognized();
                reject::<()>(SyscallError::NotSupported as u16).await;
            }
        }
    }

    NoinlineFut(async move {
        let mut hasher = ledger_device_sdk::hash::blake2::Blake2b_256::new();
        {
            let mut txn = input[0].clone();
            const CHUNK_SIZE: usize = 128;
            let (chunks, rem) = (length / CHUNK_SIZE, length % CHUNK_SIZE);
            for _ in 0..chunks {
                let b: [u8; CHUNK_SIZE] = txn.read().await;
                let _ = hasher.update(&b);
            }
            for _ in 0..rem {
                let b: [u8; 1] = txn.read().await;
                let _ = hasher.update(&b);
            }
        }
        info!("sign_apdu: hash computed");
        let mut hash: HexHash<32> = Default::default();
        let _ = hasher.finalize(&mut hash.0);

        if is_unknown_txn {
            // Show prompts after all inputs have been parsed
            info!("sign_apdu: about to confirm_blind_sign_tx");
            if ui.confirm_blind_sign_tx(&hash).is_none() {
                info!("sign_apdu: confirm_blind_sign_tx rejected");
                reject::<()>(StatusWords::UserCancelled as u16).await;
            };
            info!("sign_apdu: confirm_blind_sign_tx approved");
        }
        let path = BIP_PATH_PARSER.parse(&mut input[1].clone()).await;
        if !path.starts_with(&BIP32_PREFIX[0..2]) {
            reject::<()>(SyscallError::InvalidParameter as u16).await;
        }
        info!("sign_apdu: about to eddsa_sign and result_final");
        if let Some(sig) = { eddsa_sign(&path, true, &hash.0).ok() } {
            io.result_final(&sig.0[0..]).await;
            info!("sign_apdu: result_final sent");
        } else {
            info!("sign_apdu: eddsa_sign failed");
            reject::<()>(SyscallError::Unspecified as u16).await;
        }
    })
    .await;

    // Does nothing if not a swap mode
    ctx.set_swap_sign_success();
}

pub async fn validate_tlv(io: HostIO, ctx: &RunCtx) {
    const TLV_ERROR_OFFSET: u16 = 0x7000;

    let mut input = match io.get_params::<4>() {
        Some(bs) => bs,
        None => reject(SyscallError::InvalidParameter as u16).await,
    };

    trace!("validate_tlv\n");

    let first: [u8; 2] = input[0].read().await;
    let length = u16::from_le_bytes(first);

    trace!("data length: {}\n", HexSlice(&first));

    let mut tlv = input[0].clone();

    let mut b_arr: ArrayVec<u8, 1024> = ArrayVec::new();

    const CHUNK_SIZE: usize = 10;
    let (chunks, rem) = (length as usize / CHUNK_SIZE, length as usize % CHUNK_SIZE);
    for _ in 0..chunks {
        let b: [u8; CHUNK_SIZE] = tlv.read().await;
        let _ = b_arr.try_extend_from_slice(&b);
    }

    for _ in 0..rem {
        let b: [u8; 1] = tlv.read().await;
        let _ = b_arr.try_extend_from_slice(&b);
    }

    let mut out = DynamicTokenOut::default();

    match parse_dynamic_token_tlv(b_arr.as_slice() as &[u8], &mut out) {
        Ok(()) => trace!("tlv parsing succeed\n"),
        Err(err) => {
            trace!("tlv parsing failed: {}\n", err as u8);
            trace!("tlv data: {}\n", HexSlice(&b_arr));
            reject::<()>(TLV_ERROR_OFFSET + err as u16).await;
            return;
        }
    };

    trace!("TUID: {}\n", HexSlice(&out.tuid));

    let mut tuid: Tuid = Default::default();
    match parse_tuid(&out.tuid, &mut tuid) {
        Ok(()) => trace!("tuid parsing succeed\n"),
        Err(err) => {
            trace!("Tuid parsing failed: {}\n", err as u8);
            reject::<()>(TLV_ERROR_OFFSET + err as u16).await;
            return;
        }
    };

    trace!(
        "token contract: \nPACKAGE ADDRESS - {}\nMODULE - {}\nSTRUCT - {}\n",
        HexSlice(&tuid.package_addr),
        tuid.module.as_str(),
        tuid.struct_name.as_str(),
    );

    let module: ArrayString<COIN_STRING_LENGTH> = match ArrayString::from(tuid.module.as_str()) {
        Ok(a) => a,
        Err(_err) => {
            trace!("Module parsing failed: {}\n", _err);
            reject::<()>(TLV_ERROR_OFFSET + TlvError::UnexpectedEof as u16).await;
            return;
        }
    };
    let function: ArrayString<COIN_STRING_LENGTH> =
        match ArrayString::from(tuid.struct_name.as_str()) {
            Ok(a) => a,
            Err(_err) => {
                trace!("Function parsing failed: {}\n", _err);
                reject::<()>(TLV_ERROR_OFFSET + TlvError::UnexpectedEof as u16).await;
                return;
            }
        };

    let ticker: ArrayString<TICKER_LENGTH> = match ArrayString::from(out.ticker.as_str()) {
        Ok(a) => a,
        Err(_err) => {
            trace!("Ticker parsing failed: {}\n", _err);
            reject::<()>(TLV_ERROR_OFFSET + TlvError::UnexpectedEof as u16).await;
            return;
        }
    };

    // Same bound as the swap coin config: a magnitude past MAX_COIN_DECIMALS makes
    // the amount rendering divide by a wrapped divisor. Descriptors are signed, so
    // this is a sanity check rather than a trust boundary, but the crash would be
    // the same.
    if out.magnitude > MAX_COIN_DECIMALS {
        trace!("Descriptor magnitude out of range: {}\n", out.magnitude);
        reject::<()>(TLV_ERROR_OFFSET + TlvError::UnexpectedEof as u16).await;
        return;
    }

    ctx.set_token(tuid.package_addr, module, function, out.magnitude, ticker);

    io.result_final(&[]).await;
}

#[derive(Clone)]
struct WithObjectData {
    bs: ByteStream,
}

impl HasObjectData for WithObjectData {
    type State<'c> = impl Future<Output = Option<ObjectData>> + 'c;

    fn get_object_data<'a: 'c, 'b: 'c, 'c>(&'b self, digest: &'a ObjectDigest) -> Self::State<'c> {
        async move {
            let mut bs = self.bs.clone();
            let objects_count: Option<usize> = TryFuture(bs.read()).await.map(usize::from_le_bytes);

            match objects_count {
                None => None,
                Some(0) => None,
                Some(c) => {
                    info!("get_object_data: objects_count {}", c);
                    for _ in 0..c {
                        let length = usize::from_le_bytes(bs.read().await);
                        let mut obj_start_bs = bs.clone();

                        let hash = NoinlineFut(compute_object_hash(&mut bs, length)).await;

                        if hash.0 == digest[1..33] {
                            info!(
                                "get_object_data: found object with digest {}",
                                HexSlice(digest)
                            );
                            // Found object, now try to parse
                            return NoinlineFut(TryFuture(
                                object_parser().parse(&mut obj_start_bs),
                            ))
                            .await;
                        }
                    }
                    info!(
                        "get_object_data: did not find object with digest {}",
                        HexSlice(digest)
                    );
                    None
                }
            }
        }
    }
}
