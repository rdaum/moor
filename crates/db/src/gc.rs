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

use moor_common::model::{CommitResult, WorldStateError};
use moor_var::Obj;
use std::collections::HashSet;

#[derive(Debug, Clone, thiserror::Error)]
pub enum GCError {
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Transaction conflict")]
    TransactionConflict,
    #[error("Object not found: {0}")]
    ObjectNotFound(String),
    #[error("Commit failed: {0}")]
    CommitFailed(String),
}

/// Interface for garbage collection operations on anonymous objects
pub trait GCInterface: Send {
    /// Scan the database for anonymous object references in properties and other data structures
    fn scan_anonymous_object_references(
        &mut self,
    ) -> Result<Vec<(Obj, HashSet<Obj>)>, WorldStateError>;

    /// Get all anonymous objects
    fn get_anonymous_objects(&self) -> Result<HashSet<Obj>, WorldStateError>;

    /// Remove unreachable anonymous objects from the database
    fn collect_unreachable_anonymous_objects(
        &mut self,
        unreachable_objects: &HashSet<Obj>,
    ) -> Result<usize, WorldStateError>;
    /// Commit any pending changes to the database
    fn commit(self: Box<Self>) -> Result<CommitResult, GCError>;

    /// Rollback any pending changes
    fn rollback(self: Box<Self>) -> Result<(), GCError>;
}
