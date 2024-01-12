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

use async_trait::async_trait;
use uuid::Uuid;

use moor_values::model::objects::ObjAttrs;
use moor_values::model::objset::ObjSet;
use moor_values::model::propdef::PropDefs;
use moor_values::model::props::PropFlag;
use moor_values::model::r#match::VerbArgsSpec;
use moor_values::model::verbdef::VerbDefs;
use moor_values::model::verbs::VerbFlag;
use moor_values::model::{CommitResult, WorldStateError};
use moor_values::util::bitenum::BitEnum;
use moor_values::var::objid::Objid;
use moor_values::var::Var;

/// Interface exposed to be used by the textdump loader. Overlap of functionality with what
/// WorldState could provide, but potentially different constraints/semantics (e.g. no perms checks)
#[async_trait]
pub trait LoaderInterface: Send + Sync {
    /// For reading textdumps...
    async fn create_object(
        &self,
        objid: Option<Objid>,
        attrs: &ObjAttrs,
    ) -> Result<Objid, WorldStateError>;
    async fn set_object_parent(&self, obj: Objid, parent: Objid) -> Result<(), WorldStateError>;

    async fn set_object_location(&self, o: Objid, location: Objid) -> Result<(), WorldStateError>;
    async fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), WorldStateError>;

    async fn add_verb(
        &self,
        obj: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Vec<u8>,
    ) -> Result<(), WorldStateError>;

    async fn get_property_value(
        &self,
        obj: Objid,
        uuid: Uuid,
    ) -> Result<Option<Var>, WorldStateError>;
    async fn define_property(
        &self,
        definer: Objid,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    async fn set_property(
        &self,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    async fn commit(&self) -> Result<CommitResult, WorldStateError>;

    // For writing textdumps...

    /// Get the list of all active objects in the database
    async fn get_objects(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the list of all players.
    async fn get_players(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the attributes of a given object
    async fn get_object(&self, objid: Objid) -> Result<ObjAttrs, WorldStateError>;

    /// Get the verbs living on a given object
    async fn get_object_verbs(&self, objid: Objid) -> Result<VerbDefs, WorldStateError>;

    /// Get the binary for a given verb
    async fn get_verb_binary(&self, objid: Objid, uuid: Uuid) -> Result<Vec<u8>, WorldStateError>;

    /// Get the properties defined on a given object
    async fn get_object_properties(&self, objid: Objid) -> Result<PropDefs, WorldStateError>;
}
