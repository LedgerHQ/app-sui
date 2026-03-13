use core::ops::Mul;

#[derive(FromPrimitive, PartialEq, Debug)]
#[repr(u8)]
pub enum ProtobufWire {
    Varint = 0,
    Fixed64Bit = 1,
    LengthDelimited = 2,
    StartGroup = 3, // Deprecated
    EndGroup = 4,   // Deprecated
    Fixed32Bit = 5,
}

pub trait ProtobufWireFormat {
    const FORMAT: ProtobufWire;
}

macro_rules! VarintPrimitive {
    { $name:ident } =>
    { $crate::protobufs::async_parser::paste! {
        pub struct [<$name:camel>];
        impl ProtobufWireFormat for [<$name:camel>] {
            const FORMAT: ProtobufWire = ProtobufWire::Varint;
        }
    } }
}

VarintPrimitive! { int32 }
VarintPrimitive! { int64 }
VarintPrimitive! { uint32 }
VarintPrimitive! { uint64 }
VarintPrimitive! { sint32 }
VarintPrimitive! { sint64 }
VarintPrimitive! { bool }

// Varint
// pub struct Varint;
/*
impl ProtobufWireFormat for Varint {
    const FORMAT: ProtobufWire = ProtobufWire::Varint;
}
*/

// 64 Bit
pub struct Fixed64;
impl ProtobufWireFormat for Fixed64 {
    const FORMAT: ProtobufWire = ProtobufWire::Fixed64Bit;
}

pub struct Sfixed64;
impl ProtobufWireFormat for Sfixed64 {
    const FORMAT: ProtobufWire = ProtobufWire::Fixed64Bit;
}

pub struct Double;
impl ProtobufWireFormat for Double {
    const FORMAT: ProtobufWire = ProtobufWire::Fixed64Bit;
}

// Uses WireLengthDelimted
pub struct String;
impl ProtobufWireFormat for String {
    const FORMAT: ProtobufWire = ProtobufWire::LengthDelimited;
}
pub struct Bytes;
impl ProtobufWireFormat for Bytes {
    const FORMAT: ProtobufWire = ProtobufWire::LengthDelimited;
}
pub struct Embedded;
impl ProtobufWireFormat for Embedded {
    const FORMAT: ProtobufWire = ProtobufWire::LengthDelimited;
}
pub struct PackedRepeatedFields;
impl ProtobufWireFormat for PackedRepeatedFields {
    const FORMAT: ProtobufWire = ProtobufWire::LengthDelimited;
}

// Wire32Bit
pub struct Fixed32;
impl ProtobufWireFormat for Fixed32 {
    const FORMAT: ProtobufWire = ProtobufWire::Fixed32Bit;
}
pub struct Sfixed32;
impl ProtobufWireFormat for Sfixed32 {
    const FORMAT: ProtobufWire = ProtobufWire::Fixed32Bit;
}
pub struct Float;
impl ProtobufWireFormat for Float {
    const FORMAT: ProtobufWire = ProtobufWire::Fixed32Bit;
}

// Fields of a message
// A MessageField describes a message containing a particular field with the given type.
pub struct MessageField<const N: u32, Value>(Value);

// Denote the intersection of two formats.
pub struct Product<A, B>(A, B);
pub struct ProductIdentity;

impl<A> Mul<A> for ProductIdentity {
    type Output = Product<ProductIdentity, A>;
    fn mul(self, rhs: A) -> Self::Output {
        Product(self, rhs)
    }
}
impl<A, B, C> Mul<C> for Product<A, B> {
    type Output = Product<A, Product<B, C>>;
    fn mul(self, rhs: C) -> Self::Output {
        Product(self.0, Product(self.1, rhs))
    }
}
