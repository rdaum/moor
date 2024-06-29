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

use moor_values::model::ObjAttrs;
use moor_values::model::ObjSet;
use moor_values::model::PropFlag;
use moor_values::model::VerbArgsSpec;
use moor_values::model::VerbDefs;
use moor_values::model::{BinaryType, VerbFlag};
use moor_values::model::{CommitResult, WorldStateError};
use moor_values::model::{HasUuid, PropPerms, ValSet};
use moor_values::model::{PropDef, PropDefs};
use moor_values::util::BitEnum;
use moor_values::var::Objid;
use moor_values::var::Symbol;
use moor_values::var::Var;

use crate::db_worldstate::DbTxWorldState;
use crate::loader::LoaderInterface;

/// A loader client which uses a database transaction to load the world state.
impl LoaderInterface for DbTxWorldState {
    fn create_object(
        &self,
        objid: Option<Objid>,
        attrs: &ObjAttrs,
    ) -> Result<Objid, WorldStateError> {
        self.tx.create_object(objid, attrs.clone())
    }
    fn set_object_parent(&self, obj: Objid, parent: Objid) -> Result<(), WorldStateError> {
        self.tx.set_object_parent(obj, parent)
    }
    fn set_object_location(&self, o: Objid, location: Objid) -> Result<(), WorldStateError> {
        self.tx.set_object_location(o, location)
    }
    fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), WorldStateError> {
        self.tx.set_object_owner(obj, owner)
    }
    fn add_verb(
        &self,
        obj: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Vec<u8>,
    ) -> Result<(), WorldStateError> {
        self.tx.add_object_verb(
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
        &self,
        definer: Objid,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        self.tx.define_property(
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
        &self,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        // First find the property.
        let (propdef, _, _, _) = self
            .tx
            .resolve_property(objid, Symbol::mk_case_insensitive(propname))?;

        // Now set the value if provided.
        if let Some(value) = value {
            self.tx.set_property(objid, propdef.uuid(), value)?;
        }

        // And then set the flags and owner the child had.
        self.tx
            .update_property_info(objid, propdef.uuid(), Some(owner), Some(flags), None)?;
        Ok(())
    }

    fn commit(&mut self) -> Result<CommitResult, WorldStateError> {
        let cr = self.tx.commit()?;
        Ok(cr)
    }

    fn get_objects(&self) -> Result<ObjSet, WorldStateError> {
        self.tx.get_objects()
    }

    fn get_players(&self) -> Result<ObjSet, WorldStateError> {
        self.tx.get_players()
    }

    fn get_object(&self, objid: Objid) -> Result<ObjAttrs, WorldStateError> {
        Ok(ObjAttrs::new(
            self.tx.get_object_owner(objid)?,
            self.tx.get_object_parent(objid)?,
            self.tx.get_object_location(objid)?,
            self.tx.get_object_flags(objid)?,
            &self.tx.get_object_name(objid)?,
        ))
    }

    fn get_object_verbs(&self, objid: Objid) -> Result<VerbDefs, WorldStateError> {
        self.tx.get_verbs(objid)
    }

    fn get_verb_binary(&self, objid: Objid, uuid: Uuid) -> Result<Bytes, WorldStateError> {
        self.tx.get_verb_binary(objid, uuid)
    }

    fn get_object_properties(&self, objid: Objid) -> Result<PropDefs, WorldStateError> {
        self.tx.get_properties(objid)
    }

    fn get_property_value(
        &self,
        obj: Objid,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError> {
        self.tx.retrieve_property(obj, uuid)
    }

    #[allow(clippy::type_complexity)]
    fn get_all_property_values(
        &self,
        this: Objid,
    ) -> Result<Vec<(PropDef, (Option<Var>, PropPerms))>, WorldStateError> {
        // First get the entire inheritance hierarchy.
        let hierarchy = self.tx.ancestors(this)?;

        // Now get the property values for each of those objects, but only for the props which
        // are defined by that object.
        // At the same time, get the values.
        let mut properties = vec![];
        for obj in hierarchy.iter() {
            let obj_propdefs = self.tx.get_properties(obj)?;
            for p in obj_propdefs.iter() {
                if p.definer() != obj {
                    continue;
                }
                let value = self.tx.retrieve_property(this, p.uuid())?;
                properties.push((p.clone(), value));
            }
        }
        Ok(properties)
    }
}
