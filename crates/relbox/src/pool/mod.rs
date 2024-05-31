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

pub use buffer_pool::MmapBufferPool;
use std::sync::atomic::AtomicPtr;

mod buffer_pool;
mod size_class;

/// The unique identifier for currently extant buffers. Buffer ids can be ephemeral.
/// It is up to the pager to map these to non-ephemeral, long lived, page identifiers (Bid) through
/// whatever means makes sense.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Bid(pub u64);

#[derive(thiserror::Error, Debug)]
pub enum BufferPoolError {
    #[error("Error in setting up the page / buffer pool: {0}")]
    InitializationError(String),

    #[error("Insufficient room in buffer pool (wanted {desired:?}, had {available:?})")]
    InsufficientRoom { desired: usize, available: usize },

    #[error("Unsupported size class (wanted {0:?})")]
    UnsupportedSize(usize),

    #[error("Invalid page access")]
    CouldNotAccess,

    #[error("Invalid tuple pointer")]
    InvalidTuplePointer,

    #[error("Invalid page")]
    InvalidPage,
}

pub trait BufferPool {
    /// Allocate a buffer of the given size.
    fn alloc(&self, size: usize) -> Result<(Bid, *mut u8, usize), BufferPoolError>;
    /// Free a buffer, completely deallocating it, by which we mean removing it from the index of
    /// used pages.
    fn free(&self, page: Bid) -> Result<(), BufferPoolError>;
    /// Check if a given buffer handle is allocated.
    fn is_allocated(&self, page: Bid) -> bool;
    /// Returns the physical pointer and page size for a page.
    fn resolve_ptr(&self, bid: Bid) -> Result<(*mut u8, usize), BufferPoolError>;
    /// Find the buffer id (bid) for a given pointer. Can be used to identify the page
    /// that a pointer belongs to, in case of page fault.
    #[allow(dead_code)] // Legitimate potential future use
    fn identify_page<T>(&self, ptr: AtomicPtr<T>) -> Result<Bid, BufferPoolError>;
    /// Get the total reserved capacity of the buffer pool.
    #[allow(dead_code)] // Legitimate potential future use
    fn capacity_bytes(&self) -> usize;
    /// Get the total usable free space in the buffer pool.
    #[allow(dead_code)] // Legitimate potential future use
    fn available_bytes(&self) -> usize;
    /// Get the total used space in the buffer pool.
    #[allow(dead_code)] // Legitimate potential future use
    fn allocated_bytes(&self) -> usize;
}
