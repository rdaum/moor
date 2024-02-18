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

use crate::rdb::index::Index;
use crate::rdb::tuples::TupleId;
use crate::rdb::RelationError;
use moor_values::util::SliceRef;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub struct HashIndex {
    unique: bool,
    index: HashMap<SliceRef, HashSet<TupleId>>,
}

impl HashIndex {
    pub fn new(unique: bool) -> Self {
        Self {
            unique,
            index: HashMap::new(),
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

impl Index for HashIndex {
    fn check_for_update(&self, attr: &SliceRef) -> Result<(), RelationError> {
        if let Some(tuples) = self.index.get(attr) {
            if tuples.len() > 1 {
                return Err(RelationError::AmbiguousTuple);
            }
        }
        Ok(())
    }

    fn check_constraints(&self, attr: &SliceRef) -> Result<(), RelationError> {
        if let Some(tuples) = self.index.get(attr) {
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
        let Some(set) = self.index.get(attr) else {
            return Ok(Box::new(Iter {
                iter: Box::new(std::iter::empty()),
            }));
        };

        Ok(Box::new(Iter {
            iter: Box::new(set.iter().cloned()),
        }))
    }

    fn index_tuple(&mut self, key: &SliceRef, tuple_id: TupleId) -> Result<(), RelationError> {
        let entry = self.index.entry(key.clone()).or_default();
        if self.unique && !entry.is_empty() {
            return Err(RelationError::UniqueConstraintViolation);
        }
        entry.insert(tuple_id);
        Ok(())
    }

    fn unindex_tuple(&mut self, key: &SliceRef, tuple_id: TupleId) -> Result<(), RelationError> {
        let Some(tuples) = self.index.get_mut(key) else {
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
