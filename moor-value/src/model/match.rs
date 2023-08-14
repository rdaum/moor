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

impl PrepSpec {
    pub fn from_bytes(bytes: [u8; 2]) -> PrepSpec {
        let int_value = i16::from_le_bytes(bytes);
        match int_value {
            -2 => PrepSpec::Any,
            -1 => PrepSpec::None,
            _ => PrepSpec::Other(int_value as u16),
        }
    }
    pub fn to_bytes(&self) -> [u8; 2] {
        match self {
            PrepSpec::Any => (-2i16).to_le_bytes(),
            PrepSpec::None => (-1i16).to_le_bytes(),
            PrepSpec::Other(id) => (*id as i16).to_le_bytes(),
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
    pub fn to_bytes(&self) -> [u8; 4] {
        let mut bytes = [0u8; 4];
        bytes[0] = self.dobj as u8;
        bytes[1] = self.iobj as u8;
        bytes[2..4].copy_from_slice(&self.prep.to_bytes());
        bytes
    }
    // TODO Actually keep the args spec encoded as bytes and use setters/getters instead
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        let dobj = ArgSpec::from_int(bytes[0]).unwrap();
        let iobj = ArgSpec::from_int(bytes[1]).unwrap();
        let prep = PrepSpec::from_bytes([bytes[2], bytes[3]]);
        VerbArgsSpec { dobj, iobj, prep }
    }
}
