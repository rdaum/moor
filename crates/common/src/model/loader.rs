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

use uuid::Uuid;

use crate::model::ObjSet;
use crate::model::PropFlag;
use crate::model::VerbArgsSpec;
use crate::model::VerbDefs;
use crate::model::VerbFlag;
use crate::model::{CommitResult, WorldStateError};
use crate::model::{ObjAttrs, PropPerms};
use crate::model::{PropDef, PropDefs};
use crate::util::BitEnum;
use moor_var::Var;
use moor_var::program::ProgramType;
use moor_var::{Obj, Symbol};

/// Interface for read-only access to database snapshots for exporting/dumping
pub trait SnapshotInterface: Send {
    /// Get the list of all active objects in the database
    fn get_objects(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the list of all players.
    fn get_players(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the attributes of a given object
    fn get_object(&self, objid: &Obj) -> Result<ObjAttrs, WorldStateError>;

    /// Get the verbs living on a given object
    fn get_object_verbs(&self, objid: &Obj) -> Result<VerbDefs, WorldStateError>;

    /// Get the binary for a given verb
    fn get_verb_program(&self, objid: &Obj, uuid: Uuid) -> Result<ProgramType, WorldStateError>;

    /// Get the properties defined on a given object
    fn get_object_properties(&self, objid: &Obj) -> Result<PropDefs, WorldStateError>;

    fn get_property_value(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError>;

    /// Returns all the property common from the root of the inheritance hierarchy down to the
    /// bottom, for the given object.
    #[allow(clippy::type_complexity)]
    fn get_all_property_values(
        &self,
        objid: &Obj,
    ) -> Result<Vec<(PropDef, (Option<Var>, PropPerms))>, WorldStateError>;
}

/// Interface exposed to be used by the textdump/objdef loader for loading data into the database.
/// Overlap of functionality with what WorldState could provide, but potentially different
/// constraints/semantics (e.g. no perms checks)
pub trait LoaderInterface: Send {
    /// Create a new object with the given attributes
    fn create_object(
        &mut self,
        objid: Option<Obj>,
        attrs: &ObjAttrs,
    ) -> Result<Obj, WorldStateError>;

    /// Set the parent of an object
    fn set_object_parent(&mut self, obj: &Obj, parent: &Obj) -> Result<(), WorldStateError>;

    /// Set the location of an object
    fn set_object_location(&mut self, o: &Obj, location: &Obj) -> Result<(), WorldStateError>;

    /// Set the owner of an object
    fn set_object_owner(&mut self, obj: &Obj, owner: &Obj) -> Result<(), WorldStateError>;

    /// Add a verb to an object
    fn add_verb(
        &mut self,
        obj: &Obj,
        names: &[Symbol],
        owner: &Obj,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        program: ProgramType,
    ) -> Result<(), WorldStateError>;

    /// Define a property on an object
    fn define_property(
        &mut self,
        definer: &Obj,
        objid: &Obj,
        propname: Symbol,
        owner: &Obj,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    /// Set property attributes and value
    fn set_property(
        &mut self,
        objid: &Obj,
        propname: Symbol,
        owner: Option<Obj>,
        flags: Option<BitEnum<PropFlag>>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    /// Get the highest-numbered object in the database
    fn max_object(&self) -> Result<Obj, WorldStateError>;

    /// Commit all changes made through this loader
    fn commit(self: Box<Self>) -> Result<CommitResult, WorldStateError>;
}
