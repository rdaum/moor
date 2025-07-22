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

//! Slab allocator for Vec<T> backing storage to reduce allocation overhead

use std::cell::RefCell;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Size classes for vector backing storage: 16, 32, 64, 128 elements
const SIZE_CLASSES: [usize; 4] = [16, 32, 64, 128];
const MAX_SIZE_CLASS: usize = 3;

/// A pool for managing pre-allocated vector backing storage
#[derive(Debug)]
pub struct VecPool<T> {
    size_classes: [VecSizeClass<T>; 4],
    allocated_bytes: AtomicUsize,
    available_bytes: AtomicUsize,
}

#[derive(Debug)]
struct VecSizeClass<T> {
    buffers: Vec<Box<[MaybeUninit<T>]>>,
    free_list: Vec<usize>,
    buffer_size: usize,
}

impl<T> VecSizeClass<T> {
    fn new(buffer_size: usize) -> Self {
        Self {
            buffers: Vec::new(),
            free_list: Vec::new(),
            buffer_size,
        }
    }

    fn allocate(&mut self) -> (NonNull<MaybeUninit<T>>, usize) {
        let buffer_idx = if let Some(idx) = self.free_list.pop() {
            idx
        } else {
            // Allocate new buffer
            let mut buffer = Vec::with_capacity(self.buffer_size);
            buffer.resize_with(self.buffer_size, MaybeUninit::uninit);
            let buffer = buffer.into_boxed_slice();
            self.buffers.push(buffer);
            self.buffers.len() - 1
        };

        let buffer_ptr = NonNull::new(self.buffers[buffer_idx].as_mut_ptr()).unwrap();
        (buffer_ptr, buffer_idx)
    }

    fn deallocate(&mut self, buffer_idx: usize) {
        debug_assert!(buffer_idx < self.buffers.len());
        debug_assert!(!self.free_list.contains(&buffer_idx));
        self.free_list.push(buffer_idx);
    }
}

impl<T> VecPool<T> {
    pub fn new() -> Self {
        Self {
            size_classes: [
                VecSizeClass::new(SIZE_CLASSES[0]),
                VecSizeClass::new(SIZE_CLASSES[1]),
                VecSizeClass::new(SIZE_CLASSES[2]),
                VecSizeClass::new(SIZE_CLASSES[3]),
            ],
            allocated_bytes: AtomicUsize::new(0),
            available_bytes: AtomicUsize::new(0),
        }
    }

    fn size_class_for_capacity(capacity: usize) -> usize {
        for (i, &size) in SIZE_CLASSES.iter().enumerate() {
            if capacity <= size {
                return i;
            }
        }
        MAX_SIZE_CLASS
    }

    fn allocate_backing(&mut self, capacity: usize) -> (NonNull<MaybeUninit<T>>, usize, usize) {
        let size_class = Self::size_class_for_capacity(capacity);
        let (ptr, buffer_idx) = self.size_classes[size_class].allocate();

        let actual_capacity = SIZE_CLASSES[size_class];
        let bytes = actual_capacity * std::mem::size_of::<T>();
        self.allocated_bytes.fetch_add(bytes, Ordering::Relaxed);

        (ptr, buffer_idx, size_class)
    }

    fn deallocate_backing(&mut self, buffer_idx: usize, size_class: usize) {
        self.size_classes[size_class].deallocate(buffer_idx);

        let bytes = SIZE_CLASSES[size_class] * std::mem::size_of::<T>();
        self.allocated_bytes.fetch_sub(bytes, Ordering::Relaxed);
        self.available_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn allocated_bytes(&self) -> usize {
        self.allocated_bytes.load(Ordering::Relaxed)
    }

    pub fn available_bytes(&self) -> usize {
        self.available_bytes.load(Ordering::Relaxed)
    }
}

/// Handle that identifies a slab-allocated backing buffer
#[derive(Debug)]
struct SlabHandle {
    buffer_idx: usize,
    size_class: usize,
}

/// A Vec-like container that uses slab-allocated backing storage
pub struct SlabVec<T> {
    pool: Rc<RefCell<VecPool<T>>>,
    ptr: NonNull<MaybeUninit<T>>,
    len: usize,
    capacity: usize,
    handle: SlabHandle,
}

impl<T> SlabVec<T> {
    pub fn new(pool: Rc<RefCell<VecPool<T>>>) -> Self {
        let (ptr, buffer_idx, size_class) = pool.borrow_mut().allocate_backing(0);
        let capacity = SIZE_CLASSES[size_class];

        Self {
            pool,
            ptr,
            len: 0,
            capacity,
            handle: SlabHandle {
                buffer_idx,
                size_class,
            },
        }
    }

    pub fn with_capacity(pool: Rc<RefCell<VecPool<T>>>, capacity: usize) -> Self {
        let (ptr, buffer_idx, size_class) = pool.borrow_mut().allocate_backing(capacity);
        let actual_capacity = SIZE_CLASSES[size_class];

        Self {
            pool,
            ptr,
            len: 0,
            capacity: actual_capacity,
            handle: SlabHandle {
                buffer_idx,
                size_class,
            },
        }
    }

    pub fn from_vec(pool: Rc<RefCell<VecPool<T>>>, mut vec: Vec<T>) -> Self {
        let (ptr, buffer_idx, size_class) = pool.borrow_mut().allocate_backing(vec.len());
        let capacity = SIZE_CLASSES[size_class];
        
        if vec.len() > capacity {
            panic!("SlabVec::from_vec: vector too large for any size class");
        }

        let mut slab_vec = Self {
            pool,
            ptr,
            len: 0,
            capacity,
            handle: SlabHandle {
                buffer_idx,
                size_class,
            },
        };

        // Move elements from Vec to SlabVec
        for item in vec.drain(..) {
            slab_vec.push(item);
        }

        slab_vec
    }

    pub fn into_vec(self) -> Vec<T> {
        let mut vec = Vec::with_capacity(self.len);
        
        // Read elements directly from backing storage in correct order
        for i in 0..self.len {
            unsafe {
                let item = self.ptr.as_ptr().add(i).read().assume_init();
                vec.push(item);
            }
        }
        
        // Manually return buffer to pool without dropping elements
        self.pool
            .borrow_mut()
            .deallocate_backing(self.handle.buffer_idx, self.handle.size_class);
        
        // Prevent Drop from running since we've moved all elements and returned buffer
        std::mem::forget(self);
        
        vec
    }

    pub fn push(&mut self, value: T) {
        if self.len >= self.capacity {
            panic!("SlabVec capacity exceeded");
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
        unsafe { Some(self.ptr.as_ptr().add(self.len).read().assume_init()) }
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
        while self.len > 0 {
            self.len -= 1;
            unsafe {
                self.ptr.as_ptr().add(self.len).read();
            }
        }
    }

    pub fn truncate(&mut self, len: usize) {
        if len >= self.len {
            return;
        }

        while self.len > len {
            self.len -= 1;
            unsafe {
                self.ptr.as_ptr().add(self.len).read();
            }
        }
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
            panic!("SlabVec resize beyond capacity");
        }

        if new_len > self.len {
            // Growing - fill with cloned values
            while self.len < new_len {
                self.push(value.clone());
            }
        } else {
            // Shrinking - truncate
            self.truncate(new_len);
        }
    }
}

impl<T> Drop for SlabVec<T> {
    fn drop(&mut self) {
        // Drop all initialized elements
        self.clear();

        // Return backing buffer to pool
        self.pool
            .borrow_mut()
            .deallocate_backing(self.handle.buffer_idx, self.handle.size_class);
    }
}

unsafe impl<T: Send> Send for SlabVec<T> {}

// SAFETY: VecPool is Send when T is Send because:
// 1. Each Task owns its pools exclusively (no sharing between threads)  
// 2. Pools are moved atomically with the Task between threads
// 3. No concurrent access - only the owning thread accesses the pool
unsafe impl<T: Send> Send for VecPool<T> {}

/// A Send wrapper around Rc<RefCell<VecPool<T>>> for per-Task pools
/// SAFETY: This is only safe because each Task exclusively owns its pools
/// and Tasks are moved atomically between threads without sharing pools
pub struct TaskVecPool<T>(Rc<RefCell<VecPool<T>>>);

impl<T> TaskVecPool<T> {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(VecPool::new())))
    }
    
    pub fn inner(&self) -> &Rc<RefCell<VecPool<T>>> {
        &self.0
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

impl<T> std::ops::Index<usize> for SlabVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("index out of bounds")
    }
}

impl<T> std::ops::IndexMut<usize> for SlabVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).expect("index out of bounds")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let pool = Rc::new(RefCell::new(VecPool::new()));
        let mut vec = SlabVec::new(pool.clone());

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
        let pool = Rc::new(RefCell::new(VecPool::new()));

        // Allocate and drop a vector
        {
            let mut vec1 = SlabVec::with_capacity(pool.clone(), 16);
            vec1.push(1);
            vec1.push(2);
        }

        // Allocate another - should reuse the backing buffer
        let mut vec2 = SlabVec::with_capacity(pool.clone(), 16);
        vec2.push(3);
        assert_eq!(vec2[0], 3);
    }
}
