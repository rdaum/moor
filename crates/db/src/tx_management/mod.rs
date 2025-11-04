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

mod indexes;
mod relation;
mod relation_tx;

pub use relation::{CheckRelation, Relation};
pub use relation_tx::{RelationTransaction, WorkingSet};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Timestamp(pub u64);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Tx {
    pub ts: Timestamp,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("Duplicate key")]
    Duplicate,
    #[error("Conflict detected")]
    Conflict,
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
