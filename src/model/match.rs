use bincode::{Decode, Encode};
use int_enum::IntEnum;

/// TODO verb-matching
///
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum, Encode, Decode, Hash, Ord, PartialOrd)]
#[repr(u8)]
pub enum ArgSpec {
    None = 0,
    Any = 1,
    This = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Encode, Decode, Hash, Ord, PartialOrd)]
pub enum PrepSpec {
    Any,
    None,
    Other(
        u16, /* matches Prep 'id' as returned from match_preposition */
    ),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Encode, Decode, Hash, Ord, PartialOrd)]
pub struct VerbArgsSpec {
    pub dobj: ArgSpec,
    pub prep: PrepSpec,
    pub iobj: ArgSpec,
}

pub trait Match {}
