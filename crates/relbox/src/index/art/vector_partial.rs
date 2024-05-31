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

use std::cmp::min;

use crate::index::art::vector_key::VectorKey;
use crate::index::art::{KeyTrait, Partial};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VectorPartial {
    data: Box<[u8]>,
}

impl VectorPartial {
    pub fn key(src: &[u8]) -> Self {
        let mut data = Vec::with_capacity(src.len() + 1);
        data.extend_from_slice(src);
        data.push(0);
        Self {
            data: data.into_boxed_slice(),
        }
    }

    pub fn from_slice(src: &[u8]) -> Self {
        Self {
            data: Box::from(src),
        }
    }
    pub fn to_slice(&self) -> &[u8] {
        &self.data
    }

    #[allow(dead_code)]
    pub fn to_vec(&self) -> Vec<u8> {
        self.data.to_vec()
    }
}

impl From<&[u8]> for VectorPartial {
    fn from(src: &[u8]) -> Self {
        Self::from_slice(src)
    }
}

impl Partial for VectorPartial {
    fn partial_before(&self, length: usize) -> Self {
        assert!(length <= self.data.len());
        VectorPartial::from_slice(&self.data[..length])
    }

    fn partial_from(&self, src_offset: usize, length: usize) -> Self {
        assert!(src_offset + length <= self.data.len());
        VectorPartial::from_slice(&self.data[src_offset..src_offset + length])
    }

    fn partial_after(&self, start: usize) -> Self {
        assert!(start <= self.data.len());
        VectorPartial::from_slice(&self.data[start..self.data.len()])
    }

    fn partial_extended_with(&self, other: &Self) -> Self {
        let mut data = Vec::with_capacity(self.data.len() + other.data.len());
        data.extend_from_slice(&self.data);
        data.extend_from_slice(&other.data);
        Self {
            data: data.into_boxed_slice(),
        }
    }

    #[inline(always)]
    fn at(&self, pos: usize) -> u8 {
        assert!(pos < self.data.len());
        self.data[pos]
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.data.len()
    }

    fn prefix_length_common(&self, other: &Self) -> usize {
        self.prefix_length_slice(other.to_slice())
    }

    fn prefix_length_key<'a, K>(&self, key: &'a K, at_depth: usize) -> usize
    where
        K: KeyTrait<PartialType = Self> + 'a,
    {
        let len = min(self.data.len(), key.length_at(0));
        let mut idx = 0;
        while idx < len {
            if self.data[idx] != key.at(idx + at_depth) {
                break;
            }
            idx += 1;
        }
        idx
    }

    fn prefix_length_slice(&self, slice: &[u8]) -> usize {
        let len = min(self.data.len(), slice.len());
        let mut idx = 0;
        while idx < len {
            if self.data[idx] != slice[idx] {
                break;
            }
            idx += 1;
        }
        idx
    }

    fn to_slice(&self) -> &[u8] {
        &self.data[..self.data.len()]
    }
}

impl From<VectorKey> for VectorPartial {
    fn from(src: VectorKey) -> Self {
        src.to_partial(0)
    }
}
