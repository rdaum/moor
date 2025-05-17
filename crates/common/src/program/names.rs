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

use crate::program::names::VarName::{Named, Register};
use bincode::{Decode, Encode};
use moor_var::Symbol;
use strum::{Display, EnumCount, EnumIter, FromRepr};

/// A Name is a unique identifier for a variable or register in the program's environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, Hash)]
pub struct Name(pub u16);

/// The set of known variable names that are always set for every verb invocation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromRepr, EnumCount, Display, EnumIter)]
#[repr(usize)]
#[allow(non_camel_case_types, non_snake_case)]
pub enum GlobalName {
    player,
    this,
    caller,
    verb,
    args,
    argstr,
    dobj,
    dobjstr,
    prepstr,
    iobj,
    iobjstr,
}

/// Either a "name" or a register, but with a unique (across scopes) identifier.
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Encode, Decode)]
pub struct Variable {
    pub id: u16,
    pub nr: VarName,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Encode, Decode)]
pub enum VarName {
    Named(Symbol),
    Register(u16),
}

impl Variable {
    pub fn to_symbol(&self) -> Symbol {
        match self.nr {
            Named(sym) => sym,
            Register(r) => Symbol::mk(&format!("<register_{}>", r)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Names {
    /// The set of bound variables and their names.
    pub bound: Vec<Variable>,
    /// The size of the global scope, e.g. the size the environment should be when the frame
    /// is first created.
    pub global_width: usize,
    /// The scope-depth for each variable. E.g. 0 for global and then 1..N for nested scopes.
    pub scope_depth: Vec<u16>,
}

impl Names {
    pub fn new(global_width: usize) -> Self {
        Self {
            bound: vec![],
            global_width,
            scope_depth: vec![],
        }
    }

    pub fn find_name(&self, name: &str) -> Option<Name> {
        for (idx, n) in self.bound.iter().enumerate() {
            let Named(sym) = n.nr else {
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
        let Named(name) = self.bound[name.0 as usize].nr else {
            return None;
        };
        Some(name)
    }

    pub fn symbols(&self) -> Vec<Symbol> {
        self.bound
            .iter()
            .map(|b| match b.nr {
                Named(s) => s,
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
