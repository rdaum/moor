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

use moor_common::model::CompileError;
use moor_common::program::names::VarName::{Named, Register};
use moor_common::program::names::{GlobalName, Name, Names, Variable};
use moor_var::Symbol;
use std::collections::HashMap;
use strum::IntoEnumIterator;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VarScope {
    variables: Vec<Decl>,
    scopes: Vec<Vec<Variable>>,
    num_registers: u16,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Decl {
    pub identifier: Variable,
    pub depth: usize,
    pub constant: bool,
}

/// Policy for binding a variable when new_bound is called.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BindMode {
    /// If the variable is already bound, use it.
    Reuse,
    /// If the variable exists, return an error.
    New,
}

impl Default for VarScope {
    fn default() -> Self {
        Self {
            variables: vec![],
            scopes: vec![Vec::new()],
            num_registers: 0,
        }
    }
}

impl VarScope {
    pub fn new() -> Self {
        let mut names = Self::default();
        for global in GlobalName::iter() {
            names
                .find_or_add_name_global(global.to_string().as_str())
                .unwrap();
        }
        names
    }

    /// Find a variable name, or declare in global scope.
    pub fn find_or_add_name_global(&mut self, name: &str) -> Option<Variable> {
        let name = Symbol::mk_case_insensitive(name);

        // Check the scopes, starting at the back (innermost scope)
        for scope in self.scopes.iter().rev() {
            for v in scope {
                if let Named(sym) = v.nr {
                    if sym == name {
                        return Some(*v);
                    }
                }
            }
        }

        // If the name doesn't exist, add it to the global scope, since that's how
        // MOO currently works.
        // These types of variables are always mutable, and always re-use a variable name, to
        // maintain existing MOO language semantics.
        let unbound_name = self.new_unbound_variable(name, 0, false, BindMode::Reuse)?;
        self.scopes[0].push(unbound_name);
        Some(unbound_name)
    }

    /// Start a new lexical scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    /// Pop the current scope.
    pub fn pop_scope(&mut self) -> usize {
        let scope = self.scopes.pop().unwrap();
        scope.len()
    }

    pub fn declare_register(&mut self) -> Result<Variable, CompileError> {
        // Registers always exist at the global level, but are unique.
        let (unbound_name, _) = self.new_unbound_register(0, false)?;
        self.scopes[0].push(unbound_name);
        Ok(unbound_name)
    }

    /// Declare a (mutable) name in the current lexical scope.
    pub fn declare_name(&mut self, name: &str) -> Option<Variable> {
        let name = Symbol::mk_case_insensitive(name);
        let unbound_name =
            self.new_unbound_variable(name, self.scopes.len() - 1, false, BindMode::New)?;
        self.scopes.last_mut().unwrap().push(unbound_name);
        Some(unbound_name)
    }

    /// Declare a (mutable) name in the current lexical scope.
    pub fn declare_const(&mut self, name: &str) -> Option<Variable> {
        let name = Symbol::mk_case_insensitive(name);
        let unbound_name =
            self.new_unbound_variable(name, self.scopes.len() - 1, true, BindMode::New)?;
        self.scopes.last_mut().unwrap().push(unbound_name);
        Some(unbound_name)
    }

    pub fn declare(&mut self, name: &str, constant: bool, global: bool) -> Option<Variable> {
        if global {
            return self.find_or_add_name_global(name);
        }
        if constant {
            return self.declare_const(name);
        }
        self.declare_name(name)
    }

    /// If the same named variable exists in multiple scopes, return them all as a vector.
    pub fn find_named(&self, name: &str) -> Vec<Variable> {
        let name = Symbol::mk_case_insensitive(name);
        let mut names = vec![];
        for Decl {
            identifier: binding,
            ..
        } in &self.variables
        {
            if binding.nr == Named(name) {
                names.push(*binding)
            }
        }
        names
    }

    /// Find the first scoped name in the name table, if it exists.
    pub fn find_name(&self, name: &str) -> Option<Variable> {
        let name = Named(Symbol::mk_case_insensitive(name));
        for scope in self.scopes.iter().rev() {
            for n in scope.iter() {
                if n.nr == name {
                    return Some(*n);
                }
            }
        }
        None
    }

    /// Create a new unbound variable.
    fn new_unbound_variable(
        &mut self,
        name: Symbol,
        scope_depth: usize,
        constant: bool,
        bind_mode: BindMode,
    ) -> Option<Variable> {
        // If the variable already exists in this scope, return None
        if bind_mode == BindMode::New {
            let scope = &self.scopes[scope_depth];
            for n in scope.iter() {
                if n.nr == Named(name) {
                    return None;
                }
            }
        }
        let id = self.variables.len() as u16;
        let vr = Variable {
            id,
            nr: Named(name),
        };
        self.variables.push(Decl {
            identifier: vr,
            depth: scope_depth,
            constant,
        });
        Some(vr)
    }

    fn new_unbound_register(
        &mut self,
        scope: usize,
        constant: bool,
    ) -> Result<(Variable, u16), CompileError> {
        let r_num = self.num_registers;
        self.num_registers += 1;
        let id = self.variables.len() as u16;
        let vr = Variable {
            id,
            nr: Register(r_num),
        };
        self.variables.push(Decl {
            identifier: vr,
            depth: scope,
            constant,
        });
        Ok((vr, r_num))
    }

    pub fn decl_for(&self, name: &Variable) -> &Decl {
        self.variables
            .iter()
            .find(|d| d.identifier.eq(name))
            .unwrap()
    }

    /// Turn all unbound variables into bound variables.
    /// Run at the end of compilation to produce valid offsets.
    pub fn bind(&self) -> (Names, HashMap<Variable, Name>) {
        let mut mapping = HashMap::new();
        let mut bound = Vec::with_capacity(self.variables.len());
        // Walk the scopes, binding all unbound variables.
        // This will produce offsets for all variables in the order they should appear in the
        // environment.
        let mut scope_depth = Vec::with_capacity(self.variables.len());
        for (idx, vr) in self.variables.iter().enumerate() {
            let offset = bound.len();
            bound.push(self.variables[idx].identifier);
            scope_depth.push(self.variables[idx].depth as u16);
            mapping.insert(vr.identifier, Name(offset as u16));
        }

        let global_width = self.scopes[0].len();
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

#[cfg(test)]
mod tests {
    use crate::var_scope::VarScope;

    /// Verify simple binding of variables with just one scope.
    #[test]
    fn test_bind_global_scope() {
        let mut unbound_names = VarScope::new();
        let before_width = unbound_names.variables.len() as u16;
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
        let mut unbound_names = VarScope::new();
        let before_width = unbound_names.variables.len() as u16;
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
        let mut unbound_names = VarScope::new();
        let before_width = unbound_names.variables.len() as u16;

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
        let mut unbound_names = VarScope::new();
        let before_width = unbound_names.variables.len() as u16;
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

    #[test]
    fn test_bind_shadowed_variable() {
        let mut unbound_names = VarScope::new();
        unbound_names.declare_name("foo").unwrap();
        unbound_names.push_scope();
        let ufoo2 = unbound_names.declare_name("foo").unwrap();
        assert_eq!(unbound_names.find_name("foo").unwrap(), ufoo2);
        unbound_names.pop_scope();
    }
}
