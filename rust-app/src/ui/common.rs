use crate::parser::common::{CoinType, SUI_COIN_DIVISOR, SUI_COIN_TYPE};
use crate::utils::*;

extern crate alloc;
use alloc::format;
use alloc::vec::Vec;

use arrayvec::ArrayString;
use arrayvec::ArrayVec;
use either::*;
use ledger_crypto_helpers::common::HexSlice;

#[inline(never)]
pub fn get_coin_and_amount_fields(
    total_amount: u64,
    coin_type: CoinType,
) -> (
    (ArrayString<32>, ArrayString<32>),
    Either<ArrayString<8>, (ArrayString<4>, ArrayString<256>)>,
) {
    if let Some((ticker, divisor)) = get_known_coin_ticker(&coin_type) {
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
            "0x{}::{}::{}",
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

macro_rules! mk_coin_type {
    ($s:expr) => {{
        let parts: Vec<&str> = $s.split("::").collect();
        let hex_str = &parts[0][2..];
        let mut coin_id = [0u8; 32];
        hex::decode_to_slice(hex_str, &mut coin_id).unwrap();

        let mut module = ArrayVec::new();
        module.try_extend_from_slice(parts[1].as_bytes()).unwrap();

        let mut function = ArrayVec::new();
        function.try_extend_from_slice(parts[2].as_bytes()).unwrap();

        (coin_id, module, function)
    }};
}

struct KnownCoin<'a> {
    coin_type: CoinType,
    divisor: u8,
    ticker: &'a str,
}

#[inline(never)]
fn get_known_coin_ticker(coin_type: &CoinType) -> Option<(ArrayString<8>, u8)> {
    let known_coins: [KnownCoin; 3] = [
        KnownCoin {
            coin_type: mk_coin_type!(
                "0xa8816d3a6e3136e86bc2873b1f94a15cadc8af2703c075f2d546c2ae367f4df9::ocean::OCEAN"
            ),
            divisor: 9,
            ticker: "OCEAN",
        },
        KnownCoin {
            coin_type: mk_coin_type!(
                "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN"
            ),
            divisor: 6,
            ticker: "wUSDC",
        },
        KnownCoin {
            coin_type: mk_coin_type!(
                "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC"
            ),
            divisor: 6,
            ticker: "USDC",
        },
    ];

    if *coin_type == SUI_COIN_TYPE {
        return Some((ArrayString::from(&"SUI").unwrap(), SUI_COIN_DIVISOR));
    }

    for k in known_coins {
        if *coin_type == k.coin_type {
            return Some((ArrayString::from(&k.ticker).unwrap(), k.divisor));
        }
    }
    None
}
