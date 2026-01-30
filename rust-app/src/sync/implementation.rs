use crate::crypto_helpers::common::{try_option, Address};
use crate::crypto_helpers::eddsa::{ed25519_public_key_bytes, eddsa_sign, with_public_keys};
use crate::crypto_helpers::hasher::HexHash;
use crate::interface::*;
use crate::parser::common::{
    CoinType, HasObjectData, ObjectData, ObjectDigest, COIN_STRING_LENGTH,
};
use crate::parser::object::{compute_object_hash, object_parser};
use crate::parser::tuid::{parse_tuid, Tuid};
use crate::parser::tx::{tx_parser, KnownTx};
use crate::settings::*;
use crate::sync::ctx::{RunCtx, TICKER_LENGTH};
use crate::sync::ui::nbgl::UserInterface;
use crate::utils::*;

use arrayvec::{ArrayString, ArrayVec};
use ledger_device_sdk::hash::HashInit;
use ledger_device_sdk::io::{StatusWords, SyscallError};
use ledger_device_sdk::log::{error, info, trace};

#[cfg(feature = "speculos")]
use crate::crypto_helpers::common::HexSlice;

use core::convert::TryFrom;

pub const BIP32_PREFIX: [u32; 2] = ledger_device_sdk::ecc::make_bip32_path(b"m/44'/784'");

struct Bip32Path(ArrayVec<u32, 10>);

impl TryFrom<&[u8]> for Bip32Path {
    type Error = StatusWords;

    fn try_from(bs: &[u8]) -> Result<Self, Self::Error> {
        let mut arr = ArrayVec::<u32, 10>::new();
        // First byte is length
        let mut offset = 1;
        while offset + 4 <= bs.len() {
            let chunk: [u8; 4] = bs[offset..offset + 4]
                .try_into()
                .map_err(|_| StatusWords::BadLen)?;
            arr.try_push(u32::from_le_bytes(chunk))
                .map_err(|_| StatusWords::BadLen)?;
            offset += 4;
        }
        if offset != bs.len() {
            return Err(StatusWords::BadLen);
        }
        Ok(Bip32Path(arr))
    }
}

pub fn get_address(
    ui: &mut UserInterface,
    path: &[u8],
    prompt: bool,
) -> Result<ArrayVec<u8, 220>, StatusWords> {
    let bip32_path: Bip32Path = path.try_into()?;
    if !bip32_path.0.starts_with(&BIP32_PREFIX[0..2]) {
        error!("BIP32 path prefix mismatch");
        return Err(StatusWords::Unknown);
    }

    let mut rv = ArrayVec::<u8, 220>::new();

    if with_public_keys(&bip32_path.0, true, |key, address: &SuiPubKeyAddress| {
        try_option(|| -> Option<()> {
            if prompt {
                info!("Prompting for address confirmation");
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
        return Err(StatusWords::UserCancelled);
    }
    Ok(rv)
}

// async fn prompt_tx_params(
//     ui: &UserInterface,
//     path: &[u32],
//     TxParams {
//         amount,
//         fee,
//         destination_address,
//     }: TxParams,
//     coin_type: CoinType,
//     ctx: &RunCtx,
// ) {
//     if with_public_keys(path, true, |_, address: &SuiPubKeyAddress| {
//         try_option(ui.confirm_sign_tx(address, destination_address, amount, coin_type, fee, ctx))
//     })
//     .ok()
//     .is_none()
//     {
//         reject::<()>(StatusWords::UserCancelled as u16).await;
//     };
// }
// async fn check_tx_params(expected: &TxParams, received: &TxParams) {
//     if !swap::check_tx_params(expected, received) {
//         reject::<()>(SW_SWAP_TX_PARAM_MISMATCH).await;
//     }
// }

// pub async fn sign_apdu(io: HostIO, ctx: &mut RunCtx, settings: Settings, ui: UserInterface) {
//     let _on_failure = defer::defer(|| {
//         // In case of a swap, we need to communicate that signing failed
//         if ctx.is_swap() && !ctx.is_swap_sign_succeeded() {
//             ctx.set_swap_sign_failure();
//         }
//     });

//     let mut input = match io.get_params::<3>() {
//         Some(v) => v,
//         None => reject(SyscallError::InvalidParameter as u16).await,
//     };

//     info!("input length {}", input.len());

//     // Read length, and move input[0] by one byte
//     let length = usize::from_le_bytes(input[0].read().await);

//     info!("apdu sign tx length: {}\n", length);

//     let known_txn = {
//         let mut txn = input[0].clone();
//         let object_data_source = input.get(2).map(|bs| WithObjectData { bs: bs.clone() });
//         NoinlineFut(async move {
//             info!("Beginning tx_parse");
//             TryFuture(tx_parser(object_data_source).parse(&mut txn)).await
//         })
//         .await
//     };

//     info!("End of tx_parse");

//     let is_unknown_txn = known_txn.is_none();

//     match known_txn {
//         Some(KnownTx::TransferTx {
//             recipient,
//             total_amount,
//             coin_type,
//             gas_budget,
//         }) => {
//             info!("Known transfer tx\n");
//             let mut bs = input[1].clone();
//             let path = BIP_PATH_PARSER.parse(&mut bs).await;
//             if !path.starts_with(&BIP32_PREFIX[0..2]) {
//                 reject::<()>(SyscallError::InvalidParameter as u16).await;
//             }

//             let tx_params = TxParams {
//                 amount: total_amount,
//                 fee: gas_budget,
//                 destination_address: recipient,
//             };

//             if ctx.is_swap() {
//                 let expected = ctx.get_swap_tx_params();
//                 check_tx_params(expected, &tx_params).await;
//             } else {
//                 // Show prompts after all inputs have been parsed
//                 NoinlineFut(prompt_tx_params(
//                     &ui,
//                     path.as_slice(),
//                     tx_params,
//                     coin_type,
//                     ctx,
//                 ))
//                 .await;
//             }
//         }
//         Some(KnownTx::StakeTx {
//             recipient,
//             total_amount,
//             gas_budget,
//         }) => {
//             info!("Known stake tx\n");
//             if ctx.is_swap() {
//                 reject::<()>(SyscallError::NotSupported as u16).await;
//             }
//             let mut bs = input[1].clone();
//             let path = BIP_PATH_PARSER.parse(&mut bs).await;
//             if !path.starts_with(&BIP32_PREFIX[0..2]) {
//                 reject::<()>(SyscallError::InvalidParameter as u16).await;
//             }

//             if with_public_keys(&path, true, |_, address: &SuiPubKeyAddress| {
//                 try_option(ui.confirm_stake_tx(address, recipient, total_amount, gas_budget))
//             })
//             .ok()
//             .is_none()
//             {
//                 reject::<()>(StatusWords::UserCancelled as u16).await;
//             };
//         }
//         Some(KnownTx::UnstakeTx {
//             total_amount,
//             gas_budget,
//         }) => {
//             info!("Known unstake tx\n");
//             if ctx.is_swap() {
//                 reject::<()>(SyscallError::NotSupported as u16).await;
//             }
//             let mut bs = input[1].clone();
//             let path = BIP_PATH_PARSER.parse(&mut bs).await;
//             if !path.starts_with(&BIP32_PREFIX[0..2]) {
//                 reject::<()>(SyscallError::InvalidParameter as u16).await;
//             }

//             if with_public_keys(&path, true, |_, address: &SuiPubKeyAddress| {
//                 try_option(ui.confirm_unstake_tx(address, total_amount, gas_budget))
//             })
//             .ok()
//             .is_none()
//             {
//                 reject::<()>(StatusWords::UserCancelled as u16).await;
//             };
//         }
//         None => {
//             info!("Unknown tx\n");
//             if ctx.is_swap() {
//                 // Reject unknown transactions in swap mode
//                 reject::<()>(SyscallError::NotSupported as u16).await;
//             } else if !settings.get_blind_sign() {
//                 ui.warn_tx_not_recognized();
//                 reject::<()>(SyscallError::NotSupported as u16).await;
//             }
//         }
//     }

//     NoinlineFut(async move {
//         let mut hasher = ledger_device_sdk::hash::blake2::Blake2b_256::new();
//         {
//             let mut txn = input[0].clone();
//             const CHUNK_SIZE: usize = 128;
//             let (chunks, rem) = (length / CHUNK_SIZE, length % CHUNK_SIZE);
//             for _ in 0..chunks {
//                 let b: [u8; CHUNK_SIZE] = txn.read().await;
//                 let _ = hasher.update(&b);
//             }
//             for _ in 0..rem {
//                 let b: [u8; 1] = txn.read().await;
//                 let _ = hasher.update(&b);
//             }
//         }
//         let mut hash: HexHash<32> = Default::default();
//         let _ = hasher.finalize(&mut hash.0);

//         if is_unknown_txn {
//             // Show prompts after all inputs have been parsed
//             if ui.confirm_blind_sign_tx(&hash).is_none() {
//                 reject::<()>(StatusWords::UserCancelled as u16).await;
//             };
//         }
//         let path = BIP_PATH_PARSER.parse(&mut input[1].clone()).await;
//         if !path.starts_with(&BIP32_PREFIX[0..2]) {
//             reject::<()>(SyscallError::InvalidParameter as u16).await;
//         }
//         if let Some(sig) = { eddsa_sign(&path, true, &hash.0).ok() } {
//             io.result_final(&sig.0[0..]).await;
//         } else {
//             reject::<()>(SyscallError::Unspecified as u16).await;
//         }
//     })
//     .await;

//     // Does nothing if not a swap mode
//     ctx.set_swap_sign_success();
// }

// pub async fn validate_tlv(io: HostIO, ctx: &mut RunCtx) {
//     const TLV_ERROR_OFFSET: u16 = 0x7000;

//     let mut input = match io.get_params::<4>() {
//         Some(bs) => bs,
//         None => reject(SyscallError::InvalidParameter as u16).await,
//     };

//     trace!("validate_tlv\n");

//     let first: [u8; 2] = input[0].read().await;
//     let length = u16::from_le_bytes(first);

//     trace!("data length: {}\n", HexSlice(&first));

//     let mut tlv = input[0].clone();

//     let mut b_arr: ArrayVec<u8, 1024> = ArrayVec::new();

//     const CHUNK_SIZE: usize = 10;
//     let (chunks, rem) = (length as usize / CHUNK_SIZE, length as usize % CHUNK_SIZE);
//     for _ in 0..chunks {
//         let b: [u8; CHUNK_SIZE] = tlv.read().await;
//         let _ = b_arr.try_extend_from_slice(&b);
//     }

//     for _ in 0..rem {
//         let b: [u8; 1] = tlv.read().await;
//         let _ = b_arr.try_extend_from_slice(&b);
//     }

//     let mut out = DynamicTokenOut::default();

//     match parse_dynamic_token_tlv(b_arr.as_slice() as &[u8], &mut out) {
//         Ok(()) => trace!("tlv parsing succeed\n"),
//         Err(err) => {
//             trace!("tlv parsing failed: {}\n", err as u8);
//             trace!("tlv data: {}\n", HexSlice(&b_arr));
//             reject::<()>(TLV_ERROR_OFFSET + err as u16).await;
//             return;
//         }
//     };

//     trace!("TUID: {}\n", HexSlice(&out.tuid));

//     let mut tuid: Tuid = Default::default();
//     match parse_tuid(&out.tuid, &mut tuid) {
//         Ok(()) => trace!("tuid parsing succeed\n"),
//         Err(err) => {
//             trace!("Tuid parsing failed: {}\n", err as u8);
//             reject::<()>(TLV_ERROR_OFFSET + err as u16).await;
//             return;
//         }
//     };

//     trace!(
//         "token contract: \nPACKAGE ADDRESS - {}\nMODULE - {}\nSTRUCT - {}\n",
//         HexSlice(&tuid.package_addr),
//         tuid.module.as_str(),
//         tuid.struct_name.as_str(),
//     );

//     let module: ArrayString<COIN_STRING_LENGTH> = match ArrayString::from(tuid.module.as_str()) {
//         Ok(a) => a,
//         Err(_err) => {
//             trace!("Module parsing failed: {}\n", _err);
//             reject::<()>(TLV_ERROR_OFFSET + TlvError::UnexpectedEof as u16).await;
//             return;
//         }
//     };
//     let function: ArrayString<COIN_STRING_LENGTH> =
//         match ArrayString::from(tuid.struct_name.as_str()) {
//             Ok(a) => a,
//             Err(_err) => {
//                 trace!("Function parsing failed: {}\n", _err);
//                 reject::<()>(TLV_ERROR_OFFSET + TlvError::UnexpectedEof as u16).await;
//                 return;
//             }
//         };

//     let ticker: ArrayString<TICKER_LENGTH> = match ArrayString::from(out.ticker.as_str()) {
//         Ok(a) => a,
//         Err(_err) => {
//             trace!("Ticker parsing failed: {}\n", _err);
//             reject::<()>(TLV_ERROR_OFFSET + TlvError::UnexpectedEof as u16).await;
//             return;
//         }
//     };

//     ctx.set_token(tuid.package_addr, module, function, out.magnitude, ticker);

//     io.result_final(&[]).await;
// }

// #[derive(Clone)]
// struct WithObjectData {
//     bs: ByteStream,
// }

// impl HasObjectData for WithObjectData {
//     type State<'c> = impl Future<Output = Option<ObjectData>> + 'c;

//     fn get_object_data<'a: 'c, 'b: 'c, 'c>(&'b self, digest: &'a ObjectDigest) -> Self::State<'c> {
//         async move {
//             let mut bs = self.bs.clone();
//             let objects_count: Option<usize> = TryFuture(bs.read()).await.map(usize::from_le_bytes);

//             match objects_count {
//                 None => None,
//                 Some(0) => None,
//                 Some(c) => {
//                     info!("get_object_data: objects_count {}", c);
//                     for _ in 0..c {
//                         let length = usize::from_le_bytes(bs.read().await);
//                         let mut obj_start_bs = bs.clone();

//                         let hash = NoinlineFut(compute_object_hash(&mut bs, length)).await;

//                         if hash.0 == digest[1..33] {
//                             info!(
//                                 "get_object_data: found object with digest {}",
//                                 HexSlice(digest)
//                             );
//                             // Found object, now try to parse
//                             return NoinlineFut(TryFuture(
//                                 object_parser().parse(&mut obj_start_bs),
//                             ))
//                             .await;
//                         }
//                     }
//                     info!(
//                         "get_object_data: did not find object with digest {}",
//                         HexSlice(digest)
//                     );
//                     None
//                 }
//             }
//         }
//     }
// }
