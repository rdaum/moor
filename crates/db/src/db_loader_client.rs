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

use moor_values::model::HasUuid;
use moor_values::model::ObjAttrs;
use moor_values::model::ObjSet;
use moor_values::model::PropFlag;
use moor_values::model::VerbArgsSpec;
use moor_values::model::VerbDefs;
use moor_values::model::{BinaryType, VerbFlag};
use moor_values::model::{CommitResult, WorldStateError};
use moor_values::model::{PropDef, PropDefs};
use moor_values::util::BitEnum;
use moor_values::var::Objid;
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
            names.iter().map(|s| s.to_string()).collect(),
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
        self.tx
            .define_property(definer, objid, propname.to_string(), owner, flags, value)?;
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
        let (propdef, _) = self.tx.resolve_property(objid, propname.to_string())?;

        // Now set the value if provided.
        if let Some(value) = value {
            self.tx.set_property(objid, propdef.uuid(), value)?;
        }

        // And then set the flags and owner the child had.
        self.tx.update_property_definition(
            objid,
            propdef.uuid(),
            Some(owner),
            Some(flags),
            None,
        )?;
        Ok(())
    }

    fn commit(&self) -> Result<CommitResult, WorldStateError> {
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
        Ok(ObjAttrs {
            owner: Some(self.tx.get_object_owner(objid)?),
            name: Some(self.tx.get_object_name(objid)?),
            parent: Some(self.tx.get_object_parent(objid)?),
            location: Some(self.tx.get_object_location(objid)?),
            flags: Some(self.tx.get_object_flags(objid)?),
        })
    }

    fn get_object_verbs(&self, objid: Objid) -> Result<VerbDefs, WorldStateError> {
        self.tx.get_verbs(objid)
    }

    fn get_verb_binary(&self, objid: Objid, uuid: Uuid) -> Result<Vec<u8>, WorldStateError> {
        self.tx.get_verb_binary(objid, uuid)
    }

    fn get_object_properties(&self, objid: Objid) -> Result<PropDefs, WorldStateError> {
        self.tx.get_properties(objid)
    }

    fn get_property_value(&self, obj: Objid, uuid: Uuid) -> Result<Option<Var>, WorldStateError> {
        match self.tx.retrieve_property(obj, uuid) {
            Ok(propval) => Ok(Some(propval)),
            // Property is 'clear'
            Err(WorldStateError::PropertyNotFound(_, _)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // propvals in textdumps have wonky logic which resolve relative to position of propdefs
    // in the inheritance hierarchy of the object
    // So we need to walk ourselves up to the root of the inheritance hierarchy, and then return
    // the values for each of the properties defined by that object, in the order of the properties
    // defined by that object.
    // This should then map the the propdefs for each of those properties.
    // The bulk of this work should be done by the loader_client, which will give us that entire
    // hierarchy in a single call.
    // Really this is just a way of reordering our local propdefs to match the inheritance hierarchy.
    // which is something LambdaMOO does automagically internally, but we don't bother to.
    fn get_all_property_values(
        &self,
        this: Objid,
    ) -> Result<Vec<(PropDef, Option<Var>)>, WorldStateError> {
        // Get the property definitions for ourselves, and this is how we will resolve
        // the updated flags, owners, etc. on us vs the definition.
        let propdefs = self.tx.get_properties(this)?;

        // First get the entire inheritance hierarchy.
        let hierarchy = self.tx.ancestors(this)?;

        // Now get the property definitions for each of those objects, but only for the props which
        // are defined by that object.
        // At the same time, get the values.
        let mut properties = vec![];
        for obj in hierarchy.iter() {
            let obj_propdefs = self.tx.get_properties(obj)?;
            for p in obj_propdefs.iter() {
                if p.definer() != obj {
                    continue;
                }
                let local_def = propdefs.iter().find(|pd| pd.uuid() == p.uuid()).unwrap();
                let value = match self.tx.retrieve_property(this, p.uuid()) {
                    Ok(propval) => Some(propval),
                    // Property is 'clear'
                    Err(WorldStateError::PropertyNotFound(_, _)) => None,
                    Err(e) => return Err(e),
                };
                properties.push((local_def, value));
            }
        }
        Ok(properties)
    }
}
