use int_enum::IntEnum;

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
)]
#[repr(u8)]
pub enum ArgSpec {
    None = 0,
    Any = 1,
    This = 2,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd
)]
pub enum PrepSpec {
    Any,
    None,
    Other(
        u16, /* matches Prep 'id' as returned from match_preposition */
    ),
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd
)]
pub struct VerbArgsSpec {
    pub dobj: ArgSpec,
    pub prep: PrepSpec,
    pub iobj: ArgSpec,
}

pub trait Match {}
