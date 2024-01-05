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

use std::hash::Hash;
use std::sync::Arc;

use moor_values::util::slice_ref::ByteSource;

use crate::tuplebox::tuples::{SlotBox, TupleId};

/// A reference to a tuple in a SlotBox, owned by the SlotBox itself. TupleRefs are given a pointer to these,
/// which allows the SlotBox to manage the lifetime of the tuple, swizzling it in and out of memory as needed.
/// Adds a layer of indirection to each tuple access, but is better than passing around tuple ids + slotbox
/// references.
pub struct SlotPtr {
    sb: Arc<SlotBox>,
    id: TupleId,
    buflen: usize,
    bufaddr: *mut u8,

    _pin: std::marker::PhantomPinned,
}

unsafe impl Send for SlotPtr {}
unsafe impl Sync for SlotPtr {}

impl SlotPtr {
    pub(crate) fn create(
        sb: Arc<SlotBox>,
        tuple_id: TupleId,
        bufaddr: *mut u8,
        buflen: usize,
    ) -> Self {
        SlotPtr {
            sb: sb.clone(),
            id: tuple_id,
            bufaddr,
            buflen,
            _pin: std::marker::PhantomPinned,
        }
    }
}

impl PartialEq for SlotPtr {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for SlotPtr {}

impl Hash for SlotPtr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl SlotPtr {
    #[inline]
    pub fn id(&self) -> TupleId {
        self.id
    }

    #[inline]
    pub(crate) fn as_ptr<T>(&self) -> *const T {
        self.bufaddr as *const T
    }

    #[inline]
    pub(crate) fn as_mut_ptr<T>(&self) -> *mut T {
        self.bufaddr as *mut T
    }

    #[inline]
    fn buffer(&self) -> &[u8] {
        let buf_addr = self.as_ptr();
        unsafe { std::slice::from_raw_parts(buf_addr, self.buflen) }
    }

    #[inline]
    pub fn byte_source(&self) -> SlotByteSource {
        SlotByteSource {
            ptr: self as *const SlotPtr,
        }
    }

    #[inline]
    pub fn upcount(&self) {
        self.sb.upcount(self.id).unwrap();
    }

    #[inline]
    pub fn dncount(&self) {
        self.sb.dncount(self.id).unwrap();
    }
}

/// So we can build SliceRefs off of SlotPtrs
pub struct SlotByteSource {
    ptr: *const SlotPtr,
}

unsafe impl Send for SlotByteSource {}
unsafe impl Sync for SlotByteSource {}

impl ByteSource for SlotByteSource {
    #[inline]
    fn as_slice(&self) -> &[u8] {
        let buffer = (unsafe { &(*self.ptr) }).buffer();
        buffer
    }

    #[inline]
    fn len(&self) -> usize {
        let buffer = (unsafe { &(*self.ptr) }).buffer();
        buffer.len()
    }

    fn touch(&self) {}
}
