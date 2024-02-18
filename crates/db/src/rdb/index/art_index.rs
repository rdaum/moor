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

use crate::rdb::index::{AdaptiveRadixTree, ArrayKey, Index};
use crate::rdb::tuples::TupleId;
use crate::rdb::{AttrType, RelationError};
use moor_values::util::SliceRef;
use std::collections::HashSet;
use tracing::error;

/// Adaptive Radix Tree index for when the keys are fixed-length values.
#[derive(Clone)]
pub struct ArtArrayIndex {
    attr_type: AttrType,
    unique: bool,
    index: AdaptiveRadixTree<ArrayKey<16>, HashSet<TupleId>>,
}

impl ArtArrayIndex {
    pub fn new(attr_type: AttrType, unique: bool) -> Self {
        Self {
            attr_type,
            unique,
            index: AdaptiveRadixTree::new(),
        }
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

fn to_key(attr_type: AttrType, sr: &SliceRef) -> Result<ArrayKey<16>, RelationError> {
    match attr_type {
        AttrType::Integer => {
            let sr = sr.as_slice();
            let k = i64::from_le_bytes(sr.try_into().map_err(|_| RelationError::BadKey)?);
            Ok(k.into())
        }
        AttrType::UnsignedInteger => {
            let k = u64::from_le_bytes(
                sr.as_slice()
                    .try_into()
                    .map_err(|_| RelationError::BadKey)?,
            );
            Ok(k.into())
        }
        _ => Err(RelationError::BadKey),
    }
}

impl ArtArrayIndex {
    fn to_attr_key(&self, attr: &SliceRef) -> Result<ArrayKey<16>, RelationError> {
        to_key(self.attr_type, attr)
    }
}

impl Index for ArtArrayIndex {
    fn check_for_update(&self, attr: &SliceRef) -> Result<(), RelationError> {
        let attr_keys = self.to_attr_key(attr)?;
        if let Some(tuples) = self.index.get_k(&attr_keys) {
            if tuples.len() > 1 {
                error!("Ambiguous tuple");

                return Err(RelationError::AmbiguousTuple);
            }
        }
        Ok(())
    }

    fn check_constraints(&self, attr: &SliceRef) -> Result<(), RelationError> {
        let attr_key = self.to_attr_key(attr)?;
        if let Some(tuples) = self.index.get_k(&attr_key) {
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
        let attr_key = self.to_attr_key(attr)?;

        let Some(set) = self.index.get_k(&attr_key) else {
            return Ok(Box::new(Iter {
                iter: Box::new(std::iter::empty()),
            }));
        };

        Ok(Box::new(Iter {
            iter: Box::new(set.iter().cloned()),
        }))
    }

    fn index_tuple(&mut self, key: &SliceRef, tuple_id: TupleId) -> Result<(), RelationError> {
        let attr_key = self.to_attr_key(key)?;

        match self.index.get_k_mut(&attr_key) {
            None => {
                let mut set = HashSet::new();
                set.insert(tuple_id);
                self.index.insert_k(&attr_key, set);
            }
            Some(ref mut v) => {
                if self.unique && !v.is_empty() {
                    return Err(RelationError::UniqueConstraintViolation);
                }
                v.insert(tuple_id);
            }
        }
        Ok(())
    }

    fn unindex_tuple(&mut self, key: &SliceRef, tuple_id: TupleId) -> Result<(), RelationError> {
        let dattr_keymain = self.to_attr_key(key)?;

        let Some(ref mut keys) = self.index.get_k_mut(&dattr_keymain) else {
            return Err(RelationError::TupleNotFound);
        };

        keys.remove(&tuple_id);

        if self.unique && !keys.is_empty() {
            return Err(RelationError::UniqueConstraintViolation);
        }
        Ok(())
    }

    fn clone_index(&self) -> Box<dyn Index + Send + Sync> {
        Box::new(self.clone())
    }

    fn clear(&mut self) {
        self.index = AdaptiveRadixTree::new();
    }
}
