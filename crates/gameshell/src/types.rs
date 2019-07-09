//! Basic types used by the gameshell for input to handlers
/// Basic types used by the gameshell for input to handlers
#[derive(Clone, Debug)]
pub enum Type {
    /// A string that contains no whitespace
    Atom(String),
    /// A `true` or `false` value
    Bool(bool),
    /// A string which was enclosed by parentheses, may contain parentheses itself
    Command(String),
    /// A 32-bit floating point value
    F32(f32),
    /// A 32-bit signed integer value
    I32(i32),
    /// A string, can be created using (#)
    String(String),
    /// An unsigned 8-bit value
    U8(u8),
}

impl quickcheck::Arbitrary for Type {
    fn arbitrary<G: quickcheck::Gen>(g: &mut G) -> Self {
        use rand::Rng;
        match g.gen_range(0, 7) {
            0 => Type::Atom(String::arbitrary(g)),
            1 => Type::Bool(bool::arbitrary(g)),
            2 => Type::Command(String::arbitrary(g)),
            3 => Type::F32(f32::arbitrary(g)),
            4 => Type::I32(i32::arbitrary(g)),
            5 => Type::String(String::arbitrary(g)),
            6 => Type::U8(u8::arbitrary(g)),
            _ => unimplemented![],
        }
    }
}