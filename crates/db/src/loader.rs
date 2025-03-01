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

use byteview::ByteView;
use uuid::Uuid;

use moor_common::model::ObjSet;
use moor_common::model::PropFlag;
use moor_common::model::VerbArgsSpec;
use moor_common::model::VerbDefs;
use moor_common::model::VerbFlag;
use moor_common::model::{CommitResult, WorldStateError};
use moor_common::model::{ObjAttrs, PropPerms};
use moor_common::model::{PropDef, PropDefs};
use moor_common::util::BitEnum;
use moor_var::Obj;
use moor_var::Var;

/// Interface exposed to be used by the textdump loader. Overlap of functionality with what
/// WorldState could provide, but potentially different constraints/semantics (e.g. no perms checks)
pub trait LoaderInterface: Send {
    /// For reading textdumps...
    fn create_object(
        &mut self,
        objid: Option<Obj>,
        attrs: &ObjAttrs,
    ) -> Result<Obj, WorldStateError>;
    fn set_object_parent(&mut self, obj: &Obj, parent: &Obj) -> Result<(), WorldStateError>;

    fn set_object_location(&mut self, o: &Obj, location: &Obj) -> Result<(), WorldStateError>;
    fn set_object_owner(&mut self, obj: &Obj, owner: &Obj) -> Result<(), WorldStateError>;

    fn add_verb(
        &mut self,
        obj: &Obj,
        names: Vec<&str>,
        owner: &Obj,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Vec<u8>,
    ) -> Result<(), WorldStateError>;

    fn define_property(
        &mut self,
        definer: &Obj,
        objid: &Obj,
        propname: &str,
        owner: &Obj,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    fn set_property(
        &mut self,
        objid: &Obj,
        propname: &str,
        owner: Option<Obj>,
        flags: Option<BitEnum<PropFlag>>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    fn commit(self: Box<Self>) -> Result<CommitResult, WorldStateError>;

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
    fn get_verb_binary(&self, objid: &Obj, uuid: Uuid) -> Result<ByteView, WorldStateError>;

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
