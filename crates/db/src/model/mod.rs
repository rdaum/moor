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

use moor_var::{ByteSized, EncodingError, Obj, Var};
use std::{cmp::Ordering, collections::HashSet};
use uuid::Uuid;
use zerocopy::{FromBytes, Immutable, IntoBytes};

/// Helper function to extract anonymous object references from a Var into a HashSet
pub(crate) fn extract_anonymous_refs(var: &Var, refs: &mut HashSet<Obj>) {
    extract_anonymous_refs_recursive(var, refs);
}

/// Recursively extract anonymous object references from a Var
fn extract_anonymous_refs_recursive(var: &Var, refs: &mut HashSet<Obj>) {
    match var.variant() {
        moor_var::Variant::Obj(obj) => {
            if obj.is_anonymous() {
                refs.insert(obj);
            }
        }
        moor_var::Variant::List(list) => {
            for item in list.iter() {
                extract_anonymous_refs_recursive(&item, refs);
            }
        }
        moor_var::Variant::Map(map) => {
            for (key, value) in map.iter() {
                extract_anonymous_refs_recursive(&key, refs);
                extract_anonymous_refs_recursive(&value, refs);
            }
        }
        moor_var::Variant::Flyweight(flyweight) => {
            let delegate = flyweight.delegate();
            if delegate.is_anonymous() {
                refs.insert(*delegate);
            }

            for (_symbol, slot_value) in flyweight.slots_storage().iter() {
                extract_anonymous_refs_recursive(slot_value, refs);
            }

            for item in flyweight.contents().iter() {
                extract_anonymous_refs_recursive(&item, refs);
            }
        }
        moor_var::Variant::Err(error) => {
            if let Some(error_value) = &error.value {
                extract_anonymous_refs_recursive(error_value, refs);
            }
        }
        moor_var::Variant::Lambda(lambda) => {
            for frame in lambda.0.captured_env.iter() {
                for var in frame.iter() {
                    extract_anonymous_refs_recursive(var, refs);
                }
            }
        }
        _ => {}
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StringHolder(pub String);

impl ByteSized for StringHolder {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, IntoBytes, FromBytes, Immutable)]
#[repr(transparent)]
pub struct UUIDHolder([u8; 16]);

impl UUIDHolder {
    pub fn new(uuid: Uuid) -> Self {
        Self(*uuid.as_bytes())
    }

    pub fn uuid(&self) -> Uuid {
        Uuid::from_bytes(self.0)
    }
}

impl ByteSized for UUIDHolder {
    fn size_bytes(&self) -> usize {
        16
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BytesHolder(Vec<u8>);

impl ByteSized for BytesHolder {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable)]
#[repr(transparent)]
pub struct SystemTimeHolder(u128); // microseconds since UNIX_EPOCH

impl SystemTimeHolder {
    pub fn new(time: std::time::SystemTime) -> Result<Self, EncodingError> {
        let dur = time.duration_since(std::time::UNIX_EPOCH).map_err(|_| {
            EncodingError::CouldNotEncode("SystemTime before UNIX_EPOCH".to_string())
        })?;
        Ok(Self(dur.as_micros()))
    }

    pub fn system_time(&self) -> std::time::SystemTime {
        let dur = std::time::Duration::from_micros(self.0 as u64);
        std::time::UNIX_EPOCH + dur
    }
}

impl ByteSized for SystemTimeHolder {
    fn size_bytes(&self) -> usize {
        16
    }
}

#[derive(Clone, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable)]
#[repr(C)]
pub struct ObjAndUUIDHolder {
    pub uuid: [u8; 16],
    pub obj: Obj,
}

#[derive(Clone, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable)]
#[repr(C, packed)]
pub struct AnonymousObjectMetadata {
    created_micros: u128,
    last_accessed_micros: u128,
}

impl PartialOrd for ObjAndUUIDHolder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ObjAndUUIDHolder {
    fn cmp(&self, other: &Self) -> Ordering {
        self.uuid
            .cmp(&other.uuid)
            .then_with(|| self.obj.cmp(&other.obj))
    }
}

impl ObjAndUUIDHolder {
    pub fn new(obj: &Obj, uuid: Uuid) -> Self {
        Self {
            uuid: *uuid.as_bytes(),
            obj: *obj,
        }
    }

    pub fn obj(&self) -> Obj {
        self.obj
    }

    pub fn uuid(&self) -> Uuid {
        Uuid::from_bytes(self.uuid)
    }
}

impl std::hash::Hash for ObjAndUUIDHolder {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write(IntoBytes::as_bytes(self));
    }
}

impl std::fmt::Display for ObjAndUUIDHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.obj, Uuid::from_bytes(self.uuid))
    }
}

impl AnonymousObjectMetadata {
    pub fn new() -> Result<Self, EncodingError> {
        let now = std::time::SystemTime::now();
        let micros = now
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| EncodingError::CouldNotEncode("SystemTime before UNIX_EPOCH".to_string()))?
            .as_micros();

        Ok(Self {
            created_micros: micros,
            last_accessed_micros: micros,
        })
    }

    pub fn created_time(&self) -> std::time::SystemTime {
        let dur = std::time::Duration::from_micros(self.created_micros as u64);
        std::time::UNIX_EPOCH + dur
    }

    pub fn last_accessed_time(&self) -> std::time::SystemTime {
        let dur = std::time::Duration::from_micros(self.last_accessed_micros as u64);
        std::time::UNIX_EPOCH + dur
    }

    pub fn touch(&mut self) -> Result<(), EncodingError> {
        let now = std::time::SystemTime::now();
        let micros = now
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| EncodingError::CouldNotEncode("SystemTime before UNIX_EPOCH".to_string()))?
            .as_micros();

        self.last_accessed_micros = micros;
        Ok(())
    }

    pub fn age_millis(&self) -> u128 {
        let now = std::time::SystemTime::now();
        if let Ok(dur) = now.duration_since(self.created_time()) {
            dur.as_millis()
        } else {
            0
        }
    }
}

impl PartialOrd for AnonymousObjectMetadata {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AnonymousObjectMetadata {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_created = self.created_micros;
        let other_created = other.created_micros;
        let self_last_accessed = self.last_accessed_micros;
        let other_last_accessed = other.last_accessed_micros;

        self_created
            .cmp(&other_created)
            .then_with(|| self_last_accessed.cmp(&other_last_accessed))
    }
}

impl std::hash::Hash for AnonymousObjectMetadata {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write(IntoBytes::as_bytes(self));
    }
}

impl ByteSized for AnonymousObjectMetadata {
    fn size_bytes(&self) -> usize {
        33
    }
}

impl ByteSized for ObjAndUUIDHolder {
    fn size_bytes(&self) -> usize {
        24
    }
}

#[cfg(test)]
mod tests {
    use crate::model::ObjAndUUIDHolder;
    use moor_var::SYSTEM_OBJECT;
    use std::{
        collections::BTreeSet,
        hash::{Hash, Hasher},
    };
    use uuid::Uuid;
    use zerocopy::{FromBytes, IntoBytes};

    #[test]
    fn test_reconstitute_obj_uuid_holder() {
        let u = Uuid::new_v4();
        let oh = ObjAndUUIDHolder::new(&SYSTEM_OBJECT, u);
        let bytes = oh.as_bytes();
        let oh2 = ObjAndUUIDHolder::read_from_bytes(bytes).unwrap();
        assert_eq!(oh, oh2);
        assert_eq!(oh.uuid(), oh2.uuid());
        assert_eq!(oh.obj(), oh2.obj());
    }

    #[test]
    fn test_hash_obj_uuid_holder() {
        let u = Uuid::new_v4();
        let oh = ObjAndUUIDHolder::new(&SYSTEM_OBJECT, u);
        let oh2 = ObjAndUUIDHolder::new(&SYSTEM_OBJECT, u);

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        oh.hash(&mut hasher);
        oh2.hash(&mut hasher);
        let h1 = hasher.finish();
        let h2 = hasher.finish();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_ord_eq_obj_uuid_holder() {
        let mut tree = BTreeSet::new();
        tree.insert(ObjAndUUIDHolder::new(&SYSTEM_OBJECT, Uuid::new_v4()));
    }
}
