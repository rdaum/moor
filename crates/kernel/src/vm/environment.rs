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

//! Simple environment storage for MOO activation frames.
//! Uses standard Vec allocation instead of arena allocation for simplicity and thread safety.

use moor_var::Var;

/// Environment storage for variables in MOO stack frames.
/// Uses nested vectors for scope management.
#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    /// Stack of scopes, each containing variables
    scopes: Vec<Vec<Option<Var>>>,
}

impl Environment {
    /// Create a new empty environment.
    pub fn new() -> Self {
        Self {
            scopes: Vec::with_capacity(8),
        }
    }

    /// Push a new scope with the given width (number of variables).
    pub fn push_scope(&mut self, width: usize) {
        self.scopes.push(vec![None; width]);
    }

    /// Pop the top scope from the stack.
    /// Returns the popped scope if successful.
    pub fn pop_scope(&mut self) -> Option<Vec<Option<Var>>> {
        self.scopes.pop()
    }

    /// Get the number of scopes currently on the stack.
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    /// Set a variable in the given scope.
    /// Scope 0 is the outermost (first pushed) scope.
    /// Panics if indices are out of bounds (indicates compiler bug).
    #[inline]
    pub fn set(&mut self, scope_index: usize, var_index: usize, value: Var) {
        // SAFETY: Indices come from compiler, should always be valid.
        // Panics on out-of-bounds rather than silently failing.
        self.scopes[scope_index][var_index] = Some(value);
    }

    /// Get a variable from the given scope.
    /// Scope 0 is the outermost (first pushed) scope.
    /// Panics if indices are out of bounds (indicates compiler bug).
    #[inline]
    pub fn get(&self, scope_index: usize, var_index: usize) -> Option<&Var> {
        // SAFETY: Indices come from compiler, should always be valid.
        self.scopes[scope_index][var_index].as_ref()
    }

    /// Bulk copy a contiguous range of variables from another environment.
    /// Both environments must have the specified scope, and the range must be valid.
    /// Panics if indices are out of bounds.
    #[inline]
    pub fn copy_range_from(
        &mut self,
        source: &Environment,
        scope_index: usize,
        start: usize,
        end: usize,
    ) {
        let src_scope = &source.scopes[scope_index];
        let dst_scope = &mut self.scopes[scope_index];
        dst_scope[start..=end].clone_from_slice(&src_scope[start..=end]);
    }

    /// Set a contiguous range of variables from an array.
    /// Panics if indices are out of bounds.
    #[inline]
    pub fn set_range<const N: usize>(
        &mut self,
        scope_index: usize,
        start: usize,
        values: [Var; N],
    ) {
        let dst_scope = &mut self.scopes[scope_index];
        for (i, value) in values.into_iter().enumerate() {
            dst_scope[start + i] = Some(value);
        }
    }

    /// Convert the environment to a Vec of scopes for serialization.
    pub fn to_vec(&self) -> Vec<Vec<Option<Var>>> {
        self.scopes.clone()
    }

    /// Iterate over scopes.
    pub fn iter_scopes(&self) -> impl Iterator<Item = &Vec<Option<Var>>> {
        self.scopes.iter()
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

// Environment is automatically Send and Sync because it only contains Send+Sync types (Vec, Option, Var)
// No need for unsafe impl Send

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

        let popped = env.pop_scope();
        assert!(popped.is_some());
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
    fn test_clone() {
        let mut env = Environment::new();
        env.push_scope(5);
        env.set(0, 1, v_int(42));

        let cloned = env.clone();
        assert_eq!(cloned.get(0, 1), Some(&v_int(42)));
        assert_eq!(cloned.len(), 1);
    }
}
