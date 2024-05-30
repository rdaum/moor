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
use moor_values::var::Objid;
use moor_values::var::Var;

/// Interface exposed to be used by the textdump loader. Overlap of functionality with what
/// WorldState could provide, but potentially different constraints/semantics (e.g. no perms checks)

pub trait LoaderInterface {
    /// For reading textdumps...
    fn create_object(
        &self,
        objid: Option<Objid>,
        attrs: &ObjAttrs,
    ) -> Result<Objid, WorldStateError>;
    fn set_object_parent(&self, obj: Objid, parent: Objid) -> Result<(), WorldStateError>;

    fn set_object_location(&self, o: Objid, location: Objid) -> Result<(), WorldStateError>;
    fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), WorldStateError>;

    fn add_verb(
        &self,
        obj: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Vec<u8>,
    ) -> Result<(), WorldStateError>;

    fn define_property(
        &self,
        definer: Objid,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    fn set_property(
        &self,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    fn commit(&self) -> Result<CommitResult, WorldStateError>;

    // For writing textdumps...

    /// Get the list of all active objects in the database
    fn get_objects(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the list of all players.
    fn get_players(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the attributes of a given object
    fn get_object(&self, objid: Objid) -> Result<ObjAttrs, WorldStateError>;

    /// Get the verbs living on a given object
    fn get_object_verbs(&self, objid: Objid) -> Result<VerbDefs, WorldStateError>;

    /// Get the binary for a given verb
    fn get_verb_binary(&self, objid: Objid, uuid: Uuid) -> Result<Vec<u8>, WorldStateError>;

    /// Get the properties defined on a given object
    fn get_object_properties(&self, objid: Objid) -> Result<PropDefs, WorldStateError>;

    fn get_property_value(
        &self,
        obj: Objid,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError>;

    /// Returns all the property values from the root of the inheritance hierarchy down to the
    /// bottom, for the given object.
    fn get_all_property_values(
        &self,
        objid: Objid,
    ) -> Result<Vec<(PropDef, (Option<Var>, PropPerms))>, WorldStateError>;
}
