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

use std::mem::MaybeUninit;

use moor_values::util::{BitArray, Bitset64, BitsetTrait};

use crate::rdb::index::art::direct_mapping::DirectMapping;
use crate::rdb::index::art::keyed_mapping::KeyedMapping;
use crate::rdb::index::art::NodeMapping;

/// A mapping from keys to separate child pointers. 256 keys, usually 48 children.
pub struct IndexedMapping<N: Clone, const WIDTH: usize, Bitset: BitsetTrait> {
    pub child_ptr_indexes: Box<BitArray<u8, 256, Bitset64<4>>>,
    pub children: Box<BitArray<N, WIDTH, Bitset>>,
    pub num_children: u8,
}

impl<N: Clone, const WIDTH: usize, Bitset: BitsetTrait> Default
    for IndexedMapping<N, WIDTH, Bitset>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<N: Clone, const WIDTH: usize, Bitset: BitsetTrait> IndexedMapping<N, WIDTH, Bitset> {
    pub fn new() -> Self {
        Self {
            child_ptr_indexes: Box::default(),
            children: Box::new(BitArray::new()),
            num_children: 0,
        }
    }

    pub fn from_direct(dm: &mut DirectMapping<N>) -> Self {
        let mut indexed = IndexedMapping::new();

        let keys: Vec<usize> = dm.children.iter_keys().collect();
        for key in keys {
            let child = dm.children.erase(key).unwrap();
            indexed.add_child(key as u8, child);
        }
        indexed
    }

    pub fn from_sorted_keyed<const KM_WIDTH: usize>(km: &mut KeyedMapping<N, KM_WIDTH>) -> Self {
        let mut im: IndexedMapping<N, WIDTH, Bitset> = IndexedMapping::new();
        for i in 0..km.num_children as usize {
            let stolen = std::mem::replace(&mut km.children[i], MaybeUninit::uninit());
            im.add_child(km.keys[i], unsafe { stolen.assume_init() });
        }
        km.num_children = 0;
        im
    }

    pub fn move_into<const NEW_WIDTH: usize, NM: NodeMapping<N, NEW_WIDTH>>(
        &mut self,
        nm: &mut NM,
    ) {
        for (key, pos) in self.child_ptr_indexes.iter() {
            let node = self.children.erase(*pos as usize).unwrap();
            nm.add_child(key as u8, node);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (u8, &N)> {
        self.child_ptr_indexes
            .iter()
            .map(move |(key, pos)| (key as u8, &self.children[*pos as usize]))
    }
}

impl<N: Clone, const WIDTH: usize, Bitset: BitsetTrait> NodeMapping<N, WIDTH>
    for IndexedMapping<N, WIDTH, Bitset>
{
    fn add_child(&mut self, key: u8, node: N) {
        let pos = self.children.first_empty().unwrap();
        self.child_ptr_indexes.set(key as usize, pos as u8);
        self.children.set(pos, node);
        self.num_children += 1;
    }

    fn update_child(&mut self, key: u8, node: N) {
        if let Some(pos) = self.child_ptr_indexes.get(key as usize) {
            self.children.set(*pos as usize, node);
        }
    }

    fn seek_child(&self, key: u8) -> Option<&N> {
        if let Some(pos) = self.child_ptr_indexes.get(key as usize) {
            return self.children.get(*pos as usize);
        }
        None
    }

    fn seek_child_mut(&mut self, key: u8) -> Option<&mut N> {
        if let Some(pos) = self.child_ptr_indexes.get(key as usize) {
            return self.children.get_mut(*pos as usize);
        }
        None
    }

    fn delete_child(&mut self, key: u8) -> Option<N> {
        let pos = self.child_ptr_indexes.erase(key as usize)?;

        let old = self.children.erase(pos as usize);
        self.num_children -= 1;

        // Return what we deleted.
        old
    }

    fn num_children(&self) -> usize {
        self.num_children as usize
    }
}

impl<N: Clone, const WIDTH: usize, Bitset: BitsetTrait> Drop for IndexedMapping<N, WIDTH, Bitset> {
    fn drop(&mut self) {
        if self.num_children == 0 {
            return;
        }
        self.num_children = 0;
        self.child_ptr_indexes.clear();
        self.children.clear();
    }
}

impl<N: Clone, const WIDTH: usize, Bitset: BitsetTrait> Clone for IndexedMapping<N, WIDTH, Bitset> {
    fn clone(&self) -> Self {
        let mut new = IndexedMapping::new();
        for c in self.iter() {
            new.add_child(c.0, c.1.clone());
        }
        new.num_children = self.num_children;
        new
    }
}

#[cfg(test)]
mod test {
    use moor_values::util::Bitset16;

    use crate::rdb::index::art::NodeMapping;

    #[test]

    fn test_fits_in_cache_line() {
        assert!(
            std::mem::size_of::<super::IndexedMapping<u8, 48, Bitset16<3>>>() <= 64,
            "IndexedMapping is too large: {} bytes",
            std::mem::size_of::<super::IndexedMapping<u8, 48, Bitset16<3>>>()
        );
    }

    #[test]
    fn test_basic_mapping() {
        let mut mapping = super::IndexedMapping::<u8, 48, Bitset16<3>>::new();
        for i in 0..48 {
            mapping.add_child(i, i);
            assert_eq!(*mapping.seek_child(i).unwrap(), i);
        }
        for i in 0..48 {
            assert_eq!(*mapping.seek_child(i).unwrap(), i);
        }
        for i in 0..48 {
            assert_eq!(mapping.delete_child(i).unwrap(), i);
        }
        for i in 0..48 {
            assert!(mapping.seek_child(i as u8).is_none());
        }
    }
}
