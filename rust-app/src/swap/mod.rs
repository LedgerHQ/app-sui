use core::{convert::TryFrom, fmt::Write};

use crate::crypto_helpers::{
    common::{Address, CryptographyError},
    eddsa::with_public_keys,
};
use arrayvec::ArrayString;
use ledger_device_sdk::libcall::{
    self,
    swap::{
        get_check_address_params, get_printable_amount_params, sign_tx_params, swap_return,
        SwapResult,
    },
    LibCallCommand,
};
use ledger_device_sdk::log::{error, info, trace};
use panic_handler::{set_swap_panic_handler, swap_panic_handler};
use params::{CheckAddressParams, PrintableAmountParams, TxParams, MAX_SWAP_TICKER_LENGTH};

use crate::app_main::app_main;
use crate::parser::common::{coin_type_from_short_str, UNKNOWN_COIN_TYPE};
use crate::{ctx::RunCtx, parser::common::SUI_COIN_DECIMALS, utils::get_amount_in_decimals};
use crate::{implementation::BIP32_PREFIX, interface::SuiPubKeyAddress};

pub mod panic_handler;
pub mod params;

#[derive(Debug)]
pub enum Error {
    DecodeDPathError,
    CryptographyError(CryptographyError),
    WrongAmountLength,
    WrongFeeLength,
    BadAddressASCII,
    BadAddressLength,
    BadAddressHex,
    DecodeCoinConfig,
    BadCoinConfigTicker,
}

impl From<CryptographyError> for Error {
    fn from(e: CryptographyError) -> Self {
        Error::CryptographyError(e)
    }
}

pub fn check_address(params: &CheckAddressParams) -> Result<bool, Error> {
    let ref_addr = &params.ref_address;
    trace!("check_address: dpath: {:X?}", params.dpath);
    // trace!("check_address: ref: 0x{}", HexSlice(ref_addr));

    if !params.dpath.starts_with(&BIP32_PREFIX[0..2]) {
        return Err(Error::DecodeDPathError);
    }

    Ok(with_public_keys(
        &params.dpath,
        true,
        |_, address: &SuiPubKeyAddress| -> Result<_, CryptographyError> {
            trace!("check_address: der: {}", address);
            let der_addr = address.get_binary_address();

            Ok(ref_addr == der_addr)
        },
    )?)
}

// Outputs a string with the amount of SUI.
//
// Max sui amount 10_000_000_000 SUI.
// So max string length is 15 (ticker) + 1 (blank) + 11 (quotient) + 1 (dot) + 12 (reminder) = 40
pub fn get_printable_amount(params: &PrintableAmountParams) -> Result<ArrayString<40>, Error> {
    let mut ticker = ArrayString::<MAX_SWAP_TICKER_LENGTH>::default();
    let decimals;

    if let (Some(coin_config), false) = (params.coin_config.as_ref(), params.is_fee) {
        ticker.push_str(&coin_config.ticker);
        decimals = coin_config.decimals;
    } else {
        ticker.push_str("SUI");
        decimals = SUI_COIN_DECIMALS;
    };

    let (quotient, remainder_str) = get_amount_in_decimals(params.amount, decimals);

    let mut printable_amount = ArrayString::default();
    write!(&mut printable_amount, "{ticker} {quotient}.{remainder_str}")
        .expect("string always fits");

    trace!(
        "get_printable_amount: amount: {}",
        printable_amount.as_str()
    );

    Ok(printable_amount)
}

pub fn check_tx_params(expected: &TxParams, received: &TxParams, ctx: &RunCtx) -> bool {
    info!("check_tx_params: expected: {:X?}", expected);
    info!("check_tx_params: received: {:X?}", received);
    // A transfer of the gas coin itself can never be bound to a swap quote: gas is
    // charged out of the transferred coin, so the recipient gets the coin's balance
    // minus the gas actually consumed -- a figure that is not knowable before
    // execution. Matching `amount` here would only prove the *pre-gas* balance
    // equals the quote, letting a host underpay the counterparty by up to the gas
    // budget with no UI shown (B2CA-2793 follow-up finding 2). A swap must instead
    // split the exact amount off the gas coin and transfer that split output, which
    // is what the Exchange flow already does.
    if received.includes_gas_coin {
        info!("check_tx_params: GasCoin transfer cannot be bound to a swap quote");
        return false;
    }
    expected.amount == received.amount
        && expected.fee == received.fee
        && expected.destination_address == received.destination_address
        && coin_type_ok(expected, received, ctx)
}

fn coin_type_ok(expected: &TxParams, received: &TxParams, ctx: &RunCtx) -> bool {
    info!(
        "check_tx_params: expected coin type: {:X?}",
        expected.coin_type
    );
    info!(
        "check_tx_params: received coin type: {:X?}",
        received.coin_type
    );
    // Never take the equality shortcut for the unknown-ticker sentinel: it is a
    // representable coin type (all-zero id, empty module/name), and `received`
    // comes from host-controlled object data, so a crafted `0x0::"":""` coin
    // could otherwise match it and skip the dynamic-descriptor check below.
    // An unknown ticker must always be resolved via a signed dynamic descriptor.
    if expected.coin_type != UNKNOWN_COIN_TYPE && expected.coin_type == received.coin_type {
        return true;
    }
    info!(
        "check_tx_params: expected ticker: {}",
        expected.expected_ticker.as_str()
    );
    let dynamic_ticker = ctx.get_token_ticker();
    if dynamic_ticker.is_empty() || expected.expected_ticker.as_str() != dynamic_ticker.as_str() {
        return false;
    }
    info!(
        "check_tx_params: dynamic ticker from ctx: {}",
        dynamic_ticker
    );
    let dynamic_coin_type = coin_type_from_short_str(
        ctx.get_token_coin_id(),
        ctx.get_token_coin_module().as_str(),
        ctx.get_token_coin_function().as_str(),
    );
    info!(
        "check_tx_params: dynamic coin type from ctx: {:X?}",
        dynamic_coin_type
    );
    received.coin_type == dynamic_coin_type
}

// For some reason heavy inlining + lto cause UB here, so we disable it
#[inline(never)]
pub fn lib_main(arg0: u32) {
    let cmd = libcall::get_command(arg0);

    match cmd {
        LibCallCommand::SwapCheckAddress => {
            let mut raw_params = get_check_address_params(arg0);

            let result = CheckAddressParams::try_from(&raw_params).and_then(|params| {
                trace!("{:X?}", params);
                check_address(&params)
            });

            let is_matched = result.unwrap_or_else(|_error| {
                error!("Error happened during CHECK_ADDRESS libcall:  {:?}", _error);
                false
            });

            swap_return(SwapResult::CheckAddressResult(
                &mut raw_params,
                is_matched as i32,
            ));
        }
        LibCallCommand::SwapGetPrintableAmount => {
            let mut raw_params = get_printable_amount_params(arg0);

            let result = PrintableAmountParams::try_from(&raw_params).and_then(|params| {
                trace!("{:X?}", params);
                get_printable_amount(&params)
            });

            let amount_str = result
                .as_ref()
                .map(|amount_str| amount_str.as_str())
                .unwrap_or_else(|_error| {
                    error!(
                        "Error happened during GET_PRINTABLE_AMOUNT libcall:  {:?}",
                        _error
                    );
                    // Return empty string in case of error
                    ""
                });

            swap_return(SwapResult::PrintableAmountResult(
                &mut raw_params,
                amount_str,
            ));
        }
        LibCallCommand::SwapSignTransaction => {
            let mut raw_params = sign_tx_params(arg0);

            let result = TxParams::try_from(&raw_params).map(|params| {
                trace!("{:X?}", params);

                // SAFETY: at this point, the app is initialized,
                // so we can safely set the panic handler
                unsafe {
                    set_swap_panic_handler(swap_panic_handler);
                }

                let ctx = RunCtx::lib_swap(params);
                app_main(&ctx);

                ctx.is_swap_sign_succeeded()
            });

            let is_ok = result.unwrap_or_else(|_error| {
                error!(
                    "Error happened during SIGN_TRANSACTION libcall:  {:?}",
                    _error
                );
                false
            });

            swap_return(SwapResult::CreateTxResult(&mut raw_params, is_ok as u8));
        }
    }
}
