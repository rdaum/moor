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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Encode, Decode)]
pub enum PrepSpec {
    Any,
    None,
    Other(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Encode, Decode)]
pub struct VerbArgsSpec {
    pub dobj: ArgSpec,
    pub prep: PrepSpec,
    pub iobj: ArgSpec,
}

pub trait Match {}
