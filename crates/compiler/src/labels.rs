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

use crate::names::Name;
use bincode::{Decode, Encode};

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

/// An offset is a program offset; a bit like a jump label, but represents a *relative* program
/// position
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Offset(pub u16);

impl From<usize> for Offset {
    fn from(value: usize) -> Self {
        Offset(value as u16)
    }
}
