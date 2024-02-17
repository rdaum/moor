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

use std::sync::Arc;

use moor_values::util::Bitset64;

use crate::rdb::index::direct_mapping::DirectMapping;
use crate::rdb::index::indexed_mapping::IndexedMapping;
use crate::rdb::index::keyed_mapping::KeyedMapping;
use crate::rdb::index::NodeMapping;

use super::Partial;

#[derive(Clone)]
pub struct Node<P: Partial + Clone, V: Clone> {
    pub(crate) prefix: P,
    pub(crate) content: Arc<Content<P, V>>,
}

#[allow(dead_code)]
pub(crate) enum Content<P: Partial + Clone, V: Clone> {
    Leaf(V),
    Node4(KeyedMapping<Node<P, V>, 4>),
    Node16(KeyedMapping<Node<P, V>, 16>),
    Node48(IndexedMapping<Node<P, V>, 48, Bitset64<1>>),
    Node256(DirectMapping<Node<P, V>>),
}

#[allow(dead_code)]
impl<P: Partial + Clone, V: Clone> Node<P, V> {
    #[inline]
    pub(crate) fn new_leaf(partial: P, value: V) -> Self {
        Self {
            prefix: partial,
            content: Arc::new(Content::Leaf(value)),
        }
    }

    #[inline]
    pub(crate) fn new_inner(prefix: P) -> Self {
        let nt = Content::Node4(KeyedMapping::new());
        Self {
            prefix,
            content: Arc::new(nt),
        }
    }

    pub(crate) fn value(&self) -> Option<&V> {
        let Content::Leaf(value) = &self.content.as_ref() else {
            return None;
        };
        Some(value)
    }

    pub(crate) fn value_mut(&mut self) -> Option<&mut V> {
        let mut_contents = Arc::make_mut(&mut self.content);
        match mut_contents {
            Content::Leaf(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn is_leaf(&self) -> bool {
        matches!(&self.content.as_ref(), Content::Leaf(_))
    }

    pub(crate) fn is_inner(&self) -> bool {
        !self.is_leaf()
    }

    pub(crate) fn seek_child(&self, key: u8) -> Option<&Self> {
        if self.num_children() == 0 {
            return None;
        }

        match &self.content.as_ref() {
            Content::Node4(km) => km.seek_child(key),
            Content::Node16(km) => km.seek_child(key),
            Content::Node48(km) => km.seek_child(key),
            Content::Node256(children) => children.seek_child(key),
            Content::Leaf(_) => None,
        }
    }

    pub(crate) fn seek_child_mut(&mut self, key: u8) -> Option<&mut Self> {
        if self.num_children() == 0 {
            return None;
        }

        let mut_contents = Arc::make_mut(&mut self.content);
        match mut_contents {
            Content::Node4(km) => km.seek_child_mut(key),
            Content::Node16(km) => km.seek_child_mut(key),
            Content::Node48(km) => km.seek_child_mut(key),
            Content::Node256(children) => children.seek_child_mut(key),
            Content::Leaf(_) => None,
        }
    }

    pub(crate) fn add_child(&mut self, key: u8, node: Self) {
        if self.is_full() {
            self.grow();
        }

        let mut contents = Arc::make_mut(&mut self.content);
        match &mut contents {
            Content::Node4(ref mut km) => {
                km.add_child(key, node);
            }
            Content::Node16(ref mut km) => {
                km.add_child(key, node);
            }
            Content::Node48(ref mut im) => {
                im.add_child(key, node);
            }
            Content::Node256(ref mut pm) => {
                pm.add_child(key, node);
            }
            Content::Leaf(_) => unreachable!("Should not be possible."),
        }
    }

    pub(crate) fn delete_child(&mut self, key: u8) -> Option<Self> {
        let mut contents = Arc::make_mut(&mut self.content);
        match &mut contents {
            Content::Node4(ref mut dm) => {
                let node = dm.delete_child(key);

                if self.num_children() == 1 {
                    self.shrink();
                }

                node
            }
            Content::Node16(ref mut dm) => {
                let node = dm.delete_child(key);

                if self.num_children() < 5 {
                    self.shrink();
                }
                node
            }
            Content::Node48(ref mut im) => {
                let node = im.delete_child(key);

                if self.num_children() < 17 {
                    self.shrink();
                }
                node
            }
            Content::Node256(ref mut pm) => {
                let node = pm.delete_child(key);
                if self.num_children() < 49 {
                    self.shrink();
                }
                node
            }
            Content::Leaf(_) => unreachable!("Should not be possible."),
        }
    }

    fn capacity(&self) -> usize {
        match self.content.as_ref() {
            Content::Node4 { .. } => 4,
            Content::Node16 { .. } => 16,
            Content::Node48 { .. } => 48,
            Content::Node256 { .. } => 256,
            Content::Leaf(_) => 0,
        }
    }

    pub(crate) fn num_children(&self) -> usize {
        match self.content.as_ref() {
            Content::Node4(n) => n.num_children(),
            Content::Node16(n) => n.num_children(),
            Content::Node48(n) => n.num_children(),
            Content::Node256(n) => n.num_children(),
            Content::Leaf(_) => 0,
        }
    }
}

impl<P: Partial + Clone, V: Clone> Node<P, V> {
    #[inline]
    #[allow(dead_code)]
    pub fn new_4(prefix: P) -> Self {
        let content = Arc::new(Content::Node4(KeyedMapping::new()));
        Self { prefix, content }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn new_16(prefix: P) -> Self {
        let content = Arc::new(Content::Node16(KeyedMapping::new()));
        Self { prefix, content }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn new_48(prefix: P) -> Self {
        let content = Arc::new(Content::Node48(IndexedMapping::new()));
        Self { prefix, content }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn new_256(prefix: P) -> Self {
        let content = Arc::new(Content::Node256(DirectMapping::new()));
        Self { prefix, content }
    }

    #[inline]
    fn is_full(&self) -> bool {
        match &self.content.as_ref() {
            Content::Node4(km) => self.num_children() >= km.width(),
            Content::Node16(km) => self.num_children() >= km.width(),
            Content::Node48(im) => self.num_children() >= im.width(),
            // Should not be possible.
            Content::Node256(_) => self.num_children() >= 256,
            Content::Leaf(_) => unreachable!("Should not be possible."),
        }
    }

    fn shrink(&mut self) {
        let mut content = Arc::make_mut(&mut self.content);
        match &mut content {
            Content::Node4(ref mut km) => {
                // A node4 with only one child has its childed collapsed into it.
                // If our child is a leaf, that means we have become a leaf, and we can shrink no
                // more beyond this.
                let (_, child) = km.take_value_for_leaf();
                let prefix = child.prefix;
                self.content = child.content;
                self.prefix = self.prefix.partial_extended_with(&prefix);
            }
            Content::Node16(ref mut km) => {
                self.content = Arc::new(Content::Node4(KeyedMapping::from_resized(km)));
            }
            Content::Node48(ref mut im) => {
                let new_node = Content::Node16(KeyedMapping::from_indexed(im));
                self.content = Arc::new(new_node);
            }
            Content::Node256(ref mut dm) => {
                self.content = Arc::new(Content::Node48(IndexedMapping::from_direct(dm)));
            }
            Content::Leaf(_) => unreachable!("Should not be possible."),
        }
    }

    fn grow(&mut self) {
        let mut content = Arc::make_mut(&mut self.content);
        match &mut content {
            Content::Node4(km) => {
                self.content = Arc::new(Content::Node16(KeyedMapping::from_resized(km)));
            }
            Content::Node16(km) => {
                self.content = Arc::new(Content::Node48(IndexedMapping::from_sorted_keyed(km)));
            }
            Content::Node48(im) => {
                self.content = Arc::new(Content::Node256(DirectMapping::from_indexed(im)));
            }
            Content::Node256 { .. } => {
                unreachable!("Should never grow a node256")
            }
            Content::Leaf(_) => unreachable!("Should not be possible."),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn free(&self) -> usize {
        self.capacity() - self.num_children()
    }

    #[allow(dead_code)] // for now until iteration is finished
    pub fn iter(&self) -> Box<dyn Iterator<Item = (u8, &Self)> + '_> {
        return match self.content.as_ref() {
            Content::Node4(n) => Box::new(n.iter()),
            Content::Node16(n) => Box::new(n.iter()),
            Content::Node48(n) => Box::new(n.iter()),
            Content::Node256(n) => Box::new(n.iter()),
            Content::Leaf(_) => Box::new(std::iter::empty()),
        };
    }
}

impl<P: Partial + Clone, V: Clone> Clone for Content<P, V> {
    fn clone(&self) -> Self {
        match self {
            Content::Leaf(v) => Content::Leaf(v.clone()),
            Content::Node4(n) => Content::Node4(n.clone()),
            Content::Node16(n) => Content::Node16(n.clone()),
            Content::Node48(n) => Content::Node48(n.clone()),
            Content::Node256(n) => Content::Node256(n.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rdb::index::node::Node;
    use crate::rdb::index::vector_partial::VectorPartial;

    #[test]
    fn test_n4() {
        let test_key: VectorPartial = VectorPartial::key("abc".as_bytes());

        let mut n4 = Node::new_4(test_key.clone());
        n4.add_child(5, Node::new_leaf(test_key.clone(), 1));
        n4.add_child(4, Node::new_leaf(test_key.clone(), 2));
        let n4_pre = n4.clone();
        n4.add_child(3, Node::new_leaf(test_key.clone(), 3));
        n4.add_child(2, Node::new_leaf(test_key.clone(), 4));

        assert_eq!(*n4.seek_child(5).unwrap().value().unwrap(), 1);
        assert_eq!(*n4.seek_child(4).unwrap().value().unwrap(), 2);
        assert_eq!(*n4.seek_child(3).unwrap().value().unwrap(), 3);
        assert_eq!(*n4.seek_child(2).unwrap().value().unwrap(), 4);

        assert!(n4_pre.seek_child(3).is_none());
        assert!(n4_pre.seek_child(2).is_none());

        n4.delete_child(5);
        assert!(n4.seek_child(5).is_none());
        assert_eq!(*n4.seek_child(4).unwrap().value().unwrap(), 2);
        assert_eq!(*n4.seek_child(3).unwrap().value().unwrap(), 3);
        assert_eq!(*n4.seek_child(2).unwrap().value().unwrap(), 4);

        n4.delete_child(2);
        assert!(n4.seek_child(5).is_none());
        assert!(n4.seek_child(2).is_none());

        n4.add_child(2, Node::new_leaf(test_key, 4));
        n4.delete_child(3);
        assert!(n4.seek_child(5).is_none());
        assert!(n4.seek_child(3).is_none());
    }

    #[test]
    fn test_n16() {
        let test_key: VectorPartial = VectorPartial::key("abc".as_bytes());

        let mut n16 = Node::new_16(test_key.clone());

        // Fill up the node with keys in reverse order.
        for i in (0..16).rev() {
            n16.add_child(i, Node::new_leaf(test_key.clone(), i));
        }

        for i in 0..16 {
            assert_eq!(*n16.seek_child(i).unwrap().value().unwrap(), i);
        }

        // Delete from end doesn't affect position of others.
        n16.delete_child(15);
        n16.delete_child(14);
        assert!(n16.seek_child(15).is_none());
        assert!(n16.seek_child(14).is_none());
        for i in 0..14 {
            assert_eq!(*n16.seek_child(i).unwrap().value().unwrap(), i);
        }

        n16.delete_child(0);
        n16.delete_child(1);
        assert!(n16.seek_child(0).is_none());
        assert!(n16.seek_child(1).is_none());
        for i in 2..14 {
            assert_eq!(*n16.seek_child(i).unwrap().value().unwrap(), i);
        }

        // Delete from the middle
        n16.delete_child(5);
        n16.delete_child(6);
        assert!(n16.seek_child(5).is_none());
        assert!(n16.seek_child(6).is_none());
        for i in 2..5 {
            assert_eq!(*n16.seek_child(i).unwrap().value().unwrap(), i);
        }
        for i in 7..14 {
            assert_eq!(*n16.seek_child(i).unwrap().value().unwrap(), i);
        }
    }

    #[test]
    fn test_n48() {
        let test_key: VectorPartial = VectorPartial::key("abc".as_bytes());

        let mut n48 = Node::new_48(test_key.clone());

        // indexes in n48 have no sort order, so we don't look at that
        for i in 0..48 {
            n48.add_child(i, Node::new_leaf(test_key.clone(), i));
        }

        for i in 0..48 {
            assert_eq!(*n48.seek_child(i).unwrap().value().unwrap(), i);
        }

        n48.delete_child(47);
        n48.delete_child(46);
        assert!(n48.seek_child(47).is_none());
        assert!(n48.seek_child(46).is_none());
        for i in 0..46 {
            assert_eq!(*n48.seek_child(i).unwrap().value().unwrap(), i);
        }
    }

    #[test]
    fn test_n_256() {
        let test_key: VectorPartial = VectorPartial::key("abc".as_bytes());

        let mut n256 = Node::new_256(test_key.clone());

        for i in 0..=255 {
            n256.add_child(i, Node::new_leaf(test_key.clone(), i));
        }
        for i in 0..=255 {
            assert_eq!(*n256.seek_child(i).unwrap().value().unwrap(), i);
        }

        n256.delete_child(47);
        n256.delete_child(46);
        assert!(n256.seek_child(47).is_none());
        assert!(n256.seek_child(46).is_none());
        for i in 0..46 {
            assert_eq!(*n256.seek_child(i).unwrap().value().unwrap(), i);
        }
        for i in 48..=255 {
            assert_eq!(*n256.seek_child(i).unwrap().value().unwrap(), i);
        }
    }
}
