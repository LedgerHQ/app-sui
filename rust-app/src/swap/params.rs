use arrayvec::{ArrayString, ArrayVec};
use core::convert::{TryFrom, TryInto};
use core::mem;
use core::str;

use crate::parser::common::SuiAddressRaw;
use crate::swap::Error;

// Max SUI address str length is 32*2
const SUI_ADDRESS_STR_LENGTH: usize = 64;
const SUI_ADDRESS_PREFIX_STR_LENGTH: usize = 2;
const SUI_PREFIXED_ADDRESS_STR_LENGTH: usize =
    SUI_ADDRESS_STR_LENGTH + SUI_ADDRESS_PREFIX_STR_LENGTH;
const MAX_BIP32_PATH_LENGTH: usize = 5;
const BIP32_PATH_SEGMENT_LEN: usize = mem::size_of::<u32>();

// Should be enough for any coin ticker
pub const MAX_SWAP_TICKER_LENGTH: usize = 15;
// ticker length + ticker + decimals
const COIN_CONFIG_BUF_SIZE: usize = 1 + MAX_SWAP_TICKER_LENGTH + 1;

mod custom {
    use super::COIN_CONFIG_BUF_SIZE;
    use super::SUI_PREFIXED_ADDRESS_STR_LENGTH;
    use ledger_device_sdk::libcall;

    pub type CheckAddressParams =
        libcall::swap::CheckAddressParams<COIN_CONFIG_BUF_SIZE, SUI_PREFIXED_ADDRESS_STR_LENGTH>;

    pub type PrintableAmountParams =
        libcall::swap::PrintableAmountParams<COIN_CONFIG_BUF_SIZE, SUI_PREFIXED_ADDRESS_STR_LENGTH>;

    pub type CreateTxParams =
        libcall::swap::CreateTxParams<COIN_CONFIG_BUF_SIZE, SUI_PREFIXED_ADDRESS_STR_LENGTH>;
}

#[derive(Debug)]
pub struct CheckAddressParams {
    pub dpath: ArrayVec<u32, MAX_BIP32_PATH_LENGTH>,
    pub ref_address: SuiAddressRaw,
}

impl TryFrom<&custom::CheckAddressParams> for CheckAddressParams {
    type Error = Error;

    fn try_from(params: &custom::CheckAddressParams) -> Result<Self, Self::Error> {
        let mut dpath = ArrayVec::from([0u32; MAX_BIP32_PATH_LENGTH]);
        let dpath_len = unpack_path(
            &params.dpath[..params.dpath_len * BIP32_PATH_SEGMENT_LEN],
            &mut dpath,
        )?;
        dpath.truncate(dpath_len);

        let ref_address = address_from_hex_cstr(params.ref_address.as_ptr())?;

        Ok(CheckAddressParams { dpath, ref_address })
    }
}

#[derive(Debug)]
pub struct CoinConfig {
    pub ticker: ArrayString<MAX_SWAP_TICKER_LENGTH>,
    pub decimals: u8,
}

impl CoinConfig {
    pub fn try_from_bytes(buf: &[u8]) -> Result<Option<Self>, Error> {
        if buf.is_empty() {
            return Ok(None);
        }

        // Decoding ticker
        let ticker_len = buf[0] as usize;
        let buf = &buf[1..];

        if ticker_len > MAX_SWAP_TICKER_LENGTH || ticker_len > buf.len() {
            return Err(Error::BadCoinConfigTicker);
        }

        let ticker_bytes = &buf[..ticker_len];
        let ticker_str = str::from_utf8(ticker_bytes).map_err(|_| Error::BadCoinConfigTicker)?;
        let ticker = ArrayString::from(ticker_str).map_err(|_| Error::BadCoinConfigTicker)?;

        // Decoding decimals
        let buf = &buf[ticker_len..];
        // Decimals is the last byte in the buffer without encoded length
        if buf.len() != 1 {
            return Err(Error::DecodeCoinConfig);
        }
        let decimals = buf[0];

        Ok(Some(CoinConfig { ticker, decimals }))
    }
}

#[derive(Debug)]
pub struct PrintableAmountParams {
    pub coin_config: Option<CoinConfig>,
    pub is_fee: bool,
    pub amount: u64,
}

impl TryFrom<&custom::PrintableAmountParams> for PrintableAmountParams {
    type Error = Error;

    fn try_from(params: &custom::PrintableAmountParams) -> Result<Self, Self::Error> {
        let coin_config =
            CoinConfig::try_from_bytes(&params.coin_config[..params.coin_config_len])?;
        let amount = u64::from_be_bytes(
            params.amount[params.amount.len() - mem::size_of::<u64>()..]
                .try_into()
                .map_err(|_| Error::WrongAmountLength)?,
        );
        let is_fee = params.is_fee;

        Ok(PrintableAmountParams {
            coin_config,
            amount,
            is_fee,
        })
    }
}

#[derive(Debug, Default)]
pub struct TxParams {
    pub amount: u64,
    pub fee: u64,
    pub destination_address: SuiAddressRaw,
}

impl TryFrom<&custom::CreateTxParams> for TxParams {
    type Error = Error;

    fn try_from(params: &custom::CreateTxParams) -> Result<Self, Self::Error> {
        let amount = u64::from_be_bytes(
            params.amount[params.amount.len() - mem::size_of::<u64>()..]
                .try_into()
                .map_err(|_| Error::WrongAmountLength)?,
        );

        let fee = u64::from_be_bytes(
            params.fee_amount[params.fee_amount.len() - mem::size_of::<u64>()..]
                .try_into()
                .map_err(|_| Error::WrongFeeLength)?,
        );

        let destination_address = address_from_hex_cstr(params.dest_address.as_ptr())?;

        Ok(TxParams {
            amount,
            fee,
            destination_address,
        })
    }
}

fn unpack_path(buf: &[u8], out_path: &mut [u32]) -> Result<usize, Error> {
    if buf.len() % BIP32_PATH_SEGMENT_LEN != 0 {
        return Err(Error::DecodeDPathError);
    }

    for i in (0..buf.len()).step_by(BIP32_PATH_SEGMENT_LEN) {
        // For swap params, path segments are stored in big endian
        let path_seg = u32::from_be_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);

        out_path[i / BIP32_PATH_SEGMENT_LEN] = path_seg;
    }

    Ok(buf.len() / BIP32_PATH_SEGMENT_LEN)
}

// For some reason heavy inlining + lto cause UB here, so we disable it
#[inline(never)]
fn address_from_hex_cstr(c_str: *const u8) -> Result<SuiAddressRaw, Error> {
    // Calculate C-string length in the buffer
    let mut str_len = 0;
    while unsafe { *c_str.add(str_len) } != b'\0' && str_len <= SUI_PREFIXED_ADDRESS_STR_LENGTH {
        str_len += 1;
    }
    let str = unsafe { core::slice::from_raw_parts(c_str, str_len) };

    if str.len() < SUI_PREFIXED_ADDRESS_STR_LENGTH {
        return Err(Error::BadAddressLength);
    }

    // Trim zero terminator and '0x' prefix
    let str = &str[SUI_ADDRESS_PREFIX_STR_LENGTH..SUI_PREFIXED_ADDRESS_STR_LENGTH];

    let mut address = SuiAddressRaw::default();
    hex::decode_to_slice(str, &mut address).map_err(|_| Error::BadAddressHex)?;

    Ok(address)
}
