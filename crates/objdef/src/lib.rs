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
#![recursion_limit = "256"]

mod dump;
mod load;

use moor_common::model::WorldStateError;
use moor_compiler::ObjDefParseError;
use moor_var::{Obj, Symbol};
use std::io;
use std::path::PathBuf;

pub use dump::{collect_object, collect_object_definitions, dump_object, dump_object_definitions};
pub use load::ObjectDefinitionLoader;

#[derive(Debug, thiserror::Error)]
pub enum DirDumpReaderError {
    #[error("Directory not found: {0}")]
    DirectoryNotFound(PathBuf),
    #[error("Invalid object file name: {0} (should be number)")]
    InvalidObjectFilename(PathBuf),
    #[error("Error reading object file: {1}")]
    ObjectFileReadError(PathBuf, io::Error),
    #[error("Could not parse object definition from {0}: {1}")]
    ObjectDefParseError(String, ObjDefParseError),
    #[error("Could not create object from {0}: {1}")]
    CouldNotCreateObject(String, Obj, WorldStateError),
    #[error("Could not set object parent from {0}: {1}")]
    CouldNotSetObjectParent(String, WorldStateError),
    #[error("Could not set object location from {0}: {1}")]
    CouldNotSetObjectLocation(String, WorldStateError),
    #[error("Could not set object owner from {0}: {1}")]
    CouldNotSetObjectOwner(String, WorldStateError),
    #[error("Could not define property from {0}: {1}:{2}: {3}")]
    CouldNotDefineProperty(String, Obj, String, WorldStateError),
    #[error("Could not override property from {0}: {1}:{2}: {3}")]
    CouldNotOverrideProperty(String, Obj, String, WorldStateError),
    #[error("Could not define verb from {0}: {1}{2:?}: {3}")]
    CouldNotDefineVerb(String, Obj, Vec<Symbol>, WorldStateError),
    #[error("Expected single object definition from {0}, but got {1}")]
    SingleObjectExpected(String, usize),
}

impl DirDumpReaderError {
    pub fn source(&self) -> &str {
        match self {
            DirDumpReaderError::DirectoryNotFound(path) => path.to_str().unwrap_or("<unknown>"),
            DirDumpReaderError::InvalidObjectFilename(path) => path.to_str().unwrap_or("<unknown>"),
            DirDumpReaderError::ObjectFileReadError(path, _) => {
                path.to_str().unwrap_or("<unknown>")
            }
            DirDumpReaderError::ObjectDefParseError(source, _)
            | DirDumpReaderError::CouldNotCreateObject(source, _, _)
            | DirDumpReaderError::CouldNotSetObjectParent(source, _)
            | DirDumpReaderError::CouldNotSetObjectLocation(source, _)
            | DirDumpReaderError::CouldNotSetObjectOwner(source, _)
            | DirDumpReaderError::CouldNotDefineProperty(source, _, _, _)
            | DirDumpReaderError::CouldNotOverrideProperty(source, _, _, _)
            | DirDumpReaderError::CouldNotDefineVerb(source, _, _, _)
            | DirDumpReaderError::SingleObjectExpected(source, _) => source.as_str(),
        }
    }

    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            DirDumpReaderError::DirectoryNotFound(path)
            | DirDumpReaderError::InvalidObjectFilename(path)
            | DirDumpReaderError::ObjectFileReadError(path, _) => Some(path),
            _ => None,
        }
    }
}
