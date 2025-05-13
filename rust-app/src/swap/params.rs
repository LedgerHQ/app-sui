use arrayvec::ArrayVec;
use core::convert::{TryFrom, TryInto};
use core::mem;
use ledger_device_sdk::libcall;

use crate::interface::SuiAddressRaw;
use crate::swap::Error;

// Max SUI address str length is 32*2
const SUI_ADDRESS_STR_LENGTH: usize = 64;
const SUI_ADDRESS_PREFIX_STR_LENGTH: usize = 2;
const SUI_PREFIXED_ADDRESS_STR_LENGTH: usize =
    SUI_ADDRESS_STR_LENGTH + SUI_ADDRESS_PREFIX_STR_LENGTH;
const MAX_BIP32_PATH_LENGTH: usize = 5;
const BIP32_PATH_SEGMENT_LEN: usize = mem::size_of::<u32>();

mod custom {
    use super::SUI_PREFIXED_ADDRESS_STR_LENGTH;
    use ledger_device_sdk::libcall;

    pub type CheckAddressParams = libcall::swap::CheckAddressParams<
        { libcall::swap::DEFAULT_COIN_CONFIG_BUF_SIZE },
        SUI_PREFIXED_ADDRESS_STR_LENGTH,
    >;
    pub type CreateTxParams = libcall::swap::CreateTxParams<
        { libcall::swap::DEFAULT_COIN_CONFIG_BUF_SIZE },
        SUI_PREFIXED_ADDRESS_STR_LENGTH,
    >;
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
pub struct PrintableAmountParams {
    pub amount: u64,
}

impl TryFrom<&libcall::swap::PrintableAmountParams> for PrintableAmountParams {
    type Error = Error;

    fn try_from(params: &libcall::swap::PrintableAmountParams) -> Result<Self, Self::Error> {
        let amount = u64::from_be_bytes(
            params.amount[params.amount.len() - mem::size_of::<u64>()..]
                .try_into()
                .map_err(|_| Error::WrongAmountLength)?,
        );

        Ok(PrintableAmountParams { amount })
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
