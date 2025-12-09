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

//! Contiguous environment storage for MOO stack frames.
//! Uses a single Vec for all scopes to reduce allocations and improve cache locality.
//! Uninitialized slots use v_none() (all zeros) as a sentinel, enabling fast zero-fill.

use moor_var::Var;
use smallvec::SmallVec;
use std::ptr;

/// Inline capacity for environment values.
/// Covers 11 globals + ~13 local variables without heap allocation.
/// Falls back to heap for more complex verbs.
const INLINE_VALUES: usize = 24;

/// Environment storage for variables in a single MOO stack frame.
/// All scopes are stored contiguously, with metadata tracking scope boundaries.
/// Uses SmallVec to avoid heap allocation for simple verbs.
/// Uninitialized slots contain v_none() which is all zeros.
#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    /// Single contiguous storage for all variables across all scopes.
    /// v_none() values represent uninitialized slots (E_VARNF).
    values: SmallVec<[Var; INLINE_VALUES]>,
    /// Starting offset of each scope within values
    scope_offsets: SmallVec<[u16; 8]>,
    /// Width (number of variables) of each scope
    scope_widths: SmallVec<[u16; 8]>,
}

impl Environment {
    /// Create a new empty environment.
    pub fn new() -> Self {
        Self {
            values: SmallVec::new(),
            scope_offsets: SmallVec::new(),
            scope_widths: SmallVec::new(),
        }
    }

    /// Create environment with initial scope pre-allocated.
    #[inline]
    pub fn with_initial_scope(width: usize) -> Self {
        let mut values: SmallVec<[Var; INLINE_VALUES]> = SmallVec::new();
        values.reserve(width);
        // SAFETY: v_none() is all zeros, so we can zero-fill
        unsafe {
            ptr::write_bytes(values.as_mut_ptr(), 0, width);
            values.set_len(width);
        }
        Self {
            values,
            scope_offsets: smallvec::smallvec![0],
            scope_widths: smallvec::smallvec![width as u16],
        }
    }

    /// Create environment with initial values directly, avoiding double-write.
    /// First N slots get the provided values, remaining slots get v_none().
    #[inline]
    pub fn with_initial_values<const N: usize>(values_arr: [Var; N], total_width: usize) -> Self {
        let mut values: SmallVec<[Var; INLINE_VALUES]> = SmallVec::new();
        values.reserve(total_width);

        // SAFETY: We reserved total_width, write N values then zero-fill the rest
        unsafe {
            let ptr = values.as_mut_ptr();
            // Write initial values
            for (i, v) in values_arr.into_iter().enumerate() {
                ptr.add(i).write(v);
            }
            // Zero-fill remaining slots (v_none() is all zeros)
            if total_width > N {
                ptr::write_bytes(ptr.add(N), 0, total_width - N);
            }
            values.set_len(total_width);
        }

        Self {
            values,
            scope_offsets: smallvec::smallvec![0],
            scope_widths: smallvec::smallvec![total_width as u16],
        }
    }

    /// Push a new scope with the given width (number of variables).
    #[inline]
    pub fn push_scope(&mut self, width: usize) {
        let offset = self.values.len() as u16;
        self.scope_offsets.push(offset);
        self.scope_widths.push(width as u16);

        let old_len = self.values.len();
        let new_len = old_len + width;
        self.values.reserve(width);
        // SAFETY: v_none() is all zeros, so we can zero-fill
        unsafe {
            ptr::write_bytes(self.values.as_mut_ptr().add(old_len), 0, width);
            self.values.set_len(new_len);
        }
    }

    /// Pop the top scope from the stack.
    #[inline]
    pub fn pop_scope(&mut self) {
        if let Some(offset) = self.scope_offsets.pop() {
            self.scope_widths.pop();
            self.values.truncate(offset as usize);
        }
    }

    /// Get the number of scopes currently on the stack.
    #[inline]
    pub fn len(&self) -> usize {
        self.scope_offsets.len()
    }

    /// Compute the absolute index for a (scope_index, var_index) pair.
    #[inline]
    fn absolute_index(&self, scope_index: usize, var_index: usize) -> usize {
        self.scope_offsets[scope_index] as usize + var_index
    }

    /// Set a variable in the given scope.
    /// Scope 0 is the outermost (first pushed) scope.
    #[inline]
    pub fn set(&mut self, scope_index: usize, var_index: usize, value: Var) {
        let idx = self.absolute_index(scope_index, var_index);
        self.values[idx] = value;
    }

    /// Get a variable from the given scope.
    /// Returns None if the slot is uninitialized (contains v_none()).
    /// Scope 0 is the outermost (first pushed) scope.
    #[inline]
    pub fn get(&self, scope_index: usize, var_index: usize) -> Option<&Var> {
        let idx = self.absolute_index(scope_index, var_index);
        let v = &self.values[idx];
        if v.is_none() { None } else { Some(v) }
    }

    /// Create environment with initial values and copy remaining from source.
    /// Used for nested verb calls that inherit parsing globals from parent.
    /// Avoids double-write by building directly without initial None fill.
    #[inline]
    pub fn with_values_and_copy<const N: usize>(
        values_arr: [Var; N],
        source: &Environment,
        copy_start: usize,
        copy_end: usize,
        total_width: usize,
    ) -> Self {
        let mut values: SmallVec<[Var; INLINE_VALUES]> = SmallVec::new();
        values.reserve(total_width);

        let copy_len = copy_end - copy_start + 1;
        let src_base = source.scope_offsets[0] as usize + copy_start;

        // SAFETY: We reserved total_width
        unsafe {
            let ptr = values.as_mut_ptr();

            // Write initial values
            for (i, v) in values_arr.into_iter().enumerate() {
                ptr.add(i).write(v);
            }

            // Clone values from source
            for i in 0..copy_len {
                ptr.add(N + i).write(source.values[src_base + i].clone());
            }

            // Zero-fill remaining slots
            let filled = N + copy_len;
            if total_width > filled {
                ptr::write_bytes(ptr.add(filled), 0, total_width - filled);
            }

            values.set_len(total_width);
        }

        Self {
            values,
            scope_offsets: smallvec::smallvec![0],
            scope_widths: smallvec::smallvec![total_width as u16],
        }
    }

    /// Convert the environment to nested Vecs for serialization.
    /// Uninitialized slots (v_none()) are converted to None.
    pub fn to_vec(&self) -> Vec<Vec<Option<Var>>> {
        self.scope_offsets
            .iter()
            .zip(self.scope_widths.iter())
            .map(|(&offset, &width)| {
                let start = offset as usize;
                let end = start + width as usize;
                self.values[start..end]
                    .iter()
                    .map(|v| if v.is_none() { None } else { Some(v.clone()) })
                    .collect()
            })
            .collect()
    }

    /// Iterate over scopes, yielding slices of variables.
    pub fn iter_scopes(&self) -> impl Iterator<Item = &[Var]> {
        self.scope_offsets
            .iter()
            .zip(self.scope_widths.iter())
            .map(move |(&offset, &width)| {
                let start = offset as usize;
                let end = start + width as usize;
                &self.values[start..end]
            })
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::{v_int, v_none};

    #[test]
    fn test_push_pop_scope() {
        let mut env = Environment::new();
        assert_eq!(env.len(), 0);

        env.push_scope(5);
        assert_eq!(env.len(), 1);

        env.push_scope(3);
        assert_eq!(env.len(), 2);

        env.pop_scope();
        assert_eq!(env.len(), 1);
    }

    #[test]
    fn test_set_get_variable() {
        let mut env = Environment::new();
        env.push_scope(5);

        let value = v_int(42);
        env.set(0, 2, value.clone());

        let retrieved = env.get(0, 2);
        assert_eq!(retrieved, Some(&value));
    }

    #[test]
    fn test_uninitialized_is_none() {
        let mut env = Environment::new();
        env.push_scope(5);

        // Uninitialized slots should return None
        assert_eq!(env.get(0, 0), None);
        assert_eq!(env.get(0, 4), None);
    }

    #[test]
    fn test_multiple_scopes() {
        let mut env = Environment::new();
        env.push_scope(5);
        env.push_scope(3);

        env.set(0, 1, v_int(10));
        env.set(1, 2, v_int(20));

        assert_eq!(env.get(0, 1), Some(&v_int(10)));
        assert_eq!(env.get(1, 2), Some(&v_int(20)));
    }

    #[test]
    fn test_pop_scope_removes_values() {
        let mut env = Environment::new();
        env.push_scope(3);
        env.push_scope(2);

        env.set(0, 0, v_int(100));
        env.set(1, 0, v_int(200));

        // Pop inner scope
        env.pop_scope();

        // Outer scope still intact
        assert_eq!(env.get(0, 0), Some(&v_int(100)));
        assert_eq!(env.len(), 1);
    }

    #[test]
    fn test_to_vec() {
        let mut env = Environment::new();
        env.push_scope(3);
        env.set(0, 1, v_int(42));

        let scopes = env.to_vec();
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes[0].len(), 3);
        assert_eq!(scopes[0][0], None); // Uninitialized
        assert_eq!(scopes[0][1], Some(v_int(42)));
        assert_eq!(scopes[0][2], None); // Uninitialized
    }

    #[test]
    fn test_clone() {
        let mut env = Environment::new();
        env.push_scope(5);
        env.set(0, 1, v_int(42));

        let cloned = env.clone();
        assert_eq!(cloned.get(0, 1), Some(&v_int(42)));
        assert_eq!(cloned.len(), 1);
    }

    #[test]
    fn test_v_none_is_all_zeros() {
        // Verify our assumption that v_none() is all zeros
        let none = v_none();
        let bytes: [u8; 16] = unsafe { std::mem::transmute(none) };
        assert!(bytes.iter().all(|&b| b == 0), "v_none() must be all zeros for zero-fill to work");
    }
}
