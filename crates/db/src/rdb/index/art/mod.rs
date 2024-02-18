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

pub mod array_key;
mod array_partial;
mod direct_mapping;
mod indexed_mapping;
mod iter;
mod keyed_mapping;
mod node;
pub mod tree;
mod u8_keys;
pub mod vector_key;
mod vector_partial;

/// Trait for "node mapping" structures used internally inside the
pub trait NodeMapping<N, const NUM_CHILDREN: usize> {
    const NUM_CHILDREN: usize = NUM_CHILDREN;

    fn add_child(&mut self, key: u8, node: N);
    fn delete_child(&mut self, key: u8) -> Option<N>;
    fn num_children(&self) -> usize;
    fn seek_child(&self, key: u8) -> Option<&N>;
    fn seek_child_mut(&mut self, key: u8) -> Option<&mut N>;
    fn update_child(&mut self, key: u8, node: N);

    fn width(&self) -> usize {
        Self::NUM_CHILDREN
    }
}

/// Trait for the partial key fragments used in the radix tree nodes.
pub trait Partial {
    /// Returns a partial up to `length` bytes.
    fn partial_before(&self, length: usize) -> Self;
    /// Returns a partial from `src_offset` onwards with `length` bytes.
    fn partial_from(&self, src_offset: usize, length: usize) -> Self;
    /// Returns a partial from `start` onwards.
    fn partial_after(&self, start: usize) -> Self;
    /// Extends the partial with another partial.
    fn partial_extended_with(&self, other: &Self) -> Self;
    /// Returns the byte at `pos`.
    fn at(&self, pos: usize) -> u8;
    /// Returns the length of the partial.
    fn len(&self) -> usize;
    /// Returns true if the partial is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Returns the length of the common prefix between `self` and `other`.
    fn prefix_length_common(&self, other: &Self) -> usize;
    /// Returns the length of the common prefix between `self` and `key`.
    fn prefix_length_key<'a, K>(&self, key: &'a K, at_depth: usize) -> usize
    where
        K: KeyTrait<PartialType = Self> + 'a;
    /// Returns the length of the common prefix between `self` and `slice`.
    fn prefix_length_slice(&self, slice: &[u8]) -> usize;
    /// Return a slice form of the partial. Warning: could take copy, depending on the implementation.
    /// Really just for debugging purposes.
    fn to_slice(&self) -> &[u8];
}

// Trait for the keys used in the radix tree nodes.
pub trait KeyTrait: Clone + PartialEq + Eq {
    type PartialType: Partial + From<Self> + Clone + PartialEq;

    const MAXIMUM_SIZE: Option<usize>;

    fn new_from_slice(slice: &[u8]) -> Self;
    fn new_from_partial(partial: &Self::PartialType) -> Self;
    fn terminate_with_partial(&self, partial: &Self::PartialType) -> Self;

    fn extend_from_partial(&self, partial: &Self::PartialType) -> Self;
    fn truncate(&self, at_depth: usize) -> Self;
    fn at(&self, pos: usize) -> u8;
    fn length_at(&self, at_depth: usize) -> usize;
    fn to_partial(&self, at_depth: usize) -> Self::PartialType;
    fn matches_slice(&self, slice: &[u8]) -> bool;
}
