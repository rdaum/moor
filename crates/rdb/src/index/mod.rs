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
mod btree_index;
mod hash_index;
mod im_btree_index;
mod im_hash_index;

use crate::tuples::TupleId;
pub use art::array_key::ArrayKey;
#[allow(unused_imports)] // Future use, stop warning me.
pub use art::tree::AdaptiveRadixTree;
pub use art::vector_key::VectorKey;

use crate::index::btree_index::BtreeIndex;
use crate::index::im_btree_index::ImBtreeIndex;
use crate::RelationError;
pub use art_index::ArtArrayIndex;
pub use hash_index::HashIndex;
pub use im_hash_index::ImHashIndex;
use moor_values::util::SliceRef;
use strum::EnumString;

/// Types that domains or codomains can be for the purpose of indexing.
///
/// Note that this is not the same as the `moor` Var type, but instead used for declaring the types of the purpose of
/// indexing and querying. The actual user data can be stored in a variety of ways, but the TupleType is used by the
/// indexing code to manage e.g. encoding, ordering, hashing, etc.
#[derive(Copy, Clone, Debug, PartialEq, Eq, EnumString)]
pub enum AttrType {
    /// The tuple attribute in question is a signed 64-bit integer.
    Integer,
    /// The tuple attribute in question is an unsigned 64-bit integer.
    UnsignedInteger,
    /// The tuple attribute in question is a 64-bit floating point number.
    Float,
    /// The tuple attribute in question is a string.
    String,
    /// The tuple attribute in question is a byte array.
    Bytes,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, EnumString)]
pub enum IndexType {
    /// Unordered arbitrary keys. Lookup speed is O(1).
    Hash,
    /// Viable for integer keys. Keys are ordered and must fit in a fixed size.
    /// Lookup speed is O(log N), but real world performance lies between Hash and BTree.
    /// Linear scan is (theoretically) faster than both.
    AdaptiveRadixTree,
    /// For ordered keys, valid for everything but Bytes.
    /// Lookup speed is O(log n).
    BTree,
}

impl IndexType {
    /// Return true if the index is ordered.
    /// (Will be used to determine the kinds of operations that can be performed on the index, and the join strategies
    /// employed.)
    pub fn is_ordered(&self) -> bool {
        match self {
            IndexType::AdaptiveRadixTree => true,
            IndexType::Hash => false,
            IndexType::BTree => true,
        }
    }
}

pub trait Index {
    /// Return the index type for the index.
    fn index_type(&self) -> IndexType;
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

pub fn pick_tx_index(
    relation_info: &crate::RelationInfo,
) -> (Box<dyn Index>, Option<Box<dyn Index>>) {
    let domain_index: Box<dyn Index> = match relation_info.index_type {
        IndexType::AdaptiveRadixTree => Box::new(ArtArrayIndex::new(
            relation_info.domain_type,
            relation_info.unique_domain,
        )),
        IndexType::Hash => Box::new(HashIndex::new(relation_info.unique_domain)),
        IndexType::BTree => Box::new(BtreeIndex::new(
            relation_info.domain_type,
            relation_info.unique_domain,
        )),
    };
    let codomain_index: Option<Box<dyn Index>> = match relation_info.codomain_index_type {
        Some(IndexType::AdaptiveRadixTree) => Some(Box::new(ArtArrayIndex::new(
            relation_info.codomain_type,
            false,
        ))),
        Some(IndexType::Hash) => Some(Box::new(HashIndex::new(false))),
        None => None,
        Some(IndexType::BTree) => Some(Box::new(BtreeIndex::new(
            relation_info.codomain_type,
            false,
        ))),
    };

    (domain_index, codomain_index)
}

pub fn pick_base_index(
    relation_info: &crate::RelationInfo,
) -> (
    Box<dyn Index + Send + Sync>,
    Option<Box<dyn Index + Send + Sync>>,
) {
    let domain_index: Box<dyn Index + Send + Sync> = match relation_info.index_type {
        IndexType::AdaptiveRadixTree => Box::new(ArtArrayIndex::new(
            relation_info.domain_type,
            relation_info.unique_domain,
        )),
        IndexType::Hash => Box::new(ImHashIndex::new(relation_info.unique_domain)),
        IndexType::BTree => Box::new(ImBtreeIndex::new(
            relation_info.domain_type,
            relation_info.unique_domain,
        )),
    };
    let codomain_index: Option<Box<dyn Index + Send + Sync>> =
        match relation_info.codomain_index_type {
            Some(IndexType::AdaptiveRadixTree) => Some(Box::new(ArtArrayIndex::new(
                relation_info.codomain_type,
                false,
            ))),
            Some(IndexType::Hash) => Some(Box::new(ImHashIndex::new(false))),
            None => None,
            Some(IndexType::BTree) => Some(Box::new(ImBtreeIndex::new(
                relation_info.codomain_type,
                false,
            ))),
        };
    (domain_index, codomain_index)
}
