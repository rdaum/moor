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

use crate::encode::{DecodingError, EncodingError};
use crate::{AsByteBuffer, Symbol};
use bincode::{Decode, Encode};
use byteview::ByteView;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};
use ustr::Ustr;

/// The "system" object in MOO is a place where a bunch of basic sys functionality hangs off of, and
/// from where $name style references hang off of. A bit like the Lobby in Self.
pub const SYSTEM_OBJECT: Obj = Obj::mk_id(0);

/// Used throughout to refer to a missing object value.
pub const NOTHING: Obj = Obj::mk_id(-1);
/// Used in matching to indicate that the match was ambiguous on multiple objects in the
/// environment.
pub const AMBIGUOUS: Obj = Obj::mk_id(-2);
/// Used in matching to indicate that the match failed to find any objects in the environment.
pub const FAILED_MATCH: Obj = Obj::mk_id(-3);

/// A reference to an object.
/// For now this is the global unique DB object id.
/// In the future this may also encode other object types (anonymous objects, etc)
#[derive(
    Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode, Serialize, Deserialize,
)]
pub struct Obj(u64);

const OBJID_TYPE_CODE: u8 = 0;
const OBJLABEL_TYPE_CODE: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Objid(pub i32);

// Internal representation is lower 32 bits is db object id (if a db object), top 3 bits is a "type"
// code, with the remaining 13-bits unused for now.
impl Obj {
    fn decode_as_objid(&self) -> i32 {
        // Mask out upper 32 bits
        (self.0 & 0x0000_0000_ffff_ffff) as i32
    }

    fn decode_as_objlabel(&self) -> Ustr {
        // Mask off upper 3 bits, we only need the first 61 bits
        let lower_bits = self.0 & 0x0000_7fff_ffff_ffff;
        unsafe { std::mem::transmute::<u64, Ustr>(lower_bits) }
    }

    const fn encode_as_objid(id: i32) -> Self {
        // Transmute to u64 and then mask on the type code
        // Doing "as u32" here would sign extend the i32 to u64, which is not what we want.
        let as_u32 = unsafe { std::mem::transmute::<i32, u32>(id) };
        let as_u64 = as_u32 as u64;
        Self((as_u64 & 0x0000_ffff_ffff) | ((OBJID_TYPE_CODE as u64) << 61))
    }

    fn encode_as_objlabel(label: Ustr) -> Self {
        let as_u64 = unsafe { std::mem::transmute::<Ustr, u64>(label) };
        Self(as_u64 | ((OBJLABEL_TYPE_CODE as u64) << 61))
    }

    fn object_type_code(&self) -> u8 {
        (self.0 >> 61) as u8
    }
}

impl Display for Obj {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.object_type_code() {
            OBJID_TYPE_CODE => f.write_fmt(format_args!("#{}", self.decode_as_objid())),
            OBJLABEL_TYPE_CODE => f.write_fmt(format_args!("#{}", self.decode_as_objlabel())),
            _ => panic!("Invalid object type code"),
        }
    }
}

impl Display for Objid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("#{}", self.0))
    }
}

impl Obj {
    pub const fn mk_id(id: i32) -> Self {
        Self::encode_as_objid(id)
    }

    pub fn mk_label(label: &str) -> Self {
        Self::encode_as_objlabel(Ustr::from(label))
    }

    pub fn from_literal(literal: &str) -> Option<Self> {
        // First part must be #
        if !literal.starts_with('#') {
            return None;
        }
        // If it's an integer, then it's an Objid, otherwise a label
        match literal[1..].parse::<i32>() {
            Ok(n) => Some(Self::mk_id(n)),
            Err(_) => {
                // If it's not an integer, then it's a label
                Some(Self::mk_label(&literal[1..]))
            }
        }
    }

    #[must_use]
    pub fn to_literal(&self) -> String {
        match self.object_type_code() {
            OBJID_TYPE_CODE => format!("#{}", self.decode_as_objid()),
            OBJLABEL_TYPE_CODE => format!("#{}", self.decode_as_objlabel()),
            _ => panic!("Invalid object type code"),
        }
    }

    #[must_use]
    pub fn is_sysobj(&self) -> bool {
        self.decode_as_objid() == 0
    }

    pub fn is_nothing(&self) -> bool {
        self.object_type_code() == OBJID_TYPE_CODE && self.decode_as_objid() == -1
    }

    pub fn is_positive(&self) -> bool {
        self.object_type_code() == OBJLABEL_TYPE_CODE
            || (self.object_type_code() == OBJID_TYPE_CODE && self.decode_as_objid() >= 0)
    }

    pub fn to_sym(&self) -> Symbol {
        match self.object_type_code() {
            OBJID_TYPE_CODE => Symbol::mk(&format!("{}", self.decode_as_objid())),
            OBJLABEL_TYPE_CODE => Symbol::mk(&format!("{}", self.decode_as_objlabel())),
            _ => panic!("Invalid object type code"),
        }
    }
    pub fn id(&self) -> Option<Objid> {
        if self.object_type_code() != OBJID_TYPE_CODE {
            return None;
        }
        Some(Objid(self.decode_as_objid()))
    }
}

impl AsByteBuffer for Obj {
    fn size_bytes(&self) -> usize {
        size_of::<u64>()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(&self.0.to_le_bytes()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.to_le_bytes().to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        let content = u64::from_le_bytes(bytes.as_ref().try_into().unwrap());
        Ok(Self(content))
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        Ok(ByteView::from(self.0.to_le_bytes()))
    }
}

impl TryFrom<&str> for Obj {
    type Error = DecodingError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if let Some(value) = value.strip_prefix('#') {
            // If value is an integer, then it's an Objid, otherwise a label
            match value.parse::<i32>() {
                Ok(n) => Ok(Self::mk_id(n)),
                Err(_) => {
                    // If it's not an integer, then it's a label
                    Ok(Self::mk_label(value))
                }
            }
        } else {
            Err(DecodingError::CouldNotDecode(format!(
                "Expected object id to start with '#', got {}",
                value
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_objid() {
        let obj = Obj::mk_id(0);
        assert_eq!(obj.id(), Some(Objid(0)));
        assert_eq!(obj.to_literal(), "#0");

        let obj = Obj::mk_id(1);
        assert_eq!(obj.id(), Some(Objid(1)));
        assert_eq!(obj.to_literal(), "#1");

        let obj = Obj::mk_id(-1);
        assert_eq!(obj.id(), Some(Objid(-1)));
        assert_eq!(obj.to_literal(), "#-1");

        let obj = Obj::mk_id(-2);
        assert_eq!(obj.id(), Some(Objid(-2)));
        assert_eq!(obj.to_literal(), "#-2");

        let obj = Obj::mk_id(0x7fff_ffff);
        assert_eq!(obj.id(), Some(Objid(0x7fff_ffff)));
        assert_eq!(obj.to_literal(), "#2147483647");
    }

    #[test]
    fn test_objlabel() {
        let obj = Obj::mk_label("test");
        assert_eq!(obj.to_literal(), "#test");
    }
}
