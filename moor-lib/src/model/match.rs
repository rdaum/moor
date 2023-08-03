use bincode::{Decode, Encode};
use int_enum::IntEnum;

#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum, Hash, Ord, PartialOrd, Encode, Decode)]
#[repr(u8)]
pub enum ArgSpec {
    None = 0,
    Any = 1,
    This = 2,
}

impl ArgSpec {
    pub fn to_string(&self) -> &str {
        match self {
            ArgSpec::None => "none",
            ArgSpec::Any => "any",
            ArgSpec::This => "this",
        }
    }
    pub fn from_string(repr: &str) -> Option<ArgSpec> {
        match repr {
            "none" => Some(ArgSpec::None),
            "any" => Some(ArgSpec::Any),
            "this" => Some(ArgSpec::This),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Encode, Decode)]
pub enum PrepSpec {
    Any,
    None,
    Other(
        u16, /* matches Prep 'id' as returned from match_preposition, matching offset into PREP_LIST */
    ),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Encode, Decode)]
pub struct VerbArgsSpec {
    pub dobj: ArgSpec,
    pub prep: PrepSpec,
    pub iobj: ArgSpec,
}

impl VerbArgsSpec {
    pub fn this_none_this() -> Self {
        VerbArgsSpec {
            dobj: ArgSpec::This,
            prep: PrepSpec::None,
            iobj: ArgSpec::This,
        }
    }
    pub fn matches(&self, v: &Self) -> bool {
        (self.dobj == ArgSpec::Any || self.dobj == v.dobj)
            && (self.prep == PrepSpec::Any || self.prep == v.prep)
            && (self.iobj == ArgSpec::Any || self.iobj == v.iobj)
    }
}
