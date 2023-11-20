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

// TODO: use a more general purpose pager (e.g. my own umbra-like buffer mgr)
// TODO: store indexes in here, too (custom paged datastructure impl)
// TODO: add fixed-size slotted page impl for Sized items, providing more efficiency.
// TODO: verify locking/concurrency safety of this thing -- loom test + stateright, or jepsen.
// TODO: there is still some really gross stuff in here about the management of free space in
//       pages in the allocator list. It's probably causing excessive fragmentation because we're
//       considering only the reported available "content" area when fitting slots, and there seems
//       to be a sporadic failure where we end up with a "Page not found" error in the allocator on
//       free, meaning the page was not found in the used pages list.

use std::io;
use std::pin::Pin;
use std::ptr::null_mut;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Mutex;

use hi_sparse_bitset::BitSetInterface;
use libc::{MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE};
use sized_chunks::SparseChunk;
use thiserror::Error;
use tracing::warn;

pub use crate::tuplebox::slots::slotted_page::SlotId;
use crate::tuplebox::slots::slotted_page::{
    slot_index_overhead, slot_page_empty_size, PageWriteGuard, SlottedPage,
};
use crate::tuplebox::RelationId;

pub type PageId = usize;
pub type TupleId = (PageId, SlotId);

/// A region of memory backed by SlottedPages.
/// Is:
///     A region of anonymously mmap'd memory, a multiple of page sizes, and logically divided up
///       into SlottedPages
///     An index/list of the pages that have free space.
///     Each slot accessing by a unique id which is a combination of its page index and slot index.
///     When a page is totally empty, madvise DONTNEED is called on it, so that the OS can free it
///       from process RSS.
pub struct SlotBox {
    /// The base address of the mmap'd region.
    base_address: AtomicPtr<u8>,
    page_size: usize,
    allocator: Mutex<Allocator>,
}

#[derive(Debug, Clone, Error)]
pub enum SlotBoxError {
    #[error("Page is full, cannot insert slot of size {0} with {1} bytes remaining")]
    BoxFull(usize, usize),
    #[error("Tuple not found at index {0}")]
    TupleNotFound(usize),
}

impl SlotBox {
    pub fn new(page_size: usize, virt_size: usize) -> Self {
        assert!(virt_size % page_size == 0 && virt_size >= 64);

        // Allocate (virtual) memory region using mmap.
        let base_addr = unsafe {
            libc::mmap64(
                null_mut(),
                virt_size,
                PROT_READ | PROT_WRITE,
                MAP_ANONYMOUS | MAP_PRIVATE,
                -1,
                0,
            )
        };

        if base_addr == libc::MAP_FAILED {
            let err = io::Error::last_os_error();
            panic!("Mmap failed for size class virt_size {virt_size}: {err}");
        }

        Self {
            base_address: AtomicPtr::new(base_addr as *mut u8),
            page_size,
            allocator: Mutex::new(Allocator::new(virt_size / page_size)),
        }
    }

    /// Allocates a new slot for a tuple, somewhere in one of the pages we managed.
    /// Does not allow tuples from different relations to mix on the same page.
    pub fn allocate(
        &self,
        size: usize,
        relation_id: RelationId,
        initial_value: Option<&[u8]>,
    ) -> Result<TupleId, SlotBoxError> {
        assert!(size <= (slot_page_empty_size(self.page_size)));

        let mut allocator = self.allocator.lock().unwrap();
        let needed_space = size + slot_index_overhead();
        let (pid, offset) = allocator.find_space(
            relation_id,
            needed_space,
            slot_page_empty_size(self.page_size),
        )?;
        let mut page_handle = self.page_for(pid);
        let free_space = page_handle.available_content_bytes();
        // assert!(free_space >= size);
        let mut page_write_lock = page_handle.write_lock();
        if let Ok((slot_id, page_remaining, _)) = page_write_lock.allocate(size, initial_value) {
            allocator.finish_alloc(pid, relation_id, offset, page_remaining);
            return Ok((pid, slot_id));
        }

        // If we get here, then we failed to allocate on the page we wanted to, which means there's
        // data coherence issues between the pages last-reported free space and the actual free
        panic!(
            "Page {} failed to allocate, we wanted {} bytes, but it only has {},\
                but our records show it has {}, and its pid in that offset is {}",
            pid,
            size,
            free_space,
            allocator.available_page_space[relation_id.0][offset].0,
            allocator.available_page_space[relation_id.0][offset].1
        );
    }

    pub fn remove(&self, id: TupleId) -> Result<(), SlotBoxError> {
        let mut page_handle = self.page_for(id.0);
        let mut write_lock = page_handle.write_lock();
        self.do_remove(&mut write_lock, id)
    }

    pub fn upcount(&self, id: TupleId) -> Result<(), SlotBoxError> {
        let page_handle = self.page_for(id.0);
        page_handle.upcount(id.1)
    }

    pub fn dncount(&self, id: TupleId) -> Result<(), SlotBoxError> {
        let page_handle = self.page_for(id.0);
        if page_handle.dncount(id.1)? {
            self.remove(id)?;
        }
        Ok(())
    }

    fn do_remove(&self, page_lock: &mut PageWriteGuard, id: TupleId) -> Result<(), SlotBoxError> {
        let (new_free, _, is_empty) = page_lock.remove_slot(id.1)?;

        // Update record in allocator.
        let mut allocator = self.allocator.lock().unwrap();
        allocator.report_free(id.0, new_free, is_empty);

        // And if the page is completely free, then we can madvise DONTNEED it and let the OS free
        // it from our RSS.
        if is_empty {
            unsafe {
                let result = libc::madvise(
                    page_lock.page_ptr() as _,
                    self.page_size,
                    libc::MADV_DONTNEED,
                );
                assert_eq!(result, 0, "madvise failed");
            }
        }
        Ok(())
    }

    pub fn get(&self, id: TupleId) -> Result<Pin<&[u8]>, SlotBoxError> {
        let page_handle = self.page_for(id.0);
        let lock = page_handle.read_lock();

        let slc = lock.get_slot(id.1)?;
        Ok(slc)
    }

    pub fn update(
        &self,
        relation_id: RelationId,
        id: TupleId,
        new_value: &[u8],
    ) -> Result<TupleId, SlotBoxError> {
        // This lock scope has to be limited here, or we'll deadlock if we need to re-allocate.
        {
            let mut page_handle = self.page_for(id.0);

            // If the value size is the same as the old value, we can just update in place, otherwise
            // it's a brand new allocation, and we have to remove the old one first.
            let mut page_write = page_handle.write_lock();
            let mut existing = page_write.get_slot_mut(id.1).expect("Invalid tuple id");
            // let mut existing = page_handle.get_mut(id.1).expect("Invalid tuple id");
            if existing.len() == new_value.len() {
                existing.copy_from_slice(new_value);
                return Ok(id);
            }
            self.do_remove(&mut page_write, id)?;
        }
        let new_id = self.allocate(new_value.len(), relation_id, Some(new_value))?;
        Ok(new_id)
    }

    pub fn update_with<F: FnMut(Pin<&mut [u8]>)>(
        &self,
        id: TupleId,
        mut f: F,
    ) -> Result<(), SlotBoxError> {
        let mut page_handle = self.page_for(id.0);

        let mut page_write = page_handle.write_lock();
        let existing = page_write.get_slot_mut(id.1).expect("Invalid tuple id");
        f(existing);
        Ok(())
    }

    pub fn num_pages(&self) -> usize {
        let allocator = self.allocator.lock().unwrap();
        allocator.available_page_space.len()
    }

    pub fn used_pages(&self) -> Vec<PageId> {
        let allocator = self.allocator.lock().unwrap();
        allocator
            .available_page_space
            .iter()
            .flatten()
            .map(|(_, pid)| *pid)
            .collect()
    }

    pub fn mark_page_used(&self, relation_id: RelationId, free_space: usize, pid: PageId) {
        let mut allocator = self.allocator.lock().unwrap();

        let Some(available_page_space) = allocator.available_page_space.get_mut(relation_id.0)
        else {
            allocator
                .available_page_space
                .insert(relation_id.0, vec![(free_space, pid)]);
            return;
        };

        // allocator.bitmap.insert(pid as usize);
        available_page_space.push((free_space, pid));
        available_page_space.sort_by(|a, b| a.0.cmp(&b.0));
    }
}

impl SlotBox {
    pub fn page_for<'a>(&self, page_num: usize) -> SlottedPage<'a> {
        let base_address = self.base_address.load(SeqCst);
        let page_address = unsafe { base_address.add(page_num * self.page_size) };
        let page_handle = SlottedPage::for_page(AtomicPtr::new(page_address), self.page_size);
        page_handle
    }
}

fn find_empty<B: BitSetInterface>(bs: &B) -> usize {
    let mut iter = bs.iter();

    let mut pos: Option<usize> = None;
    // Scan forward until we find the first empty bit.
    loop {
        match iter.next() {
            Some(bit) => {
                let p: usize = bit;
                if bit != 0 && !bs.contains(p - 1) {
                    return p - 1;
                }
                pos = Some(p);
            }
            // Nothing in the set, or we've reached the end.
            None => {
                let Some(pos) = pos else {
                    return 0;
                };

                return pos + 1;
            }
        }
    }
}

struct Allocator {
    max_pages: usize,
    // TODO: could keep two separate vectors here -- one with the page sizes, separate for the page
    //   ids, so that SIMD can be used to used to search and sort.
    //   Will look into it once/if benchmarking justifies it.
    // The set of used pages, indexed by relation, in sorted order of the free space available in them.
    available_page_space: SparseChunk<Vec<(usize, PageId)>, 64>,
    bitmap: hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_128bit>,
}

impl Allocator {
    fn new(max_pages: usize) -> Self {
        Self {
            max_pages,
            available_page_space: SparseChunk::new(),
            bitmap: Default::default(),
        }
    }

    /// Find room to allocate a new slot of the given size, does not do the actual allocation yet,
    /// just finds the page to allocate it on.
    /// Returns the page id, and the offset into the `available_page_space` vector for that relation.
    fn find_space(
        &mut self,
        relation_id: RelationId,
        bytes: usize,
        empty_size: usize,
    ) -> Result<(PageId, usize), SlotBoxError> {
        // Do we have a used pages set for this relation? If not, we can start one, and allocate a
        // new full page to it, and return. When we actually do the allocation, we'll be able to
        // find the page in the used pages set.
        let Some(available_page_space) = self.available_page_space.get_mut(relation_id.0) else {
            let pid = find_empty(&self.bitmap);
            if pid >= self.max_pages {
                return Err(SlotBoxError::BoxFull(bytes, 0));
            }
            self.available_page_space
                .insert(relation_id.0, vec![(empty_size, pid)]);
            self.bitmap.insert(pid);
            return Ok((pid, 0));
        };

        // Look for the first page with enough space in our vector of used pages, which is kept
        // sorted by free space.
        let found = available_page_space.binary_search_by(|(free_space, _)| free_space.cmp(&bytes));

        return match found {
            // Exact match, highly unlikely, but possible.
            Ok(entry_num) => Ok((available_page_space[entry_num].1, entry_num)),
            // Out of room, need to allocate a new page.
            Err(position) if position == available_page_space.len() => {
                // If we didn't find a page with enough space, then we need to allocate a new page.
                // Find first empty position in the bitset.
                let first_empty = find_empty(&self.bitmap);
                assert!(!self.bitmap.contains(first_empty));
                assert!(!available_page_space
                    .iter().any(|(_, p)| *p == first_empty));
                if first_empty >= self.max_pages {
                    return Err(SlotBoxError::BoxFull(bytes, 0));
                }

                let pid = first_empty as PageId;
                available_page_space.push((empty_size, pid));

                Ok((pid, available_page_space.len() - 1))
            }
            // Found a page we add to.
            Err(entry_num) => {
                let entry = available_page_space.get_mut(entry_num).unwrap();
                Ok((entry.1, entry_num))
            }
        };
    }

    fn finish_alloc(
        &mut self,
        pid: PageId,
        relation_id: RelationId,
        offset: usize,
        page_remaining_bytes: usize,
    ) {
        let available_page_space = &mut self.available_page_space[relation_id.0];
        let entry = &mut available_page_space[offset];
        assert!(entry.0 >= page_remaining_bytes);
        assert_eq!(entry.1, pid);

        entry.0 = page_remaining_bytes;
        // If we (unlikely) consumed all the bytes, then we can remove the page from the avail pages
        // set.
        if entry.0 == 0 {
            available_page_space.remove(offset);
        }
        self.bitmap.insert(pid);
        available_page_space.sort_by(|a, b| a.0.cmp(&b.0));
    }

    fn report_free(&mut self, pid: PageId, new_size: usize, is_empty: bool) {
        // Seek the page in the available_page_space vectors, and add the bytes back to its free space.
        // We don't know the relation id here, so we have to linear scan all of them.
        for available_page_space in self.available_page_space.iter_mut() {
            let Some(found) = available_page_space.iter_mut().find(|(_, p)| *p == pid) else {
                continue;
            };

            found.0 = new_size;

            // If the page is now totally empty, then we can remove it from the available_page_space vector.
            if is_empty {
                available_page_space.retain(|(_, p)| *p != pid);
                self.bitmap.remove(pid);
            }
            available_page_space.sort_by(|a, b| a.0.cmp(&b.0));

            return;
        }

        warn!(
            "Page not found in used pages in allocator on free; pid {}",
            pid
        );
    }
}

#[cfg(test)]
mod tests {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    use crate::tuplebox::slots::slotbox::{SlotBox, SlotBoxError, TupleId};
    use crate::tuplebox::slots::slotted_page::{slot_page_empty_size, slot_page_overhead};
    use crate::tuplebox::RelationId;

    fn fill_until_full(sb: &mut SlotBox) -> Vec<(TupleId, Vec<u8>)> {
        let mut tuples = Vec::new();

        // fill until full... (SlotBoxError::BoxFull)
        loop {
            let mut rng = thread_rng();
            let tuple_len = rng.gen_range(1..(slot_page_empty_size(30000) - slot_page_overhead()));
            let tuple: Vec<u8> = rng.sample_iter(&Alphanumeric).take(tuple_len).collect();
            match sb.allocate(tuple.len(), RelationId(0), Some(&tuple)) {
                Ok(tuple_id) => {
                    tuples.push((tuple_id, tuple));
                }
                Err(SlotBoxError::BoxFull(_, _)) => {
                    break;
                }
                Err(e) => {
                    panic!("Unexpected error: {:?}", e);
                }
            }
        }
        tuples
    }
    // Generate a pile of random sized tuples (which accumulate to more than a single page size),
    // and then scan back and verify their presence/equality.
    #[test]
    fn test_basic_add_fill_etc() {
        let mut sb = SlotBox::new(32768, 32768 * 64);
        let tuples = fill_until_full(&mut sb);
        for (id, tuple) in &tuples {
            let retrieved = sb.get(*id).unwrap();
            assert_eq!(*tuple, *retrieved);
        }
        let used_pages = sb.used_pages();
        assert_ne!(used_pages.len(), tuples.len());
        // Now free them all the tuples.
        for (id, _tuple) in tuples {
            sb.remove(id).unwrap();
        }
    }

    // Verify that filling our box up and then emptying it out again works. Should end up with
    // everything mmap DONTNEED'd, and we should be able to re-fill it again, too.
    #[test]
    fn test_full_fill_and_empty() {
        let mut sb = SlotBox::new(32768, 32768 * 64);
        let tuples = fill_until_full(&mut sb);
        for (id, _) in &tuples {
            sb.remove(*id).unwrap();
        }
        // Verify that everything is gone.
        for (id, _) in tuples {
            assert!(sb.get(id).is_err());
        }
    }

    // Fill a box with tuples, then go and free some random ones, verify their non-presence, then
    // fill back up again and verify the new presence.
    #[test]
    fn test_fill_and_free_and_refill_etc() {
        let mut sb = SlotBox::new(32768, 32768 * 64);
        let mut tuples = fill_until_full(&mut sb);
        let mut rng = thread_rng();
        let mut freed_tuples = Vec::new();
        for _ in 0..tuples.len() / 2 {
            let idx = rng.gen_range(0..tuples.len());
            let (id, tuple) = tuples.remove(idx);
            sb.remove(id).unwrap();
            freed_tuples.push((id, tuple));
        }
        // What we expected to still be there is there.
        for (id, tuple) in &tuples {
            let retrieved = sb.get(*id).unwrap();
            assert_eq!(*tuple, *retrieved);
        }
        // What we expected to not be there is not there.
        for (id, _) in freed_tuples {
            assert!(sb.get(id).is_err());
        }
        // Now fill back up again.
        let new_tuples = fill_until_full(&mut sb);
        // Verify both the new tuples and the old tuples are there.
        for (id, tuple) in new_tuples {
            let retrieved = sb.get(id).unwrap();
            assert_eq!(tuple, *retrieved);
        }
        for (id, tuple) in tuples {
            let retrieved = sb.get(id).unwrap();
            assert_eq!(tuple, *retrieved);
        }
    }
}
