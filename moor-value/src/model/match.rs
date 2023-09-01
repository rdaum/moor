use crate::model::Preposition;
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
    #[must_use]
    pub fn to_string(&self) -> &str {
        match self {
            Self::None => "none",
            Self::Any => "any",
            Self::This => "this",
        }
    }
    #[must_use]
    pub fn from_string(repr: &str) -> Option<Self> {
        match repr {
            "none" => Some(Self::None),
            "any" => Some(Self::Any),
            "this" => Some(Self::This),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Encode, Decode)]
pub enum PrepSpec {
    Any,
    None,
    Other(Preposition),
}

impl PrepSpec {
    #[must_use]
    pub fn from_bytes(bytes: [u8; 2]) -> Self {
        let int_value = i16::from_le_bytes(bytes);
        match int_value {
            -2 => Self::Any,
            -1 => Self::None,
            _ => Self::Other(Preposition::from_int(int_value as u16).unwrap()),
        }
    }
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 2] {
        match self {
            Self::Any => (-2i16).to_le_bytes(),
            Self::None => (-1i16).to_le_bytes(),
            Self::Other(id) => id.int_value().to_le_bytes(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Encode, Decode)]
pub struct VerbArgsSpec {
    pub dobj: ArgSpec,
    pub prep: PrepSpec,
    pub iobj: ArgSpec,
}

impl VerbArgsSpec {
    #[must_use]
    pub fn this_none_this() -> Self {
        Self {
            dobj: ArgSpec::This,
            prep: PrepSpec::None,
            iobj: ArgSpec::This,
        }
    }
    #[must_use]
    pub fn matches(&self, v: &Self) -> bool {
        (self.dobj == ArgSpec::Any || self.dobj == v.dobj)
            && (self.prep == PrepSpec::Any || self.prep == v.prep)
            && (self.iobj == ArgSpec::Any || self.iobj == v.iobj)
    }
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 4] {
        let mut bytes = [0u8; 4];
        bytes[0] = self.dobj as u8;
        bytes[1] = self.iobj as u8;
        bytes[2..4].copy_from_slice(&self.prep.to_bytes());
        bytes
    }
    // TODO Actually keep the args spec encoded as bytes and use setters/getters instead
    #[must_use]
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        let dobj = ArgSpec::from_int(bytes[0]).unwrap();
        let iobj = ArgSpec::from_int(bytes[1]).unwrap();
        let prep = PrepSpec::from_bytes([bytes[2], bytes[3]]);
        Self { dobj, prep, iobj }
    }
}
