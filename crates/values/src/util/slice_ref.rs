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

use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::RangeBounds;
use std::sync::Arc;
use yoke::Yoke;

/// A reference to a buffer, along with a reference counted reference to the backing storage it came
/// from, and a range within that storage.
/// In this way it's possible to safely and conveniently pass around the 'slices' of things without
/// worrying about lifetimes and borrowing.
/// This is used here for the pieces of the rope, which can all be slices out of common buffer
/// storage, and we can avoid making copies of the data when doing things like splitting nodes
/// or appending to the rope etc.
#[derive(Clone)]
pub struct SliceRef(Yoke<&'static [u8], Arc<dyn ByteSource>>);

impl Debug for SliceRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SliceRef(len: {}/store: {})",
            self.len(),
            self.0.get().len()
        )
    }
}
impl PartialEq for SliceRef {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for SliceRef {}

impl Display for SliceRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(self.as_slice()))
    }
}

impl Hash for SliceRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state)
    }
}
pub trait ByteSource: Send + Sync {
    fn as_slice(&self) -> &[u8];
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn touch(&self);
}

struct VectorByteSource(Vec<u8>);
impl ByteSource for VectorByteSource {
    fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
    fn len(&self) -> usize {
        self.0.len()
    }

    fn touch(&self) {}
}

struct EmptyByteSource;
impl ByteSource for EmptyByteSource {
    fn as_slice(&self) -> &[u8] {
        &[]
    }
    fn len(&self) -> usize {
        0
    }
    fn touch(&self) {}
}

impl SliceRef {
    #[must_use]
    pub fn empty() -> Self {
        Self(Yoke::attach_to_cart(Arc::new(EmptyByteSource {}), |b| {
            b.as_slice()
        }))
    }
    #[must_use]
    pub fn from_byte_source(byte_source: impl ByteSource + 'static) -> Self {
        Self(Yoke::attach_to_cart(Arc::new(byte_source), |b| {
            b.as_slice()
        }))
    }

    #[must_use]
    pub fn from_bytes(buf: &[u8]) -> Self {
        Self(Yoke::attach_to_cart(
            Arc::new(VectorByteSource(buf.to_vec())),
            |b| b.as_slice(),
        ))
    }
    #[must_use]
    pub fn from_vec(buf: Vec<u8>) -> Self {
        Self(Yoke::attach_to_cart(Arc::new(VectorByteSource(buf)), |b| {
            b.as_slice()
        }))
    }
    #[must_use]
    pub fn split_at(&self, offset: usize) -> (Self, Self) {
        self.0.backing_cart().touch();
        let left = Self(self.0.map_project_cloned(|sl, _| &sl[..offset]));
        let right = Self(self.0.map_project_cloned(|sl, _| &sl[offset..]));
        (left, right)
    }
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        self.0.backing_cart().touch();
        self.0.get()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.backing_cart().touch();
        self.0.get().len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.backing_cart().touch();
        self.0.get().is_empty()
    }
    #[must_use]
    pub fn derive_empty(&self) -> Self {
        self.0.backing_cart().touch();
        Self(Yoke::attach_to_cart(self.0.backing_cart().clone(), |_b| {
            &[] as &[u8]
        }))
    }

    pub fn slice<'a, R>(&'a self, range: R) -> Self
    where
        R: RangeBounds<usize> + 'a + std::slice::SliceIndex<[u8], Output = [u8]>,
    {
        self.0.backing_cart().touch();
        let result = self.0.map_project_cloned(move |sl, _| &sl[range]);
        Self(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::util::slice_ref::SliceRef;

    #[test]
    fn test_buffer_ref_split() {
        let backing_buffer = b"Hello, World!";
        let buf = SliceRef::from_bytes(&backing_buffer[..]);
        let (left, right) = buf.split_at(5);
        assert_eq!(left.as_slice(), b"Hello");
        assert_eq!(right.as_slice(), b", World!");
    }

    #[test]
    fn test_buffer_ref_slice() {
        let backing_buffer = b"Hello, World!";
        let buf = SliceRef::from_bytes(&backing_buffer[..]);
        assert_eq!(buf.slice(1..5).as_slice(), b"ello");
        assert_eq!(buf.slice(1..=5).as_slice(), b"ello,");
        assert_eq!(buf.slice(..5).as_slice(), b"Hello");
    }
}
