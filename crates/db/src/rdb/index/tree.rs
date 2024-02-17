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

use crate::rdb::index::node::{Content, Node};
use crate::rdb::index::tuple_key::TupleKey;
use crate::rdb::index::vector_partial::VectorPartial;
use crate::rdb::index::{KeyTrait, Partial};
use std::cmp::min;
use std::sync::Arc;

#[derive(Clone)]
pub struct AdaptiveRadixTree<ValueType: Clone> {
    root: Option<Node<VectorPartial, ValueType>>,
    _phantom: std::marker::PhantomData<TupleKey>,
}

impl<ValueType: Clone> AdaptiveRadixTree<ValueType> {
    pub fn new() -> Self {
        Self {
            root: None,
            _phantom: Default::default(),
        }
    }

    #[inline]
    pub fn get_k(&self, key: &TupleKey) -> Option<&ValueType> {
        Self::get_iterate(self.root.as_ref()?, key)
    }

    #[inline]
    pub fn get_k_mut(&mut self, key: &TupleKey) -> Option<&mut ValueType> {
        Self::get_iterate_mut(self.root.as_mut()?, key)
    }

    #[inline]
    pub fn insert_k(&mut self, key: &TupleKey, value: ValueType) -> Option<ValueType> {
        if self.root.is_none() {
            self.root = Some(Node::new_leaf(key.to_partial(0), value));
            return None;
        };

        let root = self.root.as_mut().unwrap();

        Self::insert_recurse(root, key, value, 0)
    }

    pub fn remove_k(&mut self, key: &TupleKey) -> Option<ValueType> {
        let root = self.root.as_mut()?;

        // Don't bother doing anything if there's no prefix match on the root at all.
        let prefix_common_match = root.prefix.prefix_length_key(key, 0);
        if prefix_common_match != root.prefix.len() {
            return None;
        }

        // Special case, if the root is a leaf and matches the key, we can just remove it
        // immediately. If it doesn't match our key, then we have nothing to do here anyways.
        if root.is_leaf() {
            // Move the value of the leaf in root. To do this, replace self.root  with None and
            // then unwrap the value out of the Option & Leaf.
            let stolen = self.root.take().unwrap();
            let leaf = match stolen.content.as_ref() {
                Content::Leaf(v) => v,
                _ => unreachable!(),
            };
            return Some(leaf.clone());
        }

        let result = Self::remove_recurse(root, key, prefix_common_match);

        // Prune root out if it's now empty.
        if root.is_inner() && root.num_children() == 0 {
            self.root = None;
        }
        result
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }
}

impl<ValueType: Clone> AdaptiveRadixTree<ValueType> {
    fn get_iterate<'a>(
        cur_node: &'a Node<VectorPartial, ValueType>,
        key: &TupleKey,
    ) -> Option<&'a ValueType> {
        let mut cur_node = cur_node;
        let mut depth = 0;
        loop {
            let prefix_common_match = cur_node.prefix.prefix_length_key(key, depth);
            if prefix_common_match != cur_node.prefix.len() {
                return None;
            }

            if cur_node.prefix.len() == key.length_at(depth) {
                return cur_node.value();
            }
            let k = key.at(depth + cur_node.prefix.len());
            depth += cur_node.prefix.len();
            cur_node = cur_node.seek_child(k)?
        }
    }

    fn get_iterate_mut<'a>(
        cur_node: &'a mut Node<VectorPartial, ValueType>,
        key: &TupleKey,
    ) -> Option<&'a mut ValueType> {
        let mut cur_node = cur_node;
        let mut depth = 0;
        loop {
            let prefix_common_match = cur_node.prefix.prefix_length_key(key, depth);
            if prefix_common_match != cur_node.prefix.len() {
                return None;
            }

            if cur_node.prefix.len() == key.length_at(depth) {
                return cur_node.value_mut();
            }

            let k = key.at(depth + cur_node.prefix.len());
            depth += cur_node.prefix.len();
            cur_node = cur_node.seek_child_mut(k)?;
        }
    }

    fn insert_recurse(
        cur_node: &mut Node<VectorPartial, ValueType>,
        key: &TupleKey,
        value: ValueType,
        depth: usize,
    ) -> Option<ValueType> {
        let longest_common_prefix = cur_node.prefix.prefix_length_key(key, depth);

        let is_prefix_match =
            min(cur_node.prefix.len(), key.length_at(depth)) == longest_common_prefix;

        // Prefix fully covers this node.
        // Either sets the value or replaces the old value already here.
        if is_prefix_match && cur_node.prefix.len() == key.length_at(depth) {
            let content_mut = Arc::make_mut(&mut cur_node.content);
            if let Content::Leaf(ref mut v) = content_mut {
                return Some(std::mem::replace(v, value));
            } else {
                panic!("Node type mismatch")
            }
        }

        // Prefix is part of the current node, but doesn't fully cover it.
        // We have to break this node up, creating a new parent node, and a sibling for our leaf.
        if !is_prefix_match {
            let new_prefix = cur_node.prefix.partial_after(longest_common_prefix);
            let old_node_prefix = std::mem::replace(&mut cur_node.prefix, new_prefix);

            // We will replace this leaf node with a new inner node. The new value will join the
            // current node as sibling, both a child of the new node.
            let n4 = Node::new_inner(old_node_prefix.partial_before(longest_common_prefix));

            let k1 = old_node_prefix.at(longest_common_prefix);
            let k2 = key.at(depth + longest_common_prefix);

            let replacement_current = std::mem::replace(cur_node, n4);

            // We've deferred creating the leaf til now so that we can take ownership over the
            // key after other things are done peering at it.
            let new_leaf = Node::new_leaf(key.to_partial(depth + longest_common_prefix), value);

            // Add the old leaf node as a child of the new inner node.
            cur_node.add_child(k1, replacement_current);
            cur_node.add_child(k2, new_leaf);

            return None;
        }

        // We must be an inner node, and either we need a new baby, or one of our children does, so
        // we'll hunt and see.
        let k = key.at(depth + longest_common_prefix);

        let child_for_key = cur_node.seek_child_mut(k);
        if let Some(child) = child_for_key {
            return AdaptiveRadixTree::insert_recurse(
                child,
                key,
                value,
                depth + longest_common_prefix,
            );
        };

        // We should not be a leaf at this point. If so, something bad has happened.
        assert!(cur_node.is_inner());
        let new_leaf = Node::new_leaf(key.to_partial(depth + longest_common_prefix), value);
        cur_node.add_child(k, new_leaf);
        None
    }

    fn remove_recurse(
        parent_node: &mut Node<VectorPartial, ValueType>,
        key: &TupleKey,
        depth: usize,
    ) -> Option<ValueType> {
        // Seek the child that matches the key at this depth, which is the first character at the
        // depth we're at.
        let c = key.at(depth);
        let child_node = parent_node.seek_child_mut(c)?;

        let prefix_common_match = child_node.prefix.prefix_length_key(key, depth);
        if prefix_common_match != child_node.prefix.len() {
            return None;
        }

        // If the child is a leaf, and the prefix matches the key, we can remove it from this parent
        // node. If the prefix does not match, then we have nothing to do here.
        if child_node.is_leaf() {
            if child_node.prefix.len() != (key.length_at(depth)) {
                return None;
            }
            let node = parent_node.delete_child(c).unwrap();
            let v = match node.content.as_ref() {
                Content::Leaf(v) => v,
                _ => unreachable!(),
            };
            return Some(v.clone());
        }

        // Otherwise, recurse down the branch in that direction.
        let result =
            AdaptiveRadixTree::remove_recurse(child_node, key, depth + child_node.prefix.len());

        // If after this our child we just recursed into no longer has children of its own, it can
        // be collapsed into us. In this way we can prune the tree as we go.
        if result.is_some() && child_node.is_inner() && child_node.num_children() == 0 {
            let prefix = child_node.prefix.clone();
            let deleted = parent_node.delete_child(c).unwrap();
            assert_eq!(prefix.to_slice(), deleted.prefix.to_slice());
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use crate::rdb::index::tuple_key::TupleKey;
    use moor_values::util::SliceRef;
    use rand::Rng;

    /// Verify value is inserted and retrieved correctly, and that forked (cloned) copies behave correctly.
    #[test]
    fn simple_insert_get() {
        let key = TupleKey::new(SliceRef::from_vec(vec![1, 2, 3]));
        let mut tree = super::AdaptiveRadixTree::new();
        assert!(tree.is_empty());
        tree.insert_k(&key, 1);
        assert!(!tree.is_empty());
        assert_eq!(tree.get_k(&key), Some(&1));
        let second_key = TupleKey::new(SliceRef::from_vec(vec![1, 2, 4]));
        assert_eq!(tree.get_k(&second_key), None);

        let clone_before_mutate = tree.clone();
        tree.insert_k(&second_key, 2);
        assert_eq!(tree.get_k(&second_key), Some(&2));
        assert_eq!(tree.get_k(&key), Some(&1));
        assert_eq!(clone_before_mutate.get_k(&second_key), None);
        assert_eq!(clone_before_mutate.get_k(&key), Some(&1));
    }

    /// Use get_k_mut to modify an inserted value, and verify that the value is modified, and that cloned copies are
    /// not modified.
    #[test]
    fn insert_get_mut_modify() {
        let key = TupleKey::new(SliceRef::from_vec(vec![1, 2, 3]));
        let mut tree = super::AdaptiveRadixTree::new();
        tree.insert_k(&key, 1);
        assert_eq!(tree.get_k(&key), Some(&1));

        let clone_before_mutate = tree.clone();
        let value = tree.get_k_mut(&key).unwrap();
        *value = 2;

        assert_eq!(tree.get_k(&key), Some(&2));
        assert_eq!(clone_before_mutate.get_k(&key), Some(&1));
    }

    /// Test key removal, and verify that the removed value is returned, and that clones taken pre-removal are not
    /// affected.
    #[test]
    fn insert_get_remove() {
        let key = TupleKey::new(SliceRef::from_vec(vec![1, 2, 3]));
        let mut tree = super::AdaptiveRadixTree::new();
        tree.insert_k(&key, 1);
        assert_eq!(tree.get_k(&key), Some(&1));

        let clone_before_remove = tree.clone();
        assert_eq!(tree.remove_k(&key), Some(1));
        assert_eq!(tree.get_k(&key), None);
        assert_eq!(clone_before_remove.get_k(&key), Some(&1));
    }

    fn random_key_pair() -> (TupleKey, u32) {
        let mut rng = rand::thread_rng();
        let len = rng.gen_range(1..256);
        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            v.push(rng.gen());
        }
        (TupleKey::new(SliceRef::from_vec(v)), rng.gen())
    }

    #[test]
    /// Insert a lot of random keys and verify that they are all retrievable. Then clone the tree and verify that the
    /// clone is also able to retrieve all the keys. Then insert more into the original tree and verify that the clone
    /// is not affected.    
    fn bulk_data_insert_retrieval() {
        let many_keys = (0..1000).map(|_| random_key_pair()).collect::<Vec<_>>();
        let mut tree = super::AdaptiveRadixTree::new();
        for (key, value) in &many_keys {
            tree.insert_k(key, *value);
        }
        for (key, value) in &many_keys {
            assert_eq!(tree.get_k(key), Some(value));
        }

        let clone_before_mutate = tree.clone();
        for key in &many_keys {
            assert_eq!(clone_before_mutate.get_k(&key.0), Some(&key.1));
        }

        for (k, _) in &many_keys {
            tree.remove_k(k);
        }
        for (k, _) in &many_keys {
            assert_eq!(tree.get_k(k), None);
        }
        for (k, v) in &many_keys {
            assert_eq!(clone_before_mutate.get_k(k), Some(v));
        }

        // Now pile in some more keys and verify their insertion and that the clone is not affected.
        let more_keys = (0..1000).map(|_| random_key_pair()).collect::<Vec<_>>();
        for (key, value) in &more_keys {
            tree.insert_k(key, *value);
        }
        for (key, value) in &more_keys {
            assert_eq!(tree.get_k(key), Some(value));
        }
        for (key, value) in &many_keys {
            assert_eq!(clone_before_mutate.get_k(key), Some(value));
        }
    }
}
