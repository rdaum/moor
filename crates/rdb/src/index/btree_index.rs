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

use crate::index::{AttrType, Index};
use crate::tuples::TupleId;
use crate::{IndexType, RelationError};
use moor_values::util::SliceRef;
use std::collections::{BTreeMap, HashSet};

#[derive(Clone)]
pub struct BtreeIndex {
    attr_type: AttrType,
    unique: bool,
    index: BTreeMap<Key, HashSet<TupleId>>,
}

/// Ordered key types that can be used in the index, converted based on type.
#[derive(Clone)]
enum Key {
    Integer(SliceRef),
    UnsignedInteger(SliceRef),
    Float(SliceRef),
    String(SliceRef),
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Key::Integer(a), Key::Integer(b)) => a == b,
            (Key::UnsignedInteger(a), Key::UnsignedInteger(b)) => a == b,
            (Key::Float(a), Key::Float(b)) => a == b,
            (Key::String(a), Key::String(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Key::Integer(a), Key::Integer(b)) => a.cmp(b),
            (Key::UnsignedInteger(a), Key::UnsignedInteger(b)) => a.cmp(b),
            (Key::Float(a), Key::Float(b)) => a.cmp(b),
            (Key::String(a), Key::String(b)) => a.cmp(b),
            _ => std::cmp::Ordering::Equal,
        }
    }
}

fn to_key(slice_ref: &SliceRef, attr_type: AttrType) -> Result<Key, RelationError> {
    match attr_type {
        AttrType::Integer => Ok(Key::Integer(slice_ref.clone())),
        AttrType::UnsignedInteger => Ok(Key::UnsignedInteger(slice_ref.clone())),
        AttrType::Float => Ok(Key::Float(slice_ref.clone())),
        AttrType::String => Ok(Key::String(slice_ref.clone())),
        _ => Err(RelationError::BadKey),
    }
}

pub struct Iter<'a> {
    iter: Box<dyn Iterator<Item = TupleId> + 'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = TupleId;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl BtreeIndex {
    pub fn new(attr_type: AttrType, unique: bool) -> Self {
        Self {
            attr_type,
            unique,
            index: BTreeMap::new(),
        }
    }
}

impl Index for BtreeIndex {
    fn index_type(&self) -> IndexType {
        IndexType::BTree
    }

    fn check_for_update(&self, attr: &SliceRef) -> Result<(), RelationError> {
        let key = to_key(attr, self.attr_type)?;
        if let Some(tuples) = self.index.get(&key) {
            if tuples.len() > 1 {
                return Err(RelationError::AmbiguousTuple);
            }
        }
        Ok(())
    }

    fn check_constraints(&self, attr: &SliceRef) -> Result<(), RelationError> {
        let key = to_key(attr, self.attr_type)?;
        if let Some(tuples) = self.index.get(&key) {
            if self.unique && !tuples.is_empty() {
                return Err(RelationError::UniqueConstraintViolation);
            }
        }
        Ok(())
    }

    fn seek(
        &self,
        attr: &SliceRef,
    ) -> Result<Box<dyn Iterator<Item = TupleId> + '_>, RelationError> {
        let key = to_key(attr, self.attr_type)?;
        let Some(set) = self.index.get(&key) else {
            return Ok(Box::new(Iter {
                iter: Box::new(std::iter::empty()),
            }));
        };

        Ok(Box::new(Iter {
            iter: Box::new(set.iter().cloned()),
        }))
    }

    fn index_tuple(&mut self, key: &SliceRef, tuple_id: TupleId) -> Result<(), RelationError> {
        let key = to_key(key, self.attr_type)?;
        let entry = self.index.entry(key).or_default();
        if self.unique && !entry.is_empty() {
            return Err(RelationError::UniqueConstraintViolation);
        }
        entry.insert(tuple_id);
        Ok(())
    }

    fn unindex_tuple(&mut self, key: &SliceRef, tuple_id: TupleId) -> Result<(), RelationError> {
        let key = to_key(key, self.attr_type)?;
        let Some(tuples) = self.index.get_mut(&key) else {
            return Err(RelationError::TupleNotFound);
        };
        if !tuples.remove(&tuple_id) {
            return Err(RelationError::TupleNotFound);
        }

        if self.unique && !tuples.is_empty() {
            return Err(RelationError::UniqueConstraintViolation);
        }
        Ok(())
    }

    fn clone_index(&self) -> Box<dyn Index + Send + Sync> {
        Box::new(self.clone())
    }

    fn clear(&mut self) {
        self.index.clear();
    }
}
