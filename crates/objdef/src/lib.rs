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
    #[error("Could not parse object file {0}: {1}")]
    ObjectFileParseError(PathBuf, ObjDefParseError),
    #[error("Could not create object in {0}: {1}")]
    CouldNotCreateObject(PathBuf, Obj, WorldStateError),
    #[error("Could not set object parent in {0}: {1}")]
    CouldNotSetObjectParent(PathBuf, WorldStateError),
    #[error("Could not set object location in {0}: {1}")]
    CouldNotSetObjectLocation(PathBuf, WorldStateError),
    #[error("Could not set object owner in {0}: {1}")]
    CouldNotSetObjectOwner(PathBuf, WorldStateError),
    #[error("Could not define property in {0}: {1}:{2}: {3}")]
    CouldNotDefineProperty(PathBuf, Obj, String, WorldStateError),
    #[error("Could not override property in {0}: {1}:{2}: {3}")]
    CouldNotOverrideProperty(PathBuf, Obj, String, WorldStateError),
    #[error("Could not define verb in {0}: {1}{2:?}: {3}")]
    CouldNotDefineVerb(PathBuf, Obj, Vec<Symbol>, WorldStateError),
}

impl DirDumpReaderError {
    pub fn path(&self) -> &PathBuf {
        match self {
            DirDumpReaderError::DirectoryNotFound(path)
            | DirDumpReaderError::InvalidObjectFilename(path)
            | DirDumpReaderError::ObjectFileReadError(path, _)
            | DirDumpReaderError::ObjectFileParseError(path, _)
            | DirDumpReaderError::CouldNotCreateObject(path, _, _)
            | DirDumpReaderError::CouldNotSetObjectParent(path, _)
            | DirDumpReaderError::CouldNotSetObjectLocation(path, _)
            | DirDumpReaderError::CouldNotSetObjectOwner(path, _)
            | DirDumpReaderError::CouldNotDefineProperty(path, _, _, _)
            | DirDumpReaderError::CouldNotOverrideProperty(path, _, _, _)
            | DirDumpReaderError::CouldNotDefineVerb(path, _, _, _) => path,
        }
    }
}
