use crate::parser::common::SUI_ADDRESS_LENGTH;

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use ledger_device_sdk::log::trace;
use ledger_device_sdk::tag_to_flag_u64;
use ledger_device_sdk::tlv::tlv_generic::{
    parse, Handler, ParseCfg, Received, Result, Tag, TlvData,
};
use ledger_device_sdk::tlv::TlvError;

#[cfg(feature = "speculos")]
use ledger_crypto_helpers::common::HexSlice;

// Tags
const PACKAGE_ADDRESS_TAG: Tag = 0x10;
const MODULE_TAG: Tag = 0x11;
const STRUCT_NAME_TAG: Tag = 0x12;

tag_to_flag_u64!(PACKAGE_ADDRESS_TAG, MODULE_TAG, STRUCT_NAME_TAG);

#[derive(Default, Debug)]
pub struct Tuid {
    pub package_addr: [u8; SUI_ADDRESS_LENGTH],
    pub module: String,
    pub struct_name: String,
}

fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    trace!("hex str {} - len({})\n", hex, hex.chars().count());

    let chars_utf8: Vec<char> = hex.chars().collect();
    let mut chars: &[char] = &chars_utf8[..];
    if matches!(chars_utf8[..], ['0', 'x', ..]) {
        chars = &chars_utf8[2..];
    }

    if !chars.len().is_multiple_of(2) {
        return None; // Must be even length
    }

    let mut bytes = Vec::with_capacity(chars.len() / 2);
    for chunk in chars.to_vec().chunks(2) {
        let high = hex_char_to_nibble(chunk[0])?;
        let low = hex_char_to_nibble(chunk[1])?;
        bytes.push((high << 4) | low);
    }

    trace!("hex decoded {} - len({})\n", HexSlice(&bytes), bytes.len());

    Some(bytes)
}

fn hex_char_to_nibble(c: char) -> Option<u8> {
    match c {
        '0'..='9' => Some(c as u8 - b'0'),
        'a'..='f' => Some(c as u8 - b'a' + 10),
        'A'..='F' => Some(c as u8 - b'A' + 10),
        _ => None,
    }
}

fn on_package_addr(d: &TlvData<'_>, out: &mut Tuid) -> Result<bool> {
    let utf8_str = d.as_str()?;

    trace!("bytes to utf8: {}\n", utf8_str);

    let ascii_bytes = match hex_decode(utf8_str) {
        Some(ascii) => ascii,
        None => {
            trace!("Error while converting contract address from utf8 to ascii string");
            return Err(TlvError::LengthOverflow);
        }
    };

    if ascii_bytes.len() < SUI_ADDRESS_LENGTH {
        return Err(TlvError::UnexpectedEof);
    }

    out.package_addr.copy_from_slice(ascii_bytes.as_slice());

    Ok(true)
}
fn on_module(d: &TlvData<'_>, out: &mut Tuid) -> Result<bool> {
    out.module = String::from(d.as_str()?);
    Ok(true)
}
fn on_struct_name(d: &TlvData<'_>, out: &mut Tuid) -> Result<bool> {
    out.struct_name = String::from(d.as_str()?);
    Ok(true)
}

static HANDLERS: &[Handler<Tuid>] = &[
    Handler {
        tag: PACKAGE_ADDRESS_TAG,
        unique: true,
        func: Some(on_package_addr),
    },
    Handler {
        tag: MODULE_TAG,
        unique: true,
        func: Some(on_module),
    },
    Handler {
        tag: STRUCT_NAME_TAG,
        unique: true,
        func: Some(on_struct_name),
    },
];

pub fn parse_tuid(payload: &[u8], out: &mut Tuid) -> Result<()> {
    let mut received = Received::new(tag_to_flag_u64);

    let cfg = ParseCfg::new(HANDLERS);

    parse(&cfg, payload, out, &mut received)?;

    // Check that mandatory TAGs were received
    let mandatory_tags = tag_to_flag_u64(PACKAGE_ADDRESS_TAG)
        | tag_to_flag_u64(MODULE_TAG)
        | tag_to_flag_u64(STRUCT_NAME_TAG);
    if received.flags & mandatory_tags != mandatory_tags {
        return Err(TlvError::MissingMandatoryTag);
    }

    Ok(())
}
