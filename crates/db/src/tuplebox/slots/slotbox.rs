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

// TODO: use a more general purpose pager (e.g. my own pagebox)
// TODO: persist pages & support >mem DBs (WAL -> disk, LRU page out, etc)
// TODO: store indexes in here, too (custom paged datastructure impl)
// TODO: add fixed-size slotted page impl for Sized items, providing efficiency
// TODO: verify locking/concurrency safety of this thing -- loom test + stateright?

use std::io;
use std::pin::Pin;
use std::ptr::null_mut;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Mutex;

use libc::{MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE};
use thiserror::Error;

use crate::tuplebox::slots::slotted_page::{
    slot_page_empty_size, slot_page_overhead, PageWriteGuard, SlotId, SlottedPage,
};

type PageId = usize;
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

    pub fn allocate(
        &self,
        size: usize,
        initial_value: Option<&[u8]>,
    ) -> Result<TupleId, SlotBoxError> {
        assert!(size <= (slot_page_empty_size(self.page_size) - slot_page_overhead()));

        let mut allocator = self.allocator.lock().unwrap();
        let pid = allocator.allocate(
            size + slot_page_overhead(),
            slot_page_empty_size(self.page_size),
        )?;
        let mut page_handle = self.page_for(pid);
        let mut write_lock = page_handle.write_lock();
        let insert_result = write_lock.allocate(size, initial_value)?;
        Ok((pid, insert_result.0))
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
        page_handle.dncount(id.1)
    }

    fn do_remove(&self, write_lock: &mut PageWriteGuard, id: TupleId) -> Result<(), SlotBoxError> {
        let (_, used, is_empty) = write_lock.remove_slot(id.1)?;

        // Remove from allocator.
        let mut allocator = self.allocator.lock().unwrap();
        allocator.free(
            id.0,
            used + slot_page_overhead(),
            slot_page_empty_size(self.page_size),
        );

        // And if the page is completely free, then we can madvise DONTNEED it and let the OS free
        // it from our RSS.
        if is_empty {
            unsafe {
                let result = libc::madvise(
                    write_lock.page_ptr() as _,
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

    pub fn update(&self, id: TupleId, new_value: &[u8]) -> Result<TupleId, SlotBoxError> {
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
        let new_id = self.allocate(new_value.len(), Some(new_value))?;
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
}

impl SlotBox {
    fn page_for<'a>(&self, page_num: usize) -> SlottedPage<'a> {
        let base_address = self.base_address.load(SeqCst);
        let page_address = unsafe { base_address.add(page_num * self.page_size) };
        let page_handle = SlottedPage::for_page(AtomicPtr::new(page_address), self.page_size);
        page_handle
    }
}

struct Allocator {
    max_pages: usize,
    // TODO: could keep two separate vectors here -- one with the page sizes, separate for the page
    //   ids, so that SIMD can be used to used to search and sort.
    //   Will look into it once/if benchmarking justifies it.
    used_pages: Vec<(usize, PageId)>,
    next_page: usize,
}

impl Allocator {
    fn new(max_pages: usize) -> Self {
        Self {
            max_pages,
            used_pages: Vec::new(),
            next_page: 0,
        }
    }

    fn allocate(&mut self, bytes: usize, empty_size: usize) -> Result<PageId, SlotBoxError> {
        // Look for the first page with enough space in our vector of used pages, which is kept
        // sorted by free space.
        let found = self
            .used_pages
            .binary_search_by(|(free_space, _)| free_space.cmp(&bytes));

        let pid = match found {
            // Exact match, highly unlikely, but possible.
            Ok(entry_num) => {
                let entry = self.used_pages.remove(entry_num);
                entry.1
            }
            // Out of room, need to allocate a new page.
            Err(position) if position == self.used_pages.len() => {
                // If we didn't find a page with enough space, then we need to allocate a new page.
                if self.next_page >= self.max_pages {
                    return Err(SlotBoxError::BoxFull(bytes, 0));
                }
                let pid = self.next_page;
                self.next_page += 1;
                self.used_pages.push((empty_size - bytes, pid));
                pid
            }
            // Found a page we can split up.
            Err(entry_num) => {
                let entry = self.used_pages.get_mut(entry_num).unwrap();
                assert!(entry.0 >= bytes);
                entry.0 -= bytes;
                entry.1
            }
        };

        self.used_pages.sort_by(|a, b| a.0.cmp(&b.0));

        return Ok(pid);
    }

    fn free(&mut self, pid: PageId, bytes: usize, empty_size: usize) {
        // Seek the page in the used_pages vector, and add the bytes back to its free space.
        // If the page is now totally empty, then we can remove it from the used_pages vector.
        let found = self
            .used_pages
            .iter_mut()
            .find(|(_, p)| *p == pid)
            .expect("Page not found");
        found.0 += bytes;
        if found.0 == empty_size {
            self.used_pages.retain(|(_, p)| *p != pid);
        }
        self.used_pages.sort_by(|a, b| a.0.cmp(&b.0));
    }
}

#[cfg(test)]
mod tests {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    use crate::tuplebox::slots::slotbox::{SlotBox, SlotBoxError, TupleId};
    use crate::tuplebox::slots::slotted_page::{slot_page_empty_size, slot_page_overhead};

    fn fill_until_full(sb: &mut SlotBox) -> Vec<(TupleId, Vec<u8>)> {
        let mut tuples = Vec::new();

        // fill until full... (SlotBoxError::BoxFull)
        loop {
            let mut rng = thread_rng();
            let tuple_len = rng.gen_range(1..(slot_page_empty_size(32768) - slot_page_overhead()));
            let tuple: Vec<u8> = rng.sample_iter(&Alphanumeric).take(tuple_len).collect();
            match sb.allocate(tuple.len(), Some(&tuple)) {
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
        for (id, tuple) in tuples {
            let retrieved = sb.get(id).unwrap();
            assert_eq!(tuple, *retrieved);
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
