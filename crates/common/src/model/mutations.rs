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

//! Object mutation operations for *batch* updates in the LoaderInterface

use crate::{
    model::{
        WorldStateError, r#match::VerbArgsSpec, objects::ObjFlag, props::PropFlag, verbs::VerbFlag,
    },
    util::BitEnum,
};
use moor_var::{Obj, Symbol, Var, program::ProgramType};

/// A single mutation operation to perform on an object
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectMutation {
    // Property operations
    /// Define a new property on the object
    DefineProperty {
        name: Symbol,
        owner: Obj,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    },
    /// Delete a property from the object
    DeleteProperty { name: Symbol },
    /// Update the value of an existing property
    SetPropertyValue { name: Symbol, value: Var },
    /// Update property flags and/or owner
    SetPropertyFlags {
        name: Symbol,
        owner: Option<Obj>,
        flags: BitEnum<PropFlag>,
    },
    /// Clear a property (remove local value, inherit from parent)
    ClearProperty { name: Symbol },

    // Verb operations
    /// Define a new verb on the object
    DefineVerb {
        names: Vec<Symbol>,
        owner: Obj,
        flags: BitEnum<VerbFlag>,
        argspec: VerbArgsSpec,
        program: ProgramType,
    },
    /// Delete a verb from the object (identified by names)
    DeleteVerb { names: Vec<Symbol> },
    /// Update the program code of an existing verb
    UpdateVerbProgram {
        names: Vec<Symbol>,
        program: ProgramType,
    },
    /// Update verb metadata (names, owner, flags, argspec)
    UpdateVerbMetadata {
        names: Vec<Symbol>,
        new_names: Option<Vec<Symbol>>,
        owner: Option<Obj>,
        flags: Option<BitEnum<VerbFlag>>,
        argspec: Option<VerbArgsSpec>,
    },

    // Object attribute operations
    /// Set object flags
    SetObjectFlags { flags: BitEnum<ObjFlag> },
    /// Change the parent of the object
    SetParent { parent: Obj },
    /// Move the object to a new location
    SetLocation { location: Obj },
}

/// Result of applying a single mutation
#[derive(Debug, Clone)]
pub struct MutationResult {
    /// Index of this mutation in the original batch
    pub index: usize,
    /// The mutation that was attempted
    pub mutation: ObjectMutation,
    /// Success or error for this mutation
    pub result: Result<(), WorldStateError>,
}

/// Result of applying a batch of mutations
#[derive(Debug, Clone)]
pub struct BatchMutationResult {
    /// The object that mutations were applied to
    pub target: Obj,
    /// Results for each mutation in order
    pub results: Vec<MutationResult>,
}

impl BatchMutationResult {
    /// Check if all mutations succeeded
    pub fn all_succeeded(&self) -> bool {
        self.results.iter().all(|r| r.result.is_ok())
    }

    /// Get the index and error of the first failed mutation, if any
    pub fn first_error(&self) -> Option<(usize, &WorldStateError)> {
        self.results
            .iter()
            .find_map(|r| r.result.as_ref().err().map(|e| (r.index, e)))
    }

    /// Count of successful mutations
    pub fn succeeded_count(&self) -> usize {
        self.results.iter().filter(|r| r.result.is_ok()).count()
    }

    /// Count of failed mutations
    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|r| r.result.is_err()).count()
    }
}
