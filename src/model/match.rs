use bincode::{Decode, Encode};
use int_enum::IntEnum;

/// TODO verb-matching
///
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum, Encode, Decode)]
#[repr(u8)]
pub enum ArgSpec {
    None = 0,
    Any = 1,
    This = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum, Encode, Decode)]
#[repr(u8)]
pub enum PrepSpec {
    Any = 1,
    None = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Encode, Decode)]
pub struct VerbArgsSpec {
    dobj: ArgSpec,
    prep: PrepSpec,
    iobj: ArgSpec,
}

pub trait Match {}
