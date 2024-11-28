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

use bytes::Bytes;
use uuid::Uuid;

use moor_values::model::ObjSet;
use moor_values::model::PropFlag;
use moor_values::model::VerbArgsSpec;
use moor_values::model::VerbDefs;
use moor_values::model::VerbFlag;
use moor_values::model::{CommitResult, WorldStateError};
use moor_values::model::{ObjAttrs, PropPerms};
use moor_values::model::{PropDef, PropDefs};
use moor_values::util::BitEnum;
use moor_values::Obj;
use moor_values::Var;

/// Interface exposed to be used by the textdump loader. Overlap of functionality with what
/// WorldState could provide, but potentially different constraints/semantics (e.g. no perms checks)

pub trait LoaderInterface: Send {
    /// For reading textdumps...
    fn create_object(
        &self,
        objid: Option<Obj>,
        attrs: &ObjAttrs,
    ) -> Result<Obj, WorldStateError>;
    fn set_object_parent(&self, obj: &Obj, parent: &Obj) -> Result<(), WorldStateError>;

    fn set_object_location(&self, o: &Obj, location: &Obj) -> Result<(), WorldStateError>;
    fn set_object_owner(&self, obj: &Obj, owner: &Obj) -> Result<(), WorldStateError>;

    fn add_verb(
        &self,
        obj: &Obj,
        names: Vec<&str>,
        owner: &Obj,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Vec<u8>,
    ) -> Result<(), WorldStateError>;

    fn define_property(
        &self,
        definer: &Obj,
        objid: &Obj,
        propname: &str,
        owner: &Obj,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    fn set_property(
        &self,
        objid: &Obj,
        propname: &str,
        owner: &Obj,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    fn commit(&mut self) -> Result<CommitResult, WorldStateError>;

    // For writing textdumps...

    /// Get the list of all active objects in the database
    fn get_objects(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the list of all players.
    fn get_players(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the attributes of a given object
    fn get_object(&self, objid: &Obj) -> Result<ObjAttrs, WorldStateError>;

    /// Get the verbs living on a given object
    fn get_object_verbs(&self, objid: &Obj) -> Result<VerbDefs, WorldStateError>;

    /// Get the binary for a given verb
    fn get_verb_binary(&self, objid: &Obj, uuid: Uuid) -> Result<Bytes, WorldStateError>;

    /// Get the properties defined on a given object
    fn get_object_properties(&self, objid: &Obj) -> Result<PropDefs, WorldStateError>;

    fn get_property_value(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError>;

    /// Returns all the property values from the root of the inheritance hierarchy down to the
    /// bottom, for the given object.
    #[allow(clippy::type_complexity)]
    fn get_all_property_values(
        &self,
        objid: &Obj,
    ) -> Result<Vec<(PropDef, (Option<Var>, PropPerms))>, WorldStateError>;
}
