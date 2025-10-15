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

use crate::{Error, Timestamp};

pub mod fjall_migration;
pub mod fjall_provider;
pub mod fjall_snapshot_loader;

/// The `Provider` trait is a generic interface for a value store that backs the transactional
/// front.
/// The source of canonical values, and the place where durable writes go.
/// E.g. a key-value store or some other database.
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

/// Generic database migration interface for format changes.
/// Each storage backend (e.g., Fjall) implements this to handle version detection and migrations.
pub trait Migrator {
    /// Check if migration is needed and perform it if so.
    /// Should be idempotent - safe to call multiple times.
    fn migrate_if_needed(&self) -> Result<(), String>;

    /// Mark the database as migrated to the current version.
    /// Called after a fresh database is created.
    fn mark_current_version(&self) -> Result<(), String>;
}
