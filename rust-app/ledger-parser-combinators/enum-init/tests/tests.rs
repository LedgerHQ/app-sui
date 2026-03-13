#[macro_use]
extern crate enum_init;

#[derive(InPlaceInit)]
#[repr(u8)]
pub enum TestEnum {
    UnitVariant,
    StringVariant(String),
}

#[derive(InPlaceInit)]
#[repr(u8)]
pub enum TestEnum2<A> {
    UnitVariant,
    StringVariant(A),
}

#[derive(InPlaceInit)]
#[repr(u8)]
pub enum TestEnum3<A> {
    VariantVec(Vec<A>),
}

pub trait Z {
    type Q;
}

#[derive(InPlaceInit)]
#[repr(u8)]
pub enum TestEnum4<A>
where
    A: Z,
{
    VariantA(<A as Z>::Q),
    VariantB(Vec<<A as Z>::Q>),
}

/*
pub trait ZZ<A> {
}

#[derive(InPlaceInit)]
#[repr(u8)]
pub enum TestEnum4<A, B: ZZ<A>> {
    VariantA(B),
}
*/
