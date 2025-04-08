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

use crate::db_worldstate::DbTxWorldState;
use crate::loader::LoaderInterface;
use moor_common::model::ObjAttrs;
use moor_common::model::ObjSet;
use moor_common::model::PropFlag;
use moor_common::model::VerbArgsSpec;
use moor_common::model::VerbDefs;
use moor_common::model::{BinaryType, VerbFlag};
use moor_common::model::{CommitResult, WorldStateError};
use moor_common::model::{HasUuid, PropPerms, ValSet};
use moor_common::model::{PropDef, PropDefs};
use moor_common::util::BitEnum;
use moor_var::Obj;
use moor_var::Symbol;
use moor_var::Var;

/// A loader client which uses a database transaction to load the world state.
impl LoaderInterface for DbTxWorldState {
    fn create_object(
        &mut self,
        objid: Option<Obj>,
        attrs: &ObjAttrs,
    ) -> Result<Obj, WorldStateError> {
        self.get_tx_mut().create_object(objid, attrs.clone())
    }
    fn set_object_parent(&mut self, obj: &Obj, parent: &Obj) -> Result<(), WorldStateError> {
        self.get_tx_mut().set_object_parent(obj, parent)
    }
    fn set_object_location(&mut self, o: &Obj, location: &Obj) -> Result<(), WorldStateError> {
        self.get_tx_mut().set_object_location(o, location)
    }
    fn set_object_owner(&mut self, obj: &Obj, owner: &Obj) -> Result<(), WorldStateError> {
        self.get_tx_mut().set_object_owner(obj, owner)
    }
    fn add_verb(
        &mut self,
        obj: &Obj,
        names: Vec<&str>,
        owner: &Obj,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Vec<u8>,
    ) -> Result<(), WorldStateError> {
        self.get_tx_mut().add_object_verb(
            obj,
            owner,
            names
                .iter()
                .map(|s| Symbol::mk_case_insensitive(s))
                .collect(),
            binary,
            BinaryType::LambdaMoo18X,
            flags,
            args,
        )?;
        Ok(())
    }

    fn define_property(
        &mut self,
        definer: &Obj,
        objid: &Obj,
        propname: &str,
        owner: &Obj,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        self.get_tx_mut().define_property(
            definer,
            objid,
            Symbol::mk_case_insensitive(propname),
            owner,
            flags,
            value,
        )?;
        Ok(())
    }
    fn set_property(
        &mut self,
        objid: &Obj,
        propname: &str,
        owner: Option<Obj>,
        flags: Option<BitEnum<PropFlag>>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        // First find the property.
        let (propdef, _, _, _) = self
            .get_tx()
            .resolve_property(objid, Symbol::mk_case_insensitive(propname))?;

        // Now set the value if provided.
        if let Some(value) = value {
            self.get_tx_mut()
                .set_property(objid, propdef.uuid(), value)?;
        }

        // And then set the flags and owner the child had.
        self.get_tx_mut().update_property_info(
            objid,
            propdef.uuid(),
            owner.clone(),
            flags,
            None,
        )?;
        Ok(())
    }

    fn commit(self: Box<Self>) -> Result<CommitResult, WorldStateError> {
        self.tx.commit()
    }

    fn get_objects(&self) -> Result<ObjSet, WorldStateError> {
        self.get_tx().get_objects()
    }

    fn get_players(&self) -> Result<ObjSet, WorldStateError> {
        self.get_tx().get_players()
    }

    fn get_object(&self, objid: &Obj) -> Result<ObjAttrs, WorldStateError> {
        Ok(ObjAttrs::new(
            self.get_tx().get_object_owner(objid)?,
            self.get_tx().get_object_parent(objid)?,
            self.get_tx().get_object_location(objid)?,
            self.get_tx().get_object_flags(objid)?,
            &self.get_tx().get_object_name(objid)?,
        ))
    }

    fn get_object_verbs(&self, objid: &Obj) -> Result<VerbDefs, WorldStateError> {
        self.get_tx().get_verbs(objid)
    }

    fn get_verb_binary(&self, objid: &Obj, uuid: Uuid) -> Result<ByteView, WorldStateError> {
        self.get_tx().get_verb_binary(objid, uuid)
    }

    fn get_object_properties(&self, objid: &Obj) -> Result<PropDefs, WorldStateError> {
        self.get_tx().get_properties(objid)
    }

    fn get_property_value(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError> {
        self.get_tx().retrieve_property(obj, uuid)
    }

    #[allow(clippy::type_complexity)]
    fn get_all_property_values(
        &self,
        this: &Obj,
    ) -> Result<Vec<(PropDef, (Option<Var>, PropPerms))>, WorldStateError> {
        // First get the entire inheritance hierarchy.
        let hierarchy = self.get_tx().ancestors(this)?;

        // Now get the property common for each of those objects, but only for the props which
        // are defined by that object.
        // At the same time, get the common.
        let mut properties = vec![];
        for obj in hierarchy.iter() {
            let obj_propdefs = self.get_tx().get_properties(&obj)?;
            for p in obj_propdefs.iter() {
                if p.definer() != obj {
                    continue;
                }
                let value = self.get_tx().retrieve_property(this, p.uuid())?;
                properties.push((p.clone(), value));
            }
        }
        Ok(properties)
    }
}
