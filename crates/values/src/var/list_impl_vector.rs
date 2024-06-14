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
use std::ops::{Index, Range, RangeFrom, RangeFull, RangeTo};
use std::sync::Arc;

use crate::BincodeAsByteBufferExt;
use bincode::{Decode, Encode};

use crate::var::variant::Variant;
use crate::var::{v_empty_list, Var};

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ListImplVector {
    // TODO: Implement our own zero-copy list type and get rid of bincoding
    //   To support nested content, would require an offsets table at the front, etc.
    //   Take a look at how flatbufers, capnproto, and other zero-copy serialization formats do this.
    inner: Arc<Vec<Var>>,
}

impl ListImplVector {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn from_vec(vec: Vec<Var>) -> Self {
        Self {
            inner: Arc::new(vec),
        }
    }

    pub fn from_slice(vec: &[Var]) -> Self {
        Self {
            inner: Arc::new(vec.to_vec()),
        }
    }

    #[must_use]
    pub fn push(&mut self, v: Var) -> Self {
        // If there's only one copy of us, mutate that directly.
        match Arc::get_mut(&mut self.inner) {
            Some(vec) => {
                vec.push(v);
                self.clone()
            }
            None => {
                let mut new_vec = (*self.inner).clone();
                new_vec.push(v);
                Self::from_vec(new_vec)
            }
        }
    }

    /// Take the first item from the front, and return (item, `new_list`)
    #[must_use]
    pub fn pop_front(&self) -> (Var, Self) {
        if self.inner.is_empty() {
            return (v_empty_list(), Self::new());
        }
        let mut new_list = (*self.inner).clone();
        let item = new_list.remove(0);
        (item.clone(), Self::from_vec(new_list))
    }

    #[must_use]
    pub fn append(&mut self, other: Self) -> Self {
        match Arc::get_mut(&mut self.inner) {
            Some(vec) => {
                vec.extend_from_slice(&other.inner);
                self.clone()
            }
            None => {
                let mut new_list = (*self.inner).clone();
                new_list.extend_from_slice(&other.inner);
                Self::from_vec(new_list)
            }
        }
    }

    #[must_use]
    pub fn remove_at(&mut self, index: usize) -> Self {
        match Arc::get_mut(&mut self.inner) {
            Some(vec) => {
                vec.remove(index);
                self.clone()
            }
            None => {
                let mut new_list = (*self.inner).clone();
                new_list.remove(index);
                Self::from_vec(new_list)
            }
        }
    }

    /// Remove the first found instance of the given value from the list.
    #[must_use]
    pub fn setremove(&mut self, value: &Var) -> Self {
        if self.inner.is_empty() {
            return self.clone();
        }
        match Arc::get_mut(&mut self.inner) {
            Some(vec) => {
                for i in 0..vec.len() {
                    if vec[i].eq(value) {
                        vec.remove(i);
                        break;
                    }
                }
                self.clone()
            }
            None => {
                let mut new_list = Vec::with_capacity(self.inner.len() - 1);
                let mut found = false;
                for v in self.inner.iter() {
                    if !found && v.eq(value) {
                        found = true;
                        continue;
                    }
                    new_list.push(v.clone());
                }
                Self::from_vec(new_list)
            }
        }
    }

    #[must_use]
    pub fn insert(&mut self, index: isize, v: Var) -> Self {
        match Arc::get_mut(&mut self.inner) {
            Some(vec) => {
                let index = if index < 0 {
                    0
                } else {
                    min(index as usize, vec.len())
                };
                vec.insert(index, v);
                self.clone()
            }
            None => {
                let mut new_list = Vec::with_capacity(self.inner.len() + 1);
                let index = if index < 0 {
                    0
                } else {
                    min(index as usize, self.inner.len())
                };
                new_list.extend_from_slice(&self.inner[..index]);
                new_list.push(v);
                new_list.extend_from_slice(&self.inner[index..]);
                Self::from_vec(new_list)
            }
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    // "in" operator is case insensitive...
    #[must_use]
    pub fn contains(&self, v: &Var) -> bool {
        self.inner.contains(v)
    }

    // but bf_is_member is not... sigh.
    #[must_use]
    pub fn contains_case_sensitive(&self, v: &Var) -> bool {
        if let Variant::Str(s) = v.variant() {
            for item in self.inner.iter() {
                if let Variant::Str(s2) = item.variant() {
                    if s.as_str() == s2.as_str() {
                        return true;
                    }
                }
            }
            return false;
        }
        self.inner.contains(v)
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<Var> {
        self.inner.get(index).cloned()
    }

    #[must_use]
    pub fn set(&mut self, index: usize, value: Var) -> Self {
        match Arc::get_mut(&mut self.inner) {
            Some(vec) => {
                vec[index] = value;
                self.clone()
            }
            None => {
                let mut new_vec = (*self.inner).clone();
                new_vec[index] = value;
                Self::from_vec(new_vec)
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Var> {
        self.inner.iter()
    }
}

impl From<ListImplVector> for Vec<Var> {
    fn from(val: ListImplVector) -> Self {
        val.inner[..].to_vec()
    }
}

impl Default for ListImplVector {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<usize> for ListImplVector {
    type Output = Var;

    fn index(&self, index: usize) -> &Self::Output {
        &self.inner[index]
    }
}

impl Index<Range<usize>> for ListImplVector {
    type Output = [Var];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        &self.inner[index]
    }
}

impl Index<RangeFrom<usize>> for ListImplVector {
    type Output = [Var];

    fn index(&self, index: RangeFrom<usize>) -> &Self::Output {
        &self.inner[index]
    }
}

impl Index<RangeTo<usize>> for ListImplVector {
    type Output = [Var];

    fn index(&self, index: RangeTo<usize>) -> &Self::Output {
        &self.inner[index]
    }
}

impl Index<RangeFull> for ListImplVector {
    type Output = [Var];

    fn index(&self, index: RangeFull) -> &Self::Output {
        &self.inner[index]
    }
}

impl BincodeAsByteBufferExt for ListImplVector {}

#[cfg(test)]
mod tests {
    use crate::var::list_impl_vector::ListImplVector;
    use crate::var::{v_int, v_string};

    #[test]
    pub fn list_make_get() {
        let l = ListImplVector::new();
        assert_eq!(l.len(), 0);
        assert!(l.is_empty());
        // MOO is a bit weird here, it returns None for out of bounds.
        assert_eq!(l.get(0), None);

        let l = ListImplVector::from_slice(&[v_int(1)]);
        assert_eq!(l.len(), 1);
        assert!(!l.is_empty());
        assert_eq!(l.get(0), Some(v_int(1)));
        assert_eq!(l.get(1), None);

        let l = ListImplVector::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        assert_eq!(l.len(), 3);
        assert!(!l.is_empty());

        assert_eq!(l.get(0), Some(v_int(1)));
        assert_eq!(l.get(1), Some(v_int(2)));
        assert_eq!(l.get(2), Some(v_int(3)));
    }

    #[test]
    pub fn list_push() {
        let mut l = ListImplVector::new();
        let mut l = l.push(v_int(1));

        assert_eq!(l.len(), 1);
        let mut l = l.push(v_int(2));
        assert_eq!(l.len(), 2);
        let l = l.push(v_int(3));
        assert_eq!(l.len(), 3);

        assert_eq!(l.get(0), Some(v_int(1)));
        assert_eq!(l.get(2), Some(v_int(3)));
        assert_eq!(l.get(1), Some(v_int(2)));
    }

    #[test]
    fn list_pop_front() {
        let l = ListImplVector::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        let (item, l) = l.pop_front();
        assert_eq!(item, v_int(1));
        let (item, l) = l.pop_front();
        assert_eq!(item, v_int(2));
        let (item, l) = l.pop_front();
        assert_eq!(item, v_int(3));
        assert_eq!(l.len(), 0);
    }

    #[test]
    fn test_list_append() {
        let mut l1 = ListImplVector::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        let l2 = ListImplVector::from_slice(&[v_int(4), v_int(5), v_int(6)]);
        let l = l1.append(l2);
        assert_eq!(l.len(), 6);
        assert_eq!(l.get(0), Some(v_int(1)));
        assert_eq!(l.get(5), Some(v_int(6)));
    }

    #[test]
    fn test_list_remove() {
        let mut l = ListImplVector::from_slice(&[v_int(1), v_int(2), v_int(3)]);

        let l = l.remove_at(1);
        assert_eq!(l.len(), 2);
        assert_eq!(l.get(1), Some(v_int(3)));
        assert_eq!(l.get(0), Some(v_int(1)));
    }

    #[test]
    fn test_list_setremove() {
        let mut l = ListImplVector::from_slice(&[v_int(1), v_int(2), v_int(3), v_int(2)]);
        let l = l.setremove(&v_int(2));
        assert_eq!(l.len(), 3);
        assert_eq!(l.get(0), Some(v_int(1)));
        assert_eq!(l.get(1), Some(v_int(3)));
        assert_eq!(l.get(2), Some(v_int(2)));

        // setremove til empty
        let mut l = ListImplVector::from_slice(&[v_int(1)]);
        let l = l.setremove(&v_int(1));
        assert_eq!(l.len(), 0);
        assert_eq!(l.get(0), None);
    }

    #[test]
    fn test_list_insert() {
        let mut l = ListImplVector::new();
        let mut l = l.insert(0, v_int(4));
        assert_eq!(l.len(), 1);
        assert_eq!(l.get(0), Some(v_int(4)));

        let mut l = l.insert(0, v_int(3));
        assert_eq!(l.len(), 2);
        assert_eq!(l.get(0), Some(v_int(3)));
        assert_eq!(l.get(1), Some(v_int(4)));

        let l = l.insert(-1, v_int(5));
        assert_eq!(l.len(), 3);
        assert_eq!(l.get(0), Some(v_int(5)));
        assert_eq!(l.get(1), Some(v_int(3)));
        assert_eq!(l.get(2), Some(v_int(4)));
    }

    #[test]
    fn test_list_set() {
        let mut l = ListImplVector::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        let l = l.set(1, v_int(4));
        assert_eq!(l.len(), 3);
        assert_eq!(l.get(1), Some(v_int(4)));
    }

    #[test]
    fn test_list_contains_case_insenstive() {
        let l = ListImplVector::from_slice(&[v_string("foo".into()), v_string("bar".into())]);
        assert!(l.contains(&v_string("FOO".into())));
        assert!(l.contains(&v_string("BAR".into())));
    }

    #[test]
    fn test_list_contains_case_senstive() {
        let l = ListImplVector::from_slice(&[v_string("foo".into()), v_string("bar".into())]);
        assert!(!l.contains_case_sensitive(&v_string("FOO".into())));
        assert!(!l.contains_case_sensitive(&v_string("BAR".into())));
    }
}
