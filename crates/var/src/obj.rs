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
use byteview::ByteView;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};
use std::ops::Add;
use std::time::{SystemTime, UNIX_EPOCH};
use zerocopy::{FromBytes, Immutable, IntoBytes};

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
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Encode,
    Decode,
    Serialize,
    Deserialize,
    IntoBytes,
    FromBytes,
    Immutable,
)]
#[repr(transparent)]
pub struct Obj(u64);

const OBJID_TYPE_CODE: u8 = 0;
const UUOBJID_TYPE_CODE: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Objid(pub i32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct UuObjid(pub u64);

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
        let as_u32 = i32::cast_unsigned(id);
        let as_u64 = as_u32 as u64;
        Self((as_u64 & 0x0000_ffff_ffff) | ((OBJID_TYPE_CODE as u64) << 61))
    }

    fn decode_as_uuobjid(&self) -> UuObjid {
        // Extract the 60-bit UUID value from the lower 60 bits
        let uuid_value = self.0 & 0x0FFF_FFFF_FFFF_FFFF;
        UuObjid(uuid_value)
    }

    fn encode_as_uuobjid(uuid: UuObjid) -> Self {
        // Pack the 60-bit UUID value into the lower 60 bits with type code
        Self((uuid.0 & 0x0FFF_FFFF_FFFF_FFFF) | ((UUOBJID_TYPE_CODE as u64) << 61))
    }

    fn object_type_code(&self) -> u8 {
        (self.0 >> 61) as u8
    }
}

impl LayoutAs<u64> for Obj {
    type ReadError = DecodingError;
    type WriteError = EncodingError;

    fn try_read(v: u64) -> Result<Self, Self::ReadError> {
        Ok(Self::from_bytes(v.to_le_bytes().into()).unwrap())
    }

    fn try_write(v: Self) -> Result<u64, Self::WriteError> {
        Ok(v.0)
    }
}

impl Add for Obj {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::mk_id(self.id().0 + rhs.id().0)
    }
}

impl Debug for Obj {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.object_type_code() {
            OBJID_TYPE_CODE => f.write_fmt(format_args!("Obj(#{})", self.decode_as_objid())),
            UUOBJID_TYPE_CODE => {
                let uuid = self.decode_as_uuobjid();
                f.write_fmt(format_args!("Obj({})", uuid.to_uuid_string()))
            }
            _ => f.write_fmt(format_args!("Obj(UnknownType:{})", self.object_type_code())),
        }
    }
}

impl Display for Obj {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.object_type_code() {
            OBJID_TYPE_CODE => f.write_fmt(format_args!("#{}", self.decode_as_objid())),
            UUOBJID_TYPE_CODE => {
                let uuid = self.decode_as_uuobjid();
                f.write_fmt(format_args!("{}", uuid.to_uuid_string()))
            }
            _ => f.write_fmt(format_args!("UnknownType:{}", self.object_type_code())),
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

    #[must_use]
    pub fn to_literal(&self) -> String {
        match self.object_type_code() {
            OBJID_TYPE_CODE => format!("#{}", self.decode_as_objid()),
            UUOBJID_TYPE_CODE => {
                let uuid = self.decode_as_uuobjid();
                uuid.to_uuid_string()
            }
            _ => format!("UnknownType:{}", self.object_type_code()),
        }
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

    /// Creates a new Obj with a UuObjid
    pub fn mk_uuobjid(uuid: UuObjid) -> Self {
        Self::encode_as_uuobjid(uuid)
    }

    /// Creates a new Obj with a generated UuObjid
    pub fn mk_uuobjid_generated(autoincrement: u16) -> Self {
        Self::mk_uuobjid(UuObjid::generate(autoincrement))
    }

    /// Gets the UuObjid from this Obj if it's a UuObjid type
    pub fn uuobjid(&self) -> Option<UuObjid> {
        if self.object_type_code() == UUOBJID_TYPE_CODE {
            Some(self.decode_as_uuobjid())
        } else {
            None
        }
    }

    /// Checks if this Obj is a UuObjid
    pub fn is_uuobjid(&self) -> bool {
        self.object_type_code() == UUOBJID_TYPE_CODE
    }
}

impl AsByteBuffer for Obj {
    fn size_bytes(&self) -> usize {
        size_of::<u64>()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        // Zero-copy: direct access to the struct's bytes
        Ok(f(IntoBytes::as_bytes(self)))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        // Zero-copy to Vec
        Ok(IntoBytes::as_bytes(self).to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        let bytes = bytes.as_ref();
        if bytes.len() != 8 {
            return Err(DecodingError::CouldNotDecode(format!(
                "Expected 8 bytes for Obj, got {}",
                bytes.len()
            )));
        }

        // Use zerocopy to safely transmute from bytes
        Self::read_from_bytes(bytes)
            .map_err(|_| DecodingError::CouldNotDecode("Invalid bytes for Obj".to_string()))
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        // Zero-copy: create ByteView directly from struct bytes
        Ok(ByteView::from(IntoBytes::as_bytes(self)))
    }
}

impl TryFrom<&str> for Obj {
    type Error = DecodingError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if let Some(value) = value.strip_prefix('#') {
            let value = value.parse::<i32>().map_err(|e| {
                DecodingError::CouldNotDecode(format!("Could not parse Objid: {e}"))
            })?;
            Ok(Self::mk_id(value))
        } else if value.contains('-') {
            // Try to parse as UUID format: FFFFF-FFFFFFFFFF
            let uuid = UuObjid::from_uuid_string(value)?;
            Ok(Self::mk_uuobjid(uuid))
        } else {
            Err(DecodingError::CouldNotDecode(format!(
                "Expected Objid to start with '#' or be in UUID format FFFFF-FFFFFFFFFF, got {value}"
            )))
        }
    }
}

impl UuObjid {
    /// Creates a new UuObjid with the specified components
    /// - autoincrement: 16 bits (0-65535, wraps around)
    /// - rng: 4 bits (0-15)
    /// - epoch_ms: 40 bits (Linux epoch milliseconds, truncated to 40 bits)
    pub fn new(autoincrement: u16, rng: u8, epoch_ms: u64) -> Self {
        // Ensure inputs fit in their allocated bits
        let autoincrement = autoincrement & 0xFFFF; // 16 bits
        let rng = (rng & 0x0F) as u64; // 4 bits
        let epoch_ms = epoch_ms & 0xFFFF_FFFF_FF; // 40 bits
        
        // Pack into 60 bits: [autoincrement (16)] [rng (4)] [epoch_ms (40)]
        let packed = (autoincrement as u64) << 44 | (rng << 40) | epoch_ms;
        Self(packed)
    }
    
    /// Creates a new UuObjid with current time and random components
    pub fn generate(autoincrement: u16) -> Self {
        let mut rng = rand::thread_rng();
        let rng_val = rng.r#gen::<u8>() & 0x0F; // 4 bits
        
        // Get current time in milliseconds since Unix epoch
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        // Truncate to 40 bits (take the lower 40 bits)
        let epoch_ms = now & 0xFFFF_FFFF_FF;
        
        Self::new(autoincrement, rng_val, epoch_ms)
    }
    
    /// Extracts the components from the UuObjid
    pub fn components(&self) -> (u16, u8, u64) {
        let autoincrement = ((self.0 >> 44) & 0xFFFF) as u16;
        let rng = ((self.0 >> 40) & 0x0F) as u8;
        let epoch_ms = self.0 & 0xFFFF_FFFF_FF;
        (autoincrement, rng, epoch_ms)
    }
    
    /// Formats the UuObjid as a UUID-like string: FFFFF-FFFFFFFFFF
    /// First group: Autoincrement (16 bits) + RNG (4 bits) = 20 bits total
    /// Second group: Epoch milliseconds (40 bits)
    pub fn to_uuid_string(&self) -> String {
        let (autoincrement, rng, epoch_ms) = self.components();
        let first_group = ((autoincrement as u64) << 4) | (rng as u64);
        format!("{:05X}-{:010X}", first_group, epoch_ms)
    }
    
    /// Parses a UUID-like string back to UuObjid
    pub fn from_uuid_string(s: &str) -> Result<Self, DecodingError> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 {
            return Err(DecodingError::CouldNotDecode(
                "Expected format FFFFF-FFFFFFFFFF".to_string()
            ));
        }
        
        let first_group = u64::from_str_radix(parts[0], 16)
            .map_err(|e| DecodingError::CouldNotDecode(format!("Invalid first group: {}", e)))?;
        
        let epoch_ms = u64::from_str_radix(parts[1], 16)
            .map_err(|e| DecodingError::CouldNotDecode(format!("Invalid epoch_ms: {}", e)))?;
        
        let autoincrement = ((first_group >> 4) & 0xFFFF) as u16;
        let rng = (first_group & 0x0F) as u8;
        
        Ok(Self::new(autoincrement, rng, epoch_ms))
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

    #[test]
    fn test_uuobjid() {
        // Test manual creation
        let uuid = UuObjid::new(0x1234, 0x5, 0x1234567890);
        assert_eq!(uuid.to_uuid_string(), "12345-1234567890");
        
        let (autoincrement, rng, epoch_ms) = uuid.components();
        assert_eq!(autoincrement, 0x1234);
        assert_eq!(rng, 0x5);
        assert_eq!(epoch_ms, 0x1234567890);

        // Test parsing
        let parsed = UuObjid::from_uuid_string("12345-1234567890").unwrap();
        assert_eq!(parsed, uuid);

        // Test Obj integration
        let obj = Obj::mk_uuobjid(uuid);
        assert!(obj.is_uuobjid());
        assert!(!obj.is_sysobj());
        assert_eq!(obj.to_literal(), "12345-1234567890");
        assert_eq!(obj.uuobjid(), Some(uuid));

        // Test string parsing
        let obj_from_str = Obj::try_from("12345-1234567890").unwrap();
        assert_eq!(obj_from_str, obj);
    }

    #[test]
    fn test_uuobjid_generation() {
        let uuid = UuObjid::generate(0x1234);
        let (autoincrement, rng, epoch_ms) = uuid.components();
        
        assert_eq!(autoincrement, 0x1234);
        assert!(rng <= 0x0F);
        
        // Check that epoch_ms is reasonable (should be recent)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let now_truncated = now & 0xFFFF_FFFF_FF;
        
        // Allow for some time difference (within 1 second)
        assert!(epoch_ms >= now_truncated - 1000 || epoch_ms <= now_truncated + 1000);
    }

    #[test]
    fn test_uuobjid_edge_cases() {
        // Test maximum values
        let uuid = UuObjid::new(0xFFFF, 0x0F, 0xFFFF_FFFF_FF);
        assert_eq!(uuid.to_uuid_string(), "FFFFF-FFFFFFFFFF");
        
        // Test zero values
        let uuid = UuObjid::new(0, 0, 0);
        assert_eq!(uuid.to_uuid_string(), "00000-0000000000");
    }

    #[test]
    fn test_uuobjid_parsing_errors() {
        // Invalid format
        assert!(UuObjid::from_uuid_string("invalid").is_err());
        assert!(UuObjid::from_uuid_string("12345").is_err());
        assert!(UuObjid::from_uuid_string("12345-1234567890-extra").is_err());
        
        // Invalid hex
        assert!(UuObjid::from_uuid_string("GGGGG-1234567890").is_err());
        assert!(UuObjid::from_uuid_string("12345-GGGGGGGGGG").is_err());
    }
}
