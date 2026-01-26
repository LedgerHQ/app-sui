use core::{convert::TryFrom, fmt::Write};

use arrayvec::ArrayString;
// #[allow(unused_imports)]
// use crate::crypto_helpers::common::HexSlice;
use crate::crypto_helpers::{
    common::{Address, CryptographyError},
    eddsa::with_public_keys,
};
use ledger_device_sdk::libcall::{
    self,
    swap::{
        get_check_address_params, get_printable_amount_params, sign_tx_params, swap_return,
        SwapResult,
    },
    LibCallCommand,
};
use ledger_device_sdk::log::{error, trace};
use panic_handler::{set_swap_panic_handler, swap_panic_handler};
use params::{CheckAddressParams, PrintableAmountParams, TxParams, MAX_SWAP_TICKER_LENGTH};

use crate::main_stax::app_main;
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

pub fn check_tx_params(expected: &TxParams, received: &TxParams) -> bool {
    expected.amount == received.amount
        && expected.fee == received.fee
        && expected.destination_address == received.destination_address
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
