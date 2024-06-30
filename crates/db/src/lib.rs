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

use bytes::Bytes;
use moor_values::{AsByteBuffer, DecodingError, EncodingError};
use uuid::Uuid;

use moor_values::model::WorldStateError;
use moor_values::model::WorldStateSource;

use crate::loader::LoaderInterface;

mod db_loader_client;
pub mod db_worldstate;
pub mod loader;
mod relational_transaction;
mod relational_worldstate;
mod worldstate_tables;
pub mod worldstate_transaction;

mod worldstate_tests;

pub use relational_transaction::{RelationalError, RelationalTransaction};
pub use relational_worldstate::RelationalWorldStateTransaction;
pub use worldstate_tables::{WorldStateSequence, WorldStateTable};
pub use worldstate_tests::*;

pub trait Database: Send + WorldStateSource {
    fn loader_client(&self) -> Result<Box<dyn LoaderInterface>, WorldStateError>;
}

/// Possible backend storage engines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DatabaseFlavour {
    /// WiredTiger, a high-performance, scalable, transactional storage engine, also used in MongoDB.
    /// Adaptation still under development.
    WiredTiger,
    /// In-house in-memory MVCC transactional store based on copy-on-write hashes and trees and
    /// custom buffer pool management. Consider experimental.
    #[cfg(feature = "relbox")]
    RelBox,
}

impl From<&str> for DatabaseFlavour {
    fn from(s: &str) -> Self {
        match s {
            "wiredtiger" => DatabaseFlavour::WiredTiger,
            #[cfg(feature = "relbox")]
            "relbox" => DatabaseFlavour::RelBox,
            _ => panic!("Unknown database flavour: {}", s),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError> {
        Ok(Self(
            String::from_utf8(bytes.to_vec()).expect("Invalid UTF-8"),
        ))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        Ok(Bytes::from(self.0.as_bytes().to_vec()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError> {
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

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        Ok(Bytes::from(self.0.as_bytes().to_vec()))
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

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError> {
        Ok(Self(bytes.to_vec()))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        Ok(Bytes::from(self.0.clone()))
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

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError> {
        let bytes = bytes.as_ref();
        let micros = u128::from_le_bytes(bytes.try_into().map_err(|_| {
            DecodingError::CouldNotDecode("Expected 16 bytes for SystemTime".to_string())
        })?);
        let dur = std::time::Duration::from_micros(micros as u64);
        Ok(Self(std::time::UNIX_EPOCH + dur))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        let dur = self.0.duration_since(std::time::UNIX_EPOCH).map_err(|_| {
            EncodingError::CouldNotEncode("SystemTime before UNIX_EPOCH".to_string())
        })?;
        let micros = dur.as_micros();
        Ok(Bytes::from(micros.to_le_bytes().to_vec()))
    }
}
