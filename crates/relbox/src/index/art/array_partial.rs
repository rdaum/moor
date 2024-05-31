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
use std::ops::Index;

use crate::index::art::array_key::ArrayKey;
use crate::index::art::{KeyTrait, Partial};

#[derive(Clone, Debug, Eq)]
pub struct ArrPartial<const SIZE: usize> {
    data: [u8; SIZE],
    len: usize,
}

impl<const SIZE: usize> PartialEq for ArrPartial<SIZE> {
    fn eq(&self, other: &Self) -> bool {
        self.data[..self.len] == other.data[..other.len]
    }
}
impl<const SIZE: usize> ArrPartial<SIZE> {
    pub fn key(src: &[u8]) -> Self {
        assert!(src.len() < SIZE);
        let mut data = [0; SIZE];
        data[..src.len()].copy_from_slice(src);
        Self {
            data,
            len: src.len() + 1,
        }
    }

    pub fn from_slice(src: &[u8]) -> Self {
        assert!(
            src.len() <= SIZE,
            "data length {} is greater than maximum partial length {}",
            src.len(),
            SIZE
        );
        let mut data = [0; SIZE];
        data[..src.len()].copy_from_slice(src);
        Self {
            data,
            len: src.len(),
        }
    }

    pub fn to_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }
}

impl<const SIZE: usize> Index<usize> for ArrPartial<SIZE> {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        self.data.index(index)
    }
}
impl<const SIZE: usize> Partial for ArrPartial<SIZE> {
    fn partial_before(&self, length: usize) -> Self {
        assert!(length <= self.len);
        ArrPartial::from_slice(&self.data[..length])
    }

    fn partial_from(&self, src_offset: usize, length: usize) -> Self {
        assert!(src_offset + length <= self.len);
        ArrPartial::from_slice(&self.data[src_offset..src_offset + length])
    }

    fn partial_after(&self, start: usize) -> Self {
        assert!(start <= self.len);
        ArrPartial::from_slice(&self.data[start..self.len])
    }

    fn partial_extended_with(&self, other: &Self) -> Self {
        assert!(self.len + other.len < SIZE);
        let mut data = [0; SIZE];
        data[..self.len].copy_from_slice(&self.data[..self.len]);
        data[self.len..self.len + other.len].copy_from_slice(&other.data[..other.len]);
        Self {
            data,
            len: self.len + other.len,
        }
    }

    #[inline(always)]
    fn at(&self, pos: usize) -> u8 {
        assert!(pos < self.len);
        self.data[pos]
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.len
    }

    fn prefix_length_common(&self, other: &Self) -> usize {
        self.prefix_length_slice(other.to_slice())
    }

    fn prefix_length_key<'a, K>(&self, key: &'a K, at_depth: usize) -> usize
    where
        K: KeyTrait<PartialType = Self> + 'a,
    {
        let len = min(self.len, key.length_at(0));
        let len = min(len, SIZE);
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
        let len = min(self.len, slice.len());
        let len = min(len, SIZE);
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
        &self.data[..self.len]
    }
}

impl<const SIZE: usize> From<&[u8]> for ArrPartial<SIZE> {
    fn from(src: &[u8]) -> Self {
        Self::from_slice(src)
    }
}

impl<const SIZE: usize> From<ArrayKey<SIZE>> for ArrPartial<SIZE> {
    fn from(value: ArrayKey<SIZE>) -> Self {
        value.to_partial(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partial_before() {
        let arr: ArrPartial<16> = ArrPartial::from_slice(b"Hello, world!");
        assert_eq!(arr.partial_before(5).to_slice(), b"Hello");
    }

    #[test]
    fn test_partial_from() {
        let arr: ArrPartial<16> = ArrPartial::from_slice(b"Hello, world!");
        assert_eq!(arr.partial_from(7, 5).to_slice(), b"world");
    }

    #[test]
    fn test_prefix_after() {
        let arr: ArrPartial<16> = ArrPartial::from_slice(b"Hello, world!");
        assert_eq!(arr.partial_after(7).to_slice(), b"world!");
    }

    #[test]
    fn test_at() {
        let arr: ArrPartial<16> = ArrPartial::from_slice(b"Hello, world!");
        assert_eq!(arr.at(0), 72);
    }

    #[test]
    fn test_length() {
        let arr: ArrPartial<16> = ArrPartial::from_slice(b"Hello, world!");
        assert_eq!(arr.len(), 13);
    }

    #[test]
    fn test_prefix_length_common() {
        let arr1: ArrPartial<16> = ArrPartial::from_slice(b"Hello, world!");
        let arr2: ArrPartial<16> = ArrPartial::from_slice(b"Hello, there!");
        assert_eq!(arr1.prefix_length_common(&arr2), 7);
    }

    #[test]
    fn test_key() {
        let arr: ArrPartial<16> = ArrPartial::key(b"Hello, world!");
        assert_eq!(
            arr.to_slice(),
            &[72, 101, 108, 108, 111, 44, 32, 119, 111, 114, 108, 100, 33, 0]
        );
    }

    #[test]
    fn test_from_slice() {
        let arr: ArrPartial<16> = ArrPartial::from_slice(b"Hello, world!");
        assert_eq!(arr.to_slice(), b"Hello, world!");
    }

    #[test]
    fn test_partial_chain_with_key() {
        let arr1: ArrPartial<16> = ArrPartial::key(b"Hello, world!");
        let arr2: ArrPartial<16> = ArrPartial::key(b"Hello, there!");
        let partial1 = arr1.partial_before(6);
        assert_eq!(partial1.to_slice(), b"Hello,");
        let partial2 = arr2.partial_from(7, 5);
        assert_eq!(partial2.to_slice(), b"there");
        let partial3 = partial1.partial_after(1);
        assert_eq!(partial3.to_slice(), b"ello,");
        assert_eq!(0, partial3.prefix_length_common(&partial2));
    }
}
