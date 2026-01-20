#[derive(PartialEq, Eq)]
#[cfg(version("1.75"))]
#[derive(core::marker::ConstParamTy)]
pub enum Endianness {
    Big,
    Little,
}

pub trait FixedSized {
    // doesn't yet work
    //const Size: usize;
    type Array;
}

pub trait Convert<const E: Endianness>: FixedSized {
    fn deserialize(bytes: Self::Array) -> Self;
}

macro_rules! impl_convert {
    ($t:ty, $s:expr) => {
        impl FixedSized for $t {
            type Array = [u8; $s];
        }

        impl Convert<{ Endianness::Big }> for $t {
            fn deserialize(bytes: Self::Array) -> Self {
                <$t>::from_be_bytes(bytes)
            }
        }

        impl Convert<{ Endianness::Little }> for $t {
            fn deserialize(bytes: Self::Array) -> Self {
                <$t>::from_le_bytes(bytes)
            }
        }
    };
}

impl_convert! { u16, 2 }
impl_convert! { u32, 4 }
impl_convert! { u64, 8 }
