// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use binary_layout::LayoutAs;
use bincode::{Decode, Encode};
use strum::FromRepr;

use crate::encode::{DecodingError, EncodingError};

#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr, Hash, Ord, PartialOrd, Encode, Decode)]
#[repr(u8)]
pub enum ArgSpec {
    None = 0,
    Any = 1,
    This = 2,
}

impl LayoutAs<u8> for ArgSpec {
    type ReadError = DecodingError;
    type WriteError = EncodingError;

    fn try_read(v: u8) -> Result<Self, Self::ReadError> {
        Self::from_repr(v).ok_or(DecodingError::InvalidArgSpecValue(v))
    }

    fn try_write(v: Self) -> Result<u8, Self::WriteError> {
        Ok(v as u8)
    }
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

/// The set of prepositions that are valid for verbs, corresponding to the set of string constants
/// defined in LambdaMOO 1.8.1.
/// TODO: Refactor/rethink preposition enum.
///   Long run a proper table with some sort of dynamic look up and a way to add new ones and
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

impl Preposition {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "with" | "using" => Some(Self::WithUsing),
            "at" | "to" => Some(Self::AtTo),
            "in front of" => Some(Self::InFrontOf),
            "in" | "inside" | "into" => Some(Self::IntoIn),
            "on top of" | "on" | "onto" | "upon" => Some(Self::OnTopOfOn),
            "out of" | "from inside" | "from" => Some(Self::OutOf),
            "over" => Some(Self::Over),
            "through" => Some(Self::Through),
            "under" | "underneath" | "beneath" => Some(Self::Under),
            "behind" => Some(Self::Behind),
            "beside" => Some(Self::Beside),
            "for" | "about" => Some(Self::ForAbout),
            "is" => Some(Self::Is),
            "as" => Some(Self::As),
            "off" | "off of" => Some(Self::OffOf),
            _ => None,
        }
    }
    pub fn to_string(&self) -> &str {
        match self {
            Self::WithUsing => "with/using",
            Self::AtTo => "at/to",
            Self::InFrontOf => "in front of",
            Self::IntoIn => "in/inside/into",
            Self::OnTopOfOn => "on top of/on/onto/upon",
            Self::OutOf => "out of/from inside/from",
            Self::Over => "over",
            Self::Through => "through",
            Self::Under => "under/underneath/beneath",
            Self::Behind => "behind",
            Self::Beside => "beside",
            Self::ForAbout => "for/about",
            Self::Is => "is",
            Self::As => "as",
            Self::OffOf => "off/off of",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Encode, Decode)]
pub enum PrepSpec {
    Any,
    None,
    Other(Preposition),
}

impl LayoutAs<i16> for PrepSpec {
    type ReadError = DecodingError;
    type WriteError = EncodingError;

    fn try_read(v: i16) -> Result<Self, Self::ReadError> {
        match v {
            -2 => Ok(Self::Any),
            -1 => Ok(Self::None),
            p => Ok(Self::Other(
                Preposition::from_repr(p as u16).ok_or(DecodingError::InvalidPrepValue(p))?,
            )),
        }
    }

    fn try_write(v: Self) -> Result<i16, Self::WriteError> {
        Ok(match v {
            Self::Any => -2,
            Self::None => -1,
            Self::Other(p) => p as i16,
        })
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
}

impl LayoutAs<u32> for VerbArgsSpec {
    type ReadError = DecodingError;
    type WriteError = EncodingError;

    fn try_read(v: u32) -> Result<Self, Self::ReadError> {
        let dobj_value = v & 0x0000_00ff;
        let prep_value = ((v >> 8) & 0x0000_ffff) as i16;
        let iobj_value = (v >> 24) & 0x0000_00ff;
        let dobj = ArgSpec::try_read(dobj_value as u8)?;
        let prep = PrepSpec::try_read(prep_value)?;
        let iobj = ArgSpec::try_read(iobj_value as u8)?;
        Ok(Self { dobj, prep, iobj })
    }

    fn try_write(v: Self) -> Result<u32, Self::WriteError> {
        let mut r: u32 = 0;
        let dobj_value = ArgSpec::try_write(v.dobj)?;
        r |= u32::from(dobj_value);
        let prep_value = PrepSpec::try_write(v.prep)?;
        r |= (prep_value as u32 & 0xffff) << 8;
        let iobj_value = ArgSpec::try_write(v.iobj)?;
        r |= u32::from(iobj_value) << 24;
        Ok(r)
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
        let v = VerbArgsSpec::try_write(spec).unwrap();
        let spec2 = VerbArgsSpec::try_read(v).unwrap();
        assert_eq!(spec, spec2);
    }
}
