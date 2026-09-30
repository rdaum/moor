// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

#[cfg(test)]
use crate::{Error, Timestamp};

pub mod batch_writer;
pub(crate) mod coordinator;
pub mod fjall_format;
pub(crate) mod fjall_maintenance;
pub mod fjall_provider;
pub(crate) mod logical;
pub(crate) mod property_value_store;
pub(crate) mod snapshot_loader;

/// The `Provider` trait is a generic interface for a value store that backs the transactional
/// front.
/// The source of canonical values, and the place where durable writes go.
/// E.g. a key-value store or some other database.
#[cfg(test)]
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

pub(crate) mod backend;
pub(crate) mod fjall_reader;
pub(crate) mod fjall_relations;
pub(crate) mod read;

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
