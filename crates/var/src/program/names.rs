// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use crate::{
    Symbol,
    program::{
        Decl,
        names::VarName::{Named, Register},
    },
};
use std::collections::HashMap;
use strum::{Display, EnumCount, EnumIter, FromRepr};

/// A Name is a unique identifier for a variable or register in the program's environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(
    pub u16, /* offset */
    pub u8,  /* scope depth */
    pub u16, /* scope id */
);

/// The set of known variable names that are always set for every verb invocation.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, FromRepr, EnumCount, Display, EnumIter,
)]
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
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Variable {
    pub id: u16,
    pub scope_id: u16,
    pub nr: VarName,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum VarName {
    Named(Symbol),
    Register(u16),
}

impl Variable {
    pub fn to_symbol(&self) -> Symbol {
        match self.nr {
            Named(sym) => sym,
            Register(r) => Symbol::mk(&format!("<register_{r}>")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Names {
    /// The size of the global scope, e.g. the size the environment should be when the frame
    /// is first created.
    pub global_width: usize,
    /// The variable decl for each variable
    pub decls: HashMap<Name, Decl>,
}

impl Names {
    pub fn new(global_width: usize) -> Self {
        Self {
            decls: HashMap::new(),
            global_width,
        }
    }

    /// The size of the global scope section of the environment, e.g. the "environment_width" of the
    /// frame's environment when it is first created.
    pub fn global_width(&self) -> usize {
        self.global_width
    }

    pub fn find_variable(&self, name: &Name) -> Option<&Variable> {
        self.decls.get(name).map(|decl| &decl.identifier)
    }

    /// Return the symbol value of the given name offset, if it has one
    pub fn ident_for_name(&self, name: &Name) -> Option<Symbol> {
        for (v_name, decl) in self.decls.iter() {
            if v_name == name {
                return match decl.identifier.nr {
                    Named(sym) => Some(sym),
                    Register(r_num) => Some(Symbol::mk(&format!("<register_{r_num}>"))),
                };
            }
        }
        None
    }

    pub fn name_for_var(&self, var: &Variable) -> Option<Name> {
        for (n, decl) in self.decls.iter() {
            if &decl.identifier == var {
                return Some(*n);
            }
        }
        None
    }

    pub fn name_for_ident(&self, name: impl Into<Symbol>) -> Option<Name> {
        let name = name.into();
        for (n, decl) in self.decls.iter() {
            let decl_name = &decl.identifier.nr;
            match decl_name {
                Named(sym) if *sym == name => return Some(*n),
                _ => {
                    // Continue searching
                }
            }
        }
        None
    }

    pub fn symbols(&self) -> Vec<Symbol> {
        let names = self.decls.iter().filter_map(|(_, decl)| {
            if let Named(sym) = decl.identifier.nr {
                Some(sym)
            } else {
                None
            }
        });
        names.collect()
    }

    pub fn names(&self) -> Vec<Name> {
        self.decls.keys().cloned().collect()
    }
}
