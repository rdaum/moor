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

use crate::AsByteBuffer;
use crate::encode::{DecodingError, EncodingError};
use binary_layout::LayoutAs;
use bincode::{Decode, Encode};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};
use std::ops::Add;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Objid(pub i32);

// Internal representation is lower 32 bits is db object id (if a db object), top 3 bits is a "type"
// code, with the remaining 13-bits unused for now.
impl Obj {
    fn decode_as_objid(&self) -> i32 {
        // Mask out upper 32 bits
        (self.0 & 0x0000_ffff_ffff) as i32
    }

    const fn encode_as_objid(id: i32) -> Self {
        // Transmute to u64 and then mask on the type code
        // Doing "as u32" here would sign extend the i32 to u64, which is not what we want.
        let as_u32 = unsafe { std::mem::transmute::<i32, u32>(id) };
        let as_u64 = as_u32 as u64;
        Self((as_u64 & 0x0000_ffff_ffff) | ((OBJID_TYPE_CODE as u64) << 61))
    }

    fn object_type_code(&self) -> u8 {
        (self.0 >> 61) as u8
    }
}

impl LayoutAs<i32> for Obj {
    type ReadError = DecodingError;
    type WriteError = EncodingError;

    fn try_read(v: i32) -> Result<Self, Self::ReadError> {
        Ok(Self::mk_id(v))
    }

    fn try_write(v: Self) -> Result<i32, Self::WriteError> {
        Ok(v.id().0)
    }
}

impl Add for Obj {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::mk_id(self.id().0 + rhs.id().0)
    }
}

impl Display for Obj {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("#{}", self.decode_as_objid()))
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

    #[must_use]
    pub fn to_literal(&self) -> String {
        format!("#{}", self.decode_as_objid())
    }

    #[must_use]
    pub fn is_sysobj(&self) -> bool {
        self.decode_as_objid() == 0
    }

    pub fn is_nothing(&self) -> bool {
        self.decode_as_objid() == -1
    }

    pub fn is_positive(&self) -> bool {
        self.decode_as_objid() >= 0
    }

    pub fn id(&self) -> Objid {
        assert_eq!(self.object_type_code(), OBJID_TYPE_CODE);
        Objid(self.decode_as_objid())
    }
}

// TODO: will not encode for non-objid objects

impl AsByteBuffer for Obj {
    fn size_bytes(&self) -> usize {
        4
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, f: F) -> Result<R, EncodingError> {
        self.id().with_byte_buffer(f)
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        self.id().make_copy_as_vec()
    }

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        let id = Objid::from_bytes(bytes)?;
        Ok(Self::mk_id(id.0))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        self.id().as_bytes()
    }
}

impl AsByteBuffer for Objid {
    fn size_bytes(&self) -> usize {
        4
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(&self.0.to_le_bytes()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.to_le_bytes().to_vec())
    }

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        let bytes = bytes.as_ref();
        if bytes.len() != 4 {
            return Err(DecodingError::CouldNotDecode(format!(
                "Expected 8 bytes, got {}",
                bytes.len()
            )));
        }
        let mut buf = [0u8; 4];
        buf.copy_from_slice(bytes);
        Ok(Self(i32::from_le_bytes(buf)))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        Ok(Bytes::from(self.make_copy_as_vec()?))
    }
}

impl TryFrom<&str> for Obj {
    type Error = DecodingError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if let Some(value) = value.strip_prefix('#') {
            let value = value.parse::<i32>().map_err(|e| {
                DecodingError::CouldNotDecode(format!("Could not parse Objid: {}", e))
            })?;
            Ok(Self::mk_id(value))
        } else {
            Err(DecodingError::CouldNotDecode(format!(
                "Expected Objid to start with '#', got {}",
                value
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obj() {
        let obj = Obj::mk_id(0);
        assert_eq!(obj.id(), Objid(0));
        assert_eq!(obj.to_literal(), "#0");

        let obj = Obj::mk_id(1);
        assert_eq!(obj.id(), Objid(1));
        assert_eq!(obj.to_literal(), "#1");

        let obj = Obj::mk_id(-1);
        assert_eq!(obj.id(), Objid(-1));
        assert_eq!(obj.to_literal(), "#-1");

        let obj = Obj::mk_id(-2);
        assert_eq!(obj.id(), Objid(-2));
        assert_eq!(obj.to_literal(), "#-2");

        let obj = Obj::mk_id(0x7fff_ffff);
        assert_eq!(obj.id(), Objid(0x7fff_ffff));
        assert_eq!(obj.to_literal(), "#2147483647");
    }
}
