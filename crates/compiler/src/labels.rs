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

/// A JumpLabel is what a labels resolve to in the program.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct JumpLabel {
    // The unique id for the jump label, which is also its offset in the jump vector.
    pub id: Label,

    // If there's a unique identifier assigned to this label, it goes here.
    pub name: Option<Name>,

    // The temporary and then final resolved position of the label in terms of PC offsets.
    pub position: Offset,
}

/// A Label is a unique identifier for a jump position in the program.
/// A committed, compiled, Label can be resolved to a program offset by looking it up in program's
/// jump vector at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct Label(pub u16);

impl From<usize> for Label {
    fn from(value: usize) -> Self {
        Label(value as u16)
    }
}

impl From<i32> for Label {
    fn from(value: i32) -> Self {
        Label(value as u16)
    }
}

/// A Name is a unique identifier for a variable in the program's environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, Hash)]
pub struct Name(pub u16);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Names {
    pub names: Vec<Symbol>,
}

impl Default for Names {
    fn default() -> Self {
        Self::new()
    }
}

/// An offset is a program offset; a bit like a jump label, but represents a *relative* program
/// position
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Offset(pub u16);

impl From<usize> for Offset {
    fn from(value: usize) -> Self {
        Offset(value as u16)
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

    pub fn find_name(&self, name: &str) -> Option<Name> {
        self.find_name_offset(name).map(|x| Name(x as u16))
    }

    pub fn find_name_offset(&self, name: &str) -> Option<usize> {
        let name = Symbol::mk_case_insensitive(name);
        self.names.iter().position(|x| *x == name)
    }
    pub fn width(&self) -> usize {
        self.names.len()
    }

    pub fn name_of(&self, name: &Name) -> Option<Symbol> {
        if name.0 as usize >= self.names.len() {
            return None;
        }
        Some(self.names[name.0 as usize])
    }
}
