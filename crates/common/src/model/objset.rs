// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::model::ValSet;
use byteview::ByteView;
use itertools::Itertools;
use lazy_static::lazy_static;
use moor_var::AsByteBuffer;
use moor_var::Obj;
use moor_var::encode::{DecodingError, EncodingError};
use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter};
// TODO: this won't work for non-objid objects

lazy_static! {
    static ref EMPTY_OBJSET: ObjSet = ObjSet(ByteView::new(&[]));
}

/// When we want to refer to a set of object ids, use this type.
/// Note that equality is defined as "same bytes" buffer for efficiency reasons.
#[derive(Eq, PartialEq)]
pub struct ObjSet(ByteView);

impl Clone for ObjSet {
    fn clone(&self) -> Self {
        ObjSet(self.0.to_detached().clone())
    }
}

impl AsByteBuffer for ObjSet {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(self.0.as_ref()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.as_ref().to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        // TODO: Validate object ids on decode of ObjSet
        Ok(Self(bytes))
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        Ok(self.0.clone())
    }
}

impl Display for ObjSet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("{")?;
        f.write_str(self.iter().map(|o| o.to_literal()).join(", ").as_str())?;
        f.write_str("}")
    }
}

impl Debug for ObjSet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("ObjSet(len={} bytes={}) {{", self.len(), self.0.len()).as_str())?;
        f.write_str(self.iter().map(|o| o.to_literal()).join(", ").as_str())?;
        f.write_str("}")
    }
}

pub struct ObjSetIter {
    position: usize,
    buffer: ByteView,
}

impl Iterator for ObjSetIter {
    type Item = Obj;

    fn next(&mut self) -> Option<Self::Item> {
        if self.buffer.is_empty() {
            return None;
        }
        if self.position >= self.buffer.len() {
            return None;
        }

        let bytes = &self
            .buffer
            .slice(self.position..self.position + size_of::<Obj>());
        let obj = Obj::from_bytes(bytes.to_detached()).unwrap();
        self.position += size_of::<Obj>();
        Some(obj)
    }
}

impl FromIterator<Obj> for ObjSet {
    fn from_iter<T: IntoIterator<Item = Obj>>(iter: T) -> Self {
        let mut v = Vec::with_capacity(size_of::<Obj>());
        let mut total = 0usize;
        for item in iter {
            v.extend_from_slice(&item.as_bytes().unwrap());
            total += 1;
        }
        // If after that, total is 0, don't even bother, just throw away the buffer.
        // We want to maintain the invariant that an empty ObjSet is a 0-buf sized thing.
        if total == 0 {
            return EMPTY_OBJSET.clone();
        }
        Self(ByteView::from(v))
    }
}

impl ObjSet {
    #[must_use]
    pub fn with_inserted(&self, oid: Obj) -> Self {
        if self.0.is_empty() {
            return Self::from_items(&[oid]);
        }
        // Note, we're stupid and don't check for dupes. It's called a 'set' but it ain't.
        let _capacity = self.len();
        let mut new_buf = self.0.as_ref().to_vec();
        new_buf.extend_from_slice(&oid.as_bytes().unwrap());
        Self(ByteView::from(new_buf))
    }
    #[must_use]
    pub fn with_removed(&self, oid: Obj) -> Self {
        if self.0.is_empty() {
            return EMPTY_OBJSET.clone();
        }
        let mut new_buf = Vec::with_capacity(self.0.len() - size_of::<Obj>());
        let mut found = false;
        for i in self.iter() {
            if i == oid {
                found = true;
                continue;
            }
            let oid_bytes = i.as_bytes().unwrap();
            new_buf.extend_from_slice(&oid_bytes);
        }
        if !found {
            return self.clone();
        }
        Self(ByteView::from(new_buf))
    }
    #[must_use]
    pub fn with_all_removed(&self, oids: &[Obj]) -> Self {
        if self.0.is_empty() {
            return EMPTY_OBJSET.clone();
        }
        let mut new_buf = Vec::with_capacity(self.0.len());
        let mut found = false;
        for i in self.iter() {
            if oids.contains(&i) {
                found = true;
                continue;
            }
            new_buf.extend_from_slice(&i.as_bytes().unwrap());
        }
        if !found {
            return self.clone();
        }
        Self(ByteView::from(new_buf))
    }
    #[must_use]
    pub fn contains(&self, oid: Obj) -> bool {
        // O(N) operation. Which we're fine with, really. We're a vector.
        self.iter().any(|o| o == oid)
    }

    /// Set equality comparison, because Eq/PartialEq for this type is "same bytes", this is actual
    /// logical equality, but less efficient.
    #[must_use]
    pub fn is_same(&self, other: Self) -> bool {
        self.iter().collect::<HashSet<_>>() == other.iter().collect::<HashSet<_>>()
    }

    #[must_use]
    pub fn with_concatenated(&self, other: Self) -> Self {
        if self.0.is_empty() {
            return other;
        }
        let new_len = other.len() + self.len();
        let mut new_buf = Vec::with_capacity(size_of::<Obj>() * new_len);
        new_buf.extend_from_slice(self.0.as_ref());
        new_buf.extend_from_slice(other.0.as_ref());
        Self(ByteView::from(new_buf))
    }

    #[must_use]
    pub fn with_appended(&self, values: &[Obj]) -> Self {
        if self.0.is_empty() {
            return Self::from_items(values);
        }
        let new_len = self.len() + values.len();
        let mut new_buf = Vec::with_capacity(size_of::<u32>() + (size_of::<Obj>() * new_len));
        new_buf.extend_from_slice(self.0.as_ref());
        for i in values {
            new_buf.extend_from_slice(&i.as_bytes().unwrap());
        }
        Self(ByteView::from(new_buf))
    }
}

impl ValSet<Obj> for ObjSet {
    fn empty() -> Self {
        EMPTY_OBJSET.clone()
    }

    fn from_items(oids: &[Obj]) -> Self {
        if oids.is_empty() {
            return EMPTY_OBJSET.clone();
        }
        let mut v = Vec::with_capacity(std::mem::size_of_val(oids));
        for i in oids {
            v.extend_from_slice(&i.as_bytes().unwrap());
        }
        Self(ByteView::from(v))
    }
    fn iter(&self) -> impl Iterator<Item = Obj> {
        ObjSetIter {
            position: 0,
            buffer: self.0.clone(),
        }
    }

    fn len(&self) -> usize {
        if self.0.is_empty() {
            return 0;
        }
        self.0.len() / size_of::<Obj>()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Default for ObjSet {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::AsByteBuffer;
    use std::collections::HashSet;

    #[test]
    fn test_objset_empty() {
        let objset = ObjSet::empty();
        assert!(objset.is_empty());
        assert_eq!(objset.len(), 0);
        assert_eq!(objset.as_bytes().unwrap().len(), 0);
    }

    #[test]
    fn test_objset_from_items() {
        let objset = ObjSet::from_items(&[Obj::mk_id(1), Obj::mk_id(2), Obj::mk_id(3)]);
        assert!(!objset.is_empty());
        assert_eq!(objset.len(), 3);
    }

    #[test]
    fn test_objset_iter() {
        let objset = ObjSet::from_items(&[Obj::mk_id(1), Obj::mk_id(2), Obj::mk_id(3)]);
        let mut iter = objset.iter();
        assert_eq!(iter.next().unwrap(), Obj::mk_id(1));
        assert_eq!(iter.next().unwrap(), Obj::mk_id(2));
        assert_eq!(iter.next().unwrap(), Obj::mk_id(3));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_objset_with_inserted() {
        let objset = ObjSet::from_items(&[Obj::mk_id(1), Obj::mk_id(2), Obj::mk_id(3)]);
        let objset = objset.with_inserted(Obj::mk_id(4));
        assert_eq!(objset.len(), 4);
        assert_eq!(
            objset.iter().collect::<HashSet<_>>(),
            [Obj::mk_id(1), Obj::mk_id(2), Obj::mk_id(3), Obj::mk_id(4)]
                .iter()
                .cloned()
                .collect()
        );
    }

    #[test]
    fn test_objset_with_removed() {
        let objset = ObjSet::from_items(&[Obj::mk_id(1), Obj::mk_id(2), Obj::mk_id(3)]);
        assert_eq!(objset.len(), 3);
        assert_eq!(
            objset.iter().collect::<HashSet<_>>(),
            [Obj::mk_id(1), Obj::mk_id(2), Obj::mk_id(3),]
                .iter()
                .cloned()
                .collect()
        );
        let objset = objset.with_removed(Obj::mk_id(2));
        assert_eq!(objset.len(), 2);
        assert_eq!(
            objset.iter().collect::<HashSet<_>>(),
            [Obj::mk_id(1), Obj::mk_id(3)].iter().cloned().collect()
        );
    }
}
