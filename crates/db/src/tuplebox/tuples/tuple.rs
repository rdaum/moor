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

use binary_layout::define_layout;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

use crate::tuplebox::RelationId;
use moor_values::util::slice_ref::SliceRef;

use crate::tuplebox::tuples::slot_ptr::SlotPtr;
use crate::tuplebox::tuples::{SlotBox, SlotBoxError, TupleId};

const MAGIC_MARKER: u32 = 0xcafebabe;

define_layout!(tuple_header, LittleEndian, {
    magic_marker: u32,
    ts: u64,
    domain_size: u32,
    codomain_size: u32,
});

pub struct TupleRef {
    sp: AtomicPtr<SlotPtr>,
}

impl TupleRef {
    // Wrap an existing SlotPtr.
    // Note: to avoid deadlocking at construction, assumes that the tuple is already upcounted by the
    // caller.
    pub(crate) fn at_ptr(sp: AtomicPtr<SlotPtr>) -> Self {
        
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
        let total_size = tuple_header::SIZE.unwrap() + domain.len() + codomain.len();
        let tuple_ref = sb.clone().allocate(total_size, relation_id, None)?;
        sb.update_with(tuple_ref.id(), |mut buffer| {
            let domain_len = domain.len();
            let codomain_len = codomain.len();
            {
                let mut header = tuple_header::View::new(buffer.as_mut().get_mut());
                header.magic_marker_mut().write(MAGIC_MARKER);
                header.ts_mut().write(ts);
                header.domain_size_mut().write(domain_len as u32);
                header.codomain_size_mut().write(codomain_len as u32);
            }
            let start_pos = tuple_header::SIZE.unwrap();
            buffer[start_pos..start_pos + domain_len].copy_from_slice(domain);
            buffer[start_pos + domain_len..].copy_from_slice(codomain);
        })?;
        Ok(tuple_ref)
    }

    pub fn id(&self) -> TupleId {
        self.resolve_slot_ptr().as_ref().id()
    }

    /// Update the timestamp of the tuple.
    pub fn update_timestamp(&self, relation_id: RelationId, sb: Arc<SlotBox>, ts: u64) {
        let mut buffer = self.slot_buffer().as_slice().to_vec();
        let mut header = tuple_header::View::new(&mut buffer);
        header.ts_mut().write(ts);
        let id = self.resolve_slot_ptr().as_ref().id();
        // The update method will return a new tuple ID if the tuple is moved, and it should *not*
        // for timestamp updates.
        assert!(sb
            .update(relation_id, id, buffer.as_slice())
            .unwrap()
            .is_none());
    }

    /// The timestamp of the tuple.
    pub fn ts(&self) -> u64 {
        let buffer = self.slot_buffer();
        let header = tuple_header::View::new(buffer.as_slice());
        assert_eq!(header.magic_marker().read(), MAGIC_MARKER);
        header.ts().read()
    }

    /// The domain of the tuple.
    pub fn domain(&self) -> SliceRef {
        let buffer = self.slot_buffer();
        let header = tuple_header::View::new(buffer.as_slice());
        assert_eq!(header.magic_marker().read(), MAGIC_MARKER);
        let domain_size = header.domain_size().read();
        buffer
            .slice(tuple_header::SIZE.unwrap()..tuple_header::SIZE.unwrap() + domain_size as usize)
    }

    /// The codomain of the tuple.
    pub fn codomain(&self) -> SliceRef {
        let buffer = self.slot_buffer();
        let header = tuple_header::View::new(buffer.as_slice());
        assert_eq!(header.magic_marker().read(), MAGIC_MARKER);
        let domain_size = header.domain_size().read() as usize;
        let codomain_size = header.codomain_size().read() as usize;
        buffer.slice(
            tuple_header::SIZE.unwrap() + domain_size
                ..tuple_header::SIZE.unwrap() + domain_size + codomain_size,
        )
    }

    /// The raw buffer of the tuple, including the header, not dividing up the domain and codomain.
    pub fn slot_buffer(&self) -> SliceRef {
        let slot_ptr = self.resolve_slot_ptr();
        SliceRef::from_byte_source(slot_ptr.byte_source())
    }
}

impl TupleRef {
    fn resolve_slot_ptr(&self) -> Pin<&mut SlotPtr> {
        unsafe { Pin::new_unchecked(&mut *self.sp.load(SeqCst)) }
    }

    fn upcount(&self) {
        let slot_ptr = self.resolve_slot_ptr();
        slot_ptr.upcount();
    }

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
        let addr = self.sp.load(SeqCst);
        Self {
            sp: AtomicPtr::new(addr),
        }
    }
}
