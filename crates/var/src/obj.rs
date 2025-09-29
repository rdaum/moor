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

use crate::{
    AsByteBuffer,
    encode::{DecodingError, EncodingError},
};
use binary_layout::LayoutAs;
use bincode::{Decode, Encode};
use byteview::ByteView;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Debug, Display, Formatter},
    ops::Add,
    sync::atomic::{AtomicU16, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use zerocopy::{FromBytes, Immutable, IntoBytes};

/// Global atomic counter for UUID object sequence generation
static UUOBJID_SEQUENCE: AtomicU16 = AtomicU16::new(1);

/// Global atomic counter for anonymous object sequence generation
static ANONYMOUS_SEQUENCE: AtomicU16 = AtomicU16::new(1);

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

/// A reference to an object (object-id/dbref).
/// Contains multiple kinds of object identifiers:
/// - Traditional 32-bit DB object IDs (Objid)
/// - 62-bit UUID-based object IDs (UuObjid)
///   The top 2 bits encode the object type, with remaining bits for the identifier.
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
const ANONYMOUS_TYPE_CODE: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Objid(pub i32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct UuObjid(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AnonymousObjid(pub u64);

// Internal representation varies by type:
// - Objid: lower 32 bits contain i32 DB object id, upper 30 bits unused
// - UuObjid: lower 62 bits contain packed UUID components
// - AnonymousObjid: lower 62 bits contain anonymous object ID
// Top 2 bits always encode the object type.
impl Obj {
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    fn decode_as_objid(&self) -> i32 {
        // Mask out upper 32 bits
        (self.0 & 0x0000_ffff_ffff) as i32
    }

    const fn encode_as_objid(id: i32) -> Self {
        // Transmute to u64 and then mask on the type code
        // Doing "as u32" here would sign extend the i32 to u64, which is not what we want.
        let as_u32 = i32::cast_unsigned(id);
        let as_u64 = as_u32 as u64;
        Self((as_u64 & 0x0000_ffff_ffff) | ((OBJID_TYPE_CODE as u64) << 62))
    }

    fn decode_as_uuobjid(&self) -> UuObjid {
        // Extract the 62-bit UUID value from the lower 62 bits
        let uuid_value = self.0 & 0x3FFF_FFFF_FFFF_FFFF;
        UuObjid(uuid_value)
    }

    fn encode_as_uuobjid(uuid: UuObjid) -> Self {
        // Pack the 62-bit UUID value into the lower 62 bits with type code
        Self((uuid.0 & 0x3FFF_FFFF_FFFF_FFFF) | ((UUOBJID_TYPE_CODE as u64) << 62))
    }

    fn decode_as_anonymous(&self) -> AnonymousObjid {
        // Extract the 62-bit anonymous ID value from the lower 62 bits
        let id_value = self.0 & 0x3FFF_FFFF_FFFF_FFFF;
        AnonymousObjid(id_value)
    }

    fn encode_as_anonymous(anonymous: AnonymousObjid) -> Self {
        // Pack the 62-bit anonymous ID value into the lower 62 bits with type code
        Self((anonymous.0 & 0x3FFF_FFFF_FFFF_FFFF) | ((ANONYMOUS_TYPE_CODE as u64) << 62))
    }

    fn object_type_code(&self) -> u8 {
        (self.0 >> 62) as u8
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
                f.write_fmt(format_args!("Obj(#{})", uuid.to_uuid_string()))
            }
            ANONYMOUS_TYPE_CODE => {
                let anonymous = self.decode_as_anonymous();
                f.write_fmt(format_args!("Obj(*anonymous*:{})", anonymous.0))
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
                f.write_fmt(format_args!("#{}", uuid.to_uuid_string()))
            }
            ANONYMOUS_TYPE_CODE => f.write_fmt(format_args!("*anonymous*")),
            _ => f.write_fmt(format_args!("UnknownType:{}", self.object_type_code())),
        }
    }
}

impl Display for Objid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("#{}", self.0))
    }
}

impl Display for UuObjid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("#{}", self.to_uuid_string()))
    }
}

impl Display for AnonymousObjid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("*anonymous*:{}", self.0))
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
            ANONYMOUS_TYPE_CODE => {
                // Anonymous objects show as "*anonymous*" in keeping with toaststunt
                "*anonymous*".to_string()
            }
            _ => format!("UnknownType:{}", self.object_type_code()),
        }
    }

    #[must_use]
    pub fn is_sysobj(&self) -> bool {
        self.object_type_code() == OBJID_TYPE_CODE && self.decode_as_objid() == 0
    }

    pub fn is_nothing(&self) -> bool {
        self.object_type_code() == OBJID_TYPE_CODE && self.decode_as_objid() == -1
    }

    pub fn is_positive(&self) -> bool {
        if self.object_type_code() == OBJID_TYPE_CODE {
            self.decode_as_objid() >= 0
        } else {
            // uuid objects are always "positive" as in they are not negative connection etc style objs
            // anonymous objects (when/if we have them would also follow this principle)
            true
        }
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
    pub fn mk_uuobjid_generated() -> Self {
        let seq = UUOBJID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        Self::mk_uuobjid(UuObjid::generate(seq))
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

    /// Creates a new Obj with an AnonymousObjid
    pub fn mk_anonymous(id: AnonymousObjid) -> Self {
        Self::encode_as_anonymous(id)
    }

    /// Creates a new Obj with a generated AnonymousObjid
    pub fn mk_anonymous_generated() -> Self {
        let seq = ANONYMOUS_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        Self::mk_anonymous(AnonymousObjid::generate(seq))
    }

    /// Gets the AnonymousObjid from this Obj if it's an anonymous type
    pub fn anonymous_objid(&self) -> Option<AnonymousObjid> {
        if self.object_type_code() == ANONYMOUS_TYPE_CODE {
            Some(self.decode_as_anonymous())
        } else {
            None
        }
    }

    /// Checks if this Obj is an anonymous object
    pub fn is_anonymous(&self) -> bool {
        self.object_type_code() == ANONYMOUS_TYPE_CODE
    }

    /// Checks if this Obj contains a valid object reference (not special values like NOTHING, AMBIGUOUS, etc.)
    pub fn is_valid_object(&self) -> bool {
        match self.object_type_code() {
            OBJID_TYPE_CODE => self.decode_as_objid() >= 0,
            UUOBJID_TYPE_CODE => true, // UuObjids are always valid object references
            ANONYMOUS_TYPE_CODE => true, // Anonymous objects are always valid object references
            _ => false,
        }
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
        if let Some(stripped) = value.strip_prefix('#') {
            // First try to parse as i32
            if let Ok(id) = stripped.parse::<i32>() {
                Ok(Self::mk_id(id))
            } else if stripped.contains('-') {
                // If i32 parsing failed and contains '-', try UUID format
                let uuid = UuObjid::from_uuid_string(stripped)?;
                Ok(Self::mk_uuobjid(uuid))
            } else {
                Err(DecodingError::CouldNotDecode(format!(
                    "Could not parse '{stripped}' as either integer objid or UUID format after stripping #"
                )))
            }
        } else if value.contains('-') {
            // Try to parse as UUID format: FFFFF-FFFFFFFFFF
            let uuid = UuObjid::from_uuid_string(value)?;
            Ok(Self::mk_uuobjid(uuid))
        } else {
            Err(DecodingError::CouldNotDecode(format!(
                "Expected Objid to start with '#' or be in UUID format FFFFFF-FFFFFFFFFF, got {value}"
            )))
        }
    }
}

impl UuObjid {
    /// Creates a new UuObjid with the specified components
    /// - autoincrement: 16 bits (0-65535, wraps around)
    /// - rng: 6 bits (0-63)
    /// - epoch_ms: 40 bits (Linux epoch milliseconds, truncated to 40 bits)
    pub fn new(autoincrement: u16, rng: u8, epoch_ms: u64) -> Self {
        // Ensure inputs fit in their allocated bits
        let rng = (rng & 0x3F) as u64; // 6 bits
        let epoch_ms = epoch_ms & 0x00FF_FFFF_FFFF; // 40 bits

        // Pack into 62 bits: [autoincrement (16)] [rng (6)] [epoch_ms (40)]
        let packed = (autoincrement as u64) << 46 | (rng << 40) | epoch_ms;
        Self(packed)
    }

    /// Creates a new UuObjid with current time and random components
    pub fn generate(autoincrement: u16) -> Self {
        let mut rng = rand::thread_rng();
        let rng_val = rng.r#gen::<u8>() & 0x3F; // 6 bits

        // Get current time in milliseconds since Unix epoch
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Truncate to 40 bits (take the lower 40 bits)
        let epoch_ms = now & 0x00FF_FFFF_FFFF;

        Self::new(autoincrement, rng_val, epoch_ms)
    }

    /// Extracts the components from the UuObjid
    pub fn components(&self) -> (u16, u8, u64) {
        let autoincrement = ((self.0 >> 46) & 0xFFFF) as u16;
        let rng = ((self.0 >> 40) & 0x3F) as u8;
        let epoch_ms = self.0 & 0x00FF_FFFF_FFFF;
        (autoincrement, rng, epoch_ms)
    }

    /// Formats the UuObjid as a UUID-like string: FFFFFF-FFFFFFFFFF
    /// First group: Autoincrement (16 bits) + RNG (6 bits) = 22 bits total
    /// Second group: Epoch milliseconds (40 bits)
    pub fn to_uuid_string(&self) -> String {
        let (autoincrement, rng, epoch_ms) = self.components();
        let first_group = ((autoincrement as u64) << 6) | (rng as u64);
        format!("{first_group:06X}-{epoch_ms:010X}")
    }

    /// Parses a UUID-like string back to UuObjid
    pub fn from_uuid_string(s: &str) -> Result<Self, DecodingError> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 {
            return Err(DecodingError::CouldNotDecode(
                "Expected format FFFFFF-FFFFFFFFFF".to_string(),
            ));
        }

        let first_group = u64::from_str_radix(parts[0], 16)
            .map_err(|e| DecodingError::CouldNotDecode(format!("Invalid first group: {e}")))?;

        let epoch_ms = u64::from_str_radix(parts[1], 16)
            .map_err(|e| DecodingError::CouldNotDecode(format!("Invalid epoch_ms: {e}")))?;

        let autoincrement = ((first_group >> 6) & 0xFFFF) as u16;
        let rng = (first_group & 0x3F) as u8;

        Ok(Self::new(autoincrement, rng, epoch_ms))
    }
}

impl AnonymousObjid {
    /// Creates a new AnonymousObjid with the specified components
    /// - autoincrement: 16 bits (0-65535, wraps around)
    /// - rng: 6 bits (0-63)
    /// - epoch_ms: 40 bits (Linux epoch milliseconds, truncated to 40 bits)
    pub fn new(autoincrement: u16, rng: u8, epoch_ms: u64) -> Self {
        // Ensure inputs fit in their allocated bits
        let rng = (rng & 0x3F) as u64; // 6 bits
        let epoch_ms = epoch_ms & 0x00FF_FFFF_FFFF; // 40 bits

        // Pack into 62 bits: [autoincrement (16)] [rng (6)] [epoch_ms (40)]
        let packed = (autoincrement as u64) << 46 | (rng << 40) | epoch_ms;
        Self(packed)
    }

    /// Creates a new AnonymousObjid with current time and random components
    pub fn generate(autoincrement: u16) -> Self {
        let mut rng = rand::thread_rng();
        let rng_val = rng.r#gen::<u8>() & 0x3F; // 6 bits

        // Get current time in milliseconds since Unix epoch
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Truncate to 40 bits (take the lower 40 bits)
        let epoch_ms = now & 0x00FF_FFFF_FFFF;

        Self::new(autoincrement, rng_val, epoch_ms)
    }

    /// Extracts the components from the AnonymousObjid
    pub fn components(&self) -> (u16, u8, u64) {
        let autoincrement = ((self.0 >> 46) & 0xFFFF) as u16;
        let rng = ((self.0 >> 40) & 0x3F) as u8;
        let epoch_ms = self.0 & 0x00FF_FFFF_FFFF;
        (autoincrement, rng, epoch_ms)
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
        assert_eq!(uuid.to_uuid_string(), "048D05-1234567890");

        let (autoincrement, rng, epoch_ms) = uuid.components();
        assert_eq!(autoincrement, 0x1234);
        assert_eq!(rng, 0x5);
        assert_eq!(epoch_ms, 0x1234567890);

        // Test parsing
        let parsed = UuObjid::from_uuid_string("048D05-1234567890").unwrap();
        assert_eq!(parsed, uuid);

        // Test Obj integration
        let obj = Obj::mk_uuobjid(uuid);
        assert!(obj.is_uuobjid());
        assert!(!obj.is_sysobj());
        assert_eq!(obj.to_literal(), "048D05-1234567890");
        assert_eq!(obj.uuobjid(), Some(uuid));

        // Test string parsing
        let obj_from_str = Obj::try_from("048D05-1234567890").unwrap();
        assert_eq!(obj_from_str, obj);

        // Test parsing with # prefix
        let obj_from_str_with_hash = Obj::try_from("#048D05-1234567890").unwrap();
        assert_eq!(obj_from_str_with_hash, obj);
        assert!(obj_from_str_with_hash.is_uuobjid());
    }

    #[test]
    fn test_uuobjid_generation() {
        let obj = Obj::mk_uuobjid_generated();
        let uuid = obj.uuobjid().unwrap();
        let (autoincrement, rng, epoch_ms) = uuid.components();

        // Should be a reasonable autoincrement value (starts at 1)
        assert!(autoincrement >= 1);
        assert!(rng <= 0x3F);

        // Check that epoch_ms is reasonable (should be recent)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let now_truncated = now & 0x00FF_FFFF_FFFF;

        // Allow for some time difference (within 1 second)
        assert!(epoch_ms >= now_truncated - 1000 || epoch_ms <= now_truncated + 1000);
    }

    #[test]
    fn test_uuobjid_edge_cases() {
        // Test maximum values
        let uuid = UuObjid::new(0xFFFF, 0x3F, 0x00FF_FFFF_FFFF);
        assert_eq!(uuid.to_uuid_string(), "3FFFFF-FFFFFFFFFF");

        // Test zero values
        let uuid = UuObjid::new(0, 0, 0);
        assert_eq!(uuid.to_uuid_string(), "000000-0000000000");
    }

    #[test]
    fn test_uuobjid_parsing_errors() {
        // Invalid format
        assert!(UuObjid::from_uuid_string("invalid").is_err());
        assert!(UuObjid::from_uuid_string("123456").is_err());
        assert!(UuObjid::from_uuid_string("123456-1234567890-extra").is_err());

        // Invalid hex
        assert!(UuObjid::from_uuid_string("GGGGGG-1234567890").is_err());
        assert!(UuObjid::from_uuid_string("123456-GGGGGGGGGG").is_err());
    }

    #[test]
    fn test_is_valid_etc() {
        let obj = UuObjid::from_uuid_string("000053-9905B4734F").unwrap();
        let obj = Obj::mk_uuobjid(obj);
        assert!(obj.is_valid_object());
        assert!(obj.is_uuobjid());
    }

    #[test]
    fn test_autoincrement_wrapping() {
        // Test that the atomic counter wraps properly
        // Set counter near maximum
        UUOBJID_SEQUENCE.store(65535, Ordering::Relaxed);

        // Generate UUID at max value
        let seq1 = UUOBJID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        assert_eq!(seq1, 65535);

        // Next fetch should wrap to 0
        let seq2 = UUOBJID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        assert_eq!(seq2, 0);

        // Verify the UUIDs are generated with correct autoincrement values
        let uuid1 = UuObjid::generate(seq1);
        let uuid2 = UuObjid::generate(seq2);

        let (auto1, _, _) = uuid1.components();
        let (auto2, _, _) = uuid2.components();

        assert_eq!(auto1, 65535);
        assert_eq!(auto2, 0);
    }

    #[test]
    fn test_anonymous_objects() {
        // Test anonymous object creation
        let anonymous_id = AnonymousObjid(12345);
        let obj = Obj::mk_anonymous(anonymous_id);

        // Test type identification
        assert!(obj.is_anonymous());
        assert!(!obj.is_uuobjid());
        assert!(!obj.is_sysobj());
        assert!(obj.is_valid_object());

        // Test ID retrieval
        assert_eq!(obj.anonymous_objid(), Some(anonymous_id));
        assert_eq!(obj.uuobjid(), None);

        // Test literal representation
        assert_eq!(obj.to_literal(), "*anonymous*");

        // Test Display implementation
        assert_eq!(format!("{obj}"), "*anonymous*");

        // Test Debug implementation
        assert_eq!(format!("{obj:?}"), "Obj(*anonymous*:12345)");
    }

    #[test]
    fn test_anonymous_objid_display() {
        let anonymous_id = AnonymousObjid(98765);
        assert_eq!(format!("{anonymous_id}"), "*anonymous*:98765");
    }

    #[test]
    fn test_anonymous_object_encoding() {
        // Test that anonymous objects use the correct type code
        let anonymous_id = AnonymousObjid(0x1234567890ABCDEF);
        let obj = Obj::mk_anonymous(anonymous_id);

        assert_eq!(obj.object_type_code(), ANONYMOUS_TYPE_CODE);

        // Test that the ID is preserved in encoding/decoding
        let decoded = obj.decode_as_anonymous();
        assert_eq!(decoded.0, anonymous_id.0);
    }

    #[test]
    fn test_anonymous_object_max_id() {
        // Test with maximum possible 62-bit anonymous ID
        let max_id = AnonymousObjid(0x3FFF_FFFF_FFFF_FFFF);
        let obj = Obj::mk_anonymous(max_id);

        assert!(obj.is_anonymous());
        assert_eq!(obj.anonymous_objid(), Some(max_id));
    }

    #[test]
    fn test_anonymous_object_generation() {
        // Test generated anonymous objects
        let obj1 = Obj::mk_anonymous_generated();
        let obj2 = Obj::mk_anonymous_generated();

        // Both should be anonymous
        assert!(obj1.is_anonymous());
        assert!(obj2.is_anonymous());

        // They should have different IDs (due to time/random components)
        let id1 = obj1.anonymous_objid().unwrap();
        let id2 = obj2.anonymous_objid().unwrap();
        assert_ne!(id1, id2);

        // Check that autoincrement values are sequential
        let (auto1, rng1, epoch1) = id1.components();
        let (auto2, rng2, epoch2) = id2.components();
        assert_eq!(auto2, auto1 + 1);

        // Random components may differ
        assert!(rng1 <= 0x3F);
        assert!(rng2 <= 0x3F);

        // Epoch times should be recent and reasonably close
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let now_truncated = now & 0x00FF_FFFF_FFFF;

        assert!(epoch1 <= now_truncated);
        assert!(epoch2 <= now_truncated);

        // Both should display as *anonymous*
        assert_eq!(obj1.to_literal(), "*anonymous*");
        assert_eq!(obj2.to_literal(), "*anonymous*");
    }

    #[test]
    fn test_anonymous_objid_components() {
        // Test manual creation of AnonymousObjid
        let anonymous = AnonymousObjid::new(0x1234, 0x15, 0x9876543210);
        let (autoincrement, rng, epoch_ms) = anonymous.components();

        assert_eq!(autoincrement, 0x1234);
        assert_eq!(rng, 0x15);
        assert_eq!(epoch_ms, 0x9876543210);

        // Test generation
        let generated = AnonymousObjid::generate(42);
        let (gen_auto, gen_rng, gen_epoch) = generated.components();

        assert_eq!(gen_auto, 42);
        assert!(gen_rng <= 0x3F);
        assert!(gen_epoch > 0); // Should be recent timestamp
    }
}
