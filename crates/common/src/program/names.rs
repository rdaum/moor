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

use crate::program::Decl;
use crate::program::names::VarName::{Named, Register};
use bincode::{Decode, Encode};
use moor_var::Symbol;
use std::collections::HashMap;
use strum::{Display, EnumCount, EnumIter, FromRepr};

/// A Name is a unique identifier for a variable or register in the program's environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, Hash)]
pub struct Name(pub u16, pub u8);

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
    pub scope_id: usize,
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
    pub bound: HashMap<Name, Variable>,
    /// The size of the global scope, e.g. the size the environment should be when the frame
    /// is first created.
    pub global_width: usize,
    /// The variable decl for each variable
    pub decls: HashMap<Name, Decl>,
}

impl Names {
    pub fn new(global_width: usize) -> Self {
        Self {
            bound: HashMap::new(),
            decls: HashMap::new(),
            global_width,
        }
    }

    pub fn find_name(&self, name: impl Into<Symbol>) -> Option<Name> {
        let name = name.into();
        for (n, vr) in self.bound.iter() {
            let Named(sym) = vr.nr else {
                continue;
            };
            if sym == name {
                return Some(*n);
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
        self.bound.get(name).map(|v| match v.nr {
            Named(sym) => sym,
            Register(_) => Symbol::mk(&format!("<register_{}>", name.0)),
        })
    }

    pub fn symbols(&self) -> Vec<Symbol> {
        self.bound
            .values()
            .map(|vr| match vr.nr {
                Named(s) => s,
                Register(r_num) => Symbol::mk(&format!("<register_{r_num}>")),
            })
            .collect()
    }

    pub fn names(&self) -> Vec<Name> {
        self.bound.keys().copied().collect()
    }
}
