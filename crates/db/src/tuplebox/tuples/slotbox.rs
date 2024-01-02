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

// TODO: implement a more general purpose pager that handles LRU eviction
//       and so can be used for larger-than-ram datasets (means adding a pagetable for Pid->Bid)
// TODO: store indexes in here, too (custom paged datastructure impl)
// TODO: add fixed-size slotted page impl for Sized items, providing more efficiency.
// TODO: verify locking/concurrency safety of this thing -- loom test + stateright, or jepsen.
// TODO: there is still some really gross stuff in here about the management of free space in
//       pages in the allocator list. It's probably causing excessive fragmentation because we're
//       considering only the reported available "content" area when fitting slots, and there seems
//       to be a sporadic failure where we end up with a "Page not found" error in the allocator on
//       free, meaning the page was not found in the used pages list.
// TODO: improve TupleRef so it can hold a direct address to the tuple, and not just an id.
//       some swizzling will probably be required.  (though at this point we're never paging
//       tuples out, so we may not need to swizzle). avoiding the lookup on every reference
//       should improve performance massively.
// TODO: rename me, _I_ am the tuplebox. The "slots" are just where my tuples get stored.

use std::cmp::max;
use std::pin::Pin;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Mutex;

use sized_chunks::SparseChunk;
use thiserror::Error;
use tracing::error;

use crate::tuplebox::pool::{Bid, BufferPool, PagerError};
pub use crate::tuplebox::tuples::slotted_page::SlotId;
use crate::tuplebox::tuples::slotted_page::{
    slot_index_overhead, slot_page_empty_size, SlottedPage,
};
use crate::tuplebox::RelationId;

pub type PageId = usize;
pub type TupleId = (PageId, SlotId);

/// A SlotBox is a collection of (variable sized) pages, each of which is a collection of slots, each of which is holds
/// dynamically sized tuples.
pub struct SlotBox {
    inner: Mutex<Inner>,
}

#[derive(Debug, Clone, Error)]
pub enum SlotBoxError {
    #[error("Page is full, cannot insert slot of size {0} with {1} bytes remaining")]
    BoxFull(usize, usize),
    #[error("Tuple not found at index {0}")]
    TupleNotFound(usize),
}

impl SlotBox {
    pub fn new(virt_size: usize) -> Self {
        let pool = BufferPool::new(virt_size).expect("Could not create buffer pool");
        let inner = Mutex::new(Inner::new(pool));
        Self { inner }
    }

    /// Allocates a new slot for a tuple, somewhere in one of the pages we managed.
    /// Does not allow tuples from different relations to mix on the same page.
    pub fn allocate(
        &self,
        size: usize,
        relation_id: RelationId,
        initial_value: Option<&[u8]>,
    ) -> Result<TupleId, SlotBoxError> {
        // Pick a buffer pool size. If the tuples are small, we use a reasonable sized page that could in theory hold
        // a few tuples, but if the tuples are large, we use a page size that might hold only one or two.
        // This way really large values can be slotted into the correct page size.
        let tuple_size = size + slot_index_overhead();
        let page_size = max(32768, tuple_size.next_power_of_two());

        let needed_space = size + slot_index_overhead();

        let mut inner = self.inner.lock().unwrap();
        // Check if we have a free spot for this relation that can fit the tuple.
        let (pid, offset) =
            { inner.find_space(relation_id, needed_space, slot_page_empty_size(page_size))? };

        let mut page_handle = inner.page_for(pid)?;

        let free_space = page_handle.available_content_bytes();
        let mut page_write_lock = page_handle.write_lock();
        if let Ok((slot_id, page_remaining, _)) = page_write_lock.allocate(size, initial_value) {
            inner.finish_alloc(pid, relation_id, offset, page_remaining);
            return Ok((pid, slot_id));
        }

        // If we get here, then we failed to allocate on the page we wanted to, which means there's
        // data coherence issues between the pages last-reported free space and the actual free
        panic!(
            "Page {} failed to allocate, we wanted {} bytes, but it only has {},\
                but our records show it has {}, and its pid in that offset is {:?}",
            pid,
            size,
            free_space,
            inner.available_page_space[relation_id.0][offset].available,
            inner.available_page_space[relation_id.0][offset].bid
        );
    }

    pub fn remove(&self, id: TupleId) -> Result<(), SlotBoxError> {
        let mut inner = self.inner.lock().unwrap();
        inner.do_remove(id)
    }

    pub fn restore<'a>(&self, id: PageId) -> Result<SlottedPage<'a>, SlotBoxError> {
        let inner = self.inner.lock().unwrap();
        let (addr, page_size) = match inner.pool.restore(Bid(id as u64)) {
            Ok(v) => v,
            Err(PagerError::CouldNotAccess) => {
                return Err(SlotBoxError::TupleNotFound(id));
            }
            Err(e) => {
                panic!("Unexpected buffer pool error: {:?}", e);
            }
        };

        Ok(SlottedPage::for_page(addr, page_size))
    }

    pub fn page_for<'a>(&self, id: PageId) -> Result<SlottedPage<'a>, SlotBoxError> {
        let inner = self.inner.lock().unwrap();
        inner.page_for(id)
    }

    pub fn upcount(&self, id: TupleId) -> Result<(), SlotBoxError> {
        let inner = self.inner.lock().unwrap();
        let page_handle = inner.page_for(id.0)?;
        page_handle.upcount(id.1)
    }

    pub fn dncount(&self, id: TupleId) -> Result<(), SlotBoxError> {
        let mut inner = self.inner.lock().unwrap();
        let page_handle = inner.page_for(id.0)?;
        if page_handle.dncount(id.1)? {
            inner.do_remove(id)?;
        }
        Ok(())
    }

    pub fn get(&self, id: TupleId) -> Result<Pin<&[u8]>, SlotBoxError> {
        let inner = self.inner.lock().unwrap();
        let page_handle = inner.page_for(id.0)?;

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
        // The lock scope has to be limited here, or we'll deadlock if we need to re-allocate.
        {
            let mut inner = self.inner.lock().unwrap();
            let mut page_handle = inner.page_for(id.0)?;

            // If the value size is the same as the old value, we can just update in place, otherwise
            // it's a brand new allocation, and we have to remove the old one first.
            let mut page_write = page_handle.write_lock();
            let mut existing = page_write.get_slot_mut(id.1).expect("Invalid tuple id");
            // let mut existing = page_handle.get_mut(id.1).expect("Invalid tuple id");
            if existing.len() == new_value.len() {
                existing.copy_from_slice(new_value);
                return Ok(id);
            }
            inner.do_remove(id)?;
        }
        let new_id = self.allocate(new_value.len(), relation_id, Some(new_value))?;
        Ok(new_id)
    }

    pub fn update_with<F: FnMut(Pin<&mut [u8]>)>(
        &self,
        id: TupleId,
        mut f: F,
    ) -> Result<(), SlotBoxError> {
        let inner = self.inner.lock().unwrap();
        let mut page_handle = inner.page_for(id.0)?;

        let mut page_write = page_handle.write_lock();
        let existing = page_write.get_slot_mut(id.1).expect("Invalid tuple id");
        f(existing);
        Ok(())
    }

    pub fn num_pages(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.available_page_space.len()
    }

    pub fn used_pages(&self) -> Vec<PageId> {
        let allocator = self.inner.lock().unwrap();
        allocator
            .available_page_space
            .iter()
            .flatten()
            .map(
                |PageSpace {
                     available: _,
                     bid: pid,
                 }| pid.0 as PageId,
            )
            .collect()
    }

    pub fn mark_page_used(&self, relation_id: RelationId, free_space: usize, pid: PageId) {
        let mut allocator = self.inner.lock().unwrap();

        let bid = Bid(pid as u64);
        let Some(available_page_space) = allocator.available_page_space.get_mut(relation_id.0)
        else {
            allocator.available_page_space.insert(
                relation_id.0,
                vec![PageSpace {
                    available: free_space,
                    bid,
                }],
            );
            return;
        };

        // allocator.bitmap.insert(pid as usize);
        available_page_space.push(PageSpace {
            available: free_space,
            bid,
        });
        available_page_space.sort_by(|a, b| a.available.cmp(&b.available));
    }
}

struct PageSpace {
    available: usize,
    bid: Bid,
}

struct Inner {
    pool: BufferPool,
    // TODO: could keep two separate vectors here -- one with the page sizes, separate for the page
    //   ids, so that SIMD can be used to used to search and sort.
    //   Will look into it once/if benchmarking justifies it.
    // The set of used pages, indexed by relation, in sorted order of the free space available in them.
    available_page_space: SparseChunk<Vec<PageSpace>, 64>,
}

impl Inner {
    fn new(pool: BufferPool) -> Self {
        Self {
            available_page_space: SparseChunk::new(),
            pool,
        }
    }

    fn do_remove(&mut self, id: TupleId) -> Result<(), SlotBoxError> {
        let mut page_handle = self.page_for(id.0)?;
        let mut write_lock = page_handle.write_lock();

        let (new_free, _, is_empty) = write_lock.remove_slot(id.1)?;
        self.report_free(id.0, new_free, is_empty);

        Ok(())
    }

    fn page_for<'a>(&self, page_num: usize) -> Result<SlottedPage<'a>, SlotBoxError> {
        let (page_address, page_size) = match self.pool.resolve_ptr(Bid(page_num as u64)) {
            Ok(v) => v,
            Err(PagerError::CouldNotAccess) => {
                return Err(SlotBoxError::TupleNotFound(page_num));
            }
            Err(e) => {
                panic!("Unexpected buffer pool error: {:?}", e);
            }
        };
        let page_address = page_address.load(SeqCst);
        let page_handle = SlottedPage::for_page(AtomicPtr::new(page_address), page_size);
        Ok(page_handle)
    }

    fn alloc(
        &mut self,
        relation_id: RelationId,
        page_size: usize,
    ) -> Result<(PageId, usize), SlotBoxError> {
        // Ask the buffer pool for a new page of the given size.
        let (bid, _, actual_size) = match self.pool.alloc(page_size) {
            Ok(v) => v,
            Err(PagerError::InsufficientRoom { desired, available }) => {
                return Err(SlotBoxError::BoxFull(desired, available));
            }
            Err(e) => {
                panic!("Unexpected buffer pool error: {:?}", e);
            }
        };
        match self.available_page_space.get_mut(relation_id.0) {
            Some(available_page_space) => {
                available_page_space.push(PageSpace {
                    available: slot_page_empty_size(actual_size),
                    bid,
                });
                available_page_space.sort_by(|a, b| a.available.cmp(&b.available));
                Ok((bid.0 as PageId, available_page_space.len() - 1))
            }
            None => {
                self.available_page_space.insert(
                    relation_id.0,
                    vec![PageSpace {
                        available: slot_page_empty_size(actual_size),
                        bid,
                    }],
                );
                Ok((bid.0 as PageId, 0))
            }
        }
    }

    /// Find room to allocate a new tuple of the given size, does not do the actual allocation yet,
    /// just finds the page to allocate it on.
    /// Returns the page id, and the offset into the `available_page_space` vector for that relation.
    fn find_space(
        &mut self,
        relation_id: RelationId,
        tuple_size: usize,
        page_size: usize,
    ) -> Result<(PageId, usize), SlotBoxError> {
        // Do we have a used pages set for this relation? If not, we can start one, and allocate a
        // new full page to it, and return. When we actually do the allocation, we'll be able to
        // find the page in the used pages set.
        let Some(available_page_space) = self.available_page_space.get_mut(relation_id.0) else {
            // Ask the buffer pool for a new buffer.
            return self.alloc(relation_id, page_size);
        };

        // Look for the first page with enough space in our vector of used pages, which is kept
        // sorted by free space.
        let found = available_page_space.binary_search_by(
            |PageSpace {
                 available: free_space,
                 bid: _,
             }| free_space.cmp(&tuple_size),
        );

        return match found {
            // Exact match, highly unlikely, but possible.
            Ok(entry_num) => {
                let exact_match = (available_page_space[entry_num].bid, entry_num);
                let pid = exact_match.0 .0 as PageId;
                Ok((pid, entry_num))
            }
            // Out of room, need to allocate a new page.
            Err(position) if position == available_page_space.len() => {
                // If we didn't find a page with enough space, then we need to allocate a new page.
                return self.alloc(relation_id, page_size);
            }
            // Found a page we add to.
            Err(entry_num) => {
                let entry = available_page_space.get_mut(entry_num).unwrap();
                Ok((entry.bid.0 as PageId, entry_num))
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
        assert!(entry.available >= page_remaining_bytes);
        assert_eq!(entry.bid.0, pid as u64);

        entry.available = page_remaining_bytes;
        // If we (unlikely) consumed all the bytes, then we can remove the page from the avail pages
        // set.
        if entry.available == 0 {
            available_page_space.remove(offset);
        }
        available_page_space.sort_by(|a, b| a.available.cmp(&b.available));
    }

    fn report_free(&mut self, pid: PageId, new_size: usize, is_empty: bool) {
        // Seek the page in the available_page_space vectors, and add the bytes back to its free space.
        // We don't know the relation id here, so we have to linear scan all of them.
        for available_page_space in self.available_page_space.iter_mut() {
            let Some(found) = available_page_space.iter_mut().find(
                |PageSpace {
                     available: _,
                     bid: p,
                 }| p.0 == pid as u64,
            ) else {
                continue;
            };

            found.available = new_size;

            // If the page is now totally empty, then we can remove it from the available_page_space vector.
            if is_empty {
                available_page_space.retain(
                    |PageSpace {
                         available: _,
                         bid: p,
                     }| p.0 != pid as u64,
                );
                self.pool
                    .free(Bid(pid as u64))
                    .expect("Could not free page");
            }
            available_page_space.sort_by(|a, b| a.available.cmp(&b.available));

            return;
        }

        error!(
            "Page not found in used pages in allocator on free; pid {}; could be double-free, dangling weak reference?",
            pid
        );
    }
}

#[cfg(test)]
mod tests {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    use crate::tuplebox::tuples::slotbox::{SlotBox, SlotBoxError, TupleId};
    use crate::tuplebox::tuples::slotted_page::slot_page_empty_size;
    use crate::tuplebox::RelationId;

    fn fill_until_full(sb: &mut SlotBox) -> Vec<(TupleId, Vec<u8>)> {
        let mut tuples = Vec::new();

        // fill until full... (SlotBoxError::BoxFull)
        loop {
            let mut rng = thread_rng();
            let tuple_len = rng.gen_range(1..(slot_page_empty_size(52000)));
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

    // Just allocate a single tuple, and verify that we can retrieve it.
    #[test]
    fn test_one_page_one_slot() {
        let sb = SlotBox::new(32768 * 64);
        let tuple = vec![1, 2, 3, 4, 5];
        let tuple_id = sb
            .allocate(tuple.len(), RelationId(0), Some(&tuple))
            .unwrap();
        let retrieved = sb.get(tuple_id).unwrap();
        assert_eq!(tuple, *retrieved);
    }

    // Fill just one page and verify that we can retrieve all the tuples.
    #[test]
    fn test_one_page_a_few_slots() {
        let sb = SlotBox::new(32768 * 64);
        let mut tuples = Vec::new();
        let mut last_page_id = None;
        loop {
            let mut rng = thread_rng();
            let tuple_len = rng.gen_range(1..128);
            let tuple: Vec<u8> = rng.sample_iter(&Alphanumeric).take(tuple_len).collect();
            let tuple_id = sb
                .allocate(tuple.len(), RelationId(0), Some(&tuple))
                .unwrap();
            if let Some(last_page_id) = last_page_id {
                if last_page_id != tuple_id.0 {
                    break;
                }
            }
            last_page_id = Some(tuple_id.0);
            tuples.push((tuple_id, tuple));
        }
        for (id, tuple) in tuples {
            let retrieved = sb.get(id).unwrap();
            assert_eq!(tuple, *retrieved);
        }
    }

    // Fill one page, then overflow into another, and verify we can get the tuple that's on the next page.
    #[test]
    fn test_page_overflow() {
        let sb = SlotBox::new(32768 * 64);
        let mut tuples = Vec::new();
        let mut first_page_id = None;
        let (next_page_tuple_id, next_page_tuple) = loop {
            let mut rng = thread_rng();
            let tuple_len = rng.gen_range(1..128);
            let tuple: Vec<u8> = rng.sample_iter(&Alphanumeric).take(tuple_len).collect();
            let tuple_id = sb
                .allocate(tuple.len(), RelationId(0), Some(&tuple))
                .unwrap();
            if let Some(last_page_id) = first_page_id {
                if last_page_id != tuple_id.0 {
                    break (tuple_id, tuple);
                }
            }
            first_page_id = Some(tuple_id.0);
            tuples.push((tuple_id, tuple));
        };
        for (id, tuple) in tuples {
            let retrieved = sb.get(id).unwrap();
            assert_eq!(tuple, *retrieved);
        }
        // Now verify that the last tuple was on another, new page, and that we can retrieve it.
        assert_ne!(next_page_tuple_id.0, first_page_id.unwrap());
        let retrieved = sb.get(next_page_tuple_id).unwrap();
        assert_eq!(*retrieved, next_page_tuple);
    }

    // Generate a pile of random sized tuples (which accumulate to more than a single page size),
    // and then scan back and verify their presence/equality.
    #[test]
    fn test_basic_add_fill_etc() {
        let mut sb = SlotBox::new(32768 * 32);
        let tuples = fill_until_full(&mut sb);
        for (i, (id, tuple)) in tuples.iter().enumerate() {
            let retrieved = sb.get(*id).unwrap();
            assert_eq!(*tuple, *retrieved, "Mismatch at {}th tuple", i);
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
        let mut sb = SlotBox::new(32768 * 64);
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
        let mut sb = SlotBox::new(32768 * 64);
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
