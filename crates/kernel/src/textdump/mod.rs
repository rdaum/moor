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

/// Representation of the structure of objects verbs etc as read from a LambdaMOO textdump'd db
/// file.
use std::collections::BTreeMap;

pub use load_db::{read_textdump, textdump_load};
use moor_values::var::Objid;
use moor_values::var::Var;
pub use read::TextdumpReader;
pub use write::TextdumpWriter;
pub use write_db::make_textdump;

mod load_db;
mod read;
mod write;
mod write_db;

const VF_READ: u16 = 1;
const VF_WRITE: u16 = 2;
const VF_EXEC: u16 = 4;
const VF_DEBUG: u16 = 10;
const VF_PERMMASK: u16 = 0xf;
const VF_DOBJSHIFT: u16 = 4;
const VF_IOBJSHIFT: u16 = 6;
const VF_OBJMASK: u16 = 0x3;

const VF_ASPEC_NONE: u16 = 0;
const VF_ASPEC_ANY: u16 = 1;
const VF_ASPEC_THIS: u16 = 2;

#[derive(Clone)]
pub struct Verbdef {
    pub name: String,
    pub owner: Objid,
    pub flags: u16,
    pub prep: i16,
}

#[derive(Clone)]
pub struct Propval {
    pub value: Var,
    pub owner: Objid,
    pub flags: u8,
    pub is_clear: bool,
}

pub struct Object {
    pub id: Objid,
    pub owner: Objid,
    pub location: Objid,
    pub contents: Objid,
    pub next: Objid,
    pub parent: Objid,
    pub child: Objid,
    pub sibling: Objid,
    pub name: String,
    pub flags: u8,
    pub verbdefs: Vec<Verbdef>,
    pub propdefs: Vec<String>,
    pub propvals: Vec<Propval>,
}

#[derive(Clone, Debug)]
pub struct Verb {
    pub objid: Objid,
    pub verbnum: usize,
    pub program: Option<String>,
}

pub struct Textdump {
    #[allow(dead_code)]
    pub version: String,
    pub objects: BTreeMap<Objid, Object>,
    #[allow(dead_code)]
    pub users: Vec<Objid>,
    pub verbs: BTreeMap<(Objid, usize), Verb>,
}

const PREP_ANY: i16 = -2;
const PREP_NONE: i16 = -1;
