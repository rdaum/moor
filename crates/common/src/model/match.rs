// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use binary_layout::LayoutAs;
use strum::FromRepr;

use crate::{
    matching::{Preposition, find_preposition},
    model::PrepSpec::Other,
};
use moor_var::encode::{DecodingError, EncodingError};

#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr, Hash, Ord, PartialOrd)]
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

/// Match a preposition for the form used by set_verb_args and friends, which means it must support
/// numeric arguments for the preposition.
pub fn parse_preposition_spec(repr: &str) -> Option<PrepSpec> {
    match repr {
        "any" => Some(PrepSpec::Any),
        "none" => Some(PrepSpec::None),
        _ => find_preposition(repr).map(PrepSpec::Other),
    }
}

/// Get the English representation of a preposition spec.
pub fn preposition_to_string(ps: &PrepSpec) -> &str {
    match ps {
        PrepSpec::Any => "any",
        PrepSpec::None => "none",
        PrepSpec::Other(id) => id.to_string(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum PrepSpec {
    Any,
    None,
    Other(Preposition),
}

impl PrepSpec {
    pub fn parse(s: &str) -> Option<Self> {
        if s == "any" {
            Some(Self::Any)
        } else if s == "none" {
            Some(Self::None)
        } else {
            let p = Preposition::parse(s)?;
            Some(Other(p))
        }
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
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

    pub fn none_none_none() -> Self {
        Self {
            dobj: ArgSpec::None,
            prep: PrepSpec::None,
            iobj: ArgSpec::None,
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
