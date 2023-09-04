use anyhow::Context;
use async_trait::async_trait;
use uuid::Uuid;

use moor_value::model::defset::HasUuid;
use moor_value::model::objects::ObjAttrs;
use moor_value::model::props::PropFlag;
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verbs::{BinaryType, VerbFlag};
use moor_value::model::CommitResult;
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::Var;

use crate::db::loader::LoaderInterface;
use crate::db::DbTxWorldState;

#[async_trait]
impl LoaderInterface for DbTxWorldState {
    async fn create_object(
        &self,
        objid: Option<Objid>,
        attrs: &ObjAttrs,
    ) -> Result<Objid, anyhow::Error> {
        Ok(self.client.create_object(objid, attrs.clone()).await?)
    }
    async fn set_object_parent(&self, obj: Objid, parent: Objid) -> Result<(), anyhow::Error> {
        Ok(self.client.set_parent(obj, parent).await?)
    }
    async fn set_object_location(&self, o: Objid, location: Objid) -> Result<(), anyhow::Error> {
        Ok(self.client.set_location_of(o, location).await?)
    }
    async fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), anyhow::Error> {
        Ok(self.client.set_object_owner(obj, owner).await?)
    }
    async fn add_verb(
        &self,
        obj: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        binary: Vec<u8>,
    ) -> Result<(), anyhow::Error> {
        self.client
            .add_verb(
                obj,
                owner,
                names.iter().map(|s| s.to_string()).collect(),
                BinaryType::LambdaMoo18X,
                binary,
                flags,
                args,
            )
            .await?;
        Ok(())
    }
    async fn get_property(&self, obj: Objid, pname: &str) -> Result<Option<Uuid>, anyhow::Error> {
        Ok(self
            .client
            .get_properties(obj)
            .await?
            .find_named(pname)
            .map(|p| p.uuid()))
    }
    async fn define_property(
        &self,
        definer: Objid,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), anyhow::Error> {
        self.client
            .define_property(definer, objid, propname.to_string(), owner, flags, value)
            .await?;
        Ok(())
    }
    async fn set_update_property(
        &self,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), anyhow::Error> {
        // First find the property.
        let (propdef, _) = self
            .client
            .resolve_property(objid, propname.to_string())
            .await
            .with_context(|| {
                format!("Error resolving property {} on object {}", propname, objid)
            })?;

        // Now set the value if provided.
        if let Some(value) = value {
            self.client
                .set_property(objid, propdef.uuid(), value)
                .await
                .with_context(|| format!("Error setting value for {}.{}", objid, propname))?;
        }

        // And then set the flags and owner the child had.
        self.client
            .set_property_info(objid, propdef.uuid(), Some(owner), Some(flags), None)
            .await
            .with_context(|| format!("Error setting property info for {}.{}", objid, propname))?;
        Ok(())
    }

    async fn commit(&self) -> Result<CommitResult, anyhow::Error> {
        let cr = self.client.commit().await?;
        Ok(cr)
    }
}
