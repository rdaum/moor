use int_enum::IntEnum;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    IntEnum,
    Hash,
    Ord,
    PartialOrd,
    Archive,
    Serialize,
    Deserialize,
)]
#[archive(compare(PartialEq), check_bytes)]
#[repr(u8)]
pub enum ArgSpec {
    None = 0,
    Any = 1,
    This = 2,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Archive, Serialize, Deserialize,
)]
#[archive(compare(PartialEq), check_bytes)]
pub enum PrepSpec {
    Any,
    None,
    Other(
        u16, /* matches Prep 'id' as returned from match_preposition */
    ),
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Archive, Serialize, Deserialize,
)]
#[archive(compare(PartialEq), check_bytes)]
pub struct VerbArgsSpec {
    pub dobj: ArgSpec,
    pub prep: PrepSpec,
    pub iobj: ArgSpec,
}

pub trait Match {}
