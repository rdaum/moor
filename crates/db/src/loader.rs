use async_trait::async_trait;
use uuid::Uuid;

use moor_values::model::objects::ObjAttrs;
use moor_values::model::props::PropFlag;
use moor_values::model::r#match::VerbArgsSpec;
use moor_values::model::verbs::VerbFlag;
use moor_values::model::{CommitResult, WorldStateError};
use moor_values::util::bitenum::BitEnum;
use moor_values::var::objid::Objid;
use moor_values::var::Var;

/// Interface exposed to be used by the textdump loader. Overlap of functionality with what
/// WorldState could provide, but potentially different constraints/semantics.
#[async_trait]
pub trait LoaderInterface {
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

    async fn get_property(&self, obj: Objid, pname: &str) -> Result<Option<Uuid>, WorldStateError>;
    async fn define_property(
        &self,
        definer: Objid,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;
    async fn set_update_property(
        &self,
        objid: Objid,
        propname: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError>;

    async fn commit(&self) -> Result<CommitResult, WorldStateError>;
}
