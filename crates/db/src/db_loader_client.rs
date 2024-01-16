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

#[async_trait]
impl LoaderInterface for DbTxWorldState {
    async fn create_object(
        &self,
        objid: Option<Objid>,
        attrs: &ObjAttrs,
    ) -> Result<Objid, WorldStateError> {
        Ok(self.tx.create_object(objid, attrs.clone()).await?)
    }
    async fn set_object_parent(&self, obj: Objid, parent: Objid) -> Result<(), WorldStateError> {
        Ok(self.tx.set_object_parent(obj, parent).await?)
    }
    async fn set_object_location(&self, o: Objid, location: Objid) -> Result<(), WorldStateError> {
        Ok(self.tx.set_object_location(o, location).await?)
    }
    async fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), WorldStateError> {
        Ok(self.tx.set_object_owner(obj, owner).await?)
    }
    async fn add_verb(
        &self,
        obj: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Vec<u8>,
    ) -> Result<(), WorldStateError> {
        self.tx
            .add_object_verb(
                obj,
                owner,
                names.iter().map(|s| s.to_string()).collect(),
                binary,
                BinaryType::LambdaMoo18X,
                flags,
                args,
            )
            .await?;
        Ok(())
    }

    async fn define_property(
        &self,
        definer: Objid,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        self.tx
            .define_property(definer, objid, propname.to_string(), owner, flags, value)
            .await?;
        Ok(())
    }
    async fn set_property(
        &self,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        // First find the property.
        let (propdef, _) = self
            .tx
            .resolve_property(objid, propname.to_string())
            .await?;

        // Now set the value if provided.
        if let Some(value) = value {
            self.tx.set_property(objid, propdef.uuid(), value).await?;
        }

        // And then set the flags and owner the child had.
        self.tx
            .update_property_definition(objid, propdef.uuid(), Some(owner), Some(flags), None)
            .await?;
        Ok(())
    }

    async fn commit(&self) -> Result<CommitResult, WorldStateError> {
        let cr = self.tx.commit().await?;
        Ok(cr)
    }

    async fn get_objects(&self) -> Result<ObjSet, WorldStateError> {
        self.tx.get_objects().await
    }

    async fn get_players(&self) -> Result<ObjSet, WorldStateError> {
        self.tx.get_players().await
    }

    async fn get_object(&self, objid: Objid) -> Result<ObjAttrs, WorldStateError> {
        Ok(ObjAttrs {
            owner: Some(self.tx.get_object_owner(objid).await?),
            name: Some(self.tx.get_object_name(objid).await?),
            parent: Some(self.tx.get_object_parent(objid).await?),
            location: Some(self.tx.get_object_location(objid).await?),
            flags: Some(self.tx.get_object_flags(objid).await?),
        })
    }

    async fn get_object_verbs(&self, objid: Objid) -> Result<VerbDefs, WorldStateError> {
        self.tx.get_verbs(objid).await
    }

    async fn get_verb_binary(&self, objid: Objid, uuid: Uuid) -> Result<Vec<u8>, WorldStateError> {
        self.tx.get_verb_binary(objid, uuid).await
    }

    async fn get_object_properties(&self, objid: Objid) -> Result<PropDefs, WorldStateError> {
        self.tx.get_properties(objid).await
    }

    async fn get_property_value(
        &self,
        obj: Objid,
        uuid: Uuid,
    ) -> Result<Option<Var>, WorldStateError> {
        match self.tx.retrieve_property(obj, uuid).await {
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
    async fn get_all_property_values(
        &self,
        this: Objid,
    ) -> Result<Vec<(PropDef, Option<Var>)>, WorldStateError> {
        // Get the property definitions for ourselves, and this is how we will resolve
        // the updated flags, owners, etc. on us vs the definition.
        let propdefs = self.tx.get_properties(this).await?;

        // First get the entire inheritance hierarchy.
        let hierarchy = self.tx.ancestors(this).await?;

        // Now get the property definitions for each of those objects, but only for the props which
        // are defined by that object.
        // At the same time, get the values.
        let mut properties = vec![];
        for obj in hierarchy.iter() {
            let obj_propdefs = self.tx.get_properties(obj).await?;
            for p in obj_propdefs.iter() {
                if p.definer() != obj {
                    continue;
                }
                let local_def = propdefs.iter().find(|pd| pd.uuid() == p.uuid()).unwrap();
                let value = match self.tx.retrieve_property(this, p.uuid()).await {
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
