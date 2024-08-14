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

use crate::GlobalName;
use bincode::{Decode, Encode};
use moor_values::model::CompileError;
use moor_values::var::Symbol;
use std::collections::HashMap;
use strum::IntoEnumIterator;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UnboundNames {
    unbound_names: Vec<Decl>,
    scope: Vec<HashMap<Symbol, UnboundName>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Decl {
    pub sym: Symbol,
    pub depth: usize,
    pub constant: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct UnboundName {
    offset: usize,
}

impl Default for UnboundNames {
    fn default() -> Self {
        Self::new()
    }
}

/// Policy for binding a variable when new_bound is called.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BindMode {
    /// If the variable is already bound, use it.
    Reuse,
    /// If the variable exists, return an error.
    New,
}

impl UnboundNames {
    pub fn new() -> Self {
        let mut names = Self {
            unbound_names: vec![],
            // Start with a global scope.
            scope: vec![HashMap::new()],
        };
        for global in GlobalName::iter() {
            names
                .find_or_add_name_global(global.to_string().as_str())
                .unwrap();
        }
        names
    }

    /// Find a variable name, or declare in global scope.
    pub fn find_or_add_name_global(&mut self, name: &str) -> Result<UnboundName, CompileError> {
        let name = Symbol::mk_case_insensitive(name);

        // Check the scopes, starting at the back (innermost scope)
        for scope in self.scope.iter().rev() {
            if let Some(n) = scope.get(&name) {
                return Ok(*n);
            }
        }

        // If the name doesn't exist, add it to the global scope, since that's how
        // MOO currently works.
        // These types of variables are always mutable, and always re-use a variable name, to
        // maintain existing MOO language semantics.
        let unbound_name = self.new_unbound(name, 0, false, BindMode::Reuse)?;
        self.scope[0].insert(name, unbound_name);
        Ok(unbound_name)
    }

    /// Start a new lexical scope.
    pub fn push_scope(&mut self) {
        self.scope.push(HashMap::new());
    }

    /// Pop the current scope.
    pub fn pop_scope(&mut self) -> usize {
        let scope = self.scope.pop().unwrap();
        scope.len()
    }

    /// Declare a (mutable) name in the current lexical scope.
    pub fn declare_name(&mut self, name: &str) -> Result<UnboundName, CompileError> {
        let name = Symbol::mk_case_insensitive(name);
        let unbound_name = self.new_unbound(name, self.scope.len() - 1, false, BindMode::New)?;
        self.scope.last_mut().unwrap().insert(name, unbound_name);
        Ok(unbound_name)
    }

    /// Declare a (mutable) name in the current lexical scope.
    pub fn declare_const(&mut self, name: &str) -> Result<UnboundName, CompileError> {
        let name = Symbol::mk_case_insensitive(name);
        let unbound_name = self.new_unbound(name, self.scope.len() - 1, true, BindMode::New)?;
        self.scope.last_mut().unwrap().insert(name, unbound_name);
        Ok(unbound_name)
    }

    pub fn declare(
        &mut self,
        name: &str,
        constant: bool,
        global: bool,
    ) -> Result<UnboundName, CompileError> {
        if global {
            return self.find_or_add_name_global(name);
        }
        if constant {
            return self.declare_const(name);
        }
        self.declare_name(name)
    }

    /// If the same named variable exists in multiple scopes, return them all as a vector.
    pub fn find_named(&self, name: &str) -> Vec<UnboundName> {
        let name = Symbol::mk_case_insensitive(name);
        let mut names = vec![];
        for (i, Decl { sym, .. }) in self.unbound_names.iter().enumerate() {
            if *sym == name {
                names.push(UnboundName { offset: i });
            }
        }
        names
    }

    /// Find the first scoped name in the name table, if it exists.
    pub fn find_name(&self, name: &str) -> Option<UnboundName> {
        let name = Symbol::mk_case_insensitive(name);
        for scope in self.scope.iter().rev() {
            if let Some(n) = scope.get(&name) {
                return Some(*n);
            }
        }
        None
    }

    /// Create a new unbound variable.
    fn new_unbound(
        &mut self,
        name: Symbol,
        scope: usize,
        constant: bool,
        bind_mode: BindMode,
    ) -> Result<UnboundName, CompileError> {
        // If the variable already exists in this scope, return an error.
        if bind_mode == BindMode::New && self.scope[scope].contains_key(&name) {
            return Err(CompileError::DuplicateVariable(name));
        }
        let idx = self.unbound_names.len();
        self.unbound_names.push(Decl {
            sym: name,
            depth: scope,
            constant,
        });
        Ok(UnboundName { offset: idx })
    }

    pub fn decl_for(&self, name: &UnboundName) -> &Decl {
        &self.unbound_names[name.offset]
    }

    /// Turn all unbound variables into bound variables.
    /// Run at the end of compilation to produce valid offsets.
    pub fn bind(&self) -> (Names, HashMap<UnboundName, Name>) {
        let mut mapping = HashMap::new();
        let mut bound = Vec::with_capacity(self.unbound_names.len());
        // Walk the scopes, binding all unbound variables.
        // This will produce offsets for all variables in the order they should appear in the
        // environment.
        let mut scope_depth = Vec::with_capacity(self.unbound_names.len());
        for (idx, _) in self.unbound_names.iter().enumerate() {
            let offset = bound.len();
            bound.push(self.unbound_names[idx].sym);
            scope_depth.push(self.unbound_names[idx].depth as u16);
            mapping.insert(UnboundName { offset: idx }, Name(offset as u16));
        }

        let global_width = self.scope[0].len();
        (
            Names {
                bound,
                global_width,
                scope_depth,
            },
            mapping,
        )
    }
}

/// A Name is a unique identifier for a variable in the program's environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, Hash)]
pub struct Name(u16);

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Names {
    /// The set of bound variables and their names.
    bound: Vec<Symbol>,
    /// The size of the global scope, e.g. the size the environment should be when the frame
    /// is first created.
    global_width: usize,
    /// The scope-depth for each variable. E.g. 0 for global and then 1..N for nested scopes.
    scope_depth: Vec<u16>,
}

impl Names {
    pub fn new(global_width: usize) -> Self {
        Self {
            bound: vec![],
            global_width,
            scope_depth: vec![],
        }
    }

    /// Find the name in the name table, if it exists.
    pub fn find_name(&self, name: &str) -> Option<Name> {
        for (idx, n) in self.bound.iter().enumerate() {
            if n.as_str() == name {
                return Some(Name(idx as u16));
            }
        }
        None
    }

    /// Return the total width of the name table, to be used as the (total) environment size.
    pub fn width(&self) -> usize {
        self.bound.len()
    }

    /// The size of the global scope section of the environment, e.g. the "environment_width" of the
    /// frame's environment when it is first created.
    pub fn global_width(&self) -> usize {
        self.global_width
    }

    /// Return the symbol value of the given name offset.
    pub fn name_of(&self, name: &Name) -> Option<Symbol> {
        if name.0 as usize >= self.bound.len() {
            return None;
        }
        Some(self.bound[name.0 as usize])
    }

    pub fn symbols(&self) -> &Vec<Symbol> {
        &self.bound
    }

    pub fn names(&self) -> Vec<Name> {
        (0..self.bound.len() as u16).map(Name).collect()
    }

    pub fn depth_of(&self, name: &Name) -> Option<u16> {
        if name.0 as usize >= self.scope_depth.len() {
            return None;
        }
        Some(self.scope_depth[name.0 as usize])
    }

    /// Get the offset for a bound variable.
    pub fn offset_for(&self, name: &Name) -> Option<usize> {
        if name.0 as usize >= self.bound.len() {
            return None;
        }
        Some(name.0 as usize)
    }
}
