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

mod dump;
mod load;

use moor_compiler::ObjDefParseError;
use moor_values::model::WorldStateError;
use moor_values::{Obj, Symbol};
use std::io;
use std::path::PathBuf;

pub use dump::{collect_object_definitions, dump_object_definitions};
pub use load::ObjectDefinitionLoader;

#[derive(Debug, thiserror::Error)]
pub enum DirDumpReaderError {
    #[error("Directory not found: {0}")]
    DirectoryNotFound(PathBuf),
    #[error("Invalid object file name: {0} (should be number)")]
    InvalidObjectFilename(String),
    #[error("Error reading object file: {0}")]
    ObjectFileReadError(#[from] io::Error),
    #[error("Could not parse object file: {0}")]
    ObjectFileParseError(ObjDefParseError),
    #[error("Could not create object: {0}")]
    CouldNotCreateObject(Obj, WorldStateError),
    #[error("Could not set object parent: {0}")]
    CouldNotSetObjectParent(WorldStateError),
    #[error("Could not set object location: {0}")]
    CouldNotSetObjectLocation(WorldStateError),
    #[error("Could not set object owner: {0}")]
    CouldNotSetObjectOwner(WorldStateError),
    #[error("Could not define property: {0}:{1}: {2}")]
    CouldNotDefineProperty(Obj, String, WorldStateError),
    #[error("Could not override property: {0}:{1}: {2}")]
    CouldNotOverrideProperty(Obj, String, WorldStateError),
    #[error("Could not define verb: {0}{1:?}: {2}")]
    CouldNotDefineVerb(Obj, Vec<Symbol>, WorldStateError),
}
