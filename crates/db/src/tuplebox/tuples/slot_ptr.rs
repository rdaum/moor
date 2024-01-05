use crate::tuplebox::tuples::{SlotBox, TupleId};
use moor_values::util::slice_ref::ByteSource;
use std::hash::Hash;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

/// A reference to a tuple in a SlotBox, owned by the SlotBox itself. TupleRefs are given a pointer to these,
/// which allows the SlotBox to manage the lifetime of the tuple, swizzling it in and out of memory as needed.
/// Adds a layer of indirection to each tuple access, but is better than passing around tuple ids + slotbox
/// references.
pub struct SlotPtr {
    sb: Arc<SlotBox>,
    id: TupleId,
    buflen: usize,
    bufaddr: AtomicPtr<u8>,

    _pin: std::marker::PhantomPinned,
}

impl SlotPtr {
    pub(crate) fn create(
        sb: Arc<SlotBox>,
        tuple_id: TupleId,
        bufaddr: AtomicPtr<u8>,
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
    pub fn id(&self) -> TupleId {
        self.id
    }

    fn buffer(&self) -> &[u8] {
        let buf_addr = self.bufaddr.load(SeqCst);
        unsafe { std::slice::from_raw_parts(buf_addr, self.buflen) }
    }

    pub fn byte_source(&self) -> Box<dyn ByteSource> {
        Box::new(SlotByteSource {
            ptr: AtomicPtr::new((self as *const SlotPtr) as *mut SlotPtr),
        })
    }

    pub fn upcount(&self) {
        self.sb.upcount(self.id).unwrap();
    }

    pub fn dncount(&self) {
        self.sb.dncount(self.id).unwrap();
    }
}

/// So we can build SliceRefs off of SlotPtrs
pub struct SlotByteSource {
    ptr: AtomicPtr<SlotPtr>,
}

impl ByteSource for SlotByteSource {
    fn as_slice(&self) -> &[u8] {
        let ptr = self.ptr.load(SeqCst);
        let buffer = (unsafe { &(*ptr) }).buffer();
        buffer
    }

    fn len(&self) -> usize {
        let ptr = self.ptr.load(SeqCst);
        let buffer = (unsafe { &(*ptr) }).buffer();
        buffer.len()
    }

    fn touch(&self) {}
}
