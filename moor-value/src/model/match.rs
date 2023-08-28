use binary_layout::LayoutAs;
use bincode::{Decode, Encode};
use strum::FromRepr;

#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr, Hash, Ord, PartialOrd, Encode, Decode)]
#[repr(u8)]
pub enum ArgSpec {
    None = 0,
    Any = 1,
    This = 2,
}

impl LayoutAs<u8> for ArgSpec {
    fn read(v: u8) -> Self {
        ArgSpec::from_repr(v).expect("Invalid ArgSpec value")
    }

    fn write(v: Self) -> u8 {
        v as u8
    }
}

impl ArgSpec {
    #[must_use]
    pub fn to_string(&self) -> &str {
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

/// The set of prepositions that are valid for verbs, corresponding to the set of string constants
/// in PREP_LIST, and for now at least much 1:1 with LambdaMOO's built-in prepositions, and
/// are referred to in the database.
/// TODO: Long run a proper table with some sort of dynamic look up and a way to add new ones and
///   internationalize and so on.
#[repr(u16)]
#[derive(Copy, Clone, Debug, FromRepr, Eq, PartialEq, Hash, Encode, Decode, Ord, PartialOrd)]
pub enum Preposition {
    WithUsing = 0,
    AtTo = 1,
    InFrontOf = 2,
    IntoIn = 3,
    OnTopOfOn = 4,
    OutOf = 5,
    Over = 6,
    Through = 7,
    Under = 8,
    Behind = 9,
    Beside = 10,
    ForAbout = 11,
    Is = 12,
    As = 13,
    OffOf = 14,
}

pub const PREP_LIST: [&str; 15] = [
    "with/using",
    "at/to",
    "in front of",
    "in/inside/into",
    "on top of/on/onto/upon",
    "out of/from inside/from",
    "over",
    "through",
    "under/underneath/beneath",
    "behind",
    "beside",
    "for/about",
    "is",
    "as",
    "off/off of",
];

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
            p => Self::Other(Preposition::from_repr(p as u16).expect("Invalid preposition")),
        }
    }

    #[must_use]
    pub fn to_bytes(&self) -> [u8; 2] {
        match self {
            Self::Any => (-2i16).to_le_bytes(),
            Self::None => (-1i16).to_le_bytes(),
            Self::Other(id) => (*id as i16).to_le_bytes(),
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

    fn write(v: Self) -> u32 {
        let mut r: u32 = 0;
        let dobj_value = ArgSpec::write(v.dobj);
        r |= dobj_value as u32;
        let prep_value = PrepSpec::write(v.prep);
        r |= (prep_value as u32 & 0xffff) << 8;
        let iobj_value = ArgSpec::write(v.iobj);
        r |= (iobj_value as u32) << 24;
        r
    }
}

#[cfg(test)]
mod tests {
    use binary_layout::LayoutAs;

    #[test]
    fn verbargs_spec_to_from_u32() {
        use super::{ArgSpec, PrepSpec, VerbArgsSpec};
        let spec = VerbArgsSpec {
            dobj: ArgSpec::This,
            prep: PrepSpec::None,
            iobj: ArgSpec::This,
        };
        let v = VerbArgsSpec::write(spec);
        let spec2 = VerbArgsSpec::read(v);
        assert_eq!(spec, spec2);
    }
}
