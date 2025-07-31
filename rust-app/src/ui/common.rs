use crate::parser::common::{CoinType, SUI_COIN_DECIMALS, SUI_COIN_TYPE};
use crate::utils::*;

extern crate alloc;
use alloc::format;

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
fn get_known_coin_ticker(coin_type: &CoinType) -> Option<(ArrayString<8>, u8)> {
    if *coin_type == SUI_COIN_TYPE {
        return Some((ArrayString::from("SUI").unwrap(), SUI_COIN_DECIMALS));
    }

    for k in KNOWN_COINS {
        let mut module = ArrayVec::new();
        let _ = module.try_extend_from_slice(k.module.as_bytes());

        let mut function = ArrayVec::new();
        let _ = function.try_extend_from_slice(k.function.as_bytes());

        if *coin_type == (k.coin_id, module, function) {
            return Some((ArrayString::from(k.ticker).unwrap(), k.divisor));
        }
    }
    None
}

struct KnownCoin<'a> {
    coin_id: [u8; 32],
    module: &'a str,
    function: &'a str,
    divisor: u8,
    ticker: &'a str,
}

use hex_literal::hex;

const KNOWN_COINS: [KnownCoin; 51] = [
    KnownCoin {
        coin_id: hex!("a8816d3a6e3136e86bc2873b1f94a15cadc8af2703c075f2d546c2ae367f4df9"),
        module: "ocean",
        function: "OCEAN",
        divisor: 9,
        ticker: "OCEAN",
    },
    KnownCoin {
        coin_id: hex!("5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf"),
        module: "coin",
        function: "COIN",
        divisor: 6,
        ticker: "wUSDC",
    },
    KnownCoin {
        coin_id: hex!("dba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7"),
        module: "usdc",
        function: "USDC",
        divisor: 6,
        ticker: "USDC",
    },
    KnownCoin {
        coin_id: hex!("c060006111016b8a020ad5b33834984a437aaa7d3c74c18e09a95d48aceab08c"),
        module: "coin",
        function: "COIN",
        divisor: 6,
        ticker: "wUSDT",
    },
    KnownCoin {
        coin_id: hex!("375f70cf2ae4c00bf37117d0c85a2c71545e6ee05c4a5c7d282cd66a4504b068"),
        module: "usdt",
        function: "USDT",
        divisor: 6,
        ticker: "suiUSDT",
    },
    KnownCoin {
        coin_id: hex!("d1b72982e40348d069bb1ff701e634c117bb5f741f44dff91e472d3b01461e55"),
        module: "stsui",
        function: "STSUI",
        divisor: 9,
        ticker: "stSUI",
    },
    KnownCoin {
        coin_id: hex!("deeb7a4662eec9f2f3def03fb937a663dddaa2e215b8078a284d026b7946c270"),
        module: "deep",
        function: "DEEP",
        divisor: 6,
        ticker: "DEEP",
    },
    KnownCoin {
        coin_id: hex!("ce7ff77a83ea0cb6fd39bd8748e2ec89a3f41e8efdc3f4eb123e0ca37b184db2"),
        module: "buck",
        function: "BUCK",
        divisor: 9,
        ticker: "BUCK",
    },
    KnownCoin {
        coin_id: hex!("bde4ba4c2e274a60ce15c1cfff9e5c42e41654ac8b6d906a57efa4bd3c29f47d"),
        module: "hasui",
        function: "HASUI",
        divisor: 9,
        ticker: "haSUI",
    },
    KnownCoin {
        coin_id: hex!("06864a6f921804860930db6ddbe2e16acdf8504495ea7481637a1c8b9a8fe54b"),
        module: "cetus",
        function: "CETUS",
        divisor: 9,
        ticker: "CETUS",
    },
    KnownCoin {
        coin_id: hex!("960b531667636f39e85867775f52f6b1f220a058c4de786905bdf761e06a56bb"),
        module: "usdy",
        function: "USDY",
        divisor: 6,
        ticker: "USDY",
    },
    KnownCoin {
        coin_id: hex!("8993129d72e733985f7f1a00396cbd055bad6f817fee36576ce483c8bbb8b87b"),
        module: "sudeng",
        function: "SUDENG",
        divisor: 9,
        ticker: "HIPPO",
    },
    KnownCoin {
        coin_id: hex!("aafb102dd0902f5055cadecd687fb5b71ca82ef0e0285d90afde828ec58ca96b"),
        module: "btc",
        function: "BTC",
        divisor: 8,
        ticker: "sbWBTC",
    },
    KnownCoin {
        coin_id: hex!("d0e89b2af5e4910726fbcd8b8dd37bb79b29e5f83f7491bca830e94f7f226d29"),
        module: "eth",
        function: "ETH",
        divisor: 8,
        ticker: "sbETH",
    },
    KnownCoin {
        coin_id: hex!("5145494a5f5100e645e4b0aa950fa6b68f614e8c59e17bc5ded3495123a79178"),
        module: "ns",
        function: "NS",
        divisor: 6,
        ticker: "NS",
    },
    KnownCoin {
        coin_id: hex!("e1b45a0e641b9955a20aa0ad1c1f4ad86aad8afb07296d4085e349a50e90bdca"),
        module: "blue",
        function: "BLUE",
        divisor: 9,
        ticker: "BLUE",
    },
    KnownCoin {
        coin_id: hex!("027792d9fed7f9844eb4839566001bb6f6cb4804f66aa2da6fe1ee242d896881"),
        module: "coin",
        function: "COIN",
        divisor: 8,
        ticker: "WBTC",
    },
    KnownCoin {
        coin_id: hex!("fe3afec26c59e874f3c1d60b8203cb3852d2bb2aa415df9548b8d688e6683f93"),
        module: "alpha",
        function: "ALPHA",
        divisor: 9,
        ticker: "ALPHA",
    },
    KnownCoin {
        coin_id: hex!("2053d08c1e2bd02791056171aab0fd12bd7cd7efad2ab8f6b9c8902f14df2ff2"),
        module: "ausd",
        function: "AUSD",
        divisor: 6,
        ticker: "AUSD",
    },
    KnownCoin {
        coin_id: hex!("f325ce1300e8dac124071d3152c5c5ee6174914f8bc2161e88329cf579246efc"),
        module: "afsui",
        function: "AFSUI",
        divisor: 9,
        ticker: "AFSUI",
    },
    KnownCoin {
        coin_id: hex!("f16e6b723f242ec745dfd7634ad072c42d5c1d9ac9d62a39c381303eaa57693a"),
        module: "fdusd",
        function: "FDUSD",
        divisor: 6,
        ticker: "FDUSD",
    },
    KnownCoin {
        coin_id: hex!("549e8b69270defbfafd4f94e17ec44cdbdd99820b33bda2278dea3b9a32d3f55"),
        module: "cert",
        function: "CERT",
        divisor: 9,
        ticker: "vSUI",
    },
    KnownCoin {
        coin_id: hex!("f22da9a24ad027cccb5f2d496cbe91de953d363513db08a3a734d361c7c17503"),
        module: "LOFI",
        function: "LOFI",
        divisor: 9,
        ticker: "LOFI",
    },
    KnownCoin {
        coin_id: hex!("e44df51c0b21a27ab915fa1fe2ca610cd3eaa6d9666fe5e62b988bf7f0bd8722"),
        module: "musd",
        function: "MUSD",
        divisor: 9,
        ticker: "mUSD",
    },
    KnownCoin {
        coin_id: hex!("7016aae72cfc67f2fadf55769c0a7dd54291a583b63051a5ed71081cce836ac6"),
        module: "sca",
        function: "SCA",
        divisor: 9,
        ticker: "SCA",
    },
    KnownCoin {
        coin_id: hex!("bc732bc5f1e9a9f4bdf4c0672ee538dbf56c161afe04ff1de2176efabdf41f92"),
        module: "suai",
        function: "SUAI",
        divisor: 6,
        ticker: "SUIAI",
    },
    KnownCoin {
        coin_id: hex!("b7844e289a8410e50fb3ca48d69eb9cf29e27d223ef90353fe1bd8e27ff8f3f8"),
        module: "coin",
        function: "COIN",
        divisor: 8,
        ticker: "SOL",
    },
    KnownCoin {
        coin_id: hex!("a99b8952d4f7d947ea77fe0ecdcc9e5fc0bcab2841d6e2a5aa00c3044e5544b5"),
        module: "navx",
        function: "NAVX",
        divisor: 9,
        ticker: "NAVX",
    },
    KnownCoin {
        coin_id: hex!("bc858cb910b9914bee64fff0f9b38855355a040c49155a17b265d9086d256545"),
        module: "but",
        function: "BUT",
        divisor: 9,
        ticker: "BUT",
    },
    KnownCoin {
        coin_id: hex!("ae00e078a46616bf6e1e6fb673d18dcd2aa31319a07c9bc92f6063363f597b4e"),
        module: "AXOL",
        function: "AXOL",
        divisor: 9,
        ticker: "AXOL",
    },
    KnownCoin {
        coin_id: hex!("af8cd5edc19c4512f4259f0bee101a40d41ebed738ade5874359610ef8eeced5"),
        module: "coin",
        function: "COIN",
        divisor: 8,
        ticker: "WETH",
    },
    KnownCoin {
        coin_id: hex!("32a976482bf4154961bf20bfa3567a80122fdf8e8f8b28d752b609d8640f7846"),
        module: "miu",
        function: "MIU",
        divisor: 3,
        ticker: "MIU",
    },
    KnownCoin {
        coin_id: hex!("b45fcfcc2cc07ce0702cc2d229621e046c906ef14d9b25e8e4d25f6e8763fef7"),
        module: "send",
        function: "SEND",
        divisor: 6,
        ticker: "SEND",
    },
    KnownCoin {
        coin_id: hex!("fa7ac3951fdca92c5200d468d31a365eb03b2be9936fde615e69f0c1274ad3a0"),
        module: "BLUB",
        function: "BLUB",
        divisor: 2,
        ticker: "BLUB",
    },
    KnownCoin {
        coin_id: hex!("5d1f47ea69bb0de31c313d7acf89b890dbb8991ea8e03c6c355171f84bb1ba4a"),
        module: "turbos",
        function: "TURBOS",
        divisor: 9,
        ticker: "TURBOS",
    },
    KnownCoin {
        coin_id: hex!("b5b603827d1bfb2859200fd332d5e139ccac2598f0625de153a87cf78954e0c4"),
        module: "wewe",
        function: "WEWE",
        divisor: 9,
        ticker: "WEWE",
    },
    KnownCoin {
        coin_id: hex!("506a6fc25f1c7d52ceb06ea44a3114c9380f8e2029b4356019822f248b49e411"),
        module: "memefi",
        function: "MEMEFI",
        divisor: 9,
        ticker: "MEMEFI",
    },
    KnownCoin {
        coin_id: hex!("d976fda9a9786cda1a36dee360013d775a5e5f206f8e20f84fad3385e99eeb2d"),
        module: "aaa",
        function: "AAA",
        divisor: 6,
        ticker: "AAA",
    },
    KnownCoin {
        coin_id: hex!("4cf08813756dfa7519cb480a1a1a3472b5b4ec067592a8bee0f826808d218158"),
        module: "tardi",
        function: "TARDI",
        divisor: 9,
        ticker: "TARDI",
    },
    KnownCoin {
        coin_id: hex!("da097d57ae887fbd002fb5847dd0ab47ae7e1b183fd36832a51182c52257e1bc"),
        module: "msend_series_1",
        function: "MSEND_SERIES_1",
        divisor: 6,
        ticker: "mSEND",
    },
    KnownCoin {
        coin_id: hex!("ea65bb5a79ff34ca83e2995f9ff6edd0887b08da9b45bf2e31f930d3efb82866"),
        module: "s",
        function: "S",
        divisor: 9,
        ticker: "S",
    },
    KnownCoin {
        coin_id: hex!("06106c04a586f0f003fcdf7fb33564f373680ddcc1beb716fd22e2952e227eb3"),
        module: "tubbi",
        function: "TUBBI",
        divisor: 9,
        ticker: "TUBBI",
    },
    KnownCoin {
        coin_id: hex!("9c6d76eb273e6b5ba2ec8d708b7fa336a5531f6be59f326b5be8d4d8b12348a4"),
        module: "coin",
        function: "COIN",
        divisor: 6,
        ticker: "PYTH",
    },
    KnownCoin {
        coin_id: hex!("83556891f4a0f233ce7b05cfe7f957d4020492a34f5405b2cb9377d060bef4bf"),
        module: "spring_sui",
        function: "SPRING_SUI",
        divisor: 9,
        ticker: "sSUI",
    },
    KnownCoin {
        coin_id: hex!("76cb819b01abed502bee8a702b4c2d547532c12f25001c9dea795a5e631c26f1"),
        module: "fud",
        function: "FUD",
        divisor: 5,
        ticker: "FUD",
    },
    KnownCoin {
        coin_id: hex!("a26788cb462ae9242d9483bdbe5a82188ba0eaeae3c5e9237d30cbcb83ce7a88"),
        module: "mochi",
        function: "MOCHI",
        divisor: 6,
        ticker: "MOCHI",
    },
    KnownCoin {
        coin_id: hex!("e6b9e1033c72084ad01db37c77778ca53b9c4ebb263f28ffbfed39f4d5fd5057"),
        module: "win",
        function: "WIN",
        divisor: 9,
        ticker: "WIN",
    },
    KnownCoin {
        coin_id: hex!("6dae8ca14311574fdfe555524ea48558e3d1360d1607d1c7f98af867e3b7976c"),
        module: "flx",
        function: "FLX",
        divisor: 8,
        ticker: "FLX",
    },
    KnownCoin {
        coin_id: hex!("288710173f12f677ac38b0c2b764a0fea8108cb5e32059c3dd8f650d65e2cb25"),
        module: "pepe",
        function: "PEPE",
        divisor: 2,
        ticker: "PEPE",
    },
    KnownCoin {
        coin_id: hex!("356a26eb9e012a68958082340d4c4116e7f55615cf27affcff209cf0ae544f59"),
        module: "wal",
        function: "WAL",
        divisor: 9,
        ticker: "WAL",
    },
    KnownCoin {
        coin_id: hex!("4c981f3ff786cdb9e514da897ab8a953647dae2ace9679e8358eec1e3e8871ac"),
        module: "dmc",
        function: "DMC",
        divisor: 9,
        ticker: "DMC",
    },
];
