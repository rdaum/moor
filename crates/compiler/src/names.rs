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

use crate::GlobalName;
use crate::names::Binding::{Named, Register};
use bincode::{Decode, Encode};
use moor_common::model::CompileError;
use moor_var::Symbol;
use std::collections::HashMap;
use strum::IntoEnumIterator;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UnboundNames {
    unbound_names: Vec<Decl>,
    scope: Vec<HashMap<Binding, UnboundName>>,
    num_registers: u16,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Encode, Decode)]
pub enum Binding {
    Named(Symbol),
    Register(u16),
}

impl Binding {
    pub fn to_symbol(&self) -> Symbol {
        match self {
            Binding::Named(sym) => *sym,
            Binding::Register(r) => Symbol::mk(&format!("<register_{}>", r)),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Decl {
    pub identifier: Binding,
    pub depth: usize,
    pub constant: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct UnboundName {
    pub offset: usize,
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
            num_registers: 0,
        };
        for global in GlobalName::iter() {
            names
                .find_or_add_name_global(global.to_string().as_str())
                .unwrap();
        }
        names
    }

    /// Find a variable name, or declare in global scope.
    pub fn find_or_add_name_global(&mut self, name: &str) -> Option<UnboundName> {
        let name = Symbol::mk_case_insensitive(name);
        let bname = Named(name);

        // Check the scopes, starting at the back (innermost scope)
        for scope in self.scope.iter().rev() {
            if let Some(n) = scope.get(&bname) {
                return Some(*n);
            }
        }

        // If the name doesn't exist, add it to the global scope, since that's how
        // MOO currently works.
        // These types of variables are always mutable, and always re-use a variable name, to
        // maintain existing MOO language semantics.
        let unbound_name = self.new_unbound_named(name, 0, false, BindMode::Reuse)?;
        self.scope[0].insert(bname, unbound_name);
        Some(unbound_name)
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

    pub fn declare_register(&mut self) -> Result<UnboundName, CompileError> {
        // Registers always exist at the global level, but are unique.
        let (unbound_name, r_num) = self.new_unbound_register(0, false)?;
        self.scope[0].insert(Register(r_num), unbound_name);
        Ok(unbound_name)
    }

    /// Declare a (mutable) name in the current lexical scope.
    pub fn declare_name(&mut self, name: &str) -> Option<UnboundName> {
        let name = Symbol::mk_case_insensitive(name);
        let unbound_name =
            self.new_unbound_named(name, self.scope.len() - 1, false, BindMode::New)?;
        self.scope
            .last_mut()
            .unwrap()
            .insert(Named(name), unbound_name);
        Some(unbound_name)
    }

    /// Declare a (mutable) name in the current lexical scope.
    pub fn declare_const(&mut self, name: &str) -> Option<UnboundName> {
        let name = Symbol::mk_case_insensitive(name);
        let unbound_name =
            self.new_unbound_named(name, self.scope.len() - 1, true, BindMode::New)?;
        self.scope
            .last_mut()
            .unwrap()
            .insert(Named(name), unbound_name);
        Some(unbound_name)
    }

    pub fn declare(&mut self, name: &str, constant: bool, global: bool) -> Option<UnboundName> {
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
        for (
            i,
            Decl {
                identifier: binding,
                ..
            },
        ) in self.unbound_names.iter().enumerate()
        {
            if *binding == Named(name) {
                names.push(UnboundName { offset: i });
            }
        }
        names
    }

    /// Find the first scoped name in the name table, if it exists.
    pub fn find_name(&self, name: &str) -> Option<UnboundName> {
        let name = Named(Symbol::mk_case_insensitive(name));
        for scope in self.scope.iter().rev() {
            if let Some(n) = scope.get(&name) {
                return Some(*n);
            }
        }
        None
    }

    /// Create a new unbound variable.
    fn new_unbound_named(
        &mut self,
        name: Symbol,
        scope: usize,
        constant: bool,
        bind_mode: BindMode,
    ) -> Option<UnboundName> {
        // If the variable already exists in this scope, return None
        if bind_mode == BindMode::New && self.scope[scope].contains_key(&Named(name)) {
            return None;
        }
        let idx = self.unbound_names.len();
        self.unbound_names.push(Decl {
            identifier: Named(name),
            depth: scope,
            constant,
        });
        Some(UnboundName { offset: idx })
    }

    fn new_unbound_register(
        &mut self,
        scope: usize,
        constant: bool,
    ) -> Result<(UnboundName, u16), CompileError> {
        let idx = self.unbound_names.len();
        let r_num = self.num_registers;
        self.num_registers += 1;
        self.unbound_names.push(Decl {
            identifier: Register(r_num),
            depth: scope,
            constant,
        });
        Ok((UnboundName { offset: idx }, r_num))
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
            bound.push(self.unbound_names[idx].identifier.clone());
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
    bound: Vec<Binding>,
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
            let Named(sym) = n else {
                continue;
            };
            if sym.as_str() == name {
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

    /// Return the symbol value of the given name offset, if it has one
    pub fn name_of(&self, name: &Name) -> Option<Symbol> {
        if name.0 as usize >= self.bound.len() {
            return None;
        }
        let Named(name) = self.bound[name.0 as usize] else {
            return None;
        };
        Some(name)
    }

    pub fn symbols(&self) -> Vec<Symbol> {
        self.bound
            .iter()
            .map(|b| match b {
                Named(s) => *s,
                Register(r_num) => Symbol::mk(&format!("<register_{r_num}>")),
            })
            .collect()
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

#[cfg(test)]
mod tests {
    use crate::UnboundNames;

    /// Verify simple binding of variables with just one scope.
    #[test]
    fn test_bind_global_scope() {
        let mut unbound_names = UnboundNames::new();
        let before_width = unbound_names.unbound_names.len() as u16;
        let ufoo = unbound_names.declare_name("foo").unwrap();
        let ufob = unbound_names.declare_name("fob").unwrap();
        assert_eq!(unbound_names.find_name("foo").unwrap(), ufoo);
        assert_eq!(unbound_names.find_name("fob").unwrap(), ufob);

        let (bound_names, _) = unbound_names.bind();
        let bfoo = bound_names.find_name("foo").unwrap();
        let bfob = bound_names.find_name("fob").unwrap();
        assert_eq!(bfoo.0, before_width);
        assert_eq!(bfob.0, before_width + 1);
        assert_eq!(bound_names.depth_of(&bfoo).unwrap(), 0);
        assert_eq!(bound_names.depth_of(&bfob).unwrap(), 0);
        assert_eq!(bound_names.global_width as u16, before_width + 2);
    }

    #[test]
    fn test_bind_global_scope_w_register() {
        let mut unbound_names = UnboundNames::new();
        let before_width = unbound_names.unbound_names.len() as u16;
        let ufoo = unbound_names.declare_name("foo").unwrap();
        let ufob = unbound_names.declare_name("fob").unwrap();
        let u_reg = unbound_names.declare_register().unwrap();
        assert_eq!(unbound_names.find_name("foo").unwrap(), ufoo);
        assert_eq!(unbound_names.find_name("fob").unwrap(), ufob);

        let (bound_names, mappings) = unbound_names.bind();
        let bfoo = bound_names.find_name("foo").unwrap();
        let bfob = bound_names.find_name("fob").unwrap();
        let b_reg = mappings.get(&u_reg).unwrap();
        assert_eq!(bfoo.0, before_width);
        assert_eq!(bfob.0, before_width + 1);
        assert_eq!(b_reg.0, before_width + 2);
        assert_eq!(bound_names.depth_of(&bfoo).unwrap(), 0);
        assert_eq!(bound_names.depth_of(&bfob).unwrap(), 0);
        assert_eq!(bound_names.depth_of(b_reg).unwrap(), 0);
        assert_eq!(bound_names.global_width as u16, before_width + 3);
    }

    #[test]
    fn test_register_inside_scope() {
        let mut unbound_names = UnboundNames::new();
        let before_width = unbound_names.unbound_names.len() as u16;

        let x = unbound_names.declare_name("x").unwrap();
        unbound_names.push_scope();
        let v = unbound_names.declare_register().unwrap();
        let y = unbound_names.declare_name("y").unwrap();
        assert_eq!(unbound_names.find_name("y").unwrap(), y);
        unbound_names.pop_scope();
        let z = unbound_names.declare_name("z").unwrap();

        assert_eq!(unbound_names.find_name("x").unwrap(), x);
        assert_eq!(unbound_names.find_name("z").unwrap(), z);

        let (bound_names, mappings) = unbound_names.bind();
        let bx = bound_names.find_name("x").unwrap();
        let by = bound_names.find_name("y").unwrap();
        let bz = bound_names.find_name("z").unwrap();
        let bv = mappings.get(&v).unwrap();

        assert_eq!(bound_names.scope_depth[bx.0 as usize], 0);
        assert_eq!(bound_names.scope_depth[by.0 as usize], 1);
        assert_eq!(bound_names.scope_depth[bv.0 as usize], 0);
        assert_eq!(bound_names.scope_depth[bz.0 as usize], 0);

        assert_eq!(bx.0, before_width);
        assert_eq!(bound_names.global_width as u16, before_width + 3);
    }

    #[test]
    fn test_bind_simple_nested_scope() {
        let mut unbound_names = UnboundNames::new();
        let before_width = unbound_names.unbound_names.len() as u16;
        let ufoo = unbound_names.declare_name("foo").unwrap();
        unbound_names.push_scope();
        let ufob = unbound_names.declare_name("fob").unwrap();
        assert_eq!(unbound_names.find_name("foo").unwrap(), ufoo);
        assert_eq!(unbound_names.find_name("fob").unwrap(), ufob);
        unbound_names.pop_scope();
        assert!(unbound_names.find_name("fob").is_none());
        assert_eq!(unbound_names.find_name("foo").unwrap(), ufoo);

        let (bound_names, _) = unbound_names.bind();
        let bfoo = bound_names.find_name("foo").unwrap();
        let bfob = bound_names.find_name("fob").unwrap();
        assert_eq!(bfoo.0, before_width);
        assert_eq!(bfob.0, before_width + 1);
        assert_eq!(bound_names.depth_of(&bfoo).unwrap(), 0);
        assert_eq!(bound_names.depth_of(&bfob).unwrap(), 1);
        assert_eq!(bound_names.global_width as u16, before_width + 1);
    }
}
