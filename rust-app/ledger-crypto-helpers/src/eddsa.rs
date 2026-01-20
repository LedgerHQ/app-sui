use arrayvec::ArrayVec;
use ledger_device_sdk::ecc::*;
use ledger_device_sdk::io::SyscallError;
use ledger_device_sdk::sys::*;

use crate::common::*;

#[derive(Clone, Debug, PartialEq)]
pub struct EdDSASignature(pub [u8; 64]);

pub type Ed25519PublicKey = ECPublicKey<65, 'E'>;

pub fn eddsa_sign(
    path: &ArrayVec<u32, 10>,
    slip10: bool,
    m: &[u8],
) -> Result<EdDSASignature, CryptographyError> {
    with_private_key(path, slip10, |k| eddsa_sign_int(k, m))
}

pub fn eddsa_sign_int(
    privkey: &ECPrivateKey<32, 'E'>,
    m: &[u8],
) -> Result<EdDSASignature, CryptographyError> {
    let sig = privkey.sign(m)?;
    Ok(EdDSASignature(sig.0))
}

pub fn with_private_key<A, E>(
    path: &[u32],
    slip10: bool,
    f: impl FnOnce(&mut ledger_device_sdk::ecc::ECPrivateKey<32, 'E'>) -> Result<A, E>,
) -> Result<A, E> {
    if slip10 {
        f(&mut ledger_device_sdk::ecc::Ed25519::derive_from_path_slip10(path))
    } else {
        f(&mut ledger_device_sdk::ecc::Ed25519::derive_from_path(path))
    }
}

pub fn with_public_keys<V, E, A: Address<A, Ed25519PublicKey>, F>(
    path: &[u32],
    slip10: bool,
    f: F,
) -> Result<V, E>
where
    E: From<CryptographyError>,
    F: FnOnce(&ledger_device_sdk::ecc::ECPublicKey<65, 'E'>, &A) -> Result<V, E>,
{
    with_private_key(path, slip10, |k| with_public_keys_int(k, f))
}

pub fn with_public_keys_int<V, E, A: Address<A, Ed25519PublicKey>, F>(
    privkey: &ECPrivateKey<32, 'E'>,
    f: F,
) -> Result<V, E>
where
    E: From<CryptographyError>,
    F: FnOnce(&ledger_device_sdk::ecc::ECPublicKey<65, 'E'>, &A) -> Result<V, E>,
{
    let mut pubkey = privkey
        .public_key()
        .map_err(Into::<CryptographyError>::into)?;
    call_c_api_function!(cx_edwards_compress_point_no_throw(
        CX_CURVE_Ed25519,
        pubkey.pubkey.as_mut_ptr(),
        pubkey.keylength
    ))
    .map_err(Into::<CryptographyError>::into)?;
    pubkey.keylength = 33;
    let pkh = <A as Address<A, Ed25519PublicKey>>::get_address(&pubkey)
        .map_err(Into::<CryptographyError>::into)?;
    f(&pubkey, &pkh)
}

pub struct Ed25519RawPubKeyAddress(ledger_device_sdk::ecc::ECPublicKey<65, 'E'>);

impl Address<Ed25519RawPubKeyAddress, ledger_device_sdk::ecc::ECPublicKey<65, 'E'>>
    for Ed25519RawPubKeyAddress
{
    fn get_address(
        key: &ledger_device_sdk::ecc::ECPublicKey<65, 'E'>,
    ) -> Result<Self, SyscallError> {
        Ok(Ed25519RawPubKeyAddress(key.clone()))
    }
    fn get_binary_address(&self) -> &[u8] {
        ed25519_public_key_bytes(&self.0)
    }
}
impl core::fmt::Display for Ed25519RawPubKeyAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", HexSlice(&self.0.pubkey[1..self.0.keylength]))
    }
}

pub fn ed25519_public_key_bytes(key: &Ed25519PublicKey) -> &[u8] {
    &key.pubkey[1..33]
}
