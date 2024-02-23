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

use human_bytes::human_bytes;
use libc::{madvise, MADV_DONTNEED, MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE};
use tracing::info;

use crate::pool::PagerError;

type BitSet = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_128bit>;

pub struct SizeClass {
    pub block_size: usize,
    pub base_addr: *mut u8,
    pub virt_size: usize,
    free_list: Vec<usize>,
    allocset: BitSet,
    highest_block: usize,

    // stats
    num_blocks_used: u32,
}

unsafe impl Send for SizeClass {}
unsafe impl Sync for SizeClass {}

impl SizeClass {
    pub fn new_anon(block_size: usize, virt_size: usize) -> Result<Self, PagerError> {
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
            return Err(PagerError::InitializationError(format!(
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

            free_list: vec![],
            allocset: BitSet::new(),
            highest_block: 0,

            num_blocks_used: 0,
        })
    }

    pub fn alloc(&mut self) -> Result<usize, PagerError> {
        // Check the free list first.
        if let Some(blocknum) = self.free_list.pop() {
            self.allocset.insert(blocknum);
            self.num_blocks_used += 1;
            return Ok(blocknum);
        }

        let blocknum = self.highest_block;
        self.highest_block += 1;

        if blocknum >= self.virt_size / self.block_size {
            return Err(PagerError::InsufficientRoom {
                desired: self.block_size,
                available: self.available(),
            });
        }

        self.allocset.insert(blocknum);
        self.num_blocks_used += 1;
        Ok(blocknum)
    }

    pub fn free(&mut self, blocknum: usize) -> Result<(), PagerError> {
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
        self.allocset.remove(blocknum);
        self.free_list.push(blocknum);
        self.num_blocks_used += 1;
        Ok(())
    }

    #[allow(dead_code)] // Legitimate potential future use
    pub fn page_out(&mut self, blocknum: usize) -> Result<(), PagerError> {
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
        self.allocset.remove(blocknum);
        self.num_blocks_used += 1;
        Ok(())
    }

    pub fn is_allocated(&self, blocknum: usize) -> bool {
        self.allocset.contains(blocknum)
    }

    pub fn bytes_used(&self) -> usize {
        self.num_blocks_used as usize
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
