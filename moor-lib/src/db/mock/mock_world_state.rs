use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Error;
use async_trait::async_trait;

use moor_value::AsBytes;
use uuid::Uuid;

use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::{ObjSet, Objid};
use moor_value::var::{v_none, Var};

use crate::db::LoaderInterface;
use crate::vm::opcode::Program;
use moor_value::model::objects::{ObjAttrs, ObjFlag};
use moor_value::model::permissions::PermissionsContext;
use moor_value::model::props::{PropAttrs, PropFlag};
use moor_value::model::r#match::{PrepSpec, VerbArgsSpec};
use moor_value::model::verbs::{BinaryType, VerbAttrs, VerbFlag, VerbInfo};
use moor_value::model::world_state::{WorldState, WorldStateSource};
use moor_value::model::CommitResult;
use moor_value::model::WorldStateError;
use moor_value::model::WorldStateError::{PropertyNotFound, VerbNotFound};

struct MockStore {
    verbs: HashMap<(Objid, String), VerbInfo>,
    properties: HashMap<(Objid, String), Var>,
}
impl MockStore {
    fn set_verb(&mut self, o: Objid, name: &str, program: &Program) {
        let binary = program.as_bytes();
        self.verbs.insert(
            (o, name.to_string()),
            VerbInfo {
                names: vec![name.to_string()],
                attrs: VerbAttrs {
                    definer: Some(o),
                    owner: Some(o),
                    flags: Some(
                        BitEnum::new_with(VerbFlag::Exec) | VerbFlag::Read | VerbFlag::Debug,
                    ),
                    args_spec: Some(VerbArgsSpec::this_none_this()),
                    binary_type: BinaryType::LambdaMoo18X,
                    binary: Some(binary.to_vec()),
                },
            },
        );
    }
}

pub struct MockState(Arc<Mutex<MockStore>>);

#[async_trait]
impl WorldState for MockState {
    async fn owner_of(&mut self, _obj: Objid) -> Result<Objid, WorldStateError> {
        todo!()
    }

    async fn flags_of(&mut self, _obj: Objid) -> Result<BitEnum<ObjFlag>, WorldStateError> {
        Ok(BitEnum::all())
    }

    async fn set_flags_of(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _flags: BitEnum<ObjFlag>,
    ) -> Result<(), Error> {
        todo!()
    }

    async fn location_of(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<Objid, WorldStateError> {
        todo!()
    }

    async fn create_object(
        &mut self,
        _perms: PermissionsContext,
        _parent: Objid,
        _owner: Objid,
    ) -> Result<Objid, WorldStateError> {
        todo!()
    }

    async fn move_object(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _new_loc: Objid,
    ) -> Result<(), WorldStateError> {
        todo!()
    }

    async fn contents_of(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<ObjSet, WorldStateError> {
        todo!()
    }

    async fn verbs(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<Vec<VerbInfo>, WorldStateError> {
        todo!()
    }

    async fn properties(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<Vec<(String, PropAttrs)>, WorldStateError> {
        todo!()
    }

    async fn retrieve_property(
        &mut self,
        _perms: PermissionsContext,
        obj: Objid,
        pname: &str,
    ) -> Result<Var, WorldStateError> {
        let store = self.0.lock().unwrap();
        let p = store.properties.get(&(obj, pname.to_string()));
        match p {
            None => Err(PropertyNotFound(obj, pname.to_string())),
            Some(p) => Ok(p.clone()),
        }
    }

    async fn get_property_info(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _pname: &str,
    ) -> Result<PropAttrs, WorldStateError> {
        todo!()
    }

    async fn set_property_info(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _pname: &str,
        _attrs: PropAttrs,
    ) -> Result<(), WorldStateError> {
        todo!()
    }

    async fn update_property(
        &mut self,
        _perms: PermissionsContext,
        obj: Objid,
        pname: &str,
        value: &Var,
    ) -> Result<(), WorldStateError> {
        let mut store = self.0.lock().unwrap();
        store
            .properties
            .insert((obj, pname.to_string()), value.clone());
        Ok(())
    }

    async fn is_property_clear(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _pname: &str,
    ) -> Result<bool, WorldStateError> {
        todo!()
    }

    async fn clear_property(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _pname: &str,
    ) -> Result<(), WorldStateError> {
        todo!()
    }

    async fn define_property(
        &mut self,
        _perms: PermissionsContext,
        _definer: Objid,
        obj: Objid,
        pname: &str,
        _owner: Objid,
        _prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        let mut store = self.0.lock().unwrap();

        store
            .properties
            .insert((obj, pname.to_string()), initial_value.unwrap_or(v_none()));
        Ok(())
    }

    async fn add_verb(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _names: Vec<String>,
        _owner: Objid,
        _flags: BitEnum<VerbFlag>,
        _args: VerbArgsSpec,
        _binary: Vec<u8>,
        _binary_type: BinaryType,
    ) -> Result<(), WorldStateError> {
        todo!()
    }

    async fn remove_verb(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _vname: &str,
    ) -> Result<(), WorldStateError> {
        todo!()
    }

    async fn set_verb_info(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _vname: &str,
        _owner: Option<Objid>,
        _names: Option<Vec<String>>,
        _flags: Option<BitEnum<VerbFlag>>,
        _args: Option<VerbArgsSpec>,
    ) -> Result<(), WorldStateError> {
        todo!()
    }

    async fn set_verb_info_at_index(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _vidx: usize,
        _owner: Option<Objid>,
        _names: Option<Vec<String>>,
        _flags: Option<BitEnum<VerbFlag>>,
        _args: Option<VerbArgsSpec>,
    ) -> Result<(), WorldStateError> {
        todo!()
    }

    async fn get_verb(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _vname: &str,
    ) -> Result<VerbInfo, WorldStateError> {
        self.0
            .lock()
            .unwrap()
            .verbs
            .get(&(_obj, _vname.to_string()))
            .cloned()
            .ok_or_else(|| VerbNotFound(_obj, _vname.to_string()))
    }

    async fn get_verb_at_index(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _vidx: usize,
    ) -> Result<VerbInfo, WorldStateError> {
        todo!()
    }

    async fn find_method_verb_on(
        &mut self,
        _perms: PermissionsContext,
        obj: Objid,
        vname: &str,
    ) -> Result<VerbInfo, WorldStateError> {
        let store = self.0.lock().unwrap();
        let v = store.verbs.get(&(obj, vname.to_string()));
        match v {
            None => Err(VerbNotFound(obj, vname.to_string())),
            Some(v) => Ok(v.clone()),
        }
    }

    async fn find_command_verb_on(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _command_verb: &str,
        _dobj: Objid,
        _prep: PrepSpec,
        _iobj: Objid,
    ) -> Result<Option<VerbInfo>, WorldStateError> {
        todo!()
    }

    async fn parent_of(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<Objid, WorldStateError> {
        todo!()
    }

    async fn change_parent(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _new_parent: Objid,
    ) -> Result<(), WorldStateError> {
        todo!()
    }

    async fn children_of(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<ObjSet, WorldStateError> {
        todo!()
    }

    async fn valid(&mut self, _obj: Objid) -> Result<bool, WorldStateError> {
        Ok(true)
    }

    async fn names_of(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<(String, Vec<String>), WorldStateError> {
        todo!()
    }

    async fn commit(&mut self) -> Result<CommitResult, anyhow::Error> {
        Ok(CommitResult::Success)
    }

    async fn rollback(&mut self) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

pub struct MockWorldStateSource(Arc<Mutex<MockStore>>);

#[async_trait]
impl LoaderInterface for MockWorldStateSource {
    async fn create_object(
        &self,
        _objid: Option<Objid>,
        _attrs: &ObjAttrs,
    ) -> Result<Objid, Error> {
        todo!()
    }

    async fn set_object_parent(&self, _obj: Objid, _parent: Objid) -> Result<(), Error> {
        todo!()
    }

    async fn set_object_location(&self, _o: Objid, _location: Objid) -> Result<(), Error> {
        todo!()
    }

    async fn set_object_owner(&self, _obj: Objid, _owner: Objid) -> Result<(), Error> {
        todo!()
    }

    async fn add_verb(
        &self,
        _obj: Objid,
        _names: Vec<&str>,
        _owner: Objid,
        _flags: BitEnum<VerbFlag>,
        _args: VerbArgsSpec,
        _binary: Vec<u8>,
    ) -> Result<(), Error> {
        todo!()
    }

    async fn get_property(&self, _obj: Objid, _pname: &str) -> Result<Option<Uuid>, Error> {
        todo!()
    }

    async fn define_property(
        &self,
        _definer: Objid,
        _objid: Objid,
        _propname: &str,
        _owner: Objid,
        _flags: BitEnum<PropFlag>,
        _value: Option<Var>,
    ) -> Result<(), Error> {
        todo!()
    }

    async fn set_update_property(
        &self,
        _objid: Objid,
        _propname: &str,
        _owner: Objid,
        _flags: BitEnum<PropFlag>,
        _value: Option<Var>,
    ) -> Result<(), Error> {
        todo!()
    }

    async fn commit(self) -> Result<CommitResult, Error> {
        todo!()
    }
}
impl MockWorldStateSource {
    #[allow(dead_code)]
    pub(crate) fn new() -> Self {
        let store = MockStore {
            verbs: Default::default(),
            properties: Default::default(),
        };
        Self(Arc::new(Mutex::new(store)))
    }

    pub fn new_with_verb(name: &str, binary: &Program) -> Self {
        let mut store = MockStore {
            verbs: Default::default(),
            properties: Default::default(),
        };
        store.set_verb(Objid(0), name, binary);
        Self(Arc::new(Mutex::new(store)))
    }

    pub fn new_with_verbs(verbs: Vec<(&str, &Program)>) -> Self {
        let mut store = MockStore {
            verbs: Default::default(),
            properties: Default::default(),
        };
        for (v, b) in verbs {
            store.set_verb(Objid(0), v, b);
        }
        Self(Arc::new(Mutex::new(store)))
    }
}

#[async_trait]
impl WorldStateSource for MockWorldStateSource {
    async fn new_world_state(&mut self) -> Result<Box<dyn WorldState>, Error> {
        Ok(Box::new(MockState(self.0.clone())))
    }
}
