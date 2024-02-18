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

//! Adaptive Radix Tree implementation for use for tuple indices.
//! Supports internal CoW semantics.

mod art;
mod art_index;
mod hash_index;
mod im_hash_index;

use crate::rdb::tuples::TupleId;
pub use art::array_key::ArrayKey;
#[allow(unused_imports)] // Future use, stop warning me.
pub use art::tree::AdaptiveRadixTree;
pub use art::vector_key::VectorKey;

use crate::rdb::RelationError;
pub use art_index::ArtArrayIndex;
pub use hash_index::HashIndex;
pub use im_hash_index::ImHashIndex;
use moor_values::util::SliceRef;

pub trait Index {
    /// Check for potential duplicates which could cause ambiguous updates for update operations.
    fn check_for_update(&self, domain: &SliceRef) -> Result<(), RelationError>;
    /// Check (and trigger) the constraints of the given tuple.
    fn check_constraints(&self, domain: &SliceRef) -> Result<(), RelationError>;
    /// Seek matching tuples for the given domain value.
    fn seek(
        &self,
        domain: &SliceRef,
    ) -> Result<Box<dyn Iterator<Item = TupleId> + '_>, RelationError>;
    /// Index the given tuple.
    fn index_tuple(&mut self, key: &SliceRef, tuple_id: TupleId) -> Result<(), RelationError>;
    /// Remove the given tuple from the index.
    fn unindex_tuple(&mut self, key: &SliceRef, tuple_id: TupleId) -> Result<(), RelationError>;
    /// Clone the index.
    /// Need this because Clone is not object-safe so the trait can't declare itself clone-able directly.
    fn clone_index(&self) -> Box<dyn Index + Send + Sync>;
    /// Clear the indices.
    fn clear(&mut self);
}
