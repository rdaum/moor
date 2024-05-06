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

//! Vaguely `Umbra` inspired bufferpool.
//!
//! Buffers are allocated via anonymous memory mapping, and `MADV_DONTNEED` to page them out, using
//! a variant of VM overcommit to allow for more allocations than physical memory while keeping
//! consistent memory addresses to avoid a giant page table.
//!
//! As in Umbra, the "same" physical memory is allocated in multiple memory mapped regions, to permit
//! multiple page sizes. By this we mean, multiple size classes are allocated in multiple mmap pools
//! and as long as the sum of all *used* pages remains lower than physical memory, we can allocate
//! freely without worrying about complicated page splitting strategies.
//!
//! For now each sice class is using a simple bitmap index to manage allocation + a free list to
//! manage block allocation.

use std::cmp::max;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crate::pool::size_class::SizeClass;
use crate::pool::{Bid, BufferPool, BufferPoolError};

// 32k -> 1MB page sizes supported.
// TODO: Handle storage of big-values / big-pages / blobs
//       If we end up with values bigger than 1MB, they should probably be handled by "external" pages,
//       that is, pages that are not part of the buffer pool, but are instead read directly from file
//       references as needed, because they are likely to just thrash the crap out of the buffer pool
//       and are probably just big binary blogs like images, etc.
pub const LOWEST_SIZE_CLASS_POWER_OF: usize = 12;
pub const HIGHEST_SIZE_CLASS_POWER_OF: usize = 20;

pub struct MmapBufferPool {
    // Statistics.
    pub capacity_bytes: AtomicUsize,
    pub allocated_bytes: AtomicUsize,
    pub available_bytes: AtomicUsize,
    pub size_classes: [SizeClass; HIGHEST_SIZE_CLASS_POWER_OF - LOWEST_SIZE_CLASS_POWER_OF + 1],
}

impl MmapBufferPool {
    pub fn new(capacity: usize) -> Result<Self, BufferPoolError> {
        let region_4k = SizeClass::new_anon(1 << 12, capacity)?;
        let region_8k = SizeClass::new_anon(1 << 13, capacity)?;
        let region_16k = SizeClass::new_anon(1 << 14, capacity)?;
        let region_32k = SizeClass::new_anon(1 << 15, capacity)?;
        let region_64k = SizeClass::new_anon(1 << 16, capacity)?;
        let region_128k = SizeClass::new_anon(1 << 17, capacity)?;
        let region_256k = SizeClass::new_anon(1 << 18, capacity)?;
        let region_512k = SizeClass::new_anon(1 << 19, capacity)?;
        let region_1024k = SizeClass::new_anon(1 << 20, capacity)?;

        let size_classes = [
            region_4k,
            region_8k,
            region_16k,
            region_32k,
            region_64k,
            region_128k,
            region_256k,
            region_512k,
            region_1024k,
        ];
        Ok(Self {
            capacity_bytes: AtomicUsize::new(capacity),
            allocated_bytes: AtomicUsize::new(0),
            available_bytes: AtomicUsize::new(capacity),

            size_classes,
        })
    }

    pub fn newbid(offset: usize, size_class: u8) -> Bid {
        // Verify that the offset is aligned such that we can encode the size class in the lower 4
        // bits.
        assert_eq!(offset & 0b11, 0);

        // Verify that the size class fits in 4 bits.
        assert!(size_class < 16);

        // Size class gets encoded into the lower 4 bits.
        let bid = offset as u64 | u64::from(size_class);

        Bid(bid)
    }

    fn offset_of(bid: Bid) -> usize {
        // Offset is our value with the lower 4 bits masked out.
        bid.0 as usize & !0b1111
    }

    fn size_class_of(bid: Bid) -> u8 {
        // Size class is the lower 4 bits.
        (bid.0 & 0b1111) as u8
    }

    // Legitimate potential future use
    #[allow(dead_code)]
    pub fn page_size_of(bid: Bid) -> usize {
        1 << ((Self::size_class_of(bid) as usize) + LOWEST_SIZE_CLASS_POWER_OF)
    }

    #[allow(dead_code)]
    pub fn page_size_for_size_class(size_class: u8) -> usize {
        1 << ((size_class as usize) + LOWEST_SIZE_CLASS_POWER_OF)
    }
}

impl BufferPool for MmapBufferPool {
    /// Allocate a buffer of the given size.
    fn alloc(&self, size: usize) -> Result<(Bid, *mut u8, usize), BufferPoolError> {
        if size > self.available_bytes.load(Ordering::SeqCst) {
            return Err(BufferPoolError::InsufficientRoom {
                desired: size,
                available: self.allocated_bytes.load(Ordering::SeqCst),
            });
        }

        let np2 = (64 - (size - 1).leading_zeros()) as isize;
        let sc_idx = np2 - (LOWEST_SIZE_CLASS_POWER_OF as isize);
        let sc_idx = max(sc_idx, 0) as usize;
        if sc_idx >= self.size_classes.len() {
            return Err(BufferPoolError::UnsupportedSize(1 << np2));
        }

        let nearest_class = &self.size_classes[sc_idx];
        let block_size = nearest_class.block_size;

        // Ask the size class for its offset for allocation.
        let offset = nearest_class.alloc()? * block_size;

        // Bookkeeping
        self.allocated_bytes.fetch_add(block_size, Ordering::SeqCst);
        self.available_bytes.fetch_sub(block_size, Ordering::SeqCst);

        // The bid is the offset into the buffer pool + the size class in the lower 4 bits.
        let sc_idx = sc_idx as u8;
        let bid = Self::newbid(offset, sc_idx);

        // Note that this is the actual address, that is, it does not have the size-class encoded
        // in it (aka PagePointer)
        let addr = self.resolve_ptr(bid).unwrap().0;

        // Clear.
        unsafe {
            std::ptr::write_bytes(addr, 0, block_size);
        }
        Ok((bid, addr, block_size))
    }
    /// Free a buffer, completely deallocating it, by which we mean removing it from the index of
    /// used pages.
    fn free(&self, page: Bid) -> Result<(), BufferPoolError> {
        let sc = Self::size_class_of(page);
        let sc = &self.size_classes[sc as usize];
        let block_size = sc.block_size;
        let offset = Self::offset_of(page);
        sc.free(offset / block_size)?;

        // Bookkeeping
        self.allocated_bytes.fetch_sub(block_size, Ordering::SeqCst);
        self.available_bytes.fetch_add(block_size, Ordering::SeqCst);

        Ok(())
    }
    /// Check if a given buffer handle is allocated.
    fn is_allocated(&self, page: Bid) -> bool {
        let sc_num = Self::size_class_of(page);
        let sc = &self.size_classes[sc_num as usize];
        let block_size = sc.block_size;
        let offset = Self::offset_of(page);
        sc.is_allocated(offset / block_size)
    }
    /// Returns the physical pointer and page size for a page.
    fn resolve_ptr(&self, bid: Bid) -> Result<(*mut u8, usize), BufferPoolError> {
        if !Self::is_allocated(self, bid) {
            return Err(BufferPoolError::CouldNotAccess);
        }

        let sc_num = Self::size_class_of(bid);
        let sc = &self.size_classes[sc_num as usize];
        let offset = Self::offset_of(bid);

        assert!(offset < sc.virt_size, "Offset out of bound for size class");

        let addr = sc.base_addr;
        let addr = unsafe { addr.add(offset) };

        Ok((addr as _, sc.block_size))
    }
    /// Find the buffer id (bid) for a given pointer. Can be used to identify the page
    /// that a pointer belongs to, in case of page fault.
    #[allow(dead_code)] // Legitimate potential future use
    fn identify_page<T>(&self, ptr: AtomicPtr<T>) -> Result<Bid, BufferPoolError> {
        // Look at the address ranges for each size class to find the one that contains the pointer.
        for (sc_idx, sc) in self.size_classes.iter().enumerate() {
            let base = sc.base_addr as usize;
            let end = base + sc.virt_size;
            let ptr = ptr.load(Ordering::SeqCst) as usize;
            if ptr >= base && ptr < end {
                // Found the size class that contains the pointer. Now we need to find the offset
                // within the size class.
                let offset = ptr - base;
                let offset = offset / sc.block_size;
                let offset = offset * sc.block_size;
                let bid = Self::newbid(offset, sc_idx as u8);
                return Ok(bid);
            }
        }
        Err(BufferPoolError::InvalidTuplePointer)
    }
    /// Get the total reserved capacity of the buffer pool.
    #[allow(dead_code)] // Legitimate potential future use
    fn capacity_bytes(&self) -> usize {
        self.capacity_bytes.load(Ordering::Relaxed)
    }
    /// Get the total usable free space in the buffer pool.
    #[allow(dead_code)] // Legitimate potential future use
    fn available_bytes(&self) -> usize {
        self.available_bytes.load(Ordering::Relaxed)
    }
    /// Get the total used space in the buffer pool.
    #[allow(dead_code)] // Legitimate potential future use
    fn allocated_bytes(&self) -> usize {
        self.allocated_bytes.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use crate::pool::buffer_pool::{BufferPool, MmapBufferPool, HIGHEST_SIZE_CLASS_POWER_OF};
    use crate::pool::BufferPoolError;

    const MB_256: usize = 1 << 28;

    #[test]
    fn test_empty_pool() {
        let capacity = MB_256;
        let bp = MmapBufferPool::new(capacity).unwrap();

        assert_eq!(bp.capacity_bytes(), capacity);
        assert_eq!(bp.available_bytes(), capacity);
        assert_eq!(bp.allocated_bytes(), 0);
    }

    #[test]
    fn test_buffer_allocation_perfect() {
        let capacity = MB_256;
        let bp = MmapBufferPool::new(capacity).unwrap();

        // Allocate buffers that fit just inside powers of 2, so no fragmentation will occur due to
        // rounding up nearest size.
        let buffer_sizes = [1 << 12, 1 << 14, 1 << 16, 1 << 18];
        let mut bids = Vec::new();
        for &size in &buffer_sizes {
            let bid = bp.alloc(size).unwrap().0;
            bids.push(bid);
        }

        // In this scenario, allocation will always match requested. So should be 0 fragmentation
        // so no lost bytes.
        let expected_allocated_bytes: usize = buffer_sizes.iter().sum();
        assert_eq!(bp.allocated_bytes(), expected_allocated_bytes);
        assert_eq!(bp.available_bytes(), capacity - expected_allocated_bytes);

        // Free the buffers and check that they are released
        for pid in bids {
            bp.free(pid).unwrap();
        }
        assert_eq!(bp.available_bytes(), capacity);
        assert_eq!(bp.allocated_bytes(), 0);
    }

    #[test]
    fn test_buffer_allocation_fragmented() {
        let capacity = MB_256;
        let bp = MmapBufferPool::new(capacity).unwrap();

        // Allocate buffers that fit 10 bytes under some powers of 2, so we accumulate some
        // fragmentation.
        let fb = 10;
        let buffer_sizes = [
            (1 << 12) - fb,
            (1 << 14) - fb,
            (1 << 16) - fb,
            (1 << 18) - fb,
        ];
        let mut pids = Vec::new();
        for &size in &buffer_sizes {
            let pid = bp.alloc(size).unwrap().0;
            pids.push(pid);
            assert!(bp.is_allocated(pid));
        }

        let expected_lost_bytes: usize = fb * buffer_sizes.len();
        let expected_requested_bytes: usize = buffer_sizes.iter().sum();
        let expected_allocated_bytes: usize = expected_requested_bytes + expected_lost_bytes;

        assert_eq!(bp.allocated_bytes(), expected_allocated_bytes);
        assert_eq!(bp.available_bytes(), capacity - expected_allocated_bytes);

        // Free the buffers and check that they are released
        for pid in pids {
            bp.free(pid).unwrap();
            assert!(!bp.is_allocated(pid));
        }
        assert_eq!(bp.available_bytes(), capacity);
        assert_eq!(bp.allocated_bytes(), 0);
    }

    #[test]
    fn test_error_conditions() {
        let capacity = MB_256;
        let bp = MmapBufferPool::new(capacity).unwrap();

        // Test capacity limit
        let res = bp.alloc(capacity + 1);
        assert!(matches!(res, Err(BufferPoolError::InsufficientRoom { .. })));

        // Test unsupported size class
        let res = bp.alloc(1 << (HIGHEST_SIZE_CLASS_POWER_OF + 1));
        assert!(matches!(res, Err(BufferPoolError::UnsupportedSize(_))));

        // Test unable to allocate
        let mut allocated = vec![];
        for _ in 0..bp.size_classes.len() {
            let res = bp.alloc(capacity + 1);
            match res {
                Ok(bh) => {
                    allocated.push(bh);
                }
                Err(e) => {
                    assert!(matches!(e, BufferPoolError::InsufficientRoom { .. }));
                    break;
                }
            }
        }
    }
}
