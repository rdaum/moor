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
use moor_common::program::{Decl, DeclType};
use moor_var::Symbol;
use std::collections::HashMap;
use strum::IntoEnumIterator;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VarScope {
    pub variables: Vec<Decl>,
    pub scopes: Vec<Vec<Variable>>,
    pub scope_id_stack: Vec<usize>,
    pub num_registers: u16,
    pub scope_id_seq: usize,
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
            scope_id_seq: 0,
            scope_id_stack: vec![0],
        }
    }
}

impl VarScope {
    pub fn new() -> Self {
        let mut names = Self::default();
        for global in GlobalName::iter() {
            names
                .find_or_add_name_global(global.to_string().as_str(), DeclType::Global)
                .unwrap();
        }
        names
    }

    /// Find a variable name, or declare in global scope.
    pub fn find_or_add_name_global(&mut self, name: &str, decl_type: DeclType) -> Option<Variable> {
        let name = Symbol::mk_case_insensitive(name);

        // Check the current scopes, starting at the back (innermost scope)
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
        let unbound_name = self.new_unbound_variable(name, 0, false, BindMode::Reuse, decl_type)?;
        self.scopes[0].push(unbound_name);
        Some(unbound_name)
    }

    pub fn find_decl(&self, p0: &Variable) -> Option<&Decl> {
        self.variables
            .iter()
            .find(|d| d.identifier.eq(p0))
            .or_else(|| {
                // If the variable is a register, find it by id.
                if let Register(r) = p0.nr {
                    self.variables.iter().find(|d| d.identifier.id == r)
                } else {
                    None
                }
            })
    }

    /// Start a new lexical scope.
    pub fn enter_new_scope(&mut self) {
        self.scope_id_seq += 1;
        self.scope_id_stack.push(self.scope_id_seq);
        self.scopes.push(Vec::new());
    }

    /// Pop the current scope.
    pub fn exit_scope(&mut self) -> usize {
        let scope = self.scopes.pop().unwrap();
        self.scope_id_stack.pop().unwrap();
        scope.len()
    }

    pub fn declare_register(&mut self) -> Result<Variable, CompileError> {
        let (unbound_name, _) = self.new_unbound_register(0, false)?;
        self.scopes[0].push(unbound_name);
        Ok(unbound_name)
    }

    pub fn declare_or_use_name(&mut self, name: &str, decl_type: DeclType) -> Variable {
        let name = Symbol::mk_case_insensitive(name);
        let unbound_name = self
            .new_unbound_variable(
                name,
                self.scopes.len() - 1,
                false,
                BindMode::Reuse,
                decl_type,
            )
            .unwrap();
        self.scopes.last_mut().unwrap().push(unbound_name);
        unbound_name
    }

    /// Declare a (mutable) name in the current lexical scope.
    pub fn declare_name(&mut self, name: &str, decl_type: DeclType) -> Option<Variable> {
        let name = Symbol::mk_case_insensitive(name);
        let unbound_name = self.new_unbound_variable(
            name,
            self.scopes.len() - 1,
            false,
            BindMode::New,
            decl_type,
        )?;
        self.scopes.last_mut().unwrap().push(unbound_name);
        Some(unbound_name)
    }

    /// Declare a (mutable) name in the current lexical scope.
    pub fn declare_const(&mut self, name: &str, decl_type: DeclType) -> Option<Variable> {
        let name = Symbol::mk_case_insensitive(name);
        let unbound_name =
            self.new_unbound_variable(name, self.scopes.len() - 1, true, BindMode::New, decl_type)?;
        self.scopes.last_mut().unwrap().push(unbound_name);
        Some(unbound_name)
    }

    pub fn declare(
        &mut self,
        name: &str,
        constant: bool,
        global: bool,
        decl_type: DeclType,
    ) -> Option<Variable> {
        if global {
            return self.find_or_add_name_global(name, decl_type);
        }
        if constant {
            return self.declare_const(name, decl_type);
        }
        self.declare_name(name, decl_type)
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
        decl_type: DeclType,
    ) -> Option<Variable> {
        // If the variable already exists in this scope && we're New, return None

        let scope = &self.scopes[scope_depth];
        for n in scope.iter() {
            if n.nr == Named(name) {
                if bind_mode == BindMode::New {
                    return None;
                } else {
                    // If we're re-using, return the existing variable.
                    return Some(*n);
                }
            }
        }
        let id = self.variables.len() as u16;
        let scope_id = self.scope_id_stack[scope_depth];
        let vr = Variable {
            id,
            nr: Named(name),
            scope_id,
        };
        self.variables.push(Decl {
            identifier: vr,
            depth: scope_depth,
            constant,
            decl_type,
            scope_id,
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
        let scope_id = self.scope_id_stack[0];
        let vr = Variable {
            id,
            nr: Register(r_num),
            scope_id,
        };
        self.variables.push(Decl {
            identifier: vr,
            depth: scope,
            constant,
            decl_type: DeclType::Register,
            scope_id,
        });
        Ok((vr, r_num))
    }

    pub fn decl_for_mut(&mut self, name: &Variable) -> &mut Decl {
        self.variables
            .iter_mut()
            .find(|d| d.identifier.eq(name))
            .unwrap()
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

        let mut sorted_by_depth = self.variables.clone();
        sorted_by_depth.sort_by(|a, b| a.depth.cmp(&b.depth));

        let mut current_offset = 0;
        let mut current_scope = 0;
        let mut bound = HashMap::new();
        let mut decls = HashMap::new();
        for vr in sorted_by_depth.iter() {
            if vr.identifier.scope_id != current_scope {
                // We've moved to a new scope.
                current_scope = vr.identifier.scope_id;
                current_offset = 0;
            }
            let offset = current_offset;
            current_offset += 1;
            let name = Name(offset as u16, vr.depth as u8);
            bound.insert(name, vr.identifier);
            mapping.insert(vr.identifier, name);
            decls.insert(name, vr.clone());
        }

        let global_width = self.scopes[0].len();
        (
            Names {
                bound,
                global_width,
                decls,
            },
            mapping,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::var_scope::{DeclType, VarScope};

    /// Verify simple binding of variables with just one scope.
    #[test]
    fn test_bind_global_scope() {
        let mut unbound_names = VarScope::new();
        let before_width = unbound_names.variables.len() as u16;
        let ufoo = unbound_names.declare_name("foo", DeclType::Let).unwrap();
        let ufob = unbound_names.declare_name("fob", DeclType::Let).unwrap();
        assert_eq!(unbound_names.find_name("foo").unwrap(), ufoo);
        assert_eq!(unbound_names.find_name("fob").unwrap(), ufob);

        let (bound_names, _) = unbound_names.bind();
        let bfoo = bound_names.find_name("foo").unwrap();
        let bfob = bound_names.find_name("fob").unwrap();
        assert_eq!(bfoo.0, before_width);
        assert_eq!(bfob.0, before_width + 1);
        assert_eq!(bfoo.1, 0);
        assert_eq!(bfob.1, 0);
        assert_eq!(bound_names.global_width as u16, before_width + 2);
    }

    #[test]
    fn test_bind_global_scope_w_register() {
        let mut unbound_names = VarScope::new();
        let before_width = unbound_names.variables.len() as u16;
        let ufoo = unbound_names.declare_name("foo", DeclType::Let).unwrap();
        let ufob = unbound_names.declare_name("fob", DeclType::Let).unwrap();
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
        assert_eq!(bound_names.global_width as u16, before_width + 3);
    }

    #[test]
    fn test_register_inside_scope() {
        let mut unbound_names = VarScope::new();
        let before_width = unbound_names.variables.len() as u16;

        let x = unbound_names.declare_name("x", DeclType::Let).unwrap();
        unbound_names.enter_new_scope();
        let v = unbound_names.declare_register().unwrap();
        let y = unbound_names.declare_name("y", DeclType::Let).unwrap();
        assert_eq!(unbound_names.find_name("y").unwrap(), y);
        unbound_names.exit_scope();
        let z = unbound_names.declare_name("z", DeclType::Let).unwrap();

        assert_eq!(unbound_names.find_name("x").unwrap(), x);
        assert_eq!(unbound_names.find_name("z").unwrap(), z);

        let (bound_names, mappings) = unbound_names.bind();
        let bx = bound_names.find_name("x").unwrap();
        let by = bound_names.find_name("y").unwrap();
        let bz = bound_names.find_name("z").unwrap();
        let bv = mappings.get(&v).unwrap();

        assert_eq!(bx.1, 0);
        assert_eq!(by.1, 1);
        assert_eq!(bz.1, 0);
        assert_eq!(bv.1, 0);

        assert_eq!(bx.0, before_width);
        assert_eq!(bound_names.global_width as u16, before_width + 3);
    }

    #[test]
    fn test_bind_simple_nested_scope() {
        let mut unbound_names = VarScope::new();
        let before_width = unbound_names.variables.len() as u16;
        let ufoo = unbound_names.declare_name("foo", DeclType::Let).unwrap();
        unbound_names.enter_new_scope();
        let ufob = unbound_names.declare_name("fob", DeclType::Let).unwrap();
        assert_eq!(unbound_names.find_name("foo").unwrap(), ufoo);
        assert_eq!(unbound_names.find_name("fob").unwrap(), ufob);
        unbound_names.exit_scope();
        assert!(unbound_names.find_name("fob").is_none());
        assert_eq!(unbound_names.find_name("foo").unwrap(), ufoo);

        let (bound_names, _) = unbound_names.bind();
        let bfoo = bound_names.find_name("foo").unwrap();
        let bfob = bound_names.find_name("fob").unwrap();
        assert_eq!(bfoo.0, before_width);
        assert_eq!(bfob.0, 0);
        assert_eq!(bfob.1, 1);
        assert_eq!(bound_names.global_width as u16, before_width + 1);
    }

    #[test]
    fn test_bind_shadowed_variable() {
        let mut unbound_names = VarScope::new();
        unbound_names.declare_name("foo", DeclType::Let).unwrap();
        unbound_names.enter_new_scope();
        let ufoo2 = unbound_names.declare_name("foo", DeclType::Let).unwrap();
        assert_eq!(unbound_names.find_name("foo").unwrap(), ufoo2);
        unbound_names.exit_scope();
    }
}
