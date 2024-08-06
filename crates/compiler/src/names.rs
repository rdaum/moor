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
use moor_values::var::Symbol;
use std::collections::HashMap;
use strum::IntoEnumIterator;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UnboundNames {
    unbound_names: Vec<Symbol>,
    scope: Vec<HashMap<Symbol, UnboundName>>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct UnboundName(usize);

impl UnboundNames {
    pub fn new() -> Self {
        let mut names = Self {
            unbound_names: vec![],
            // Start with a global scope.
            scope: vec![HashMap::new()],
        };
        for global in GlobalName::iter() {
            names.find_or_add_name_global(global.to_string().as_str());
        }
        names
    }

    /// Find a variable name, or declare in global scope.
    pub fn find_or_add_name_global(&mut self, name: &str) -> UnboundName {
        let name = Symbol::mk_case_insensitive(name);

        // Check the scopes, starting at the back (innermost scope)
        for scope in self.scope.iter().rev() {
            if let Some(n) = scope.get(&name) {
                return *n;
            }
        }

        // If the name doesn't exist, add it to the global scope, since that's how
        // MOO currently works.
        let unbound_name = self.new_unbound(name);
        self.scope[0].insert(name, unbound_name);
        unbound_name
    }

    /// Start a new lexical scope.
    pub fn push_scope(&mut self) {
        self.scope.push(HashMap::new());
    }

    /// Pop the current scope.
    pub fn pop_scope(&mut self) {
        self.scope.pop();
    }

    /// Declare a name in the current lexical scope.
    pub fn declare_name(&mut self, name: &str) -> UnboundName {
        let name = Symbol::mk_case_insensitive(name);
        let unbound_name = self.new_unbound(name);
        self.scope.last_mut().unwrap().insert(name, unbound_name);
        unbound_name
    }

    /// Find the name in the name table, if it exists.
    pub fn find_name(&self, name: &str) -> Option<UnboundName> {
        self.find_name_offset(Symbol::mk_case_insensitive(name))
            .map(|x| UnboundName(x))
    }

    /// Return the environment offset of the name, if it exists.
    pub fn find_name_offset(&self, name: Symbol) -> Option<usize> {
        for scope in self.scope.iter().rev() {
            if let Some(n) = scope.get(&name) {
                return Some(n.0 as usize);
            }
        }
        None
    }

    /// Create a new unbound variable.
    fn new_unbound(&mut self, name: Symbol) -> UnboundName {
        let idx = self.unbound_names.len();
        self.unbound_names.push(name);
        UnboundName(idx)
    }

    /// Turn all unbound variables into bound variables.
    /// Run at the end of compilation to produce valid offsets.
    pub fn bind(&self) -> (Names, HashMap<UnboundName, Name>) {
        let mut mapping = HashMap::new();
        let mut bound = vec![];
        // Walk the scopes, binding all unbound variables.
        // This will produce offsets for all variables in the order they should appear in the
        // environment.
        for _ in self.scope.iter() {
            for idx in 0..self.unbound_names.len() {
                let offset = bound.len();
                bound.push(self.unbound_names[idx]);
                mapping.insert(UnboundName(idx), Name(offset as u16));
            }
        }
        (Names { bound }, mapping)
    }
}

/// A Name is a unique identifier for a variable in the program's environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, Hash)]
pub struct Name(u16);

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Names {
    bound: Vec<Symbol>,
}

impl Default for Names {
    fn default() -> Self {
        Self::new()
    }
}

impl Names {
    pub fn new() -> Self {
        Self { bound: vec![] }
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

    /// Return the width of the name table, to be used as the (total) environment size.
    pub fn width(&self) -> usize {
        self.bound.len()
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

    /// Get the offset for a bound variable.
    pub fn offset_for(&self, name: &Name) -> Option<usize> {
        if name.0 as usize >= self.bound.len() {
            return None;
        }
        Some(name.0 as usize)
    }
}
