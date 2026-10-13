//! Segwit address encoding (BIP-173 bech32 for witness v0, BIP-350 bech32m for v1+),
//! used to show the Bitcoin destination of a Hashi withdrawal.

use arrayvec::ArrayString;

const CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const BECH32_CONST: u32 = 1;
const BECH32M_CONST: u32 = 0x2bc8_30a3;

/// `tb1p` + 52 data characters + 6 checksum characters = 62; the longest address shown.
pub const SEGWIT_ADDRESS_LEN: usize = 64;

/// Program data, regrouped to 5 bits, for a 32-byte program: ceil(256 / 5) = 52.
const MAX_DATA_5BIT: usize = 1 + 52;

fn polymod(values: impl Iterator<Item = u8>) -> u32 {
    const GEN: [u32; 5] = [
        0x3b6a_57b2,
        0x2650_8e6d,
        0x1ea1_19fa,
        0x3d42_33dd,
        0x2a14_62b3,
    ];
    let mut chk: u32 = 1;
    for v in values {
        let top = chk >> 25;
        chk = ((chk & 0x01ff_ffff) << 5) ^ v as u32;
        for (i, g) in GEN.iter().enumerate() {
            if (top >> i) & 1 == 1 {
                chk ^= g;
            }
        }
    }
    chk
}

/// Encodes a witness program as a segwit address. Only the shapes Hashi pays out are
/// accepted: v0 with a 20-byte program and v1 with a 32-byte program. Returns `None`
/// for anything else, and never panics.
pub fn segwit_address(
    hrp: &str,
    version: u8,
    program: &[u8],
) -> Option<ArrayString<SEGWIT_ADDRESS_LEN>> {
    let constant = match (version, program.len()) {
        (0, 20) => BECH32_CONST,
        (1, 32) => BECH32M_CONST,
        _ => return None,
    };
    if hrp.is_empty() || !hrp.bytes().all(|c| c.is_ascii_lowercase()) {
        return None;
    }

    // Witness version, then the program regrouped from 8-bit to 5-bit words.
    let mut data = [0u8; MAX_DATA_5BIT];
    data[0] = version;
    let mut n = 1;
    let mut acc: u32 = 0;
    let mut bits = 0;
    for &b in program {
        acc = (acc << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            *data.get_mut(n)? = ((acc >> bits) & 31) as u8;
            n += 1;
        }
    }
    if bits > 0 {
        *data.get_mut(n)? = ((acc << (5 - bits)) & 31) as u8;
        n += 1;
    }
    let data = &data[..n];

    let hrp_expanded = hrp
        .bytes()
        .map(|c| c >> 5)
        .chain(core::iter::once(0))
        .chain(hrp.bytes().map(|c| c & 31));
    let checksum = polymod(hrp_expanded.chain(data.iter().copied()).chain([0u8; 6])) ^ constant;

    let mut out = ArrayString::new();
    out.try_push_str(hrp).ok()?;
    out.try_push('1').ok()?;
    for &d in data {
        out.try_push(CHARSET[d as usize] as char).ok()?;
    }
    for i in 0..6 {
        out.try_push(CHARSET[((checksum >> (5 * (5 - i))) & 31) as usize] as char)
            .ok()?;
    }
    Some(out)
}
