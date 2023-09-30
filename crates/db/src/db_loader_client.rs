use async_trait::async_trait;
use uuid::Uuid;

use moor_values::model::defset::HasUuid;
use moor_values::model::objects::ObjAttrs;
use moor_values::model::props::PropFlag;
use moor_values::model::r#match::VerbArgsSpec;
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
        Ok(self.tx.set_parent(obj, parent).await?)
    }
    async fn set_object_location(&self, o: Objid, location: Objid) -> Result<(), WorldStateError> {
        Ok(self.tx.set_location_of(o, location).await?)
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
    async fn get_property(&self, obj: Objid, pname: &str) -> Result<Option<Uuid>, WorldStateError> {
        Ok(self
            .tx
            .get_properties(obj)
            .await?
            .find_first_named(pname)
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
    ) -> Result<(), WorldStateError> {
        self.tx
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
            .set_property_info(objid, propdef.uuid(), Some(owner), Some(flags), None)
            .await?;
        Ok(())
    }

    async fn commit(&self) -> Result<CommitResult, WorldStateError> {
        let cr = self.tx.commit().await?;
        Ok(cr)
    }
}
