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
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release, SeqCst};

use atomic_wait::{wait, wake_all, wake_one};
use tracing::error;

use crate::paging::TupleBoxError;

pub type SlotId = u32;

// Note that if a page is empty, either because it's new, or because all its slots have been
// removed, then used_bytes is 0.
// In this way both a madvise DONTNEEDed and an otherwise empty page generally are effectively
// identical, and we can just start using them right away in a null-state without doing any
// initialization.
#[repr(C, align(8))]
pub struct PageHeader {
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

impl PageHeader {
    /// Explicit unlock. Used by both the guard
    fn unlock_for_writes(self: Pin<&mut Self>) {
        self.lock_state.store(0, Release);
        self.writer_wake_counter.fetch_add(1, Release);
        wake_one(&self.writer_wake_counter);
        wake_all(&self.lock_state);
    }

    // Update used size in the positive direction.
    #[allow(dead_code)] // Not used right now because find_fit usage is commented out.
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
        let padded_size = (size + 7) & !7;
        unsafe {
            let new_slot = self.num_slots as SlotId;
            let header = self.as_mut().get_unchecked_mut();
            header.used_bytes += size as u32;
            header.num_slots += 1;
            header.content_length += padded_size as u32;
            header.index_length += std::mem::size_of::<IndexEntry>() as u32;
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

#[repr(C, align(8))]
struct IndexEntry {
    used: bool,
    // The number of live references to this slot
    refcount: u16,
    // The offset of the slot in the content region
    offset: u32,
    // The allocated length of the slot in the content region. This remains constant for the life
    // of the slot, even if it is freed and re-used.
    allocated: u32,
    // The actual in-use length of the data. When a slot is freed, this is set to 0.
    used_bytes: u32,

    _pin: std::marker::PhantomPinned,
}

impl IndexEntry {
    // Update accounting for the presence of a new entry.
    fn alloc(
        mut self: Pin<&mut Self>,
        content_position: usize,
        content_length: usize,
        tuple_size: usize,
    ) {
        // The content must be always rounded up to the nearest 8-byte boundary.
        assert_eq!(
            content_position % 8,
            0,
            "content position {} is not 8-byte aligned",
            content_position
        );
        assert_eq!(
            content_length % 8,
            0,
            "content length {} is not 8-byte aligned",
            content_length
        );
        unsafe {
            let index_entry = self.as_mut().get_unchecked_mut();
            index_entry.offset = content_position as u32;

            // Net-new slots always have their full size used in their new index entry.
            index_entry.used_bytes = tuple_size as u32;
            index_entry.allocated = content_length as u32;
            index_entry.refcount = 0;
            index_entry.used = true;
        }
    }

    // Mark a previously free entry as used.
    #[allow(dead_code)] // Not used right now because find_fit usage is commented out.
    fn mark_used(mut self: Pin<&mut Self>, size: usize) {
        unsafe {
            let entry = self.as_mut().get_unchecked_mut();
            entry.used = true;
            entry.used_bytes = size as u32;
        }
    }

    // Mark a previously used entry as free, and return the number of bytes it was using.
    fn mark_free(mut self: Pin<&mut Self>) -> usize {
        unsafe {
            let index_entry = self.as_mut().get_unchecked_mut();
            let used_bytes = index_entry.used_bytes as usize;
            index_entry.used = false;
            index_entry.used_bytes = 0;
            index_entry.refcount = 0;
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
    pub(crate) base_address: *mut u8,
    pub(crate) page_size: u32,

    _marker: std::marker::PhantomData<&'a u8>,
}

/// The size in bytes this page would be if completely empty.
pub fn slot_page_empty_size(page_size: usize) -> usize {
    page_size - slot_page_overhead()
}

pub const fn slot_page_overhead() -> usize {
    std::mem::size_of::<PageHeader>()
}

pub const fn slot_index_overhead() -> usize {
    std::mem::size_of::<IndexEntry>()
}

impl<'a> SlottedPage<'a> {
    pub fn for_page(base_address: *const u8, page_size: usize) -> PageReadGuard<'a> {
        Self::as_page(base_address, page_size).read_lock()
    }

    pub fn for_page_mut(base_address: *mut u8, page_size: usize) -> PageWriteGuard<'a> {
        Self::as_page_mut(base_address, page_size).write_lock()
    }

    fn as_page(base_address: *const u8, page_size: usize) -> Self {
        Self {
            base_address: base_address as *mut u8,
            page_size: page_size as u32,
            _marker: Default::default(),
        }
    }

    fn as_page_mut(base_address: *mut u8, page_size: usize) -> Self {
        Self {
            base_address,
            page_size: page_size as u32,
            _marker: Default::default(),
        }
    }

    /// How much space is available in this page?
    #[allow(dead_code)]
    pub(crate) fn free_space_bytes(&self) -> usize {
        let header = self.header();
        let used = (header.num_slots * std::mem::size_of::<IndexEntry>() as u32) as usize
            + header.used_bytes as usize
            + std::mem::size_of::<PageHeader>();
        (self.page_size as usize).saturating_sub(used)
    }

    /// How many bytes are available for appending to this page (i.e. not counting the space
    /// we could re-use, via e.g. used_bytes)
    pub(crate) fn available_content_bytes(&self) -> usize {
        let header = self.header();
        let content_length = header.content_length as usize;
        let index_length = header.index_length as usize;
        let header_size = std::mem::size_of::<PageHeader>();

        let consumed = index_length + content_length + header_size;
        (self.page_size as usize).saturating_sub(consumed)
    }

    /// Add the slot into the page, copying it into the memory region, and returning the slot id
    /// and the number of bytes remaining in the page.
    fn allocate(
        &self,
        size: usize,
        initial_value: Option<&[u8]>,
    ) -> Result<(SlotId, usize, Pin<&'a mut [u8]>), TupleBoxError> {
        // See if we can use an existing slot to put the slot in, or if there's any fit at all.
        let (can_fit, fit_slot) = self.find_fit(size);
        if !can_fit {
            return Err(TupleBoxError::BoxFull(size, self.available_content_bytes()));
        }
        let header = self.header_mut();
        if let Some(fit_slot) = fit_slot {
            let content_position = self.offset_of(fit_slot).unwrap().0;
            let memory_as_slice = unsafe {
                std::slice::from_raw_parts_mut(self.base_address, self.page_size as usize)
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
            header.add_used(size);

            let slc = unsafe {
                Pin::new_unchecked(&mut memory_as_slice[content_position..content_position + size])
            };
            return Ok((fit_slot, self.available_content_bytes(), slc));
        }

        // Find position and verify that we can fit the slot.
        let current_content_length = header.content_length as usize;
        let current_index_end = header.index_length as usize;
        let content_size = (size + 7) & !7;
        let content_start_position =
            self.page_size as usize - current_content_length - content_size;

        // Align to 8-byte boundary cuz that's what we'll actually need.
        let content_start_position = (content_start_position + 7) & !7;

        // If the content start bleeds over into the index (+ our new entry), then we can't fit the slot.
        let index_entry_size = std::mem::size_of::<IndexEntry>();
        if content_start_position <= current_index_end + index_entry_size {
            return Err(TupleBoxError::BoxFull(
                size + index_entry_size,
                self.available_content_bytes(),
            ));
        }

        let memory_as_slice =
            unsafe { std::slice::from_raw_parts_mut(self.base_address, self.page_size as usize) };

        // If there's an initial value provided, copy it in.
        if let Some(initial_value) = initial_value {
            memory_as_slice[content_start_position..content_start_position + size]
                .copy_from_slice(initial_value);
        }

        // Add the index entry and expand the index region.
        let mut index_entry = self.get_index_entry_mut(header.num_slots as SlotId);
        index_entry
            .as_mut()
            .alloc(content_start_position, content_size, size);

        // Update the header to subtract the used space.
        let new_slot = header.add_entry(size);

        // Return the slot id and the number of bytes remaining to append at the end.
        let slc = unsafe {
            Pin::new_unchecked(
                &mut memory_as_slice[content_start_position..content_start_position + size],
            )
        };
        Ok((new_slot, self.available_content_bytes(), slc))
    }

    /// Load into this page from an external byte source, which is assumed to be in our page
    /// format, and then reset all refcounts to 0, clear lock state, and return the set of all valid
    /// slot IDs.
    pub(crate) fn load<LF: FnMut(Pin<&mut [u8]>)>(
        &self,
        mut lf: LF,
    ) -> Vec<(SlotId, usize, *mut u8)> {
        // First copy in the physical bytes into our address.
        let memory_as_slice = unsafe {
            Pin::new_unchecked(std::slice::from_raw_parts_mut(
                self.base_address,
                self.page_size as usize,
            ))
        };
        lf(memory_as_slice);

        // Locks from the previous use of this page are now invalid, so reset.
        let header = self.header_mut();
        header.lock_state.store(0, SeqCst);
        header.writer_wake_counter.store(0, SeqCst);

        // Now reset all the refcounts to 1, and collect the list of all active slots.,
        let mut slots = vec![];
        let num_slots = header.num_slots;
        for i in 0..num_slots {
            let mut index_entry = self.get_index_entry_mut(i as SlotId);
            if index_entry.used {
                unsafe { index_entry.as_mut().get_unchecked_mut() }.refcount = 1;
                let slot_id = i as SlotId;
                let ptr = unsafe { self.base_address.offset(index_entry.offset as isize) };
                slots.push((slot_id, index_entry.used_bytes as usize, ptr));
            }
        }
        slots
    }

    fn remove_slot(&self, slot_id: SlotId) -> Result<(usize, usize, bool), TupleBoxError> {
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

        // TODO: join adjacent free tuple slots.
        //   Likewise at insert, support splitting slots.
        let is_empty = header.used_bytes == 0;
        if is_empty {
            header.clear();
        }
        Ok((self.available_content_bytes(), slot_size, is_empty))
    }

    fn refcount(&self, slot_id: SlotId) -> Result<u16, TupleBoxError> {
        let index_entry = self.get_index_entry(slot_id);
        if !index_entry.used {
            return Err(TupleBoxError::TupleNotFound(slot_id as usize));
        }
        Ok(index_entry.refcount)
    }

    fn upcount(&self, slot_id: SlotId) -> Result<(), TupleBoxError> {
        let mut index_entry = self.get_index_entry_mut(slot_id);
        unsafe { index_entry.as_mut().get_unchecked_mut() }.refcount += 1;
        Ok(())
    }

    fn dncount(&self, slot_id: SlotId) -> Result<bool, TupleBoxError> {
        let mut index_entry = self.get_index_entry_mut(slot_id);
        unsafe { index_entry.as_mut().get_unchecked_mut() }.refcount -= 1;
        if index_entry.refcount == 0 {
            return Ok(true);
        }
        Ok(false)
    }

    #[allow(dead_code)]
    fn get_slot(&self, slot_id: SlotId) -> Result<Pin<&'a [u8]>, TupleBoxError> {
        // Check that the index is in bounds
        let num_slots = self.header().num_slots as SlotId;
        if slot_id >= num_slots {
            error!(
                "slot_id {} is out of bounds for page with {} slots",
                slot_id, num_slots
            );
            return Err(TupleBoxError::TupleNotFound(slot_id as usize));
        }

        // Read the index entry;
        let index_entry = self.get_index_entry(slot_id);
        if !index_entry.used {
            error!("slot_id {} is not used, invalid tuple", slot_id);
            return Err(TupleBoxError::TupleNotFound(slot_id as usize));
        }
        let offset = index_entry.offset as usize;
        let length = index_entry.used_bytes as usize;

        // Must be 8-byte aligned.
        assert_eq!(offset % 8, 0, "slot {} is not 8-byte aligned", slot_id);

        let memory_as_slice =
            unsafe { std::slice::from_raw_parts(self.base_address, self.page_size as usize) };
        Ok(unsafe { Pin::new_unchecked(&memory_as_slice[offset..offset + length]) })
    }

    fn get_slot_mut(&self, slot_id: SlotId) -> Result<Pin<&'a mut [u8]>, TupleBoxError> {
        // Check that the index is in bounds
        let num_slots = self.header().num_slots as SlotId;
        if slot_id >= num_slots {
            return Err(TupleBoxError::TupleNotFound(slot_id as usize));
        }

        // Read the index entry;
        let index_entry = self.get_index_entry(slot_id);
        if !index_entry.used {
            return Err(TupleBoxError::TupleNotFound(slot_id as usize));
        }
        let offset = index_entry.offset as usize;
        let length = index_entry.used_bytes as usize;

        // Must be 8-byte aligned.
        assert_eq!(offset % 8, 0, "slot {} is not 8-byte aligned", slot_id);

        assert!(
            offset + length <= self.page_size as usize,
            "slot {} is out of bounds",
            slot_id
        );

        let memory_as_slice =
            unsafe { std::slice::from_raw_parts_mut(self.base_address, self.page_size as usize) };
        Ok(unsafe { Pin::new_unchecked(&mut memory_as_slice[offset..offset + length]) })
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
                            base_address: self.base_address,
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

    pub fn write_lock<'b>(&'a mut self) -> PageWriteGuard<'b> {
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
                            base_address: self.base_address,
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

    #[inline]
    fn header(&self) -> Pin<&PageHeader> {
        // Cast the base address to a pointear to the header
        let header_ptr = self.base_address as *const PageHeader;

        unsafe { Pin::new_unchecked(&*header_ptr) }
    }

    #[inline]
    fn header_mut(&self) -> Pin<&mut PageHeader> {
        // Cast the base address to a pointer to the header
        let header_ptr = self.base_address as *mut PageHeader;

        unsafe { Pin::new_unchecked(&mut *header_ptr) }
    }

    /// Return the offset, size of the slot at the given index.
    pub(crate) fn offset_of(&self, tid: SlotId) -> Result<(usize, usize), TupleBoxError> {
        // Check that the index is in bounds
        let num_slots = self.header().num_slots as SlotId;
        if tid >= num_slots {
            return Err(TupleBoxError::TupleNotFound(tid as usize));
        }

        // Read the index entry;
        let index_entry = self.get_index_entry(tid);
        Ok((index_entry.offset as usize, index_entry.allocated as usize))
    }

    #[allow(dead_code)] // Not used right now because usage is commented out.
    fn find_fit(&self, size: usize) -> (bool, Option<SlotId>) {
        // Find the smallest possible fit by building full-scan candidate, and then sorting.
        let mut fits = vec![];
        let header = self.header();
        let num_slots = header.num_slots;
        for i in 0..num_slots {
            let index_entry = self.get_index_entry(i as SlotId);
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
            return (true, Some(*tid as SlotId));
        }

        let index_length = header.index_length as isize;
        let content_length = header.content_length as isize;
        let header_size = std::mem::size_of::<PageHeader>() as isize;
        let total_needed = index_length + content_length + header_size;

        // Align to 8-byte boundary cuz that's what we'll actually need.
        let total_needed = (total_needed + 7) & !7;

        let avail = (self.page_size as isize) - total_needed;
        if avail < size as isize {
            return (true, None);
        }
        (avail >= size as isize, None)
    }

    fn get_index_entry(&self, slot_id: SlotId) -> Pin<&IndexEntry> {
        let index_offset = std::mem::size_of::<PageHeader>()
            + ((slot_id as usize) * std::mem::size_of::<IndexEntry>());

        let base_address = self.base_address;

        unsafe {
            let slot_address = base_address.add(index_offset);

            assert_eq!(
                slot_address as usize % 8,
                0,
                "slot {} is not 8-byte aligned",
                slot_id
            );
            Pin::new_unchecked(&*(slot_address as *const IndexEntry))
        }
    }

    fn get_index_entry_mut(&self, slot_id: SlotId) -> Pin<&mut IndexEntry> {
        let index_offset = std::mem::size_of::<PageHeader>()
            + ((slot_id as usize) * std::mem::size_of::<IndexEntry>());
        let base_address = self.base_address;

        unsafe {
            let slot_address = base_address.add(index_offset);

            assert_eq!(
                slot_address as usize % 8,
                0,
                "slot {} is not 8-byte aligned",
                slot_id
            );
            Pin::new_unchecked(&mut *(slot_address as *mut IndexEntry))
        }
    }
}

pub struct PageWriteGuard<'a> {
    base_address: *mut u8,
    page_size: u32,

    _marker: std::marker::PhantomData<&'a u8>,
}

impl<'a> PageWriteGuard<'a> {
    #[inline(always)]
    #[allow(dead_code)]
    fn free_space_bytes(&self) -> usize {
        let sp = SlottedPage::as_page_mut(self.base_address, self.page_size as usize);
        sp.free_space_bytes()
    }

    #[inline(always)]
    #[allow(dead_code)]
    fn header(&self) -> Pin<&PageHeader> {
        let header_ptr = self.base_address as *const PageHeader;

        unsafe { Pin::new_unchecked(&*header_ptr) }
    }

    #[inline]
    pub fn get_slot_mut(&mut self, slot_id: SlotId) -> Result<Pin<&'a mut [u8]>, TupleBoxError> {
        let sp = SlottedPage::as_page_mut(self.base_address, self.page_size as usize);
        sp.get_slot_mut(slot_id)
    }

    #[inline]
    pub fn header_mut(&self) -> Pin<&mut PageHeader> {
        let header_ptr = self.base_address as *mut PageHeader;
        unsafe { Pin::new_unchecked(&mut *header_ptr) }
    }

    #[inline]
    pub fn allocate(
        &mut self,
        size: usize,
        initial_value: Option<&[u8]>,
    ) -> Result<(SlotId, usize, Pin<&'a mut [u8]>), TupleBoxError> {
        let sp = SlottedPage::as_page_mut(self.base_address, self.page_size as usize);
        sp.allocate(size, initial_value)
    }

    #[inline]
    pub fn remove_slot(&mut self, slot_id: SlotId) -> Result<(usize, usize, bool), TupleBoxError> {
        let sp = SlottedPage::as_page_mut(self.base_address, self.page_size as usize);
        sp.remove_slot(slot_id)
    }

    #[inline]
    pub fn load<LF: FnMut(Pin<&mut [u8]>)>(&mut self, lf: LF) -> Vec<(SlotId, usize, *mut u8)> {
        let sp = SlottedPage::as_page_mut(self.base_address, self.page_size as usize);
        sp.load(lf)
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub fn get_slot(&self, slot_id: SlotId) -> Result<Pin<&'a [u8]>, TupleBoxError> {
        let sp = SlottedPage::as_page(self.base_address, self.page_size as usize);
        sp.get_slot(slot_id)
    }

    #[inline(always)]
    pub(crate) fn upcount(&mut self, slot_id: SlotId) -> Result<(), TupleBoxError> {
        let sp = SlottedPage::as_page_mut(self.base_address, self.page_size as usize);
        sp.upcount(slot_id)
    }

    #[inline(always)]
    pub(crate) fn dncount(&mut self, slot_id: SlotId) -> Result<bool, TupleBoxError> {
        let sp = SlottedPage::as_page_mut(self.base_address, self.page_size as usize);
        sp.dncount(slot_id)
    }

    #[inline(always)]
    pub fn available_content_bytes(&self) -> usize {
        let sp = SlottedPage::as_page_mut(self.base_address, self.page_size as usize);
        sp.available_content_bytes()
    }
}

impl<'a> Drop for PageWriteGuard<'a> {
    #[inline(always)]
    fn drop(&mut self) {
        let header = self.header_mut();
        header.unlock_for_writes();
    }
}

pub struct PageReadGuard<'a> {
    base_address: *const u8,
    page_size: u32,

    _marker: std::marker::PhantomData<&'a u8>,
}

impl<'a> PageReadGuard<'a> {
    #[inline(always)]
    fn header(&self) -> Pin<&PageHeader> {
        let header_ptr = self.base_address as *const PageHeader;

        unsafe { Pin::new_unchecked(&*header_ptr) }
    }

    /// Write the header + index portion of the page into the provided buffer
    pub(crate) fn write_header(&self, buf: &mut [u8]) {
        let header = self.header();
        let total_header_index_length =
            header.index_length as usize + std::mem::size_of::<PageHeader>();
        let header_as_slice =
            unsafe { std::slice::from_raw_parts(self.base_address, total_header_index_length) };
        buf.copy_from_slice(header_as_slice);
    }

    pub(crate) fn header_size(&self) -> usize {
        let header = self.header();

        header.index_length as usize + std::mem::size_of::<PageHeader>()
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub(crate) fn get_slot(&self, slot_id: SlotId) -> Result<Pin<&'a [u8]>, TupleBoxError> {
        let sp = SlottedPage::as_page(self.base_address as *mut u8, self.page_size as usize);
        sp.get_slot(slot_id)
    }

    #[inline(always)]
    pub fn offset_of(&self, tid: SlotId) -> Result<(usize, usize), TupleBoxError> {
        let sp = SlottedPage::as_page(self.base_address as *mut u8, self.page_size as usize);
        sp.offset_of(tid)
    }

    #[inline(always)]
    pub fn refcount(&self, slot_id: SlotId) -> Result<u16, TupleBoxError> {
        let sp = SlottedPage::as_page(self.base_address as *mut u8, self.page_size as usize);
        sp.refcount(slot_id)
    }
}

impl<'a> Drop for PageReadGuard<'a> {
    #[inline(always)]
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
    use crate::paging::slotted_page::{
        slot_page_empty_size, IndexEntry, PageWriteGuard, SlotId, SlottedPage,
    };
    use crate::paging::TupleBoxError;

    fn random_fill(page: &mut PageWriteGuard) -> Vec<(SlotId, Vec<u8>)> {
        let mut collected_slots = vec![];
        loop {
            let size = rand::random::<usize>() % 100;
            let avail = page.free_space_bytes();
            let result = page.allocate(size, Some(&vec![123; size]));
            // If avail can't fit the size of the slot plus the index entry, then we should be
            // getting an error.
            if avail < size + std::mem::size_of::<IndexEntry>() {
                assert!(matches!(result, Err(TupleBoxError::BoxFull(_, _))));
                break;
            }
            // Sometimes we can cease allocation because that's how the cookie crumbles with padding,
            if matches!(result, Err(TupleBoxError::BoxFull(_, _))) {
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
        let mut page = SlottedPage::for_page_mut(page_ptr, 4096);
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
        let mut page = SlottedPage::for_page_mut(page_ptr, 4096);

        let collected_slots = random_fill(&mut page);

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
        let mut page = SlottedPage::for_page_mut(page_ptr, 4096);

        let collected_slots = random_fill(&mut page);

        // Now randomly delete, and verify that the slot is gone.
        let mut removed_slots = vec![];
        for (tid, slot) in &collected_slots {
            let retrieved = page.get_slot(*tid).unwrap();
            assert_eq!(*slot, *retrieved);
            page.remove_slot(*tid).unwrap();
            assert!(matches!(
                page.get_slot(*tid),
                Err(TupleBoxError::TupleNotFound(_))
            ));
            removed_slots.push(*tid);
        }

        // Now randomly re-fill, and verify that the slots are back.
        let new_filled_slots = random_fill(&mut page);
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
        let mut page = SlottedPage::for_page_mut(page_ptr, 4096);

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
        let everything = random_fill(&mut page);
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
        let mut page = SlottedPage::for_page_mut(page_ptr, 4096);

        // Fill the page with random slots.
        let collected_slots = random_fill(&mut page);
        // Then remove them all and verify it is now completely empty.
        let mut old_remaining = page.available_content_bytes();
        let mut last_is_empty = false;
        for (i, (tid, _)) in collected_slots.iter().enumerate() {
            let (new_remaining, _, empty) = page.remove_slot(*tid).unwrap();
            assert!(
                new_remaining >= old_remaining,
                "new_remaining {} should be >= old_remaining {} on {i}th iteration",
                new_remaining,
                old_remaining
            );
            old_remaining = new_remaining;
            last_is_empty = empty;
        }
        assert!(last_is_empty);
        assert_eq!(old_remaining, slot_page_empty_size(4096));
        assert_eq!(page.free_space_bytes(), slot_page_empty_size(4096));
    }
}
