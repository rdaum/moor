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

use crate::rdb::index::vector_partial::VectorPartial;
use crate::rdb::index::{KeyTrait, Partial};
use moor_values::util::SliceRef;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TupleKey {
    width: usize,
    sr: SliceRef,
}

impl TupleKey {
    pub fn new(slice_ref: SliceRef) -> Self {
        Self {
            // We add 1 to the length to account for prefix null termination.
            // It's possible we could fix this in the tree iteration logic instead, but for now
            // we'll just do it this way.
            width: slice_ref.len() + 1,
            sr: slice_ref,
        }
    }
}

impl KeyTrait for TupleKey {
    type PartialType = VectorPartial;
    const MAXIMUM_SIZE: Option<usize> = None;

    fn new_from_slice(slice: &[u8]) -> Self {
        Self {
            width: slice.len() + 1,
            sr: SliceRef::from_vec(slice.to_vec()),
        }
    }

    fn new_from_partial(partial: &Self::PartialType) -> Self {
        Self {
            width: partial.len() + 1,
            sr: SliceRef::from_vec(partial.to_vec()),
        }
    }

    fn extend_from_partial(&self, partial: &Self::PartialType) -> Self {
        let mut data = Vec::with_capacity(self.sr.len() + partial.len());
        data.extend_from_slice(self.sr.as_slice());
        data.extend_from_slice(partial.to_slice());
        Self {
            width: data.len() + 1,
            sr: SliceRef::from_vec(data),
        }
    }

    fn truncate(&self, at_depth: usize) -> Self {
        Self {
            width: at_depth,
            sr: self.sr.clone(),
        }
    }

    fn at(&self, pos: usize) -> u8 {
        // Note: Null termination is expected.
        if pos == self.sr.len() {
            0
        } else {
            self.sr.as_slice()[pos]
        }
    }

    fn length_at(&self, at_depth: usize) -> usize {
        self.width - at_depth
    }

    fn to_partial(&self, at_depth: usize) -> Self::PartialType {
        VectorPartial::key(&self.sr.as_slice()[at_depth..])
    }

    fn matches_slice(&self, slice: &[u8]) -> bool {
        self.width == slice.len() && self.sr.as_slice() == &slice[..self.sr.len()]
    }
}

impl From<VectorPartial> for TupleKey {
    fn from(src: VectorPartial) -> Self {
        Self {
            width: src.len(),
            sr: SliceRef::from_vec(src.to_vec()),
        }
    }
}

impl From<TupleKey> for VectorPartial {
    fn from(src: TupleKey) -> Self {
        VectorPartial::from_slice(src.sr.as_slice())
    }
}
