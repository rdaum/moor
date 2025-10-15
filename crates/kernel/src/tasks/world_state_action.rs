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

use moor_common::model::{ObjectRef, PropDef, PropPerms, VerbDef, VerbDefs};
use moor_var::{Obj, Symbol, Var};
use uuid::Uuid;

/// Represents actions that can be performed on the WorldState in a batched transaction.
/// This enum focuses on the operations currently implemented in the scheduler.
#[derive(Debug, Clone)]
pub enum WorldStateAction {
    /// Program a verb with new code
    ProgramVerb {
        player: Obj,
        perms: Obj,
        obj: ObjectRef,
        verb_name: Symbol,
        code: Vec<String>,
    },

    /// Request a system property value
    RequestSystemProperty {
        player: Obj,
        obj: ObjectRef,
        property: Symbol,
    },

    /// Request all properties on an object
    RequestProperties {
        player: Obj,
        perms: Obj,
        obj: ObjectRef,
        inherited: bool,
    },

    /// Request a specific property's info and value
    RequestProperty {
        player: Obj,
        perms: Obj,
        obj: ObjectRef,
        property: Symbol,
    },

    /// Request all verbs on an object
    RequestVerbs {
        player: Obj,
        perms: Obj,
        obj: ObjectRef,
        inherited: bool,
    },

    /// Request verb code and definition
    RequestVerbCode {
        player: Obj,
        perms: Obj,
        obj: ObjectRef,
        verb: Symbol,
    },

    /// Resolve an ObjectRef to an actual object
    ResolveObject { player: Obj, obj: ObjectRef },

    /// Request all objects in the database
    RequestAllObjects { player: Obj },
    // TODO: Add more operations
    // Future additions might include:
    // - CreateObject
    // - SetProperty
    // - AddVerb
    // - DeleteVerb
    // - UpdateVerbFlags
    // - MoveObject
}

/// A request wrapper that includes a correlation ID for tracking
#[derive(Debug, Clone)]
pub struct WorldStateRequest {
    pub id: Uuid,
    pub action: WorldStateAction,
}

/// Response from executing a WorldStateAction
#[derive(Debug, Clone)]
pub enum WorldStateResponse {
    Success {
        id: Uuid,
        result: WorldStateResult,
    },
    Error {
        id: Uuid,
        error: moor_common::tasks::SchedulerError,
    },
}

/// The actual result data from a successful WorldStateAction
#[derive(Debug, Clone)]
pub enum WorldStateResult {
    /// Result of ProgramVerb
    VerbProgrammed { object: Obj, verb: Symbol },

    /// Result of RequestSystemProperty
    SystemProperty(Var),

    /// Result of RequestProperties
    Properties(Vec<(PropDef, PropPerms)>),

    /// Result of RequestProperty
    Property(PropDef, PropPerms, Var),

    /// Result of RequestVerbs
    Verbs(VerbDefs),

    /// Result of RequestVerbCode
    VerbCode(VerbDef, Vec<String>),

    /// Result of ResolveObject
    ResolvedObject(Var), // Either v_obj(oid) or v_err(E_INVIND)

    /// Result of RequestAllObjects
    AllObjects(Vec<Obj>),
}

impl WorldStateRequest {
    pub fn new(action: WorldStateAction) -> Self {
        Self {
            id: Uuid::new_v4(),
            action,
        }
    }
}
