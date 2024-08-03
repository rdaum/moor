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
use strum::IntoEnumIterator;

/// A Name is a unique identifier for a variable in the program's environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, Hash)]
pub struct Name(pub u16);

// TODO: create a proper compiler symbol table which tracks the entrance and exit of lexical scopes

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Names {
    /// The list of names in the program, in order of their appearance, with the offsets into the
    /// vector being the unique identifier for the name.
    names: Vec<Symbol>,
}

impl Default for Names {
    fn default() -> Self {
        Self::new()
    }
}

impl Names {
    pub fn new() -> Self {
        let mut names = Self { names: vec![] };
        for global in GlobalName::iter() {
            names.find_or_add_name(global.to_string().as_str());
        }
        names
    }

    /// Add a name to the name table, if it doesn't already exist.
    /// If it does exist, return the existing name.
    pub fn find_or_add_name(&mut self, name: &str) -> Name {
        let name = Symbol::mk_case_insensitive(name);
        match self.names.iter().position(|n| *n == name) {
            None => {
                let pos = self.names.len();
                self.names.push(name);
                Name(pos as u16)
            }
            Some(n) => Name(n as u16),
        }
    }

    /// Find the name in the name table, if it exists.
    pub fn find_name(&self, name: &str) -> Option<Name> {
        self.find_name_offset(name).map(|x| Name(x as u16))
    }

    /// Return the environment offset of the name, if it exists.
    pub fn find_name_offset(&self, name: &str) -> Option<usize> {
        let name = Symbol::mk_case_insensitive(name);
        self.names.iter().position(|x| *x == name)
    }

    /// Return the width of the name table, to be used as the (total) environment size.
    pub fn width(&self) -> usize {
        self.names.len()
    }

    /// Return the symbol value of the given name offset.
    pub fn name_of(&self, name: &Name) -> Option<Symbol> {
        if name.0 as usize >= self.names.len() {
            return None;
        }
        Some(self.names[name.0 as usize])
    }

    pub fn names(&self) -> &Vec<Symbol> {
        &self.names
    }
}
