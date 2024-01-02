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

//! Functions for storing *dynamically* sized "slots" in a contiguous memory region with a fixed
//! size.
//!
//! It's a Mullet: Offsets at the start, slots at the back. Growing til they meet in the middle.
//! When the page is full a new one can be used.
//! API:
//!     - insert - returns a success and id and number of bytes remaining free
//!     - remove - returns the number of bytes remaining free
//!     - lookup - returns Pin to the buffer
//!
//! When the page is empty, the first 4 bytes it points to is filled with nulls, and the memory
//! region can be madvise DONTNEED away, so that it is no longer resident in process RSS.
//!
//! In this way a large contiguous region of memory can be used to store slots in many (page-sized)
//! pages but only the ones that are in use will be resident in memory, forming a sparse array of
//! slots.
use std::pin::Pin;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release, SeqCst};
use std::sync::atomic::{AtomicPtr, AtomicU16, AtomicU32};

use atomic_wait::{wait, wake_all, wake_one};
use tracing::error;

use crate::tuplebox::slots::slotbox::SlotBoxError;

pub type SlotId = usize;

// Note that if a page is empty, either because it's new, or because all its slots have been
// removed, then used_bytes is 0.
// In this way both a madvise DONTNEEDed and an otherwise empty page generally are effectively
// identical, and we can just start using them right away in a null-state without doing any
// initialization.
#[repr(C)]
struct SlottedPageHeader {
    // The number of bytes used in the page
    used_bytes: u32,
    // The length of our slots index in bytes. Starts at initial zero.
    index_length: u32,
    // The length of our slots content in bytes. Starts at initial zero.
    // The page is full when index_length + content_length + sizeof(slotted_page) == PAGE_SIZE
    content_length: u32,
    // The number of available/used slots in the page
    num_slots: u32,

    /// The number of read locks times two, plus one if there's a writer waiting.
    /// u32::MAX if write locked.
    lock_state: AtomicU32,
    /// Incremented to wake up writers.
    writer_wake_counter: AtomicU32,

    _pin: std::marker::PhantomPinned,
}

impl SlottedPageHeader {
    /// Explicit unlock. Used by both the guard
    fn unlock_for_writes(self: Pin<&mut Self>) {
        self.lock_state.store(0, Release);
        self.writer_wake_counter.fetch_add(1, Release);
        wake_one(&self.writer_wake_counter);
        wake_all(&self.lock_state);
    }

    // Update used size in the positive direction.
    fn add_used(mut self: Pin<&mut Self>, size: usize) {
        unsafe {
            let header = self.as_mut().get_unchecked_mut();
            header.used_bytes += size as u32;
        }
    }

    // Update used size in the negative direction.
    fn sub_used(mut self: Pin<&mut Self>, size: usize) {
        unsafe {
            let header = self.as_mut().get_unchecked_mut();
            header.used_bytes -= size as u32;
        }
    }

    // Update accounting for the presence of a new entry.
    fn add_entry(mut self: Pin<&mut Self>, size: usize) -> SlotId {
        unsafe {
            let new_slot = self.num_slots as SlotId;
            let header = self.as_mut().get_unchecked_mut();
            header.used_bytes += size as u32;
            header.num_slots += 1;
            header.content_length += size as u32;
            header.index_length += std::mem::size_of::<SlotIndexEntry>() as u32;
            new_slot
        }
    }

    // Clear this page, and all the slots.
    fn clear(mut self: Pin<&mut Self>) {
        unsafe {
            let header = self.as_mut().get_unchecked_mut();
            header.num_slots = 0;
            header.index_length = 0;
            header.content_length = 0;
        }
    }
}

#[repr(C)]
struct SlotIndexEntry {
    used: bool,
    // The number of live references to this slot
    refcount: AtomicU16,
    // The offset of the slot in the content region
    offset: u16,
    // The allocated length of the slot in the content region. This remains constant for the life
    // of the slot, even if it is freed and re-used.
    allocated: u16,
    // The actual in-use length of the data. When a slot is freed, this is set to 0.
    used_bytes: u16,

    _pin: std::marker::PhantomPinned,
}

impl SlotIndexEntry {
    // Update accounting for the presence of a new entry.
    fn alloc(mut self: Pin<&mut Self>, content_position: usize, size: usize) {
        unsafe {
            let index_entry = self.as_mut().get_unchecked_mut();
            index_entry.offset = content_position as u16;

            // Net-new slots always have their full size used in their new index entry.
            index_entry.used_bytes = size as u16;
            index_entry.allocated = size as u16;
            index_entry.refcount = AtomicU16::new(0);
            index_entry.used = true;
        }
    }

    // Mark a previously free entry as used.
    fn mark_used(mut self: Pin<&mut Self>, size: usize) {
        unsafe {
            let entry = self.as_mut().get_unchecked_mut();
            entry.used = true;
            entry.used_bytes = size as u16;
        }
    }

    // Mark a previously used entry as free, and return the number of bytes it was using.
    fn mark_free(mut self: Pin<&mut Self>) -> usize {
        unsafe {
            let index_entry = self.as_mut().get_unchecked_mut();
            let used_bytes = index_entry.used_bytes as usize;
            index_entry.used = false;
            index_entry.used_bytes = 0;
            index_entry.refcount.store(0, SeqCst);
            used_bytes
        }
    }
}
/// The 'handle' for the page is a pointer to the base address of the page and its size, and the
/// page size, and from this, all other information can be derived by looking inside its content.
///
/// The contents of the page itself is expected to be page-able; that is, the representation on-disk
/// is the same as the representation in-memory.
pub struct SlottedPage<'a> {
    pub(crate) base_address: AtomicPtr<u8>,
    pub(crate) page_size: usize,

    _marker: std::marker::PhantomData<&'a u8>,
}

/// The size in bytes this page would be if completely empty.
pub fn slot_page_empty_size(page_size: usize) -> usize {
    page_size - slot_page_overhead()
}

pub const fn slot_page_overhead() -> usize {
    std::mem::size_of::<SlottedPageHeader>()
}

pub const fn slot_index_overhead() -> usize {
    std::mem::size_of::<SlotIndexEntry>()
}

impl<'a> SlottedPage<'a> {
    pub fn for_page(base_address: AtomicPtr<u8>, page_size: usize) -> Self {
        Self {
            base_address,
            page_size,
            _marker: Default::default(),
        }
    }

    /// How much space is available in this page?
    pub fn free_space_bytes(&self) -> usize {
        let header = self.header();
        let used = (header.num_slots * std::mem::size_of::<SlotIndexEntry>() as u32) as usize
            + header.used_bytes as usize
            + std::mem::size_of::<SlottedPageHeader>();
        self.page_size - used
    }

    /// How many bytes are available for appending to this page (i.e. not counting the space
    /// we could re-use, via e.g. used_bytes)
    pub fn available_content_bytes(&self) -> usize {
        let header = self.header();
        let content_length = header.content_length as usize;
        let index_length = header.index_length as usize;
        let header_size = std::mem::size_of::<SlottedPageHeader>();

        self.page_size - (index_length + content_length + header_size)
    }

    /// Add the slot into the page, copying it into the memory region, and returning the slot id
    /// and the number of bytes remaining in the page.
    fn allocate(
        &self,
        size: usize,
        initial_value: Option<&[u8]>,
    ) -> Result<(SlotId, usize, Pin<&'a mut [u8]>), SlotBoxError> {
        // See if we can use an existing slot to put the slot in, or if there's any fit at all.
        let (can_fit, fit_slot) = self.find_fit(size);
        if !can_fit {
            return Err(SlotBoxError::BoxFull(size, self.available_content_bytes()));
        }
        if let Some(fit_slot) = fit_slot {
            let content_position = self.offset_of(fit_slot).unwrap().0;
            let memory_as_slice = unsafe {
                std::slice::from_raw_parts_mut(self.base_address.load(SeqCst), self.page_size)
            };

            // If there's an initial value provided, copy it in.
            if let Some(initial_value) = initial_value {
                assert_eq!(initial_value.len(), size);
                memory_as_slice[content_position..content_position + size]
                    .copy_from_slice(initial_value);
            }

            let mut index_entry = self.get_index_entry_mut(fit_slot);
            index_entry.as_mut().mark_used(size);

            // Update used bytes in the header
            let header = self.header_mut();
            header.add_used(size);

            let slc = unsafe {
                Pin::new_unchecked(&mut memory_as_slice[content_position..content_position + size])
            };
            return Ok((fit_slot, self.available_content_bytes(), slc));
        }

        // Do we have enough room to add new content?
        let avail = self.available_content_bytes();
        if avail < size + std::mem::size_of::<SlotIndexEntry>() {
            return Err(SlotBoxError::BoxFull(
                size + std::mem::size_of::<SlotIndexEntry>(),
                avail,
            ));
        }

        // Add the slot to the content region. The start offset is PAGE_SIZE - content_length -
        // slot_length. So first thing, copy the bytes into the content region at that position.
        let content_length = self.header_mut().content_length as usize;
        let content_position = self.page_size - content_length - size;
        let memory_as_slice = unsafe {
            std::slice::from_raw_parts_mut(self.base_address.load(SeqCst), self.page_size)
        };

        // If there's an initial value provided, copy it in.
        if let Some(initial_value) = initial_value {
            assert_eq!(initial_value.len(), size);
            memory_as_slice[content_position..content_position + size]
                .copy_from_slice(initial_value);
        }

        // Add the index entry and expand the index region.
        let mut index_entry = self.get_index_entry_mut(self.header_mut().num_slots as usize);
        index_entry.as_mut().alloc(content_position, size);

        // Update the header
        let header = self.header_mut();
        let new_slot = header.add_entry(size);

        // Return the slot id and the number of bytes remaining to append at the end.
        let slc = unsafe {
            Pin::new_unchecked(&mut memory_as_slice[content_position..content_position + size])
        };
        Ok((new_slot, self.available_content_bytes(), slc))
    }

    /// Load into this page from an external byte source, which is assumed to be in our page
    /// format, and then reset all refcounts to 0, clear lock state, and return the set of all valid
    /// slot IDs.
    pub fn load<LF: FnMut(Pin<&mut [u8]>)>(&self, mut lf: LF) -> Vec<SlotId> {
        // First copy in the physical bytes into our address.
        let memory_as_slice = unsafe {
            Pin::new_unchecked(std::slice::from_raw_parts_mut(
                self.base_address.load(SeqCst),
                self.page_size,
            ))
        };
        lf(memory_as_slice);

        // Locks from the previous use of this page are now invalid, so reset.
        let header = self.header_mut();
        header.lock_state.store(0, SeqCst);
        header.writer_wake_counter.store(0, SeqCst);

        // Now reset all the refcounts to 0, and collect the list of all active slots.,
        let mut slot_ids = vec![];
        let num_slots = header.num_slots;
        for i in 0..num_slots {
            let index_entry = self.get_index_entry_mut(i as usize);
            if index_entry.used {
                index_entry.refcount.store(0, SeqCst);
                slot_ids.push(i as SlotId);
            }
        }
        slot_ids
    }

    /// Copy the contents of this page into a slice.
    pub fn save_into(&self, buf: &mut [u8]) {
        let _ = self.read_lock();
        let memory_as_slice = unsafe {
            std::slice::from_raw_parts_mut(self.base_address.load(SeqCst), self.page_size)
        };
        buf.copy_from_slice(memory_as_slice);
    }

    fn remove_slot(&self, slot_id: SlotId) -> Result<(usize, usize, bool), SlotBoxError> {
        // TODO: slots at start of content-length can be removed by shrinking the content-length
        //   portion.

        let mut index_entry = self.get_index_entry_mut(slot_id);
        assert!(
            index_entry.used,
            "attempt to free unused slot {}; double-free?",
            slot_id
        );
        let slot_size = index_entry.as_mut().mark_free();

        let mut header = self.header_mut();
        header.as_mut().sub_used(slot_size);

        // TODO: join adjacent free slots. Likewise at insert, support splitting slots.
        let is_empty = header.used_bytes == 0;
        if is_empty {
            header.clear();
        }
        Ok((self.available_content_bytes(), slot_size, is_empty))
    }

    pub(crate) fn upcount(&self, slot_id: SlotId) -> Result<(), SlotBoxError> {
        let index_entry = self.get_index_entry_mut(slot_id);
        index_entry.refcount.fetch_add(1, SeqCst);
        Ok(())
    }

    pub(crate) fn dncount(&self, slot_id: SlotId) -> Result<bool, SlotBoxError> {
        let index_entry = self.get_index_entry_mut(slot_id);
        let new_count = index_entry.refcount.fetch_sub(1, SeqCst);
        // Return true to indicate that the slot is now unused, but the slotbox will do the actual removal.
        assert_ne!(
            new_count, 0,
            "attempt to double-free slot {}; how did it come to this?",
            slot_id
        );
        if new_count == 1 {
            return Ok(true);
        }
        Ok(false)
    }

    fn get_slot(&self, slot_id: SlotId) -> Result<Pin<&'a [u8]>, SlotBoxError> {
        // Check that the index is in bounds
        let num_slots = self.header().num_slots as usize;
        if slot_id >= num_slots {
            error!(
                "slot_id {} is out of bounds for page with {} slots",
                slot_id, num_slots
            );
            return Err(SlotBoxError::TupleNotFound(slot_id));
        }

        // Read the index entry;
        let index_entry = self.get_index_entry(slot_id);
        if !index_entry.used {
            error!("slot_id {} is not used, invalid tuple", slot_id);
            return Err(SlotBoxError::TupleNotFound(slot_id));
        }
        let offset = index_entry.offset as usize;
        let length = index_entry.used_bytes as usize;

        let memory_as_slice =
            unsafe { std::slice::from_raw_parts(self.base_address.load(SeqCst), self.page_size) };
        Ok(unsafe { Pin::new_unchecked(&memory_as_slice[offset..offset + length]) })
    }

    fn get_slot_mut(&self, slot_id: SlotId) -> Result<Pin<&'a mut [u8]>, SlotBoxError> {
        // Check that the index is in bounds
        let num_slots = self.header().num_slots as usize;
        if slot_id >= num_slots {
            return Err(SlotBoxError::TupleNotFound(slot_id));
        }

        // Read the index entry;
        let index_entry = self.get_index_entry(slot_id);
        if !index_entry.used {
            return Err(SlotBoxError::TupleNotFound(slot_id));
        }
        let offset = index_entry.offset as usize;
        let length = index_entry.used_bytes as usize;

        let memory_as_slice = unsafe {
            std::slice::from_raw_parts_mut(self.base_address.load(SeqCst), self.page_size)
        };
        Ok(unsafe { Pin::new_unchecked(&mut memory_as_slice[offset..offset + length]) })
    }

    pub fn num_active_slots(&self) -> usize {
        let ns = self.header().num_slots as usize;
        let mut total = 0;
        for i in 0..ns {
            let index_entry = self.get_index_entry(i);
            if index_entry.used {
                total += 1;
            }
        }
        total
    }
}

impl<'a> SlottedPage<'a> {
    pub fn read_lock<'b>(&self) -> PageReadGuard<'b> {
        let header = self.header();
        let mut s = header.lock_state.load(Relaxed);
        loop {
            if s % 2 == 0 {
                // Even.
                assert!(s < u32::MAX - 2, "too many readers");
                match header
                    .lock_state
                    .compare_exchange_weak(s, s + 2, Acquire, Relaxed)
                {
                    Ok(_) => {
                        return PageReadGuard {
                            base_address: self.base_address.load(SeqCst),
                            page_size: self.page_size,
                            _marker: Default::default(),
                        }
                    }
                    Err(e) => s = e,
                }
            }
            if s % 2 == 1 {
                // Odd.
                wait(&header.lock_state, s);
                s = header.lock_state.load(Relaxed);
            }
        }
    }

    pub fn write_lock(&'a mut self) -> PageWriteGuard<'a> {
        let header = self.header();
        let mut s = header.lock_state.load(Relaxed);
        loop {
            // Try to lock if unlocked.
            if s <= 1 {
                match header
                    .lock_state
                    .compare_exchange(s, u32::MAX, Acquire, Relaxed)
                {
                    Ok(_) => {
                        return PageWriteGuard {
                            base_address: self.base_address.load(SeqCst),
                            page_size: self.page_size,
                            _marker: Default::default(),
                        }
                    }
                    Err(e) => {
                        s = e;
                        continue;
                    }
                }
            }
            // Block new readers, by making sure the state is odd.
            if s % 2 == 0 {
                match header
                    .lock_state
                    .compare_exchange(s, s + 1, Relaxed, Relaxed)
                {
                    Ok(_) => {}
                    Err(e) => {
                        s = e;
                        continue;
                    }
                }
            }
            // Wait, if it's still locked
            let w = header.writer_wake_counter.load(Acquire);
            s = header.lock_state.load(Relaxed);
            if s >= 2 {
                wait(&header.writer_wake_counter, w);
                s = header.lock_state.load(Relaxed);
            }
        }
    }

    fn header(&self) -> Pin<&SlottedPageHeader> {
        // Cast the base address to a pointear to the header
        let header_ptr = self.base_address.load(SeqCst) as *const SlottedPageHeader;

        unsafe { Pin::new_unchecked(&*header_ptr) }
    }

    fn header_mut(&self) -> Pin<&mut SlottedPageHeader> {
        // Cast the base address to a pointer to the header
        let header_ptr = self.base_address.load(SeqCst) as *mut SlottedPageHeader;

        unsafe { Pin::new_unchecked(&mut *header_ptr) }
    }

    /// Return the offset, size of the slot at the given index.
    fn offset_of(&self, tid: SlotId) -> Result<(usize, usize), SlotBoxError> {
        // Check that the index is in bounds
        let num_slots = self.header().num_slots as usize;
        if tid >= num_slots {
            return Err(SlotBoxError::TupleNotFound(tid));
        }

        // Read the index entry;
        let index_entry = self.get_index_entry(tid);
        Ok((index_entry.offset as usize, index_entry.allocated as usize))
    }

    fn find_fit(&self, size: usize) -> (bool, Option<SlotId>) {
        // Find the smallest possible fit by building full-scan candidate, and then sorting.
        let mut fits = vec![];
        let header = self.header();
        let num_slots = header.num_slots;
        for i in 0..num_slots {
            let index_entry = self.get_index_entry(i as usize);
            if index_entry.used {
                continue;
            }
            let slot_size = index_entry.allocated as usize;
            if slot_size >= size {
                fits.push((i as usize, slot_size));
            }
        }

        // Sort
        fits.sort_by(|a, b| a.1.cmp(&b.1));
        if let Some((tid, _)) = fits.first() {
            return (true, Some(*tid));
        }

        let index_length = header.index_length as isize;
        let content_length = header.content_length as isize;
        let header_size = std::mem::size_of::<SlottedPageHeader>() as isize;
        let avail = (self.page_size as isize) - (index_length + content_length + header_size);
        if avail < size as isize {
            return (true, None);
        }
        (avail >= size as isize, None)
    }

    fn get_index_entry(&self, slot_id: SlotId) -> Pin<&SlotIndexEntry> {
        let index_offset = std::mem::size_of::<SlottedPageHeader>()
            + (slot_id * std::mem::size_of::<SlotIndexEntry>());

        let base_address = self.base_address.load(SeqCst);

        unsafe {
            let slot_address = base_address.add(index_offset);
            Pin::new_unchecked(&*(slot_address as *const SlotIndexEntry))
        }
    }

    fn get_index_entry_mut(&self, slot_id: SlotId) -> Pin<&mut SlotIndexEntry> {
        let index_offset = std::mem::size_of::<SlottedPageHeader>()
            + (slot_id * std::mem::size_of::<SlotIndexEntry>());
        let base_address = self.base_address.load(SeqCst);

        unsafe {
            let slot_address = base_address.add(index_offset);
            Pin::new_unchecked(&mut *(slot_address as *mut SlotIndexEntry))
        }
    }
}

pub struct PageWriteGuard<'a> {
    base_address: *mut u8,
    page_size: usize,

    _marker: std::marker::PhantomData<&'a u8>,
}

impl<'a> PageWriteGuard<'a> {
    pub fn page_ptr(&self) -> *mut u8 {
        self.base_address
    }

    pub fn get_slot_mut(&mut self, slot_id: SlotId) -> Result<Pin<&'a mut [u8]>, SlotBoxError> {
        let sp = SlottedPage {
            base_address: AtomicPtr::new(self.base_address),
            page_size: self.page_size,
            _marker: Default::default(),
        };
        sp.get_slot_mut(slot_id)
    }

    pub fn allocate(
        &mut self,
        size: usize,
        initial_value: Option<&[u8]>,
    ) -> Result<(SlotId, usize, Pin<&'a mut [u8]>), SlotBoxError> {
        let sp = SlottedPage {
            base_address: AtomicPtr::new(self.base_address),
            page_size: self.page_size,
            _marker: Default::default(),
        };
        sp.allocate(size, initial_value)
    }
    pub fn remove_slot(&mut self, slot_id: SlotId) -> Result<(usize, usize, bool), SlotBoxError> {
        let sp = SlottedPage {
            base_address: AtomicPtr::new(self.base_address),
            page_size: self.page_size,
            _marker: Default::default(),
        };
        sp.remove_slot(slot_id)
    }

    fn header_mut(&self) -> Pin<&mut SlottedPageHeader> {
        let header_ptr = self.base_address as *mut SlottedPageHeader;

        unsafe { Pin::new_unchecked(&mut *header_ptr) }
    }
}

impl<'a> Drop for PageWriteGuard<'a> {
    fn drop(&mut self) {
        let header = self.header_mut();
        header.unlock_for_writes();
    }
}

pub struct PageReadGuard<'a> {
    base_address: *const u8,
    page_size: usize,

    _marker: std::marker::PhantomData<&'a u8>,
}

impl<'a> PageReadGuard<'a> {
    fn header(&self) -> Pin<&SlottedPageHeader> {
        let header_ptr = self.base_address as *const SlottedPageHeader;

        unsafe { Pin::new_unchecked(&*header_ptr) }
    }

    pub fn get_slot(&self, slot_id: SlotId) -> Result<Pin<&'a [u8]>, SlotBoxError> {
        let sp = SlottedPage {
            base_address: AtomicPtr::new(self.base_address as _),
            page_size: self.page_size,
            _marker: Default::default(),
        };
        sp.get_slot(slot_id)
    }
}

impl<'a> Drop for PageReadGuard<'a> {
    fn drop(&mut self) {
        let header = self.header();
        // Decrement the state by 2 to remove one read-lock.
        if header.lock_state.fetch_sub(2, Release) == 3 {
            // If we decremented from 3 to 1, that means
            // the RwLock is now unlocked _and_ there is
            // a waiting writer, which we wake up.
            header.writer_wake_counter.fetch_add(1, Release);
            wake_one(&header.writer_wake_counter);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicPtr;

    use crate::tuplebox::slots::slotbox::SlotBoxError;
    use crate::tuplebox::slots::slotted_page::{
        slot_page_empty_size, SlotId, SlotIndexEntry, SlottedPage,
    };

    fn random_fill(page: &SlottedPage) -> Vec<(SlotId, Vec<u8>)> {
        let mut collected_slots = vec![];
        loop {
            let size = rand::random::<usize>() % 100;
            let avail = page.free_space_bytes();
            let result = page.allocate(size, Some(&vec![123; size]));
            // If avail can't fit the size of the slot plus the index entry, then we should be
            // getting an error.
            if avail < size + std::mem::size_of::<SlotIndexEntry>() {
                assert!(matches!(result, Err(SlotBoxError::BoxFull(_, _))));
                break;
            }
            // Otherwise, we should be getting a slot id and a new avail that is smaller than the
            // previous avail.
            let (tid, new_avail, _) = result.unwrap();
            assert!(new_avail < avail);
            // And we should be able to get the slot back out.
            let slotvalue = page.get_slot(tid).unwrap();
            assert_eq!(slotvalue.len(), size);

            collected_slots.push((tid, slotvalue.to_vec()));
        }
        collected_slots
    }

    #[test]
    fn simple_add_get() {
        let mut page_memory = vec![0; 4096];
        let page_ptr = page_memory.as_mut_ptr();
        let page: SlottedPage = SlottedPage::for_page(AtomicPtr::new(page_ptr), 4096);
        let avail_before = page.free_space_bytes();
        let test_data = b"hello".to_vec();
        let (tid, free, slc) = page.allocate(test_data.len(), Some(&test_data)).unwrap();
        assert_eq!(tid, 0);
        assert!(avail_before > free);
        let slot = page.get_slot(tid).unwrap();
        assert_eq!(slc, slot);
        assert_eq!(test_data, *slot);
    }

    #[test]
    fn fill_until_full() {
        // Fill the page with a bunch of randomly sized slices of bytes, until we fill, to verify
        // that we can fill the page.
        let mut page_memory = vec![0; 4096];
        let page_ptr = page_memory.as_mut_ptr();
        let page: SlottedPage = SlottedPage::for_page(AtomicPtr::new(page_ptr), 4096);

        let collected_slots = random_fill(&page);

        for (tid, slot) in collected_slots {
            let retrieved = page.get_slot(tid).unwrap();
            assert_eq!(slot, *retrieved);
        }
    }

    #[test]
    fn test_add_remove_random() {
        let mut page_memory = vec![0; 4096];
        let page_ptr = page_memory.as_mut_ptr();

        // Fill the page with random slots.
        let page: SlottedPage = SlottedPage::for_page(AtomicPtr::new(page_ptr), 4096);

        let collected_slots = random_fill(&page);

        // Now randomly delete, and verify that the slot is gone.
        let mut removed_slots = vec![];
        for (tid, slot) in &collected_slots {
            let retrieved = page.get_slot(*tid).unwrap();
            assert_eq!(*slot, *retrieved);
            page.remove_slot(*tid).unwrap();
            assert!(matches!(
                page.get_slot(*tid),
                Err(SlotBoxError::TupleNotFound(_))
            ));
            removed_slots.push(*tid);
        }

        // Now randomly re-fill, and verify that the slots are back.
        let new_filled_slots = random_fill(&page);
        for (tid, slot) in new_filled_slots {
            let retrieved = page.get_slot(tid).unwrap();
            assert_eq!(slot, *retrieved);
        }

        // And verify that all the non-removed slots are still-intact.
        for (tid, slot) in collected_slots {
            if removed_slots.contains(&tid) {
                continue;
            }
            let retrieved = page.get_slot(tid).unwrap();
            assert_eq!(slot, *retrieved);
        }
    }

    // Verify that a page that is empty is all nulls in its first 4 bytes.
    #[test]
    fn test_verify_null_header() {
        let mut page_memory = vec![0; 4096];
        let page_ptr = page_memory.as_mut_ptr();
        let page: SlottedPage = SlottedPage::for_page(AtomicPtr::new(page_ptr), 4096);

        // Verify that the header is all nulls (well, duh, we made it that way, but let's be
        // paranoid)
        {
            let header = page.header();
            assert_eq!(header.num_slots, 0);
            assert_eq!(header.used_bytes, 0);

            let memory_as_slice = unsafe { std::slice::from_raw_parts(page_ptr, 4096) };
            assert_eq!(memory_as_slice[0..4], vec![0; 4][..]);
        }
        // Fill it, then free everything after.
        let everything = random_fill(&page);
        for (tid, _) in &everything {
            page.remove_slot(*tid).unwrap();
        }
        {
            let header = page.header();
            assert_eq!(header.num_slots, 0);
            assert_eq!(header.used_bytes, 0);

            let memory_as_slice = unsafe { std::slice::from_raw_parts(page_ptr, 4096) };
            assert_eq!(memory_as_slice[0..4], vec![0; 4][..]);
        }
    }

    #[test]
    fn fill_and_empty() {
        let mut page_memory = vec![0; 4096];
        let page_ptr = page_memory.as_mut_ptr();
        let page: SlottedPage = SlottedPage::for_page(AtomicPtr::new(page_ptr), 4096);

        // Fill the page with random slots.
        let collected_slots = random_fill(&page);
        // Then remove them all and verify it is now completely empty.
        let mut remaining = page.free_space_bytes();
        let mut last_is_empty = false;
        for (tid, _) in &collected_slots {
            let (new_remaining, _, empty) = page.remove_slot(*tid).unwrap();
            assert!(new_remaining >= remaining);
            remaining = new_remaining;
            last_is_empty = empty;
        }
        assert!(last_is_empty);
        assert_eq!(remaining, slot_page_empty_size(4096));
        assert_eq!(page.free_space_bytes(), slot_page_empty_size(4096));
    }
}