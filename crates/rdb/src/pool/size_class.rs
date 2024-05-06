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

use std::io;
use std::ptr::null_mut;
use std::sync::atomic::AtomicUsize;
use std::sync::Mutex;

use human_bytes::human_bytes;
use libc::{madvise, MADV_DONTNEED, MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE};
use tracing::info;

use crate::pool::BufferPoolError;

type BitSet = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_128bit>;

pub struct SizeClass {
    pub block_size: usize,
    pub base_addr: *mut u8,
    pub virt_size: usize,
    free_list: crossbeam_queue::ArrayQueue<usize>,
    allocset: Mutex<BitSet>,
    highest_block: AtomicUsize,

    // stats
    num_blocks_used: AtomicUsize,
}

unsafe impl Send for SizeClass {}
unsafe impl Sync for SizeClass {}

impl SizeClass {
    pub fn new_anon(block_size: usize, virt_size: usize) -> Result<Self, BufferPoolError> {
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
            return Err(BufferPoolError::InitializationError(format!(
                "Mmap failed for size class block_size: {block_size}, virt_size {virt_size}: {err}"
            )));
        }

        info!(
            "Mapped {:?} bytes at {:?} for size class {}",
            human_bytes(virt_size as f64),
            base_addr,
            human_bytes(block_size as f64),
        );

        let base_addr = base_addr.cast::<u8>();

        // Build the bitmap index
        Ok(Self {
            block_size,
            base_addr,
            virt_size,

            free_list: crossbeam_queue::ArrayQueue::new(256),
            allocset: Mutex::new(BitSet::new()),
            highest_block: AtomicUsize::new(0),

            num_blocks_used: AtomicUsize::new(0),
        })
    }

    pub fn alloc(&self) -> Result<usize, BufferPoolError> {
        // Check the free list first.
        if let Some(blocknum) = self.free_list.pop() {
            let mut allocset = self.allocset.lock().unwrap();
            allocset.insert(blocknum);
            self.num_blocks_used
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(blocknum);
        }

        let blocknum = self
            .highest_block
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        if blocknum >= self.virt_size / self.block_size {
            return Err(BufferPoolError::InsufficientRoom {
                desired: self.block_size,
                available: self.available(),
            });
        }

        let mut allocset = self.allocset.lock().unwrap();
        allocset.insert(blocknum);
        self.num_blocks_used
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(blocknum)
    }

    pub fn free(&self, blocknum: usize) -> Result<(), BufferPoolError> {
        unsafe {
            let base_addr = self.base_addr;
            let addr = base_addr.offset(blocknum as isize * self.block_size as isize);
            // Panic on fail here because this working is a fundamental invariant that we cannot
            // recover from.
            let madv_resp = madvise(addr.cast(), self.block_size, MADV_DONTNEED);
            if madv_resp != 0 {
                panic!(
                    "MADV_DONTNEED failed, errno: {}",
                    io::Error::last_os_error()
                );
            }
        }
        let mut allocset = self.allocset.lock().unwrap();
        allocset.remove(blocknum);
        // Attempt to push to the free list, unless it's full.
        // If so, that's ok, that's just an optimization, and we'll hunt for free blocks manually
        // when it's empty
        self.free_list.push(blocknum).ok();
        self.num_blocks_used
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    #[allow(dead_code)] // Legitimate potential future use
    pub fn page_out(&mut self, blocknum: usize) -> Result<(), BufferPoolError> {
        unsafe {
            let addr = self.base_addr;
            // Panic on fail here because this working is a fundamental invariant that we cannot
            // recover from.
            let madv_result = madvise(
                addr.offset(blocknum as isize * self.block_size as isize)
                    .cast(),
                self.block_size,
                MADV_DONTNEED,
            );
            if madv_result != 0 {
                panic!(
                    "MADV_DONTNEED failed, errno: {}",
                    io::Error::last_os_error()
                );
            }
        }
        let mut allocset = self.allocset.lock().unwrap();
        allocset.remove(blocknum);
        self.num_blocks_used
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    pub fn is_allocated(&self, blocknum: usize) -> bool {
        let allocset = self.allocset.lock().unwrap();
        allocset.contains(blocknum)
    }

    pub fn bytes_used(&self) -> usize {
        self.num_blocks_used
            .load(std::sync::atomic::Ordering::Relaxed)
            * self.block_size
    }

    pub fn available(&self) -> usize {
        self.virt_size - self.bytes_used()
    }
}

impl Drop for SizeClass {
    fn drop(&mut self) {
        let result = unsafe {
            let base_addr = self.base_addr;
            libc::munmap(
                base_addr.cast::<libc::c_void>(),
                self.virt_size as libc::size_t,
            )
        };

        if result != 0 {
            let err = io::Error::last_os_error();
            panic!("Unable to munmap buffer pool: {err}");
        }
    }
}
