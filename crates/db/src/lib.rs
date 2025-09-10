#![recursion_limit = "256"]

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

use byteview::ByteView;
use moor_common::model::{CommitResult, WorldStateSource};
use moor_common::model::{WorldState, WorldStateError};
use moor_var::{AsByteBuffer, DecodingError, EncodingError, Obj, Var};
use std::cmp::Ordering;
use std::path::Path;
use std::sync::Arc;

use uuid::Uuid;
use zerocopy::{FromBytes, Immutable, IntoBytes};

use moor_common::model::loader::{LoaderInterface, SnapshotInterface};

pub type SnapshotCallback = Box<
    dyn FnOnce(Result<Box<dyn SnapshotInterface>, WorldStateError>) -> Result<(), WorldStateError>
        + Send,
>;

mod db_loader_client;
pub mod db_worldstate;
mod fjall_provider;
#[cfg(test)]
mod gc_tests;
pub(crate) mod moor_db;
#[cfg(test)]
mod moor_db_concurrent_tests;
mod moor_db_tests;
mod relation_defs;
mod ws_transaction;

use crate::db_worldstate::DbWorldState;
use crate::moor_db::{Caches, MoorDB, WorkingSets};
pub use config::{DatabaseConfig, TableConfig};
mod config;
mod gc;
pub mod prop_cache;
mod snapshot_loader;
mod tx_management;
mod utils;
pub mod verb_cache;

pub use db_worldstate::db_counters;
use fast_counter::ConcurrentCounter;
pub use gc::{GCError, GCInterface};
pub use tx_management::Provider;
pub use tx_management::{Error, Relation, RelationTransaction, Timestamp, Tx, WorkingSet};

// Re-export sequence constants for use in VM
pub use moor_db::SEQUENCE_MAX_OBJECT;

pub trait Database: Send + WorldStateSource {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError>;
    fn create_snapshot_async(&self, callback: SnapshotCallback) -> Result<(), WorldStateError>;
    fn gc_interface(&self) -> Result<Box<dyn GCInterface>, WorldStateError>;
}

#[derive(Clone)]
pub struct TxDB {
    storage: Arc<MoorDB>,
}

impl TxDB {
    pub fn open(path: Option<&Path>, database_config: DatabaseConfig) -> (Self, bool) {
        let (storage, fresh) = MoorDB::open(path, database_config);
        (Self { storage }, fresh)
    }
}
impl WorldStateSource for TxDB {
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let tx = self.storage.start_transaction();
        let tx = DbWorldState { tx };
        Ok(Box::new(tx))
    }

    fn checkpoint(&self) -> Result<(), WorldStateError> {
        // TODO: noop for now... but this should probably do a sync of sequences to disk and make
        //   sure all data is durable.
        Ok(())
    }
}

impl Database for TxDB {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError> {
        let tx = self.storage.start_transaction();
        let tx = DbWorldState { tx };
        Ok(Box::new(tx))
    }

    fn create_snapshot(&self) -> Result<Box<dyn SnapshotInterface>, WorldStateError> {
        self.storage
            .create_snapshot()
            .map_err(|e| WorldStateError::DatabaseError(e.to_string()))
    }

    fn create_snapshot_async(&self, callback: SnapshotCallback) -> Result<(), WorldStateError> {
        let storage = self.storage.clone();
        std::thread::spawn(move || {
            let snapshot_result = storage
                .create_snapshot()
                .map_err(|e| WorldStateError::DatabaseError(e.to_string()));

            if let Err(e) = callback(snapshot_result) {
                tracing::error!("Snapshot callback failed: {}", e);
            }
        });
        Ok(())
    }

    fn gc_interface(&self) -> Result<Box<dyn GCInterface>, WorldStateError> {
        let tx = self.storage.start_transaction();
        let tx = DbWorldState { tx };
        Ok(Box::new(tx))
    }
}

/// Helper function to extract anonymous object references from a Var
fn extract_anonymous_refs(var: &Var) -> Vec<Obj> {
    let mut refs = Vec::new();
    extract_anonymous_refs_recursive(var, &mut refs);
    refs
}

/// Recursively extract anonymous object references from a Var
fn extract_anonymous_refs_recursive(var: &Var, refs: &mut Vec<Obj>) {
    match var.variant() {
        moor_var::Variant::Obj(obj) => {
            if obj.is_anonymous() {
                refs.push(*obj);
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
            // Check delegate
            let delegate = flyweight.delegate();
            if delegate.is_anonymous() {
                refs.push(*delegate);
            }

            // Check slots (Symbol -> Var pairs)
            for (_symbol, slot_value) in flyweight.slots().iter() {
                extract_anonymous_refs_recursive(slot_value, refs);
            }

            // Check contents (List)
            for item in flyweight.contents().iter() {
                extract_anonymous_refs_recursive(&item, refs);
            }
        }
        moor_var::Variant::Err(error) => {
            // Check the error's optional value field
            if let Some(error_value) = &error.value {
                extract_anonymous_refs_recursive(error_value, refs);
            }
        }
        moor_var::Variant::Lambda(lambda) => {
            // Check captured environment (stack frames)
            for frame in lambda.0.captured_env.iter() {
                for var in frame.iter() {
                    extract_anonymous_refs_recursive(var, refs);
                }
            }
        }
        _ => {} // Other types (None, Bool, Int, Float, Str, Sym, Binary) don't contain object references
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StringHolder(pub String);

impl AsByteBuffer for StringHolder {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(self.0.as_bytes()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.as_bytes().to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        Ok(Self(
            String::from_utf8(bytes.to_vec()).expect("Invalid UTF-8"),
        ))
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        Ok(ByteView::from(self.0.as_bytes().to_vec()))
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

impl AsByteBuffer for UUIDHolder {
    fn size_bytes(&self) -> usize {
        16
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        // Zero-copy: direct access to the struct's bytes
        Ok(f(IntoBytes::as_bytes(self)))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        // Zero-copy to Vec
        Ok(IntoBytes::as_bytes(self).to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        let bytes = bytes.as_ref();
        if bytes.len() != 16 {
            return Err(DecodingError::CouldNotDecode(format!(
                "Expected 16 bytes, got {}",
                bytes.len()
            )));
        }

        // Use zerocopy to safely transmute from bytes
        Self::read_from_bytes(bytes)
            .map_err(|_| DecodingError::CouldNotDecode("Invalid bytes for UUIDHolder".to_string()))
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        // Zero-copy: create ByteView directly from struct bytes
        Ok(ByteView::from(IntoBytes::as_bytes(self)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BytesHolder(Vec<u8>);

impl AsByteBuffer for BytesHolder {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(self.0.as_ref()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.clone())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        Ok(Self(bytes.to_vec()))
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        Ok(ByteView::from(self.0.clone()))
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

impl AsByteBuffer for SystemTimeHolder {
    fn size_bytes(&self) -> usize {
        16
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        // Zero-copy: direct access to the struct's bytes
        Ok(f(IntoBytes::as_bytes(self)))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        // Zero-copy to Vec
        Ok(IntoBytes::as_bytes(self).to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        let bytes = bytes.as_ref();
        if bytes.len() != 16 {
            return Err(DecodingError::CouldNotDecode(format!(
                "Expected 16 bytes for SystemTimeHolder, got {}",
                bytes.len()
            )));
        }

        // Use zerocopy to safely transmute from bytes
        Self::read_from_bytes(bytes).map_err(|_| {
            DecodingError::CouldNotDecode("Invalid bytes for SystemTimeHolder".to_string())
        })
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        // Zero-copy: create ByteView directly from struct bytes
        Ok(ByteView::from(IntoBytes::as_bytes(self)))
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
    created_micros: u128,       // microseconds since UNIX_EPOCH
    last_accessed_micros: u128, // microseconds since UNIX_EPOCH
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
        // Use zerocopy to write the entire struct as contiguous bytes
        // This is the ultimate optimization - zero-copy hashing of the entire struct
        state.write(IntoBytes::as_bytes(self));
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
        // Copy packed struct fields to avoid unaligned references
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
        // Use zerocopy to write the entire struct as contiguous bytes
        state.write(IntoBytes::as_bytes(self));
    }
}

impl AsByteBuffer for AnonymousObjectMetadata {
    fn size_bytes(&self) -> usize {
        33 // Fixed size: 1 byte (generation) + 16 bytes (created) + 16 bytes (last_accessed)
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        // Zero-copy: direct access to the struct's bytes
        Ok(f(IntoBytes::as_bytes(self)))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        // Zero-copy to Vec
        Ok(IntoBytes::as_bytes(self).to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        let bytes = bytes.as_ref();
        if bytes.len() != 33 {
            return Err(DecodingError::CouldNotDecode(format!(
                "Expected 33 bytes for AnonymousObjectMetadata, got {}",
                bytes.len()
            )));
        }

        // Use zerocopy to safely transmute from bytes
        Self::read_from_bytes(bytes).map_err(|_| {
            DecodingError::CouldNotDecode("Invalid bytes for AnonymousObjectMetadata".to_string())
        })
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        // Zero-copy: create ByteView directly from struct bytes
        Ok(ByteView::from(IntoBytes::as_bytes(self).to_vec()))
    }
}

impl AsByteBuffer for ObjAndUUIDHolder {
    fn size_bytes(&self) -> usize {
        24 // Fixed size: 16 bytes (UUID) + 8 bytes (u64)
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        // Zero-copy: direct access to the struct's bytes
        Ok(f(IntoBytes::as_bytes(self)))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        // Zero-copy to Vec
        Ok(IntoBytes::as_bytes(self).to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        let bytes = bytes.as_ref();
        if bytes.len() != 24 {
            return Err(DecodingError::CouldNotDecode(format!(
                "Expected 24 bytes for ObjAndUUIDHolder, got {}",
                bytes.len()
            )));
        }

        // Use zerocopy to safely transmute from bytes
        Self::read_from_bytes(bytes).map_err(|_| {
            DecodingError::CouldNotDecode("Invalid bytes for ObjAndUUIDHolder".to_string())
        })
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        // Zero-copy: create ByteView directly from struct bytes
        Ok(ByteView::from(IntoBytes::as_bytes(self).to_vec()))
    }
}

enum CommitSet {
    /// Commit the working sets of a transaction.
    CommitWrites(Box<WorkingSets>, oneshot::Sender<CommitResult>),
    /// This is a read only commit, we didn't do any mutations. We can just fire and forget,
    /// just (maybe) updating the caches on the DB side, no need for locks, flushes, anything.
    CommitReadOnly(Caches),
}

/// Unified cache statistics structure
pub struct CacheStats {
    hits: ConcurrentCounter,
    misses: ConcurrentCounter,
    flushes: ConcurrentCounter,
    num_entries: ConcurrentCounter,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheStats {
    pub fn new() -> Self {
        Self {
            hits: ConcurrentCounter::new(0),
            misses: ConcurrentCounter::new(0),
            flushes: ConcurrentCounter::new(0),
            num_entries: ConcurrentCounter::new(0),
        }
    }

    pub fn hit(&self) {
        self.hits.add(1);
    }
    pub fn miss(&self) {
        self.misses.add(1);
    }
    pub fn flush(&self) {
        self.flushes.add(1);
    }

    pub fn add_entry(&self) {
        self.num_entries.add(1);
    }

    pub fn remove_entries(&self, count: isize) {
        self.num_entries.add(-count);
    }

    pub fn hit_count(&self) -> isize {
        self.hits.sum()
    }
    pub fn miss_count(&self) -> isize {
        self.misses.sum()
    }
    pub fn flush_count(&self) -> isize {
        self.flushes.sum()
    }

    pub fn num_entries(&self) -> isize {
        self.num_entries.sum()
    }

    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.sum() as f64;
        let misses = self.misses.sum() as f64;
        let total = hits + misses;
        if total > 0.0 {
            (hits / total) * 100.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ObjAndUUIDHolder;
    use moor_var::{AsByteBuffer, SYSTEM_OBJECT};
    use std::collections::BTreeSet;
    use std::hash::{Hash, Hasher};
    use uuid::Uuid;

    #[test]
    fn test_reconstitute_obj_uuid_holder() {
        let u = Uuid::new_v4();
        let oh = ObjAndUUIDHolder::new(&SYSTEM_OBJECT, u);
        let bytes = oh.as_bytes().unwrap();
        let oh2 = ObjAndUUIDHolder::from_bytes(bytes).unwrap();
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
