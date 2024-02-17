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

use crate::rdb::index::indexed_mapping::IndexedMapping;
use moor_values::util::{BitArray, Bitset64, BitsetTrait};

use super::NodeMapping;

pub struct DirectMapping<N: Clone> {
    pub(crate) children: BitArray<N, 256, Bitset64<4>>,
    num_children: usize,
}

impl<N: Clone> Default for DirectMapping<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N: Clone> DirectMapping<N> {
    pub fn new() -> Self {
        Self {
            children: BitArray::new(),
            num_children: 0,
        }
    }

    #[allow(dead_code)]
    pub fn from_indexed<const WIDTH: usize, FromBitset: BitsetTrait>(
        im: &mut IndexedMapping<N, WIDTH, FromBitset>,
    ) -> Self {
        let mut new_mapping = DirectMapping::<N>::new();
        im.num_children = 0;
        im.move_into(&mut new_mapping);
        new_mapping
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (u8, &N)> {
        self.children.iter().map(|(key, node)| (key as u8, node))
    }
}

impl<N: Clone> Clone for DirectMapping<N> {
    fn clone(&self) -> Self {
        let mut new = DirectMapping::new();
        for (key, node) in self.iter() {
            new.add_child(key, node.clone());
        }
        new
    }
}

impl<N: Clone> NodeMapping<N, 256> for DirectMapping<N> {
    #[inline]
    fn add_child(&mut self, key: u8, node: N) {
        self.children.set(key as usize, node);
        self.num_children += 1;
    }

    fn update_child(&mut self, key: u8, node: N) {
        if let Some(n) = self.children.get_mut(key as usize) {
            *n = node;
        }
    }

    #[inline]
    fn seek_child(&self, key: u8) -> Option<&N> {
        self.children.get(key as usize)
    }

    #[inline]
    fn seek_child_mut(&mut self, key: u8) -> Option<&mut N> {
        self.children.get_mut(key as usize)
    }

    #[inline]
    fn delete_child(&mut self, key: u8) -> Option<N> {
        let n = self.children.erase(key as usize);
        if n.is_some() {
            self.num_children -= 1;
        }
        n
    }

    #[inline]
    fn num_children(&self) -> usize {
        self.num_children
    }
}

#[cfg(test)]
mod tests {
    use super::NodeMapping;

    #[test]
    fn direct_mapping_test() {
        let mut dm = super::DirectMapping::new();
        for i in 0..255 {
            dm.add_child(i, i);
            assert_eq!(*dm.seek_child(i).unwrap(), i);
            assert_eq!(dm.delete_child(i), Some(i));
            assert_eq!(dm.seek_child(i), None);
        }
    }
}
