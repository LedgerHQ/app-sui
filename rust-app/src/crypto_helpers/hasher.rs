use core::default::Default;
use core::fmt;
use core::ops::DerefMut;
use zeroize::{Zeroize, Zeroizing};

pub trait Hash<const N: usize> {
    fn new(v: [u8; N]) -> Self;
    fn as_mut_ptr(&mut self) -> *mut u8;
}

impl<const N: usize> Hash<N> for [u8; N] {
    fn new(v: [u8; N]) -> Self {
        v
    }
    fn as_mut_ptr(&mut self) -> *mut u8 {
        (self as &mut [u8]).as_mut_ptr()
    }
}

impl<const N: usize, H: Hash<N> + zeroize::Zeroize> Hash<N> for Zeroizing<H> {
    fn new(v: [u8; N]) -> Self {
        Zeroizing::new(H::new(v))
    }
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.deref_mut().as_mut_ptr()
    }
}

#[derive(Clone, Copy)]
pub struct HexHash<const N: usize>(pub [u8; N]);

impl<const N: usize> Hash<N> for HexHash<N> {
    fn new(v: [u8; N]) -> Self {
        HexHash(v)
    }
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0[..].as_mut_ptr() // Slice apparently required; hangs if not used.
    }
}

impl<const N: usize> fmt::Display for HexHash<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", crate::crypto_helpers::common::HexSlice(&self.0))
    }
}

impl<const N: usize> Default for HexHash<N> {
    fn default() -> Self {
        Self([Default::default(); N])
    }
}
// Skip because Zeroize impl below is more safe re stack usage
//impl<const N: usize> zeroize::DefaultIsZeroes for HexHash<N> { }

impl<const N: usize> Zeroize for HexHash<N> {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

pub struct Base64Hash<const N: usize>(pub [u8; N]);

impl<const N: usize> Base64Hash<N> {
    const BUF_SIZE: usize = (N / 3) * 4 + 4;
}

impl<const N: usize> Hash<N> for Base64Hash<N> {
    fn new(v: [u8; N]) -> Self {
        Base64Hash(v)
    }
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0[..].as_mut_ptr() // Slice apparently required; hangs if not used.
    }
}

impl<const N: usize> fmt::Display for Base64Hash<N>
where
    [(); Self::BUF_SIZE]:,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Select a sufficiently large buf size for handling hashes of up to 64 bytes
        // const OUT_BUF_SIZE: usize = (N/3)*4;
        let mut buf: [u8; Self::BUF_SIZE] = [0; Self::BUF_SIZE];
        let bytes_written = base64::encode_config_slice(self.0, base64::URL_SAFE_NO_PAD, &mut buf);
        let str = core::str::from_utf8(&buf[0..bytes_written]).or(Err(core::fmt::Error))?;
        write!(f, "{}", str)
    }
}

impl<const N: usize> Default for Base64Hash<N> {
    fn default() -> Self {
        Self([Default::default(); N])
    }
}
// Skip because Zeroize impl below is more safe re stack usage
//impl<const N: usize> zeroize::DefaultIsZeroes for HexHash<N> { }

impl<const N: usize> Zeroize for Base64Hash<N> {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}
