// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! Pool allocator for Vec<T> backing storage using mmap and bitmap tracking

use libc::{MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, PROT_READ, PROT_WRITE, mmap, munmap};
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::ptr::{self, NonNull};
use std::rc::Rc;

/// Helper for safe length management during fill operations
/// Sets length on drop to ensure consistency even if clone() panics
struct SetLenOnDrop<'a> {
    len: &'a mut usize,
    local_len: usize,
}

impl<'a> SetLenOnDrop<'a> {
    fn new(len: &'a mut usize) -> Self {
        SetLenOnDrop {
            local_len: *len,
            len,
        }
    }

    fn increment_len(&mut self, increment: usize) {
        self.local_len += increment;
    }
}

impl Drop for SetLenOnDrop<'_> {
    fn drop(&mut self) {
        *self.len = self.local_len;
    }
}

/// Size classes for vector backing storage: 16, 32, 64, 128 elements
const SIZE_CLASSES: [usize; 4] = [16, 32, 64, 128];
const MAX_SIZE_CLASS: usize = 3;
/// Lowest size class is 16 = 2^4
const LOWEST_SIZE_CLASS_POWER: u32 = 4;
/// Minimum allocation granularity (16 elements)
const MIN_GRANULARITY: usize = SIZE_CLASSES[0];
/// Default pool size in number of minimum-granularity chunks
const DEFAULT_POOL_CHUNKS: usize = 8192; // 128KB worth of 16-element chunks

/// Represents a free chunk in the intrusive free list
/// The "next" pointer is stored directly in the freed memory
#[repr(C)]
struct FreeChunk {
    next: Option<NonNull<FreeChunk>>,
}

/// A pool for managing pre-allocated vector backing storage using mmap
pub struct VecPool<T> {
    /// Raw mmap-allocated memory region
    memory_region: *mut MaybeUninit<T>,
    /// Size of the mmap region in bytes
    region_size_bytes: usize,
    /// Total capacity in minimum-granularity chunks
    total_chunks: usize,
    /// Intrusive free list heads for each size class
    size_class_free_heads: [Option<NonNull<FreeChunk>>; 4],
    /// Next chunk to try for bump allocation
    next_free_chunk: usize,
    /// Allocation tracking
    allocated_bytes: usize,
    available_bytes: usize,
}

/// Encode chunk offset and size class into a buffer index
/// Lower 2 bits store size class, upper bits store chunk offset
fn encode_buffer_idx(chunk_offset: usize, size_class: usize) -> usize {
    debug_assert!(size_class < 4);
    (chunk_offset << 2) | size_class
}

/// Decode buffer index into chunk offset and size class
fn decode_buffer_idx(buffer_idx: usize) -> (usize, usize) {
    let size_class = buffer_idx & 0x3;
    let chunk_offset = buffer_idx >> 2;
    (chunk_offset, size_class)
}

impl<T> VecPool<T> {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_POOL_CHUNKS)
    }

    pub fn with_capacity(total_chunks: usize) -> Self {
        let region_size_bytes = total_chunks * MIN_GRANULARITY * std::mem::size_of::<T>();

        // Allocate mmap region
        let memory_region = unsafe {
            let ptr = mmap(
                std::ptr::null_mut(),
                region_size_bytes,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            );

            if ptr == MAP_FAILED {
                panic!("Failed to mmap memory region for VecPool");
            }

            ptr as *mut MaybeUninit<T>
        };

        let mut pool = Self {
            memory_region,
            region_size_bytes,
            total_chunks,
            size_class_free_heads: [None, None, None, None],
            next_free_chunk: 0,
            allocated_bytes: 0,
            available_bytes: region_size_bytes,
        };

        // Pre-populate free lists to avoid runtime allocation overhead
        pool.prepopulate_free_lists();
        pool
    }

    fn prepopulate_free_lists(&mut self) {
        // Pre-allocate a small number of buffers for each size class
        // Just enough to avoid the initial bump allocator calls
        let prealloc_counts = [4, 2, 1, 1]; // Much more conservative

        for (size_class, &count) in prealloc_counts.iter().enumerate() {
            let chunks_needed = SIZE_CLASSES[size_class] / MIN_GRANULARITY;

            for _ in 0..count {
                if self.next_free_chunk + chunks_needed <= self.total_chunks {
                    let chunk_offset = self.next_free_chunk;
                    self.next_free_chunk += chunks_needed;
                    // Push to intrusive free list
                    self.push_free_chunk(size_class, chunk_offset);
                } else {
                    break; // Out of space for this size class
                }
            }
        }
    }

    fn size_class_for_capacity(capacity: usize) -> usize {
        if capacity == 0 {
            return 0; // Use smallest size class for empty requests
        }

        // Find next power of 2 >= capacity using bit twiddling
        let np2 = (64 - (capacity - 1).leading_zeros()) as isize;
        let sc_idx = np2 - (LOWEST_SIZE_CLASS_POWER as isize);
        let sc_idx = std::cmp::max(sc_idx, 0) as usize;

        // Clamp to maximum size class
        std::cmp::min(sc_idx, MAX_SIZE_CLASS)
    }

    /// Push a chunk onto the intrusive free list for a size class
    fn push_free_chunk(&mut self, size_class: usize, chunk_offset: usize) {
        let element_offset = chunk_offset * MIN_GRANULARITY;
        let chunk_ptr = unsafe { self.memory_region.add(element_offset) as *mut FreeChunk };

        unsafe {
            // Store the current head as the next pointer in this chunk
            (*chunk_ptr).next = self.size_class_free_heads[size_class];
            // Make this chunk the new head
            self.size_class_free_heads[size_class] = Some(NonNull::new_unchecked(chunk_ptr));
        }
    }

    /// Pop a chunk from the intrusive free list for a size class
    fn pop_free_chunk(&mut self, size_class: usize) -> Option<usize> {
        let head = self.size_class_free_heads[size_class]?;

        unsafe {
            // Get the next chunk from the current head
            let next = (*head.as_ptr()).next;
            // Update the head to point to the next chunk
            self.size_class_free_heads[size_class] = next;

            // Calculate the chunk offset from the pointer
            let chunk_ptr = head.as_ptr() as *mut MaybeUninit<T>;
            let element_offset = chunk_ptr.offset_from(self.memory_region) as usize;
            let chunk_offset = element_offset / MIN_GRANULARITY;

            Some(chunk_offset)
        }
    }

    fn allocate_backing(&mut self, capacity: usize) -> (NonNull<MaybeUninit<T>>, usize, usize) {
        let size_class = Self::size_class_for_capacity(capacity);
        let chunks_needed = SIZE_CLASSES[size_class] / MIN_GRANULARITY;

        // Fast path: Try intrusive free list first
        if let Some(chunk_offset) = self.pop_free_chunk(size_class) {
            debug_assert!(chunk_offset + chunks_needed <= self.total_chunks);

            let buffer_idx = encode_buffer_idx(chunk_offset, size_class);
            let element_offset = chunk_offset * MIN_GRANULARITY;
            let ptr = unsafe { NonNull::new_unchecked(self.memory_region.add(element_offset)) };

            let bytes = SIZE_CLASSES[size_class] * std::mem::size_of::<T>();
            self.allocated_bytes += bytes;
            self.available_bytes -= bytes;

            return (ptr, buffer_idx, size_class);
        }

        // Bump allocator: Try to allocate from the end of used space
        if self.next_free_chunk + chunks_needed <= self.total_chunks {
            let chunk_offset = self.next_free_chunk;
            self.next_free_chunk += chunks_needed;

            let buffer_idx = encode_buffer_idx(chunk_offset, size_class);
            let element_offset = chunk_offset * MIN_GRANULARITY;
            let ptr = unsafe { NonNull::new_unchecked(self.memory_region.add(element_offset)) };

            let bytes = SIZE_CLASSES[size_class] * std::mem::size_of::<T>();
            self.allocated_bytes += bytes;
            self.available_bytes -= bytes;

            return (ptr, buffer_idx, size_class);
        }

        // Out of memory
        panic!(
            "VecPool: insufficient memory for allocation. Pool stats: {}",
            self.debug_stats()
        );
    }

    fn deallocate_backing(&mut self, buffer_idx: usize, size_class: usize) {
        let (chunk_offset, decoded_size_class) = decode_buffer_idx(buffer_idx);
        debug_assert_eq!(size_class, decoded_size_class);

        // Add to intrusive free list
        self.push_free_chunk(size_class, chunk_offset);

        let bytes = SIZE_CLASSES[size_class] * std::mem::size_of::<T>();
        self.allocated_bytes -= bytes;
        self.available_bytes += bytes;
    }

    pub fn allocated_bytes(&self) -> usize {
        self.allocated_bytes
    }

    pub fn available_bytes(&self) -> usize {
        self.available_bytes
    }

    /// Debug method to show pool statistics
    pub fn debug_stats(&self) -> String {
        format!(
            "Pool stats: allocated={} bytes, available={} bytes, total_chunks={}, free_lists=[{}, {}, {}, {}]",
            self.allocated_bytes(),
            self.available_bytes(),
            self.total_chunks,
            self.count_free_chunks(0),
            self.count_free_chunks(1),
            self.count_free_chunks(2),
            self.count_free_chunks(3)
        )
    }

    /// Count the number of free chunks in a size class (for debugging)
    fn count_free_chunks(&self, size_class: usize) -> usize {
        let mut count = 0;
        let mut current = self.size_class_free_heads[size_class];

        unsafe {
            while let Some(chunk) = current {
                count += 1;
                current = (*chunk.as_ptr()).next;
                // Safety check to avoid infinite loops
                if count > 10000 {
                    break;
                }
            }
        }

        count
    }
}

impl<T> Drop for VecPool<T> {
    fn drop(&mut self) {
        // Unmap the memory region
        let bytes_before = self.allocated_bytes();
        unsafe {
            if munmap(
                self.memory_region as *mut libc::c_void,
                self.region_size_bytes,
            ) != 0
            {
                eprintln!("Warning: Failed to munmap VecPool memory region");
            }
        }
        if bytes_before > 0 {
            eprintln!("VecPool dropped: freed {} bytes from mmap", bytes_before);
        }
    }
}

/// Handle that identifies a pool-allocated backing buffer
/// The buffer_idx encodes both chunk offset and size class
#[derive(Debug)]
struct PoolHandle {
    buffer_idx: usize,
}

/// A Vec-like container that uses pool-allocated backing storage
pub struct PoolVec<T> {
    pool: Rc<UnsafeCell<VecPool<T>>>,
    ptr: NonNull<MaybeUninit<T>>,
    len: usize,
    capacity: usize,
    handle: PoolHandle,
}

impl<T: std::fmt::Debug> std::fmt::Debug for PoolVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PoolVec")
            .field("len", &self.len)
            .field("capacity", &self.capacity)
            .field("elements", &self.as_debug_vec())
            .finish()
    }
}

impl<T: std::fmt::Debug> PoolVec<T> {
    fn as_debug_vec(&self) -> Vec<&T> {
        let mut vec = Vec::new();
        for i in 0..self.len {
            unsafe {
                vec.push(&*self.ptr.as_ptr().add(i).cast::<T>());
            }
        }
        vec
    }
}

/// Iterator for PoolVec
pub struct PoolVecIter<'a, T> {
    vec: &'a PoolVec<T>,
    index: usize,
}

impl<'a, T> Iterator for PoolVecIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.vec.len {
            let item = self.vec.get(self.index)?;
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

impl<T> PoolVec<T> {
    pub fn new(pool: Rc<UnsafeCell<VecPool<T>>>) -> Self {
        let (ptr, buffer_idx, size_class) = unsafe { (*pool.get()).allocate_backing(0) };
        let capacity = SIZE_CLASSES[size_class];

        Self {
            pool,
            ptr,
            len: 0,
            capacity,
            handle: PoolHandle { buffer_idx },
        }
    }

    pub fn with_capacity(pool: Rc<UnsafeCell<VecPool<T>>>, capacity: usize) -> Self {
        let (ptr, buffer_idx, size_class) = unsafe { (*pool.get()).allocate_backing(capacity) };
        let actual_capacity = SIZE_CLASSES[size_class];

        Self {
            pool,
            ptr,
            len: 0,
            capacity: actual_capacity,
            handle: PoolHandle { buffer_idx },
        }
    }

    /// Create a new PoolVec filled with `len` copies of `value` - optimized fast path
    /// Uses Vec-style optimizations: writes directly to memory and avoids needless clone of last element
    pub fn new_filled(pool: Rc<UnsafeCell<VecPool<T>>>, len: usize, value: T) -> Self
    where
        T: Clone,
    {
        let (ptr, buffer_idx, size_class) = unsafe { (*pool.get()).allocate_backing(len) };
        let actual_capacity = SIZE_CLASSES[size_class];

        if len > actual_capacity {
            panic!(
                "PoolVec::new_filled: requested length {} exceeds capacity {}",
                len, actual_capacity
            );
        }

        let mut pool_vec = Self {
            pool,
            ptr,
            len: 0,
            capacity: actual_capacity,
            handle: PoolHandle { buffer_idx },
        };

        if len > 0 {
            unsafe {
                let mut write_ptr = pool_vec.ptr.as_ptr();
                // Use SetLenOnDrop to work around potential aliasing issues
                // and ensure length is set correctly even if clone() panics
                let mut local_len = SetLenOnDrop::new(&mut pool_vec.len);

                // Write all elements except the last one with cloning
                for _ in 1..len {
                    ptr::write(write_ptr, MaybeUninit::new(value.clone()));
                    write_ptr = write_ptr.add(1);
                    local_len.increment_len(1);
                }

                // Write the last element directly without needless clone
                ptr::write(write_ptr, MaybeUninit::new(value));
                local_len.increment_len(1);

                // len set by SetLenOnDrop's Drop implementation
            }
        }

        pool_vec
    }

    pub fn from_vec(pool: Rc<UnsafeCell<VecPool<T>>>, mut vec: Vec<T>) -> Self {
        // Check size before allocating to avoid memory leaks
        let size_class = VecPool::<T>::size_class_for_capacity(vec.len());
        let capacity = SIZE_CLASSES[size_class];

        if vec.len() > capacity {
            panic!("PoolVec::from_vec: vector too large for any size class");
        }

        let (ptr, buffer_idx, _size_class) = unsafe { (*pool.get()).allocate_backing(vec.len()) };

        let mut pool_vec = Self {
            pool,
            ptr,
            len: 0,
            capacity,
            handle: PoolHandle { buffer_idx },
        };

        // Move elements from Vec to PoolVec
        for item in vec.drain(..) {
            pool_vec.push(item);
        }

        pool_vec
    }

    pub fn into_vec(self) -> Vec<T> {
        let mut vec = Vec::with_capacity(self.len);

        // Read elements directly from backing storage in correct order
        for i in 0..self.len {
            unsafe {
                let item = (*self.ptr.as_ptr().add(i)).assume_init_read();
                vec.push(item);
            }
        }

        // Manually return buffer to pool without dropping elements
        let (_, size_class) = decode_buffer_idx(self.handle.buffer_idx);
        unsafe {
            (*self.pool.get()).deallocate_backing(self.handle.buffer_idx, size_class);
        }

        // Prevent Drop from running since we've moved all elements and returned buffer
        std::mem::forget(self);

        vec
    }

    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        let mut vec = Vec::with_capacity(self.len);

        for i in 0..self.len {
            unsafe {
                let item = &*self.ptr.as_ptr().add(i).cast::<T>();
                vec.push(item.clone());
            }
        }

        vec
    }

    pub fn push(&mut self, value: T) {
        if self.len >= self.capacity {
            panic!("PoolVec capacity exceeded");
        }

        unsafe {
            self.ptr
                .as_ptr()
                .add(self.len)
                .write(MaybeUninit::new(value));
        }
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;
        unsafe { Some((*self.ptr.as_ptr().add(self.len)).assume_init_read()) }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        if self.len > 0 {
            unsafe {
                // Use Vec's approach: drop the entire slice at once
                std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(
                    self.ptr.as_ptr().cast::<T>(),
                    self.len,
                ));
            }
            self.len = 0;
        }
    }

    pub fn truncate(&mut self, len: usize) {
        if len >= self.len {
            return;
        }

        unsafe {
            // Drop the tail elements as a slice (Vec's approach)
            let drop_ptr = self.ptr.as_ptr().add(len).cast::<T>();
            let drop_len = self.len - len;
            std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(drop_ptr, drop_len));
        }
        self.len = len;
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }

        unsafe { Some(&*self.ptr.as_ptr().add(index).cast::<T>()) }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }

        unsafe { Some(&mut *self.ptr.as_ptr().add(index).cast::<T>()) }
    }

    /// Get element without bounds check - faster for Index trait
    pub fn get_unchecked(&self, index: usize) -> &T {
        debug_assert!(index < self.len);
        unsafe { &*self.ptr.as_ptr().add(index).cast::<T>() }
    }

    /// Get mutable element without bounds check - faster for IndexMut trait
    pub fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        debug_assert!(index < self.len);
        unsafe { &mut *self.ptr.as_ptr().add(index).cast::<T>() }
    }

    pub fn last(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    pub fn last_mut(&mut self) -> Option<&mut T> {
        if self.len == 0 {
            None
        } else {
            let idx = self.len - 1;
            self.get_mut(idx)
        }
    }

    /// Resize the vector, filling with default values if growing
    pub fn resize(&mut self, new_len: usize, value: T)
    where
        T: Clone,
    {
        if new_len > self.capacity {
            panic!("PoolVec resize beyond capacity");
        }

        if new_len > self.len {
            // Growing - use optimized extend_with
            let extend_count = new_len - self.len;
            self.extend_with(extend_count, value);
        } else {
            // Shrinking - truncate
            self.truncate(new_len);
        }
    }

    /// Extend the vector with `n` copies of `value` using Vec-style optimizations
    pub fn extend_with(&mut self, n: usize, value: T)
    where
        T: Clone,
    {
        if n == 0 {
            return;
        }

        if self.len + n > self.capacity {
            panic!("PoolVec extend_with beyond capacity");
        }

        unsafe {
            let mut write_ptr = self.ptr.as_ptr().add(self.len);
            // Use SetLenOnDrop to ensure length consistency if clone() panics
            let mut local_len = SetLenOnDrop::new(&mut self.len);

            // Write all elements except the last one with cloning
            for _ in 1..n {
                ptr::write(write_ptr, MaybeUninit::new(value.clone()));
                write_ptr = write_ptr.add(1);
                local_len.increment_len(1);
            }

            // Write the last element directly without needless clone
            ptr::write(write_ptr, MaybeUninit::new(value));
            local_len.increment_len(1);

            // len set by SetLenOnDrop's Drop implementation
        }
    }

    /// Return an iterator over the elements
    pub fn iter(&self) -> PoolVecIter<T> {
        PoolVecIter {
            vec: self,
            index: 0,
        }
    }

    /// Return a mutable slice for interfacing with existing code
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.len == 0 {
            return &mut [];
        }

        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr().cast::<T>(), self.len) }
    }

    /// Return the underlying pool for creating new PoolVecs with the same pool
    pub fn pool(&self) -> &std::rc::Rc<std::cell::UnsafeCell<VecPool<T>>> {
        &self.pool
    }
}

impl<T> Drop for PoolVec<T> {
    fn drop(&mut self) {
        // Drop all initialized elements
        self.clear();

        // Return backing buffer to pool
        let (_, size_class) = decode_buffer_idx(self.handle.buffer_idx);
        unsafe {
            (*self.pool.get()).deallocate_backing(self.handle.buffer_idx, size_class);
        }
    }
}

unsafe impl<T: Send> Send for PoolVec<T> {}

impl<T: Clone> Clone for PoolVec<T> {
    fn clone(&self) -> Self {
        // Create a new PoolVec with the same pool and capacity as the original
        let mut new_vec = PoolVec::with_capacity(self.pool.clone(), self.len);

        for i in 0..self.len {
            unsafe {
                let item = &*self.ptr.as_ptr().add(i).cast::<T>();
                new_vec.push(item.clone());
            }
        }

        new_vec
    }
}

impl<T: PartialEq> PartialEq for PoolVec<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }

        for i in 0..self.len {
            unsafe {
                let self_item = &*self.ptr.as_ptr().add(i).cast::<T>();
                let other_item = &*other.ptr.as_ptr().add(i).cast::<T>();
                if self_item != other_item {
                    return false;
                }
            }
        }

        true
    }
}

// SAFETY: VecPool is Send when T is Send because:
// 1. Each Task owns its pools exclusively (no sharing between threads)
// 2. Pools are moved atomically with the Task between threads
// 3. No concurrent access - only the owning thread accesses the pool
unsafe impl<T: Send> Send for VecPool<T> {}

/// A Send wrapper around Rc<UnsafeCell<VecPool<T>>> for per-Task pools
/// SAFETY: This is only safe because each Task exclusively owns its pools
/// and Tasks are moved atomically between threads without sharing pools
pub struct TaskVecPool<T>(Rc<UnsafeCell<VecPool<T>>>);

impl<T> TaskVecPool<T> {
    pub fn new() -> Self {
        Self(Rc::new(UnsafeCell::new(VecPool::new())))
    }

    pub fn inner(&self) -> &Rc<UnsafeCell<VecPool<T>>> {
        &self.0
    }
}

impl<T> Default for TaskVecPool<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for TaskVecPool<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for TaskVecPool<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TaskVecPool({:?})", self.0)
    }
}

// SAFETY: TaskVecPool is Send because:
// 1. Each Task exclusively owns its pools (no sharing between Tasks)
// 2. The entire Task (including all its pools) moves atomically between threads
// 3. No concurrent access to the same pool from multiple threads
// 4. RefCell provides runtime borrow checking within the single owning thread
unsafe impl<T: Send> Send for TaskVecPool<T> {}

impl<T> std::ops::Index<usize> for PoolVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get_unchecked(index)
    }
}

impl<T> std::ops::IndexMut<usize> for PoolVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_unchecked_mut(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let pool = Rc::new(UnsafeCell::new(VecPool::new()));
        let mut vec = PoolVec::new(pool.clone());

        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());

        vec.push(42);
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0], 42);

        vec.push(84);
        assert_eq!(vec.len(), 2);
        assert_eq!(vec[1], 84);

        assert_eq!(vec.pop(), Some(84));
        assert_eq!(vec.len(), 1);

        assert_eq!(vec.pop(), Some(42));
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
    }

    #[test]
    fn test_size_class_selection() {
        assert_eq!(VecPool::<i32>::size_class_for_capacity(8), 0);
        assert_eq!(VecPool::<i32>::size_class_for_capacity(16), 0);
        assert_eq!(VecPool::<i32>::size_class_for_capacity(17), 1);
        assert_eq!(VecPool::<i32>::size_class_for_capacity(32), 1);
        assert_eq!(VecPool::<i32>::size_class_for_capacity(33), 2);
        assert_eq!(VecPool::<i32>::size_class_for_capacity(128), 3);
        assert_eq!(VecPool::<i32>::size_class_for_capacity(200), 3);
    }

    #[test]
    fn test_pool_reuse() {
        let pool = Rc::new(UnsafeCell::new(VecPool::new()));

        // Allocate and drop a vector
        {
            let mut vec1 = PoolVec::with_capacity(pool.clone(), 16);
            vec1.push(1);
            vec1.push(2);
        }

        // Allocate another - should reuse the backing buffer
        let mut vec2 = PoolVec::with_capacity(pool.clone(), 16);
        vec2.push(3);
        assert_eq!(vec2[0], 3);
    }
}
