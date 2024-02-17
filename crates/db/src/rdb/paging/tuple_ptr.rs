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
use std::sync::atomic::AtomicPtr;
use std::sync::Arc;

use moor_values::util::ByteSource;

use crate::rdb::paging::TupleBox;
use crate::rdb::tuples::TupleId;

/// A reference to a tuple in a TupleBox, managed by the TupleBox itself. TupleRefs are given a pointer to these,
/// which allows the TupleBox to manage the lifetime of the tuple, swizzling it in and out of memory as needed.
/// Adds a layer of indirection to each tuple access, but is better than passing around tuple ids + TupleBox
/// references.

// TODO: rather than decoding a tuple out of a buffer in a slot, the slot should just hold the tuple structure
pub struct TuplePtr {
    tb: Arc<TupleBox>,
    id: TupleId,
    buflen: u32,
    bufaddr: AtomicPtr<u8>,

    _pin: std::marker::PhantomPinned,
}

impl TuplePtr {
    pub(crate) fn create(
        sb: Arc<TupleBox>,
        tuple_id: TupleId,
        bufaddr: *mut u8,
        buflen: usize,
    ) -> Self {
        TuplePtr {
            tb: sb.clone(),
            id: tuple_id,
            bufaddr: AtomicPtr::new(bufaddr),
            buflen: buflen as u32,
            _pin: std::marker::PhantomPinned,
        }
    }
}

impl PartialEq for TuplePtr {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TuplePtr {}

impl Hash for TuplePtr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl TuplePtr {
    #[inline]
    pub fn id(&self) -> TupleId {
        self.id
    }

    /// Mark the tuple as paged out. Accesses to the tuple will fault, and we'll need to page it back in.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn mark_paged_out(&mut self) {
        self.bufaddr
            .store(std::ptr::null_mut(), std::sync::atomic::Ordering::SeqCst);
    }

    #[allow(dead_code)]
    pub(crate) fn mark_paged_in(&mut self, bufaddr: *mut u8) {
        self.bufaddr
            .store(bufaddr, std::sync::atomic::Ordering::SeqCst);
    }

    #[inline]
    pub(crate) fn as_ptr<T>(&self) -> *const T {
        if self
            .bufaddr
            .load(std::sync::atomic::Ordering::SeqCst)
            .is_null()
        {
            self.tb.page_fault(self.id).unwrap();
        }
        self.bufaddr.load(std::sync::atomic::Ordering::SeqCst) as *const T
    }

    #[inline]
    pub(crate) fn as_mut_ptr<T>(&mut self) -> *mut T {
        // TODO: if the ptr is null, this is a page fault, and we'll
        //   need to ask the tuplebox to ask the pager to page us in
        self.bufaddr.load(std::sync::atomic::Ordering::SeqCst) as *mut T
    }

    #[inline]
    pub(crate) fn buffer(&self) -> &[u8] {
        let buf_addr = self.as_ptr();
        unsafe { std::slice::from_raw_parts(buf_addr, self.buflen as usize) }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn byte_source(&self) -> SlotByteSource {
        SlotByteSource::new(self)
    }

    #[allow(dead_code)]
    pub fn is_paged_out(&self) -> bool {
        self.bufaddr
            .load(std::sync::atomic::Ordering::SeqCst)
            .is_null()
    }

    #[inline]
    pub fn refcount(&self) -> u16 {
        self.tb.refcount(self.id).unwrap()
    }

    #[inline]
    pub fn upcount(&self) {
        self.tb.upcount(self.id).unwrap();
    }

    #[inline]
    pub fn dncount(&self) {
        self.tb.dncount(self.id).unwrap();
    }
}

/// So we can build SliceRefs off of TuplePtrs
pub struct SlotByteSource {
    ptr: *const TuplePtr,
}

unsafe impl Send for SlotByteSource {}
unsafe impl Sync for SlotByteSource {}

impl SlotByteSource {
    fn new(ptr: *const TuplePtr) -> Self {
        // upcnt
        let tp = unsafe { &(*ptr) };
        tp.upcount();
        SlotByteSource { ptr }
    }
}
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

impl Clone for SlotByteSource {
    fn clone(&self) -> Self {
        // Upcount
        let tp = unsafe { &(*self.ptr) };
        tp.upcount();
        SlotByteSource { ptr: self.ptr }
    }
}

impl Drop for SlotByteSource {
    fn drop(&mut self) {
        // Downcount
        let tp = unsafe { &(*self.ptr) };
        tp.dncount();
    }
}
