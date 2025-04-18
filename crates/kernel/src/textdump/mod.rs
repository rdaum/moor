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

pub use load_textdump::{read_textdump, textdump_load};
use moor_var::Obj;
use moor_var::Var;
pub use read::TextdumpReader;
use serde::{Deserialize, Serialize};
/// Representation of the structure of objects verbs etc as read from a LambdaMOO textdump'd db
/// file.
use std::collections::BTreeMap;
use std::str::FromStr;
pub use write::TextdumpWriter;
pub use write_textdump::make_textdump;

mod load_textdump;
mod read;
mod write;
mod write_textdump;

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

/// What mode to use for strings that contain non-ASCII characters.
///
/// Note that LambdaMOO imports are always in ISO-8859-1, but exports can be in UTF-8.
/// To make things backwards compatible to LambdaMOO servers, choose ISO-8859-1.
/// The default is UTF-8.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EncodingMode {
    // windows-1252 / ISO-8859-1
    ISO8859_1,
    #[default]
    UTF8,
}

impl TryFrom<&str> for EncodingMode {
    type Error = &'static str;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "ISO-8859-1" | "iso-8859-1" | "iso8859-1" => Ok(EncodingMode::ISO8859_1),
            "UTF8" | "UTF-8" | "utf8" | "utf-8" => Ok(EncodingMode::UTF8),
            _ => Err("Invalid encoding mode"),
        }
    }
}

impl FromStr for EncodingMode {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        EncodingMode::try_from(s)
    }
}

#[derive(Clone)]
pub struct Verbdef {
    pub name: String,
    pub owner: Obj,
    pub flags: u16,
    pub prep: i16,
}

#[derive(Clone)]
pub struct Propval {
    pub value: Var,
    pub owner: Obj,
    pub flags: u8,
    pub is_clear: bool,
}

pub struct Object {
    pub id: Obj,
    pub owner: Obj,
    pub location: Obj,
    pub contents: Obj,
    pub next: Obj,
    pub parent: Obj,
    pub child: Obj,
    pub sibling: Obj,
    pub name: String,
    pub flags: u8,
    pub verbdefs: Vec<Verbdef>,
    pub propdefs: Vec<String>,
    pub propvals: Vec<Propval>,
}

#[derive(Clone, Debug)]
pub struct Verb {
    pub objid: Obj,
    pub verbnum: usize,
    pub program: Option<String>,
}

pub struct Textdump {
    pub version_string: String,
    pub objects: BTreeMap<Obj, Object>,
    #[allow(dead_code)]
    pub users: Vec<Obj>,
    pub verbs: BTreeMap<(Obj, usize), Verb>,
}

const PREP_ANY: i16 = -2;
const PREP_NONE: i16 = -1;
