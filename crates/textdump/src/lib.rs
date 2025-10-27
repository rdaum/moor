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
use moor_compiler::CompileOptions;
use moor_var::{Obj, Symbol, Var};
pub use read::TextdumpReader;
use semver::Version;
use serde::{Deserialize, Serialize};
/// Representation of the structure of objects verbs etc as read from a LambdaMOO textdump'd db
/// file.
use std::collections::BTreeMap;
use std::str::FromStr;
use strum::{Display, FromRepr};
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

#[derive(Debug, Eq, PartialEq)]
pub enum TextdumpVersion {
    LambdaMOO(LambdaMOODBVersion),
    ToastStunt(ToastStuntDBVersion),
    Moor(Version, CompileOptions, EncodingMode),
}

/// Versions corresponding to ToastStunt's version.h
#[repr(u16)]
#[derive(Debug, Eq, PartialEq, Display, Ord, PartialOrd, Copy, Clone, FromRepr)]
pub enum LambdaMOODBVersion {
    DbvPrehistory = 0, // Before format versions
    DbvExceptions = 1, // Addition of the `try', `except', `finally', and `endtry' keywords.
    DbvBreakCont = 2,  // Addition of the `break' and `continue' keywords.
    DbvFloat = 3, // Addition of `FLOAT' and `INT' variables and the `E_FLOAT' keyword, along with version numbers on each frame of a suspended task.
    DbvBfbugFixed = 4, // Bug in built-in function overrides fixed by making it use tail-calling. This DB_Version change exists solely to turn off special bug handling in read_bi_func_data().
}

#[repr(u16)]
#[derive(Debug, Eq, PartialEq, Display, Ord, PartialOrd, Copy, Clone, FromRepr)]
pub enum ToastStuntDBVersion {
    ToastDbvNextGen = 5, // Introduced the next-generation database format which fixes the data locality problems in the v4 format.
    ToastDbvTaskLocal = 6, // Addition of task local value.
    ToastDbvMap = 7,     // Addition of `MAP' variables
    ToastDbvFileIo = 8,  // Includes addition of the 'E_FILE' keyword.
    ToastDbvExec = 9,    // Includes addition of the 'E_EXEC' keyword.
    ToastDbvInterrupt = 10, // Includes addition of the 'E_INTRPT' keyword.
    ToastDbvThis = 11,   // Varification of `this'.
    ToastDbvIter = 12,   // Addition of map iterator
    ToastDbvAnon = 13,   // Addition of anonymous objects
    ToastDbvWaif = 14,   // Addition of waifs
    ToastDbvLastMove = 15, // Addition of the 'last_move' built-in property
    ToastDbvThreaded = 16, // Store threading information
    ToastDbvBool = 17,   // Boolean type
}

impl TextdumpVersion {
    pub fn parse(s: &str) -> Option<TextdumpVersion> {
        if s.starts_with("** LambdaMOO Database, Format Version ") {
            let version = s
                .trim_start_matches("** LambdaMOO Database, Format Version ")
                .trim_end_matches(" **");
            let version = version.parse::<u16>().ok()?;
            // For now anything over 4 is assumed to be ToastStunt
            if version > 4 {
                return Some(TextdumpVersion::ToastStunt(ToastStuntDBVersion::from_repr(
                    version,
                )?));
            } else {
                return Some(TextdumpVersion::LambdaMOO(LambdaMOODBVersion::from_repr(
                    version,
                )?));
            }
        } else if s.starts_with("Moor ") {
            let parts = s.split(", ").collect::<Vec<_>>();
            let version = parts.iter().find(|s| s.starts_with("Moor "))?;
            let version = version.trim_start_matches("Moor ");
            // "Moor 0.1.0, features: "flyweight_type=true lexical_scopes=true", encoding: UTF8"
            let semver = version.split(' ').next()?;
            let semver = semver::Version::parse(semver).ok()?;
            let features = parts.iter().find(|s| s.starts_with("features: "))?;
            let features = features
                .trim_start_matches("features: \"")
                .trim_end_matches("\"");
            let features = features.split(' ').collect::<Vec<_>>();
            let features = CompileOptions {
                flyweight_type: features.iter().any(|s| s == &"flyweight_type=true"),
                lexical_scopes: features.iter().any(|s| s == &"lexical_scopes=true"),
                ..Default::default()
            };
            let encoding = parts.iter().find(|s| s.starts_with("encoding: "))?;
            let encoding = encoding.trim_start_matches("encoding: ");
            let encoding = EncodingMode::try_from(encoding).ok()?;
            return Some(TextdumpVersion::Moor(semver, features, encoding));
        }
        None
    }

    pub fn to_version_string(&self) -> String {
        match self {
            TextdumpVersion::LambdaMOO(v) => {
                format!("** LambdaMOO Database, Format Version {v} **")
            }
            TextdumpVersion::ToastStunt(v) => {
                unimplemented!("ToastStunt dump format ({v}) not supported for output");
            }
            TextdumpVersion::Moor(v, features, encoding) => {
                let features = format!(
                    "flyweight_type={} lexical_scopes={}",
                    features.flyweight_type, features.lexical_scopes,
                );
                format!("Moor {v}, features: \"{features}\", encoding: {encoding:?}")
            }
        }
    }
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
    pub propdefs: Vec<Symbol>,
    pub propvals: Vec<Propval>,
}

#[derive(Clone, Debug)]
pub struct Verb {
    pub objid: Obj,
    pub verbnum: usize,
    pub program: Option<String>,
    pub start_line: usize,
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

#[cfg(test)]
mod tests {
    use crate::{LambdaMOODBVersion, TextdumpVersion};
    use moor_compiler::CompileOptions;

    #[test]
    fn parse_textdump_version_lambda() {
        let version = super::TextdumpVersion::parse("** LambdaMOO Database, Format Version 4 **");
        assert_eq!(
            version,
            Some(super::TextdumpVersion::LambdaMOO(
                LambdaMOODBVersion::DbvBfbugFixed
            ))
        );
    }

    #[test]
    fn parse_textdump_version_moor() {
        let td = TextdumpVersion::Moor(
            semver::Version::parse("0.1.0").unwrap(),
            CompileOptions {
                flyweight_type: true,
                lexical_scopes: true,
                ..Default::default()
            },
            super::EncodingMode::UTF8,
        );
        let version = td.to_version_string();
        let parsed = TextdumpVersion::parse(&version);
        assert_eq!(parsed, Some(td));
    }
}
