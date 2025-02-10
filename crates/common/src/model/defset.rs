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

use crate::Symbol;
use crate::model::ValSet;
use crate::{AsByteBuffer, DecodingError, EncodingError};
use byteview::ByteView;
use itertools::Itertools;
use std::convert::TryInto;
use std::fmt::{Debug, Display, Formatter};
use uuid::Uuid;

pub trait HasUuid {
    fn uuid(&self) -> Uuid;
}

pub trait Named {
    fn matches_name(&self, name: Symbol) -> bool;
    fn names(&self) -> Vec<&str>;
}

/// A container for verb or property defs.
/// Immutable, and can be iterated over in sequence, or searched by name.
#[derive(Eq, PartialEq)]
pub struct Defs<T: AsByteBuffer + Clone + Sized + HasUuid + Named + 'static> {
    bytes: ByteView,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: AsByteBuffer + Clone + Sized + HasUuid + Named + 'static> Clone for Defs<T> {
    fn clone(&self) -> Self {
        Self {
            bytes: self.bytes.to_detached().clone(),
            _phantom: Default::default(),
        }
    }
}

impl<T: AsByteBuffer + Clone + Sized + HasUuid + Named + 'static> Display for Defs<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let names = self
            .iter()
            .map(|p| p.names().iter().map(|s| s.to_string()).join(":"))
            .collect::<Vec<_>>()
            .join(", ");
        f.write_fmt(format_args!("{{{}}}", names))
    }
}

impl<T: AsByteBuffer + Clone + Sized + HasUuid + Named + 'static> Debug for Defs<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Just use the display
        Display::fmt(self, f)
    }
}
pub struct DefsIter<T: AsByteBuffer> {
    position: usize,
    buffer: ByteView,
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
            self.buffer[self.position..self.position + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        self.position += 4;
        // Read the bytes for the next item.
        let item_slice = self.buffer.slice(self.position..self.position + len);
        self.position += len;
        // Build the item from the bytes.
        Some(T::from_bytes(item_slice).expect("Failed to decode defs item"))
    }
}

impl<T: AsByteBuffer + Clone + HasUuid + Named> ValSet<T> for Defs<T> {
    #[must_use]
    fn empty() -> Self {
        Self {
            bytes: ByteView::default(),
            _phantom: Default::default(),
        }
    }

    fn from_items(items: &[T]) -> Self {
        let mut bytes = Vec::new();
        for item in items {
            item.with_byte_buffer(|item_bytes| {
                let len = item_bytes.len() as u32;
                bytes.extend_from_slice(&len.to_le_bytes());
                bytes.extend_from_slice(item_bytes);
            })
            .expect("Failed to encode item");
        }
        Self {
            bytes: bytes.into(),
            _phantom: Default::default(),
        }
    }
    fn iter(&self) -> impl Iterator<Item = T> {
        DefsIter {
            position: 0,
            buffer: self.bytes.to_detached(),
            _phantom: Default::default(),
        }
    }
    // Provides the number of items in the buffer.
    #[must_use]
    fn len(&self) -> usize {
        self.iter().count()
    }

    #[must_use]
    fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }
}

impl<T: AsByteBuffer + Clone + HasUuid + Named> FromIterator<T> for Defs<T> {
    fn from_iter<X: IntoIterator<Item = T>>(iter: X) -> Self {
        let mut bytes = Vec::new();
        for item in iter {
            item.with_byte_buffer(|item_bytes| {
                let len = item_bytes.len() as u32;
                bytes.extend_from_slice(&len.to_le_bytes());
                bytes.extend_from_slice(item_bytes);
            })
            .expect("Failed to encode item");
        }
        Self {
            bytes: bytes.into(),
            _phantom: Default::default(),
        }
    }
}

impl<T: AsByteBuffer + Clone + HasUuid + Named> Defs<T> {
    #[must_use]
    pub fn contains(&self, uuid: Uuid) -> bool {
        self.iter().any(|p| p.uuid() == uuid)
    }
    #[must_use]
    pub fn find(&self, uuid: &Uuid) -> Option<T> {
        self.iter().find(|p| &p.uuid() == uuid)
    }
    #[must_use]
    pub fn find_named(&self, name: Symbol) -> Vec<T> {
        self.iter().filter(|p| p.matches_name(name)).collect()
    }
    #[must_use]
    pub fn find_first_named(&self, name: Symbol) -> Option<T> {
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
                let len = bytes.len() as u32;
                buf.extend_from_slice(&len.to_le_bytes());
                buf.extend_from_slice(&bytes);
            })
            .expect("Failed to encode item");
        }
        Some(Self::from_bytes(ByteView::from(buf)).unwrap())
    }

    #[must_use]
    pub fn with_all_removed(&self, uuids: &[Uuid]) -> Self {
        let mut buf = Vec::with_capacity(self.bytes.len());
        for v in self.iter().filter(|v| !uuids.contains(&v.uuid())) {
            v.with_byte_buffer(|bytes| {
                let len = bytes.len() as u32;
                buf.extend_from_slice(&len.to_le_bytes());
                buf.extend_from_slice(&bytes);
            })
            .expect("Failed to encode item");
        }
        Self::from_bytes(ByteView::from(buf)).unwrap()
    }

    // TODO Add builder patterns for these that construct in-place, building the buffer right in us.
    pub fn with_added(&self, v: T) -> Self {
        let mut buf = self.bytes.to_vec();
        v.with_byte_buffer(|bytes| {
            let len = bytes.len() as u32;
            buf.extend_from_slice(&len.to_le_bytes());
            buf.extend_from_slice(&bytes);
        })
        .expect("Failed to encode item");
        Self::from_bytes(ByteView::from(buf)).unwrap()
    }
    pub fn with_all_added(&self, v: &[T]) -> Self {
        let mut buf = self.bytes.to_vec();
        for i in v {
            i.with_byte_buffer(|bytes| {
                let len = bytes.len() as u32;
                buf.extend_from_slice(&len.to_le_bytes());
                buf.extend_from_slice(&bytes);
            })
            .expect("Failed to encode item");
        }
        Self::from_bytes(ByteView::from(buf)).unwrap()
    }
    pub fn with_updated<F: Fn(&T) -> T>(&self, uuid: Uuid, f: F) -> Option<Self> {
        if !self.contains(uuid) {
            return None;
        }
        // Copy until we find the uuid, then build the updated item, then copy the rest.
        let mut buf = Vec::new();
        for v in self.iter() {
            if v.uuid() == uuid {
                f(&v)
                    .with_byte_buffer(|bytes| {
                        let len = bytes.len() as u32;
                        buf.extend_from_slice(&len.to_le_bytes());
                        buf.extend_from_slice(bytes);
                    })
                    .expect("Failed to encode item");
            } else {
                v.with_byte_buffer(|bytes| {
                    let len = bytes.len() as u32;
                    buf.extend_from_slice(&len.to_le_bytes());
                    buf.extend_from_slice(bytes);
                })
                .expect("Failed to encode item");
            };
        }
        Some(Self::from_bytes(ByteView::from(buf)).unwrap())
    }
}

impl<T: AsByteBuffer + Clone + HasUuid + Named> AsByteBuffer for Defs<T> {
    fn size_bytes(&self) -> usize {
        self.bytes.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(self.bytes.as_ref()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.bytes.as_ref().to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, DecodingError> {
        Ok(Self {
            bytes,
            _phantom: Default::default(),
        })
    }

    fn as_bytes(&self) -> Result<ByteView, EncodingError> {
        Ok(self.bytes.to_detached())
    }
}
