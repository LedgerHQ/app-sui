use crate::ctx::{RunCtx, TICKER_LENGTH};
use crate::parser::common::{CoinType, SUI_COIN_DECIMALS, SUI_COIN_TYPE};
use crate::utils::*;

extern crate alloc;
use alloc::format;

use arrayvec::ArrayString;
use either::*;
use hex_literal::hex;
use ledger_crypto_helpers::common::HexSlice;

use ledger_log::trace;

pub const LEDGER_STAKE_ADDRESS: [u8; 32] =
    hex!("3d9fb148e35ef4d74fcfc36995da14fc504b885d5f2bfeca37d6ea2cc044a32d");

#[inline(never)]
pub fn get_coin_and_amount_fields(
    total_amount: u64,
    coin_type: CoinType,
    ctx: &RunCtx,
) -> (
    (ArrayString<32>, ArrayString<32>),
    Either<ArrayString<8>, (ArrayString<4>, ArrayString<256>)>,
) {
    if let Some((ticker, divisor)) = get_known_coin_ticker(&coin_type, ctx) {
        let (quotient, remainder_str) = get_amount_in_decimals(total_amount, divisor);
        let v1 = format!(
            "{} {}.{}",
            ticker.as_str(),
            quotient,
            remainder_str.as_str()
        );
        let amount = (
            ArrayString::from("Amount").unwrap(),
            ArrayString::from(&v1).unwrap(),
        );
        (amount, Left(ticker))
    } else {
        let v1 = format!("{}", total_amount);
        let amount = (
            ArrayString::from("Raw Amount").unwrap(),
            ArrayString::from(&v1).unwrap(),
        );
        let (coin_id, module, name) = coin_type;

        let v2 = format!(
            "{}::{}::{}",
            HexSlice(&coin_id),
            core::str::from_utf8(module.as_slice()).unwrap_or("invalid utf-8"),
            core::str::from_utf8(name.as_slice()).unwrap_or("invalid utf-8")
        );
        let coin = Right((
            ArrayString::from("Coin").unwrap(),
            ArrayString::from(&v2).unwrap(),
        ));
        (amount, coin)
    }
}

#[inline(never)]
fn get_known_coin_ticker(
    coin_type: &CoinType,
    ctx: &RunCtx,
) -> Option<(ArrayString<TICKER_LENGTH>, u8)> {
    if *coin_type == SUI_COIN_TYPE {
        return Some((ArrayString::from("SUI").unwrap(), SUI_COIN_DECIMALS));
    }

    let (coin_id, module, function) = coin_type;

    let ctx_coin_id = ctx.get_token_coin_id();

    if *coin_id == ctx_coin_id
        && module.as_slice() == ctx.get_token_coin_module().as_bytes()
        && function.as_slice() == ctx.get_token_coin_function().as_bytes()
    {
        return Some((ctx.get_token_ticker(), ctx.get_token_divisor()));
    }

    trace!(
        "coin_id ({})\nctx_coin_id ({})\n",
        HexSlice(coin_id),
        HexSlice(&ctx_coin_id)
    );

    None
}
