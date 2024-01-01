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

use crate::util::slice_ref::SliceRef;
use crate::AsByteBuffer;
use bytes::BufMut;
use std::convert::TryInto;
use uuid::Uuid;

pub trait HasUuid {
    fn uuid(&self) -> Uuid;
}

pub trait Named {
    fn matches_name(&self, name: &str) -> bool;
}

/// A container for verb or property defs.
/// Immutable, and can be iterated over in sequence, or searched by name.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Defs<T: AsByteBuffer + Clone + Sized + HasUuid + Named + 'static> {
    bytes: SliceRef,
    _phantom: std::marker::PhantomData<T>,
}

pub struct DefsIter<T: AsByteBuffer> {
    position: usize,
    buffer: SliceRef,
    _phantom: std::marker::PhantomData<T>,
}
impl<T: AsByteBuffer> Iterator for DefsIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.buffer.len() {
            return None;
        }
        // Read length prefix
        let len = u32::from_le_bytes(
            self.buffer.as_slice()[self.position..self.position + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        self.position += 4;
        // Read the bytes for the next item.
        let item_slice = self.buffer.slice(self.position..self.position + len);
        self.position += len;
        // Build the item from the bytes.
        Some(T::from_sliceref(item_slice))
    }
}

impl<T: AsByteBuffer + Clone + HasUuid + Named> Defs<T> {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            bytes: SliceRef::empty(),
            _phantom: Default::default(),
        }
    }
    #[must_use]
    pub fn from_sliceref(bytes: SliceRef) -> Self {
        Self {
            bytes,
            _phantom: Default::default(),
        }
    }
    pub fn from_items(items: &[T]) -> Self {
        let mut bytes = Vec::new();
        for item in items {
            item.with_byte_buffer(|item_bytes| {
                bytes.put_u32_le(item_bytes.len() as u32);
                bytes.put_slice(item_bytes);
            });
        }
        Self {
            bytes: SliceRef::from_bytes(&bytes),
            _phantom: Default::default(),
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = T> {
        DefsIter {
            position: 0,
            buffer: self.bytes.clone(),
            _phantom: Default::default(),
        }
    }
    // Provides the number of items in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    #[must_use]
    pub fn contains(&self, uuid: Uuid) -> bool {
        self.iter().any(|p| p.uuid() == uuid)
    }
    #[must_use]
    pub fn find(&self, uuid: &Uuid) -> Option<T> {
        self.iter().find(|p| &p.uuid() == uuid)
    }
    #[must_use]
    pub fn find_named(&self, name: &str) -> Vec<T> {
        self.iter().filter(|p| p.matches_name(name)).collect()
    }
    #[must_use]
    pub fn find_first_named(&self, name: &str) -> Option<T> {
        self.iter().find(|p| p.matches_name(name))
    }
    #[must_use]
    pub fn with_removed(&self, uuid: Uuid) -> Option<Self> {
        // Return None if the uuid isn't found, otherwise return a copy with the verb removed.
        // This is an O(N) operation, and then we do another O(N) operation to copy the buffer, but
        // if we didn't do this, we'd waste a buffer, so...
        if !self.contains(uuid) {
            return None;
        }
        // Construct a brand new buffer.
        let mut buf = Vec::with_capacity(self.bytes.len());
        for v in self.iter().filter(|v| v.uuid() != uuid) {
            v.with_byte_buffer(|bytes| {
                // Write the length prefix.
                buf.put_u32_le(bytes.len() as u32);
                // Write the bytes for the item.
                buf.put_slice(bytes);
            });
        }
        Some(Self::from_sliceref(SliceRef::from_bytes(&buf)))
    }

    #[must_use]
    pub fn with_all_removed(&self, uuids: &[Uuid]) -> Self {
        let mut buf = Vec::with_capacity(self.bytes.len());
        for v in self.iter().filter(|v| !uuids.contains(&v.uuid())) {
            v.with_byte_buffer(|bytes| {
                // Write the length prefix.
                buf.put_u32_le(bytes.len() as u32);
                // Write the bytes for the item.
                buf.put_slice(bytes);
            });
        }
        Self::from_sliceref(SliceRef::from_bytes(&buf))
    }

    // TODO Add builder patterns for these that construct in-place, building the buffer right in us.
    pub fn with_added(&self, v: T) -> Self {
        let mut new_buf = self.bytes.as_slice().to_vec();
        v.with_byte_buffer(|bytes| {
            // Write the length prefix.
            new_buf.put_u32_le(bytes.len() as u32);
            // Write the bytes for the item.
            new_buf.put_slice(bytes);
        });
        Self::from_sliceref(SliceRef::from_bytes(&new_buf))
    }
    pub fn with_all_added(&self, v: &[T]) -> Self {
        let mut new_buf = self.bytes.as_slice().to_vec();
        for v in v {
            v.with_byte_buffer(|bytes| {
                // Write the length prefix.
                new_buf.put_u32_le(bytes.len() as u32);
                // Write the bytes for the item.
                new_buf.put_slice(bytes);
            });
        }
        Self::from_sliceref(SliceRef::from_bytes(&new_buf))
    }
    pub fn with_updated<F: Fn(&T) -> T>(&self, uuid: Uuid, f: F) -> Option<Self> {
        if !self.contains(uuid) {
            return None;
        }
        // Copy until we find the uuid, then build the updated item, then copy the rest.
        let mut new_buf = Vec::new();
        for v in self.iter() {
            if v.uuid() == uuid {
                f(&v).with_byte_buffer(|bytes| {
                    // Write the length prefix.
                    new_buf.put_u32_le(bytes.len() as u32);
                    // Write the bytes for the item.
                    new_buf.put_slice(bytes);
                });
            } else {
                v.with_byte_buffer(|bytes| {
                    // Write the length prefix.
                    new_buf.put_u32_le(bytes.len() as u32);
                    // Write the bytes for the item.
                    new_buf.put_slice(bytes);
                });
            };
        }
        Some(Self::from_sliceref(SliceRef::from_bytes(&new_buf)))
    }
}

impl<T: AsByteBuffer + Clone + HasUuid + Named> AsByteBuffer for Defs<T> {
    fn size_bytes(&self) -> usize {
        self.bytes.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> R {
        f(self.bytes.as_slice())
    }

    fn make_copy_as_vec(&self) -> Vec<u8> {
        self.bytes.as_slice().to_vec()
    }

    fn from_sliceref(bytes: SliceRef) -> Self {
        Self {
            bytes,
            _phantom: Default::default(),
        }
    }

    fn as_sliceref(&self) -> SliceRef {
        self.bytes.clone()
    }
}
