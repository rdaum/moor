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

//! Arena allocator for MOO activation frame environment storage.
//! Provides fast, thread-local allocation without global allocator contention.

use std::cell::RefCell;
use std::mem::{align_of, size_of};
use std::ptr::NonNull;
use thiserror::Error;

// Concrete types for Var - this module is specialized for moor_var::Var
pub type VarArena = EnvironmentArena;
pub type VarEnvironment = ArenaEnvironment;

/// Errors that can occur during arena operations.
#[derive(Debug, Error, Clone)]
pub enum ArenaError {
    #[error("Arena exhausted: requested {requested} bytes, but only {available} bytes remaining")]
    Exhausted { requested: usize, available: usize },

    #[error("Cannot pop scope: scope stack is empty")]
    EmptyScopeStack,
}

/// Size of a memory page (4KB) for alignment
const PAGE_SIZE: usize = 4096;

/// Default arena size: 512KB
/// This should be enough for ~32 deep call stacks with reasonable variable counts
const DEFAULT_ARENA_SIZE: usize = 512 * 1024;

/// Maximum number of arenas to cache per thread.
/// With rayon's default thread pool size (~num_cpus), this bounds total memory.
/// Example: 8 CPUs * 8 arenas * 512KB = 32MB worst case
const MAX_ARENAS_PER_THREAD: usize = 8;

// Thread-local arena pool to avoid mmap/munmap overhead.
// Each worker thread in the rayon pool maintains its own cache of recycled arenas.
thread_local! {
    static ARENA_POOL: RefCell<Vec<EnvironmentArena>> =
        const { RefCell::new(Vec::new()) };
}

// Helper functions for arena pooling
fn try_acquire_from_pool() -> Option<EnvironmentArena> {
    ARENA_POOL.with(|pool| pool.borrow_mut().pop())
}

fn try_return_to_pool(arena: EnvironmentArena) -> bool {
    ARENA_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < MAX_ARENAS_PER_THREAD {
            pool.push(arena);
            true
        } else {
            false
        }
    })
}

/// Arena allocator for environment variable storage.
/// Uses mmap to allocate a large region and bump-allocates within it.
/// This avoids heap allocation overhead and global allocator contention.
/// Specialized for moor_var::Var.
#[derive(Debug)]
pub struct EnvironmentArena {
    /// Pointer to the mmap'd memory region
    memory: NonNull<u8>,
    /// Total size of the arena in bytes
    size: usize,
    /// Current allocation offset within the arena
    current_offset: usize,
}

impl EnvironmentArena {
    /// Create a new environment arena with the default size.
    pub fn new() -> Result<Self, std::io::Error> {
        Self::with_size(DEFAULT_ARENA_SIZE)
    }

    /// Create a new environment arena with a specific size.
    /// The size will be rounded up to the nearest page boundary.
    pub fn with_size(size: usize) -> Result<Self, std::io::Error> {
        // Round size up to page boundary
        let size = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        // Allocate memory using mmap
        // SAFETY: mmap syscall - we're requesting anonymous private memory
        let memory = unsafe {
            let ptr = libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            );

            if ptr == libc::MAP_FAILED {
                return Err(std::io::Error::last_os_error());
            }

            NonNull::new_unchecked(ptr as *mut u8)
        };

        Ok(Self {
            memory,
            size,
            current_offset: 0,
        })
    }

    /// Allocate space for a scope with the given number of items.
    /// Returns a pointer to the allocated space and the offset where it was allocated.
    /// Returns None if there's not enough space remaining in the arena.
    pub fn alloc_scope(&mut self, count: usize) -> Option<(NonNull<Option<moor_var::Var>>, usize)> {
        // Calculate required size in bytes
        let required_size = count * size_of::<Option<moor_var::Var>>();

        // Align to Option<moor_var::Var> alignment
        let align = align_of::<Option<moor_var::Var>>();
        let offset = (self.current_offset + align - 1) & !(align - 1);

        // Check if we have enough space
        if offset + required_size > self.size {
            return None;
        }

        // Calculate pointer to allocation
        // SAFETY: We've verified the offset is within bounds
        let ptr = unsafe {
            let base = self.memory.as_ptr();
            NonNull::new_unchecked(base.add(offset) as *mut Option<moor_var::Var>)
        };

        // Initialize the memory to None
        // SAFETY: We own this memory and it's properly aligned
        unsafe {
            for i in 0..count {
                std::ptr::write(ptr.as_ptr().add(i), None);
            }
        }

        let alloc_offset = offset;
        self.current_offset = offset + required_size;

        Some((ptr, alloc_offset))
    }

    /// Reset the arena to a previous offset, effectively freeing all allocations
    /// made after that point.
    ///
    /// SAFETY: Caller must ensure that:
    /// - No references to memory after this offset exist
    /// - Drop has been called on all Var values in the freed region
    pub unsafe fn reset_to(&mut self, offset: usize) {
        debug_assert!(
            offset <= self.current_offset,
            "Cannot reset to future offset"
        );

        // Optional: advise the kernel we don't need these pages anymore
        // This can help with memory pressure but adds syscall overhead
        if self.current_offset > offset {
            let start_page = (offset + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
            let end_page = self.current_offset & !(PAGE_SIZE - 1);

            if end_page > start_page {
                // SAFETY: We own this memory region and the calculation is within bounds
                unsafe {
                    let addr = self.memory.as_ptr().add(start_page) as *mut libc::c_void;
                    let len = end_page - start_page;
                    // Ignore errors from madvise - it's just a hint
                    libc::madvise(addr, len, libc::MADV_DONTNEED);
                }
            }
        }

        self.current_offset = offset;
    }

    /// Get the current allocation offset.
    pub fn current_offset(&self) -> usize {
        self.current_offset
    }

    /// Get the total size of the arena.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get the number of bytes remaining in the arena.
    pub fn remaining(&self) -> usize {
        self.size - self.current_offset
    }
}

impl Drop for EnvironmentArena {
    fn drop(&mut self) {
        // SAFETY: We own this memory and allocated it with mmap
        unsafe {
            libc::munmap(self.memory.as_ptr() as *mut libc::c_void, self.size);
        }
    }
}

// SAFETY: EnvironmentArena can be sent between threads.
// It's only accessed by one thread at a time (the task's execution thread).
unsafe impl Send for EnvironmentArena {}

/// Metadata for a single scope in the arena.
#[derive(Debug, Clone)]
struct ScopeInfo {
    /// Pointer to the scope's variable storage in the arena
    ptr: NonNull<Option<moor_var::Var>>,
    /// Number of variables in this scope
    width: usize,
    /// Offset in the arena where this scope starts (for reset_to)
    arena_offset: usize,
}

/// Environment storage backed by an arena allocator.
/// This replaces Vec<Vec<Option<moor_var::Var>>> to avoid heap allocation overhead.
pub struct ArenaEnvironment {
    /// Pointer to the shared arena that backs this environment.
    /// The arena is owned by VMExecState and shared across all frames in a task.
    /// SAFETY: This pointer is valid for the lifetime of the task.
    arena: *mut EnvironmentArena,
    /// Metadata for each scope (just pointers and sizes, not the actual data)
    scopes: Vec<ScopeInfo>,
    /// Whether this ArenaEnvironment owns the arena and should drop it.
    /// True for clones and test cases, false for shared arena from VMExecState.
    owns_arena: bool,
}

impl ArenaEnvironment {
    /// Create a new arena environment that uses the given arena.
    /// The arena must outlive this ArenaEnvironment.
    ///
    /// SAFETY: The caller must ensure that:
    /// - The arena pointer is valid for the lifetime of this ArenaEnvironment
    /// - The arena is not freed while this ArenaEnvironment exists
    pub fn new_with_arena(arena: *mut EnvironmentArena) -> Self {
        Self {
            arena,
            scopes: Vec::with_capacity(16),
            owns_arena: false, // We're borrowing the arena, don't drop it
        }
    }

    /// Create a new arena environment with the specified arena size.
    /// This creates its own owned arena (used for tests and special cases).
    #[cfg(test)]
    pub fn new(size: usize) -> Result<Self, std::io::Error> {
        Ok(Self {
            arena: Box::into_raw(Box::new(EnvironmentArena::with_size(size)?)),
            scopes: Vec::with_capacity(16),
            owns_arena: true, // We own this arena, must drop it
        })
    }

    /// Create a new arena environment with the default size (512KB).
    /// This creates its own owned arena (used for tests and special cases).
    #[cfg(test)]
    pub fn new_default() -> Result<Self, std::io::Error> {
        Ok(Self {
            arena: Box::into_raw(Box::new(EnvironmentArena::new()?)),
            scopes: Vec::with_capacity(16),
            owns_arena: true, // We own this arena, must drop it
        })
    }

    /// Create an arena environment from an existing Vec<Vec<Option<moor_var::Var>>>.
    /// This is used for lambda activations where the environment is built dynamically.
    pub fn from_vec(
        arena: *mut EnvironmentArena,
        env: Vec<Vec<Option<moor_var::Var>>>,
    ) -> Result<Self, ArenaError> {
        let mut arena_env = Self::new_with_arena(arena);

        // Push each scope and copy its contents
        for scope in env {
            let width = scope.len();
            arena_env
                .push_scope(width)
                .expect("Failed to push scope during from_vec");

            for (var_idx, var_opt) in scope.into_iter().enumerate() {
                if let Some(var) = var_opt {
                    arena_env.set(arena_env.scopes.len() - 1, var_idx, var);
                }
            }
        }

        Ok(arena_env)
    }

    /// Push a new scope with the given width.
    pub fn push_scope(&mut self, width: usize) -> Result<(), ArenaError> {
        // SAFETY: arena pointer is guaranteed valid by VMExecState lifetime
        let (ptr, offset) = unsafe { (*self.arena).alloc_scope(width) }.ok_or_else(|| {
            let required_size = width * size_of::<Option<moor_var::Var>>();
            ArenaError::Exhausted {
                requested: required_size,
                available: unsafe { (*self.arena).remaining() },
            }
        })?;

        self.scopes.push(ScopeInfo {
            ptr,
            width,
            arena_offset: offset,
        });

        Ok(())
    }

    /// Pop the most recent scope.
    ///
    /// SAFETY: Caller must ensure that all Var values in the scope have been dropped.
    pub unsafe fn pop_scope(&mut self) -> Result<(), ArenaError> {
        let scope = self.scopes.pop().ok_or(ArenaError::EmptyScopeStack)?;

        // SAFETY: We own this scope and the pointers are valid
        unsafe {
            // Drop the entire scope as a slice - let the compiler optimize this
            // This is better than an explicit loop because the compiler can see
            // it's a contiguous region and potentially vectorize/unroll it
            let slice_ptr = std::ptr::slice_from_raw_parts_mut(scope.ptr.as_ptr(), scope.width);
            std::ptr::drop_in_place(slice_ptr);

            // Reset the arena to reclaim the memory
            // SAFETY: arena pointer is guaranteed valid by VMExecState lifetime
            (*self.arena).reset_to(scope.arena_offset);
        }

        Ok(())
    }

    /// Get a reference to an item in a specific scope.
    /// Returns None if the scope or variable index is out of bounds.
    pub fn get(&self, scope_idx: usize, var_idx: usize) -> Option<&moor_var::Var> {
        let scope = self.scopes.get(scope_idx)?;

        if var_idx >= scope.width {
            return None;
        }

        // SAFETY: We've verified the indices are in bounds
        unsafe {
            let var_ptr = scope.ptr.as_ptr().add(var_idx);
            (*var_ptr).as_ref()
        }
    }

    /// Get a mutable reference to an item in a specific scope.
    /// Returns None if the scope or variable index is out of bounds.
    #[allow(dead_code)]
    pub fn get_mut(
        &mut self,
        scope_idx: usize,
        var_idx: usize,
    ) -> Option<&mut Option<moor_var::Var>> {
        let scope = self.scopes.get(scope_idx)?;

        if var_idx >= scope.width {
            return None;
        }

        // SAFETY: We've verified the indices are in bounds
        unsafe {
            let var_ptr = scope.ptr.as_ptr().add(var_idx);
            Some(&mut *var_ptr)
        }
    }

    /// Set an item in a specific scope.
    /// SAFETY: Caller must ensure scope_idx and var_idx are valid.
    pub fn set(&mut self, scope_idx: usize, var_idx: usize, value: moor_var::Var) {
        debug_assert!(scope_idx < self.scopes.len(), "scope index out of bounds");
        debug_assert!(
            var_idx < self.scopes[scope_idx].width,
            "var index out of bounds"
        );

        let scope = &self.scopes[scope_idx];

        // SAFETY: Caller guarantees indices are valid
        unsafe {
            let var_ptr = scope.ptr.as_ptr().add(var_idx);
            *var_ptr = Some(value);
        }
    }

    /// Initialize a variable slot without dropping the old value.
    /// This is an optimization for initial frame setup where we know the slot contains None.
    ///
    /// SAFETY: Caller must ensure the slot at (scope_idx, var_idx) contains None
    /// and has not been initialized yet. Using this on an already-initialized slot
    /// will leak the old value.
    pub fn init(&mut self, scope_idx: usize, var_idx: usize, value: moor_var::Var) {
        debug_assert!(scope_idx < self.scopes.len(), "scope index out of bounds");
        debug_assert!(
            var_idx < self.scopes[scope_idx].width,
            "var index out of bounds"
        );

        let scope = &self.scopes[scope_idx];

        // SAFETY: Caller guarantees this slot is uninitialized (contains None from alloc_scope)
        // and that indices are valid. We use ptr::write to avoid dropping the None value.
        unsafe {
            let var_ptr = scope.ptr.as_ptr().add(var_idx);
            std::ptr::write(var_ptr, Some(value));
        }
    }

    /// Get the number of scopes.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    /// Check if there are no scopes.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }

    /// Get the width of a specific scope.
    #[allow(dead_code)]
    pub fn scope_width(&self, scope_idx: usize) -> Option<usize> {
        self.scopes.get(scope_idx).map(|s| s.width)
    }

    /// Iterate over all scopes, returning slices of Option<moor_var::Var> for each scope.
    /// Used for scanning variables during GC and other purposes.
    pub fn iter_scopes(&self) -> impl Iterator<Item = &[Option<moor_var::Var>]> + '_ {
        self.scopes.iter().map(|scope| {
            // SAFETY: The scope pointer is valid and points to `scope.width` elements
            unsafe { std::slice::from_raw_parts(scope.ptr.as_ptr(), scope.width) }
        })
    }

    /// Convert the environment to Vec<Vec<Option<moor_var::Var>>> for serialization or other purposes.
    pub fn to_vec(&self) -> Vec<Vec<Option<moor_var::Var>>> {
        let mut result = Vec::with_capacity(self.scopes.len());

        for scope in &self.scopes {
            let mut scope_vec = Vec::with_capacity(scope.width);
            unsafe {
                for i in 0..scope.width {
                    let ptr = scope.ptr.as_ptr().add(i);
                    scope_vec.push((*ptr).clone());
                }
            }
            result.push(scope_vec);
        }

        result
    }
}

impl Drop for ArenaEnvironment {
    fn drop(&mut self) {
        // Pop all scopes to ensure values are properly dropped
        while !self.scopes.is_empty() {
            unsafe {
                // In drop, we can't propagate errors, so just ignore failures
                let _ = self.pop_scope();
            }
        }

        // Only recycle or drop the arena if we own it
        if self.owns_arena {
            unsafe {
                // Reset the arena to initial state for reuse
                (*self.arena).reset_to(0);

                // Convert raw pointer back to Box to regain ownership and move out the arena
                let arena = *Box::from_raw(self.arena);
                // Try to recycle - if not recycled, will be dropped
                try_return_to_pool(arena);
            }
        }
    }
}

impl Clone for ArenaEnvironment {
    fn clone(&self) -> Self {
        // SAFETY: arena pointer is guaranteed valid by VMExecState lifetime
        let arena_size = unsafe { (*self.arena).size() };

        // Try to get a recycled arena from the thread-local pool
        let arena = try_acquire_from_pool().unwrap_or_else(|| {
            EnvironmentArena::with_size(arena_size).expect("Failed to create arena during clone")
        });

        let mut new_env = Self {
            arena: Box::into_raw(Box::new(arena)),
            scopes: Vec::with_capacity(16),
            owns_arena: true, // Clone owns its own arena
        };

        // Clone each scope and its contents
        for scope in &self.scopes {
            // Push a new scope with the same width
            new_env
                .push_scope(scope.width)
                .expect("Failed to push scope during clone");

            // Copy all the variables from the old scope to the new scope
            unsafe {
                for i in 0..scope.width {
                    let old_ptr = scope.ptr.as_ptr().add(i);
                    if let Some(value) = &*old_ptr {
                        new_env.set(new_env.scopes.len() - 1, i, value.clone());
                    }
                }
            }
        }

        new_env
    }
}

impl std::fmt::Debug for ArenaEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // SAFETY: arena pointer is guaranteed valid by VMExecState lifetime
        let (arena_size, arena_used) =
            unsafe { ((*self.arena).size(), (*self.arena).current_offset()) };
        f.debug_struct("ArenaEnvironment")
            .field("num_scopes", &self.scopes.len())
            .field(
                "scope_widths",
                &self.scopes.iter().map(|s| s.width).collect::<Vec<_>>(),
            )
            .field("arena_size", &arena_size)
            .field("arena_used", &arena_used)
            .finish()
    }
}

impl PartialEq for ArenaEnvironment {
    fn eq(&self, other: &Self) -> bool {
        // Compare by converting to Vec - this is the simplest approach
        self.to_vec() == other.to_vec()
    }
}

// SAFETY: ArenaEnvironment can be sent between threads as long as the arena can be.
unsafe impl Send for ArenaEnvironment {}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::{Var, v_int};

    #[test]
    fn test_arena_creation() {
        let arena = EnvironmentArena::new().expect("Failed to create arena");
        assert_eq!(arena.current_offset(), 0);
        assert_eq!(arena.size(), DEFAULT_ARENA_SIZE);
        assert_eq!(arena.remaining(), DEFAULT_ARENA_SIZE);
    }

    #[test]
    fn test_arena_custom_size() {
        let size = 8192;
        let arena = EnvironmentArena::with_size(size).expect("Failed to create arena");
        assert_eq!(arena.size(), size);
    }

    #[test]
    fn test_arena_size_alignment() {
        // Request a size that's not page-aligned
        let arena = EnvironmentArena::with_size(5000).expect("Failed to create arena");
        // Should be rounded up to next page boundary
        assert_eq!(arena.size(), 8192);
    }

    #[test]
    fn test_alloc_scope() {
        let mut arena = EnvironmentArena::new().expect("Failed to create arena");

        // Allocate space for 10 variables
        let result = arena.alloc_scope(10);
        assert!(result.is_some());

        let (ptr, offset) = result.unwrap();
        assert_eq!(offset, 0); // First allocation should be at offset 0
        assert!(arena.current_offset() > 0);

        // Verify the memory is initialized to None
        unsafe {
            for i in 0..10 {
                let var_ptr = ptr.as_ptr().add(i);
                assert!((*var_ptr).is_none());
            }
        }
    }

    #[test]
    fn test_multiple_allocs() {
        let mut arena = EnvironmentArena::new().expect("Failed to create arena");

        // Allocate several scopes
        let (_, offset1) = arena.alloc_scope(5).unwrap();
        let (_, offset2) = arena.alloc_scope(10).unwrap();
        let (_, offset3) = arena.alloc_scope(3).unwrap();

        // Offsets should be increasing
        assert!(offset2 > offset1);
        assert!(offset3 > offset2);
    }

    #[test]
    fn test_write_and_read_vars() {
        let mut arena = EnvironmentArena::new().expect("Failed to create arena");

        let (ptr, _) = arena.alloc_scope(5).unwrap();

        // Write some values
        unsafe {
            *ptr.as_ptr().add(0) = Some(v_int(42));
            *ptr.as_ptr().add(1) = Some(v_int(100));
            *ptr.as_ptr().add(2) = None;
            *ptr.as_ptr().add(3) = Some(v_int(-5));
            *ptr.as_ptr().add(4) = Some(v_int(999));
        }

        // Read them back
        unsafe {
            assert_eq!(*ptr.as_ptr().add(0), Some(v_int(42)));
            assert_eq!(*ptr.as_ptr().add(1), Some(v_int(100)));
            assert_eq!(*ptr.as_ptr().add(2), None);
            assert_eq!(*ptr.as_ptr().add(3), Some(v_int(-5)));
            assert_eq!(*ptr.as_ptr().add(4), Some(v_int(999)));
        }
    }

    #[test]
    fn test_reset_to() {
        let mut arena = EnvironmentArena::new().expect("Failed to create arena");

        let (_, offset1) = arena.alloc_scope(10).unwrap();
        let _ = arena.alloc_scope(10).unwrap();
        let _ = arena.alloc_scope(10).unwrap();

        let offset_before_reset = arena.current_offset();
        assert!(offset_before_reset > offset1);

        // Reset to first allocation
        unsafe {
            arena.reset_to(offset1);
        }

        assert_eq!(arena.current_offset(), offset1);

        // Should be able to allocate again at the same spot
        let (_, offset_new) = arena.alloc_scope(10).unwrap();
        assert_eq!(offset_new, offset1);
    }

    #[test]
    fn test_arena_exhaustion() {
        // Create a small arena
        let mut arena = EnvironmentArena::with_size(PAGE_SIZE).expect("Failed to create arena");

        // Allocate until we run out of space
        let mut allocations = 0;
        loop {
            if arena.alloc_scope(100).is_none() {
                break;
            }
            allocations += 1;
        }

        assert!(allocations > 0);
        assert!(arena.remaining() < 100 * size_of::<Option<Var>>());
    }

    #[test]
    fn test_arena_alignment() {
        let mut arena = EnvironmentArena::new().expect("Failed to create arena");

        // Allocate an odd number of bytes to test alignment
        let (ptr1, _) = arena.alloc_scope(1).unwrap();
        let (ptr2, _) = arena.alloc_scope(1).unwrap();

        // Both pointers should be properly aligned
        assert_eq!(ptr1.as_ptr() as usize % align_of::<Option<Var>>(), 0);
        assert_eq!(ptr2.as_ptr() as usize % align_of::<Option<Var>>(), 0);
    }

    #[test]
    fn test_drop_cleanup() {
        // Create and drop an arena
        {
            let _arena = EnvironmentArena::new().expect("Failed to create arena");
            // Arena should be dropped here
        }
        // If we get here without crashing, munmap succeeded
    }

    // ArenaEnvironment tests

    #[test]
    fn test_arena_env_creation() {
        let env = ArenaEnvironment::new_default().expect("Failed to create arena");

        assert_eq!(env.len(), 0);
        assert!(env.is_empty());
    }

    #[test]
    fn test_arena_env_push_scope() {
        let mut env = ArenaEnvironment::new_default().expect("Failed to create arena");

        env.push_scope(10).expect("Failed to push scope");
        assert_eq!(env.len(), 1);
        assert_eq!(env.scope_width(0), Some(10));
    }

    #[test]
    fn test_arena_env_push_scope_exhaustion() {
        let mut env = ArenaEnvironment::new(PAGE_SIZE).expect("Failed to create arena");

        // Try to allocate until we get an exhaustion error
        let mut count = 0;
        loop {
            match env.push_scope(100) {
                Ok(()) => count += 1,
                Err(e) => panic!("Unexpected error: {e}"),
                Err(e) => panic!("Unexpected error: {}", e),
            }
        }

        assert!(count > 0, "Should have succeeded at least once");
    }

    #[test]
    fn test_arena_env_multiple_scopes() {
        let mut env = ArenaEnvironment::new_default().expect("Failed to create arena");

        env.push_scope(5).expect("Failed to push scope");
        env.push_scope(10).expect("Failed to push scope");
        env.push_scope(3).expect("Failed to push scope");

        assert_eq!(env.len(), 3);
        assert_eq!(env.scope_width(0), Some(5));
        assert_eq!(env.scope_width(1), Some(10));
        assert_eq!(env.scope_width(2), Some(3));
    }

    #[test]
    fn test_arena_env_get_set() {
        let mut env = ArenaEnvironment::new_default().expect("Failed to create arena");

        env.push_scope(5).expect("Failed to push scope");

        // Initially all should be None
        assert_eq!(env.get(0, 0), None);

        // Set some values
        env.set(0, 0, v_int(42));
        env.set(0, 2, v_int(100));

        // Read them back
        assert_eq!(env.get(0, 0), Some(&v_int(42)));
        assert_eq!(env.get(0, 1), None);
        assert_eq!(env.get(0, 2), Some(&v_int(100)));
    }

    #[test]
    fn test_arena_env_multiple_scope_access() {
        let mut env = ArenaEnvironment::new_default().expect("Failed to create arena");

        env.push_scope(3).expect("Failed to push scope");
        env.push_scope(3).expect("Failed to push scope");

        env.set(0, 0, v_int(1));
        env.set(0, 1, v_int(2));
        env.set(1, 0, v_int(10));
        env.set(1, 2, v_int(12));

        assert_eq!(env.get(0, 0), Some(&v_int(1)));
        assert_eq!(env.get(0, 1), Some(&v_int(2)));
        assert_eq!(env.get(1, 0), Some(&v_int(10)));
        assert_eq!(env.get(1, 2), Some(&v_int(12)));
    }

    #[test]
    fn test_arena_env_pop_scope() {
        let mut env = ArenaEnvironment::new_default().expect("Failed to create arena");

        env.push_scope(5).expect("Failed to push scope");
        env.push_scope(3).expect("Failed to push scope");
        assert_eq!(env.len(), 2);

        unsafe {
            env.pop_scope().expect("Failed to pop scope");
        }
        assert_eq!(env.len(), 1);

        unsafe {
            env.pop_scope().expect("Failed to pop scope");
        }
        assert_eq!(env.len(), 0);
    }

    #[test]
    fn test_arena_env_bounds_checking() {
        let mut env = ArenaEnvironment::new_default().expect("Failed to create arena");

        env.push_scope(5).expect("Failed to push scope");

        // Out of bounds access should return None for get
        assert_eq!(env.get(0, 10), None);
        assert_eq!(env.get(1, 0), None);

        // Out of bounds access for set is UB - debug builds will catch it with debug_assert
        // In release builds, the compiler trusts us. These tests are removed since we
        // can't test UB safely.
    }

    #[test]
    fn test_arena_env_pop_empty_stack() {
        let mut env = ArenaEnvironment::new_default().expect("Failed to create arena");

        // Popping from empty stack should return error
        match unsafe { env.pop_scope() } {
            Err(ArenaError::EmptyScopeStack) => {}
            other => panic!("Expected EmptyScopeStack error, got {other:?}"),
        }
    }

    #[test]
    fn test_arena_env_drop() {
        {
            let mut env = ArenaEnvironment::new_default().expect("Failed to create arena");
            env.push_scope(10).expect("Failed to push scope");
            env.set(0, 0, v_int(42));
            // env should be dropped here, cleaning up all scopes
        }
        // If we get here without crashing, drop succeeded
    }

    #[test]
    fn test_arena_env_with_heap_allocated_vars() {
        use moor_var::{v_list, v_str, v_string};

        let mut env = ArenaEnvironment::new_default().expect("Failed to create arena");

        env.push_scope(5).expect("Failed to push scope");

        // Store heap-allocated Vars that have Drop implementations
        env.set(0, 0, v_string("hello world".to_string()));
        env.set(0, 1, v_str("test"));
        env.set(0, 2, v_list(&[v_int(1), v_int(2), v_int(3)]));

        // Read them back to verify they're stored correctly
        let s0 = env.get(0, 0).expect("Should have value");
        assert!(matches!(s0.variant(), moor_var::Variant::Str(_)));

        let s1 = env.get(0, 1).expect("Should have value");
        assert!(matches!(s1.variant(), moor_var::Variant::Str(_)));

        let l = env.get(0, 2).expect("Should have value");
        match l.variant() {
            moor_var::Variant::List(list) => {
                assert_eq!(list.iter().count(), 3);
            }
            _ => panic!("Expected list"),
        }

        // Pop the scope - this should call Drop on all the Vars
        unsafe {
            env.pop_scope().expect("Failed to pop scope");
        }

        // If we get here without leaking or crashing, Drop worked
    }

    #[test]
    fn test_arena_env_pop_drops_values() {
        use moor_var::v_string;

        let mut env = ArenaEnvironment::new_default().expect("Failed to create arena");

        // Create multiple scopes with heap-allocated values
        env.push_scope(3).expect("Failed to push scope");
        env.push_scope(3).expect("Failed to push scope");

        env.set(0, 0, v_string("scope 0 var 0".to_string()));
        env.set(0, 1, v_string("scope 0 var 1".to_string()));
        env.set(1, 0, v_string("scope 1 var 0".to_string()));
        env.set(1, 1, v_string("scope 1 var 1".to_string()));

        // Pop inner scope - should drop scope 1's values
        unsafe {
            env.pop_scope().expect("Failed to pop scope");
        }

        // Scope 0 should still be accessible
        assert_eq!(env.len(), 1);
        let s = env.get(0, 0).expect("Should have value");
        assert!(matches!(s.variant(), moor_var::Variant::Str(_)));

        // Pop remaining scope - should drop scope 0's values
        unsafe {
            env.pop_scope().expect("Failed to pop scope");
        }

        assert_eq!(env.len(), 0);
    }

    #[test]
    fn test_arena_env_final_drop_cleans_all_scopes() {
        use moor_var::v_string;

        {
            let mut env = ArenaEnvironment::new_default().expect("Failed to create arena");

            // Create multiple scopes without popping them
            env.push_scope(3).expect("Failed to push scope");
            env.push_scope(3).expect("Failed to push scope");
            env.push_scope(3).expect("Failed to push scope");

            // Fill with heap-allocated values
            for scope in 0..3 {
                for var in 0..3 {
                    env.set(scope, var, v_string(format!("s{scope}v{var}")));
                }
            }

            // Don't explicitly pop - let Drop handle it
        }
        // ArenaEnvironment's Drop should clean up all 3 scopes and their 9 values
        // If this doesn't leak or crash, Drop is working correctly
    }

    #[test]
    fn test_arena_env_overwrite_drops_old_value() {
        use moor_var::v_string;

        let mut env = ArenaEnvironment::new_default().expect("Failed to create arena");

        env.push_scope(3).expect("Failed to push scope");

        // Set a value
        env.set(0, 0, v_string("first value".to_string()));

        // Overwrite it - the old value should be dropped
        env.set(0, 0, v_string("second value".to_string()));

        // Verify the new value is there
        let v = env.get(0, 0).expect("Should have value");
        match v.variant() {
            moor_var::Variant::Str(s) => {
                assert_eq!(s.as_str(), "second value");
            }
            _ => panic!("Expected string"),
        }

        // Clean up
        unsafe {
            env.pop_scope().expect("Failed to pop scope");
        }
    }
}
