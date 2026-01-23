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

mod indexes;
mod relation;
mod relation_tx;

pub use relation::{CheckRelation, Relation};
pub use relation_tx::{RelationTransaction, WorkingSet};

use std::fmt::{Debug, Display};
use std::hash::Hash;

// ============================================================================
// Trait Bounds for Relation Domain and Codomain Types
// ============================================================================
//
// These traits reduce boilerplate in type parameter bounds throughout the
// tx_management and provider modules. They use the blanket impl pattern since
// Rust stable doesn't have native trait aliases.

/// Trait alias for types that can be used as a domain (key) in a relation.
///
/// Domain types must support:
/// - `Hash + Eq`: For use in hash-based indexes
/// - `Clone`: For copying keys during operations
/// - `Debug`: For error messages and conflict reporting
/// - `Send + Sync + 'static`: For thread-safe, owned storage
pub trait RelationDomain: Hash + Eq + Clone + Debug + Display + Send + Sync + 'static {}

impl<T> RelationDomain for T where T: Hash + Eq + Clone + Debug + Display + Send + Sync + 'static {}

/// Trait alias for types that can be used as a codomain (value) in a relation.
///
/// Codomain types must support:
/// - `Clone`: For copying values during operations
/// - `PartialEq`: For conflict detection and comparison
/// - `Send + Sync + 'static`: For thread-safe, owned storage
pub trait RelationCodomain: Clone + PartialEq + Send + Sync + 'static {}

impl<T> RelationCodomain for T where T: Clone + PartialEq + Send + Sync + 'static {}

/// Extended trait alias for codomain types that can be used with secondary indexes.
///
/// In addition to `RelationCodomain` bounds, these types must also support:
/// - `Hash + Eq`: For reverse lookups in secondary indexes
pub trait RelationCodomainHashable: RelationCodomain + Hash + Eq {}

impl<T> RelationCodomainHashable for T where T: RelationCodomain + Hash + Eq {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Timestamp(pub u64);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Tx {
    pub ts: Timestamp,
    pub snapshot_version: u64,
}

pub use moor_common::model::{ConflictInfo, ConflictType};

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("Duplicate key")]
    Duplicate,
    #[error("Conflict detected: {0}")]
    Conflict(ConflictInfo),
    #[error("Retrieval error from backing store")]
    RetrievalFailure(String),
    #[error("Store failure when writing to backing store: #{0}")]
    StorageFailure(String),
    #[error("Encoding error")]
    EncodingFailure,
}

/// Trait for handling persistence of a specific type T.
/// Provider implementations implement this trait multiple times for different types,
/// allowing per-type encoding and storage decisions.
///
/// This trait does NOT assume a universal byte representation - each type's impl
/// can encode and persist however it wants.
pub trait EncodeFor<T> {
    /// Type representing the stored form - could be bytes, SQL row, etc.
    type Stored;

    /// Encode a value to its stored representation
    fn encode(&self, value: &T) -> Result<Self::Stored, Error>;

    /// Decode from stored representation
    fn decode(&self, stored: Self::Stored) -> Result<T, Error>;
}

/// Represents a "canonical" source for some domain/codomain pair, to be supplied to a
/// transaction.
pub trait Canonical<Domain, Codomain> {
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error>;
    fn scan<F>(&self, f: &F) -> Result<Vec<(Timestamp, Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool;
    fn get_by_codomain(&self, codomain: &Codomain) -> Vec<Domain>;
}
