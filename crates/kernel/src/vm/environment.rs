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

use moor_var::Var;
use smallvec::SmallVec;

/// Inline capacity for environment values.
/// Covers 11 globals + ~13 local variables without heap allocation.
/// Falls back to heap for more complex verbs.
const INLINE_VALUES: usize = 24;

/// Environment storage for variables in a single MOO stack frame.
/// All scopes are stored contiguously, with metadata tracking scope boundaries.
/// Uses SmallVec to avoid heap allocation for simple verbs.
#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    /// Single contiguous storage for all variables across all scopes.
    /// None values represent uninitialized slots.
    values: SmallVec<[Option<Var>; INLINE_VALUES]>,
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
        let mut values: SmallVec<[Option<Var>; INLINE_VALUES]> = SmallVec::new();
        values.resize(width, None);
        Self {
            values,
            scope_offsets: smallvec::smallvec![0],
            scope_widths: smallvec::smallvec![width as u16],
        }
    }

    /// Create environment with initial values directly, avoiding double-write.
    /// First N slots get the provided values (wrapped in Some), remaining slots get None.
    #[inline]
    pub fn with_initial_values<const N: usize>(values_arr: [Var; N], total_width: usize) -> Self {
        let mut values: SmallVec<[Option<Var>; INLINE_VALUES]> = SmallVec::new();
        values.reserve(total_width);
        for v in values_arr {
            values.push(Some(v));
        }
        values.resize(total_width, None);
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
        self.values.resize(self.values.len() + width, None);
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
        self.values[idx] = Some(value);
    }

    /// Get a variable from the given scope.
    /// Returns None if the slot is uninitialized.
    /// Scope 0 is the outermost (first pushed) scope.
    #[inline]
    pub fn get(&self, scope_index: usize, var_index: usize) -> Option<&Var> {
        let idx = self.absolute_index(scope_index, var_index);
        self.values[idx].as_ref()
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
        let mut values: SmallVec<[Option<Var>; INLINE_VALUES]> = SmallVec::new();
        values.reserve(total_width);

        // Push initial values directly
        for v in values_arr {
            values.push(Some(v));
        }

        // Clone values from source for the copy range
        let src_base = source.scope_offsets[0] as usize + copy_start;
        let copy_len = copy_end - copy_start + 1;
        for i in 0..copy_len {
            values.push(source.values[src_base + i].clone());
        }

        // Fill remaining slots with None
        values.resize(total_width, None);

        Self {
            values,
            scope_offsets: smallvec::smallvec![0],
            scope_widths: smallvec::smallvec![total_width as u16],
        }
    }

    /// Convert the environment to nested Vecs for serialization.
    pub fn to_vec(&self) -> Vec<Vec<Option<Var>>> {
        self.scope_offsets
            .iter()
            .zip(self.scope_widths.iter())
            .map(|(&offset, &width)| {
                let start = offset as usize;
                let end = start + width as usize;
                self.values[start..end].to_vec()
            })
            .collect()
    }

    /// Iterate over scopes, yielding slices of variables.
    pub fn iter_scopes(&self) -> impl Iterator<Item = &[Option<Var>]> {
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
    use moor_var::v_int;

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
        assert_eq!(scopes[0][1], Some(v_int(42)));
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
}
