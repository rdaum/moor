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

pub mod indexes;
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
    #[error("Store failure when writing to backing store: #[0]")]
    StorageFailure(String),
    #[error("Encoding error")]
    EncodingFailure,
}

/// The `Provider` trait is a generic interface for a key-value store that back the transactional
/// global cache.
pub trait Provider<Domain, Codomain>: Clone {
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error>;
    fn put(&self, timestamp: Timestamp, domain: &Domain, codomain: &Codomain) -> Result<(), Error>;
    fn del(&self, timestamp: Timestamp, domain: &Domain) -> Result<(), Error>;

    /// Scan the database for all keys match the given predicate
    fn scan<F>(&self, predicate: &F) -> Result<Vec<(Timestamp, Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool;

    // Stop any background processing that is running on this provider.
    fn stop(&self) -> Result<(), Error>;
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
