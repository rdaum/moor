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

use moor_values::model::defset::HasUuid;
use moor_values::model::objects::ObjAttrs;
use moor_values::model::objset::ObjSet;
use moor_values::model::propdef::PropDefs;
use moor_values::model::props::PropFlag;
use moor_values::model::r#match::VerbArgsSpec;
use moor_values::model::verbdef::VerbDefs;
use moor_values::model::verbs::{BinaryType, VerbFlag};
use moor_values::model::{CommitResult, WorldStateError};
use moor_values::util::bitenum::BitEnum;
use moor_values::var::objid::Objid;
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

    async fn get_property_value(
        &self,
        obj: Objid,
        uuid: Uuid,
    ) -> Result<Option<Var>, WorldStateError> {
        let Ok(propval) = self.tx.retrieve_property(obj, uuid).await else {
            return Ok(None);
        };
        Ok(Some(propval))
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
}
