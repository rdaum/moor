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
use moor_values::model::WorldStateSource;
use moor_values::model::{WorldState, WorldStateError};
use moor_values::{AsByteBuffer, DecodingError, EncodingError, Obj};
use std::cmp::Ordering;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use crate::loader::LoaderInterface;

mod db_loader_client;
pub mod db_worldstate;
pub mod loader;
pub mod worldstate_transaction;

mod db_transaction;
mod fjall_provider;
pub(crate) mod worldstate_db;
mod worldstate_tests;

use crate::db_worldstate::DbTxWorldState;
use crate::worldstate_db::WorldStateDB;
pub use config::{DatabaseConfig, TableConfig};
pub use worldstate_tests::*;
mod config;
mod tx;

pub use tx::Provider;
pub use tx::{Error, Timestamp, TransactionalCache, TransactionalTable, Tx, WorkingSet};

pub trait Database: Send + WorldStateSource {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError>;
}

#[derive(Clone)]
pub struct TxDB {
    storage: Arc<WorldStateDB>,
}

impl TxDB {
    pub fn open(path: Option<&Path>, database_config: DatabaseConfig) -> (Self, bool) {
        let (storage, fresh) = WorldStateDB::open(path, database_config);
        (Self { storage }, fresh)
    }
}
impl WorldStateSource for TxDB {
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let tx = self.storage.start_transaction();
        let tx = DbTxWorldState { tx };
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
        let tx = DbTxWorldState { tx };
        Ok(Box::new(tx))
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UUIDHolder(Uuid);

impl AsByteBuffer for UUIDHolder {
    fn size_bytes(&self) -> usize {
        16
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(&self.0.as_bytes()[..]))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.as_bytes().to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        let bytes = bytes.as_ref();
        if bytes.len() != 16 {
            return Err(DecodingError::CouldNotDecode(format!(
                "Expected 16 bytes, got {}",
                bytes.len()
            )));
        }
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes.copy_from_slice(bytes);
        Ok(Self(Uuid::from_bytes(uuid_bytes)))
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        Ok(ByteView::from(self.0.as_bytes().to_vec()))
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemTimeHolder(pub std::time::SystemTime);

impl AsByteBuffer for SystemTimeHolder {
    fn size_bytes(&self) -> usize {
        16
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        let dur = self.0.duration_since(std::time::UNIX_EPOCH).map_err(|_| {
            EncodingError::CouldNotEncode("SystemTime before UNIX_EPOCH".to_string())
        })?;
        let micros = dur.as_micros();
        Ok(f(&micros.to_le_bytes()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        let dur = self.0.duration_since(std::time::UNIX_EPOCH).map_err(|_| {
            EncodingError::CouldNotEncode("SystemTime before UNIX_EPOCH".to_string())
        })?;
        let micros = dur.as_micros();
        Ok(micros.to_le_bytes().to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        let bytes = bytes.as_ref();
        let micros = u128::from_le_bytes(bytes.try_into().map_err(|_| {
            DecodingError::CouldNotDecode("Expected 16 bytes for SystemTime".to_string())
        })?);
        let dur = std::time::Duration::from_micros(micros as u64);
        Ok(Self(std::time::UNIX_EPOCH + dur))
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        let dur = self.0.duration_since(std::time::UNIX_EPOCH).map_err(|_| {
            EncodingError::CouldNotEncode("SystemTime before UNIX_EPOCH".to_string())
        })?;
        let micros = dur.as_micros();
        Ok(ByteView::from(micros.to_le_bytes().to_vec()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObjAndUUIDHolder {
    pub obj: Obj,
    pub uuid: Uuid,
}

impl PartialOrd for ObjAndUUIDHolder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.uuid
            .partial_cmp(&other.uuid)
            .or_else(|| self.obj.partial_cmp(&other.obj))
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
            obj: obj.clone(),
            uuid,
        }
    }
}

impl AsByteBuffer for ObjAndUUIDHolder {
    fn size_bytes(&self) -> usize {
        self.obj.size_bytes() + 16
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        let mut bytes = Vec::with_capacity(self.size_bytes());
        bytes.extend_from_slice(self.uuid.as_bytes());
        bytes.extend_from_slice(self.obj.as_bytes()?.as_ref());
        Ok(f(&bytes))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        let mut bytes = Vec::with_capacity(self.size_bytes());
        bytes.extend_from_slice(self.uuid.as_bytes());
        bytes.extend_from_slice(self.obj.as_bytes()?.as_ref());
        Ok(bytes)
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        let bytes = bytes.as_ref();
        let uuid_bytes = bytes.get(..16).ok_or(DecodingError::CouldNotDecode(
            "Expected 16 bytes for UUID".to_string(),
        ))?;
        let obj_bytes = bytes.get(16..).ok_or(DecodingError::CouldNotDecode(
            "Expected 16 bytes for UUID".to_string(),
        ))?;
        let uuid = Uuid::from_bytes(uuid_bytes.try_into().map_err(|_| {
            DecodingError::CouldNotDecode("Expected 16 bytes for UUID".to_string())
        })?);
        let obj = Obj::from_bytes(ByteView::from(obj_bytes.to_vec()))?;
        Ok(Self { obj, uuid })
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        let mut bytes = Vec::with_capacity(self.size_bytes());
        bytes.extend_from_slice(self.uuid.as_bytes());
        bytes.extend_from_slice(self.obj.as_bytes()?.as_ref());
        Ok(ByteView::from(bytes))
    }
}

#[cfg(test)]
mod tests {
    use crate::ObjAndUUIDHolder;
    use moor_values::{AsByteBuffer, SYSTEM_OBJECT};
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
        assert_eq!(oh.uuid, oh2.uuid);
        assert_eq!(oh.obj, oh2.obj);
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
