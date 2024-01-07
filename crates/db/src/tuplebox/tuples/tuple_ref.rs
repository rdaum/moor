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

use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::Arc;

use moor_values::util::slice_ref::SliceRef;

use crate::tuplebox::tuples::tuple_ptr::TuplePtr;
use crate::tuplebox::tuples::{SlotBox, SlotBoxError, TupleId};
use crate::tuplebox::RelationId;

pub struct TupleRef {
    // Yo dawg I heard you like pointers, so I put a pointer in your pointer.
    sp: *mut TuplePtr,
}

#[repr(C, align(8))]
struct Header {
    ts: u64,
    domain_size: u32,
    codomain_size: u32,
}

unsafe impl Send for TupleRef {}
unsafe impl Sync for TupleRef {}
impl TupleRef {
    // Wrap an existing SlotPtr.
    // Note: to avoid deadlocking at construction, assumes that the tuple is already upcounted by the
    // caller.
    pub(crate) fn at_ptr(sp: *mut TuplePtr) -> Self {
        Self { sp }
    }
    /// Allocate the given tuple in a slotbox.
    pub fn allocate(
        relation_id: RelationId,
        sb: Arc<SlotBox>,
        ts: u64,
        domain: &[u8],
        codomain: &[u8],
    ) -> Result<TupleRef, SlotBoxError> {
        let total_size = std::mem::size_of::<Header>() + domain.len() + codomain.len();
        let tuple_ref = sb.clone().allocate(total_size, relation_id, None)?;
        sb.update_with(tuple_ref.id(), |mut buffer| {
            let domain_len = domain.len();
            let codomain_len = codomain.len();
            {
                let header_ptr = buffer.as_mut().as_mut_ptr() as *mut Header;
                let header = unsafe { &mut *header_ptr };
                header.ts = ts;
                header.domain_size = domain_len as u32;
                header.codomain_size = codomain_len as u32;
            }
            let start_pos = std::mem::size_of::<Header>();
            let codomain_start = start_pos + domain_len;
            let codomain_end = codomain_start + codomain_len;
            buffer[start_pos..start_pos + domain_len].copy_from_slice(domain);
            buffer[codomain_start..codomain_end].copy_from_slice(codomain);
        })?;

        // Initial refcount should be 1, because we have a reference to it.
        assert_eq!(tuple_ref.resolve_slot_ptr().refcount(), 1);
        Ok(tuple_ref)
    }

    /// The id of the tuple.
    #[inline]
    pub fn id(&self) -> TupleId {
        self.resolve_slot_ptr().as_ref().id()
    }

    /// Update the timestamp of the tuple.
    #[inline]
    pub fn update_timestamp(&mut self, ts: u64) {
        let header = self.header_mut();
        header.ts = ts;
    }

    /// The timestamp of the tuple.
    #[inline]
    pub fn ts(&self) -> u64 {
        let header = self.header();
        header.ts
    }

    /// The domain of the tuple.
    #[inline]
    pub fn domain(&self) -> SliceRef {
        let header = self.header();
        let domain_size = header.domain_size as usize;
        let buffer = self.slot_buffer();
        let domain_start = std::mem::size_of::<Header>();
        buffer.slice(domain_start..domain_start + domain_size)
    }

    /// The codomain of the tuple.
    #[inline]
    pub fn codomain(&self) -> SliceRef {
        let header = self.header();
        let domain_size = header.domain_size as usize;
        let codomain_size = header.codomain_size as usize;
        let buffer = self.slot_buffer();
        let codomain_start = std::mem::size_of::<Header>() + domain_size;
        buffer.slice(codomain_start..codomain_start + codomain_size)
    }

    /// The raw buffer of the tuple, including the header, not dividing up the domain and codomain.
    pub fn slot_buffer(&self) -> SliceRef {
        let slot_ptr = self.resolve_slot_ptr();
        SliceRef::from_byte_source(slot_ptr.byte_source())
    }
}

impl TupleRef {
    #[inline]
    fn header(&self) -> &Header {
        let slot_ptr = self.resolve_slot_ptr();
        let header: *const Header = slot_ptr.as_ptr();
        unsafe { &*header }
    }

    #[inline]
    fn header_mut(&mut self) -> &mut Header {
        let slot_ptr = self.resolve_slot_ptr_mut();
        let header: *mut Header = unsafe { slot_ptr.get_unchecked_mut() }.as_mut_ptr();
        unsafe { &mut *header }
    }

    #[inline]
    fn resolve_slot_ptr(&self) -> Pin<&TuplePtr> {
        unsafe { Pin::new_unchecked(&*self.sp) }
    }

    #[inline]
    fn resolve_slot_ptr_mut(&mut self) -> Pin<&mut TuplePtr> {
        unsafe { Pin::new_unchecked(&mut *self.sp) }
    }

    #[inline]
    fn upcount(&self) {
        let slot_ptr = self.resolve_slot_ptr();
        slot_ptr.upcount();
    }

    #[inline]
    fn dncount(&self) {
        let slot_ptr = self.resolve_slot_ptr();
        slot_ptr.dncount();
    }
}

impl Hash for TupleRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let id = self.id();
        id.hash(state);
    }
}

impl PartialEq for TupleRef {
    fn eq(&self, other: &Self) -> bool {
        let (id, other_id) = (self.id(), other.id());
        id == other_id
    }
}

impl Eq for TupleRef {}

impl Drop for TupleRef {
    fn drop(&mut self) {
        self.dncount()
    }
}

impl Clone for TupleRef {
    fn clone(&self) -> Self {
        self.upcount();
        let sp = self.sp;
        Self { sp }
    }
}
