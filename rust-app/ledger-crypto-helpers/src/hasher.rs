use core::convert::TryInto;
use core::default::Default;
use core::fmt;
use core::fmt::Write;
use core::ops::DerefMut;
use ledger_device_sdk::sys::*;
use zeroize::{Zeroize, Zeroizing};

pub trait Hasher {
    const N: usize;
    fn new() -> Self;
    fn update(&mut self, bytes: &[u8]);
    fn finalize<H: Hash<{ Self::N }>>(&mut self) -> H;
    fn clear(&mut self);
}

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
        write!(f, "{}", crate::common::HexSlice(&self.0))
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

#[derive(Clone, Copy)]
pub struct SHA256(cx_sha256_s);

impl Hasher for SHA256 {
    const N: usize = 32;
    fn new() -> Self {
        let mut rv = cx_sha256_s::default();
        unsafe { cx_sha256_init_no_throw(&mut rv) };
        Self(rv)
    }

    fn clear(&mut self) {
        unsafe { cx_sha256_init_no_throw(&mut self.0) };
    }

    fn update(&mut self, bytes: &[u8]) {
        unsafe {
            cx_hash_update(
                &mut self.0 as *mut cx_sha256_s as *mut cx_hash_t,
                bytes.as_ptr(),
                bytes.len(),
            );
        }
    }

    fn finalize<H: Hash<32>>(&mut self) -> H {
        let mut rv = H::new([0; 32]);
        unsafe {
            cx_hash_final(
                &mut self.0 as *mut cx_sha256_s as *mut cx_hash_t,
                rv.as_mut_ptr(),
            )
        };
        rv
    }
}

#[derive(Clone, Copy)]
pub struct SHA512(cx_sha512_s);

impl Hasher for SHA512 {
    const N: usize = 64;
    fn new() -> SHA512 {
        let mut rv = cx_sha512_s::default();
        unsafe { cx_sha512_init_no_throw(&mut rv) };
        Self(rv)
    }

    fn clear(&mut self) {
        unsafe { cx_sha512_init_no_throw(&mut self.0) };
    }

    fn update(&mut self, bytes: &[u8]) {
        unsafe {
            cx_hash_update(
                &mut self.0 as *mut cx_sha512_s as *mut cx_hash_t,
                bytes.as_ptr(),
                bytes.len(),
            );
        }
    }

    fn finalize<H: Hash<64>>(&mut self) -> H {
        let mut rv = H::new([0; 64]);
        unsafe {
            cx_hash_final(
                &mut self.0 as *mut cx_sha512_s as *mut cx_hash_t,
                rv.as_mut_ptr(),
            )
        };
        rv
    }
}

#[derive(Clone, Copy)]
pub struct SHA3<const N: usize>(cx_sha3_s);

pub type SHA3_224 = SHA3<{ 224 / 8 }>;
pub type SHA3_256 = SHA3<{ 256 / 8 }>;
pub type SHA3_384 = SHA3<{ 384 / 8 }>;
pub type SHA3_512 = SHA3<{ 512 / 8 }>;

impl<const N: usize> Hasher for SHA3<N> {
    const N: usize = N;
    fn new() -> SHA3<N> {
        let mut rv = Self(cx_sha3_s::default());
        rv.clear();
        rv
    }

    fn clear(&mut self) {
        unsafe { cx_sha3_init_no_throw(&mut self.0, (N * 8).try_into().unwrap()) };
    }

    fn update(&mut self, bytes: &[u8]) {
        unsafe {
            cx_hash_update(
                &mut self.0 as *mut cx_sha3_s as *mut cx_hash_t,
                bytes.as_ptr(),
                bytes.len(),
            );
        }
    }

    fn finalize<H: Hash<{ Self::N }>>(&mut self) -> H {
        let mut rv = H::new([0; { Self::N }]);
        unsafe {
            cx_hash_final(
                &mut self.0 as *mut cx_sha3_s as *mut cx_hash_t,
                rv.as_mut_ptr(),
            )
        };
        rv
    }
}

#[derive(Clone, Copy)]
pub struct Blake2b(cx_blake2b_s);

impl Hasher for Blake2b {
    const N: usize = 32;
    fn new() -> Self {
        let mut rv = cx_blake2b_s::default();
        unsafe { cx_blake2b_init_no_throw(&mut rv, 256) };
        Self(rv)
    }

    fn clear(&mut self) {
        unsafe { cx_blake2b_init_no_throw(&mut self.0, 256) };
    }

    fn update(&mut self, bytes: &[u8]) {
        unsafe {
            cx_hash_update(
                &mut self.0 as *mut cx_blake2b_s as *mut cx_hash_t,
                bytes.as_ptr(),
                bytes.len(),
            );
        }
    }

    fn finalize<H: Hash<32>>(&mut self) -> H {
        let mut rv = H::new([0; 32]);
        unsafe {
            cx_hash_final(
                &mut self.0 as *mut cx_blake2b_s as *mut cx_hash_t,
                rv.as_mut_ptr(),
            )
        };
        rv
    }
}

impl Write for Blake2b {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.update(s.as_bytes());
        Ok(())
    }
}
