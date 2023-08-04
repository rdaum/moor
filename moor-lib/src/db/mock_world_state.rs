use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Error;
use async_trait::async_trait;

use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::{v_none, Var};

use crate::db::rocksdb::LoaderInterface;
use crate::db::CommitResult;
use crate::model::objects::{ObjAttrs, ObjFlag};
use crate::model::permissions::PermissionsContext;
use crate::model::props::{PropAttrs, PropFlag};
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::{VerbAttrs, VerbFlag, VerbInfo};
use crate::model::world_state::{WorldState, WorldStateSource};
use crate::model::ObjectError;
use crate::model::ObjectError::{PropertyNotFound, VerbNotFound};
use crate::tasks::command_parse::ParsedCommand;
use crate::vm::opcode::Binary;

struct MockStore {
    verbs: HashMap<(Objid, String), VerbInfo>,
    properties: HashMap<(Objid, String), Var>,
}
impl MockStore {
    fn set_verb(&mut self, o: Objid, name: &str, binary: &Binary) {
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
                    program: Some(binary.clone()),
                },
            },
        );
    }
}

pub struct MockState(Arc<Mutex<MockStore>>);

#[async_trait]
impl WorldState for MockState {
    async fn owner_of(&mut self, _obj: Objid) -> Result<Objid, ObjectError> {
        todo!()
    }

    async fn flags_of(&mut self, _obj: Objid) -> Result<BitEnum<ObjFlag>, ObjectError> {
        Ok(BitEnum::all())
    }

    async fn location_of(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<Objid, ObjectError> {
        todo!()
    }

    async fn contents_of(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<Vec<Objid>, ObjectError> {
        todo!()
    }

    async fn verbs(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<Vec<VerbInfo>, ObjectError> {
        todo!()
    }

    async fn properties(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<Vec<(String, PropAttrs)>, ObjectError> {
        todo!()
    }

    async fn retrieve_property(
        &mut self,
        _perms: PermissionsContext,
        obj: Objid,
        pname: &str,
    ) -> Result<Var, ObjectError> {
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
    ) -> Result<PropAttrs, ObjectError> {
        todo!()
    }

    async fn set_property_info(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _pname: &str,
        _attrs: PropAttrs,
    ) -> Result<(), ObjectError> {
        todo!()
    }

    async fn update_property(
        &mut self,
        _perms: PermissionsContext,
        obj: Objid,
        pname: &str,
        value: &Var,
    ) -> Result<(), ObjectError> {
        let mut store = self.0.lock().unwrap();
        store
            .properties
            .insert((obj, pname.to_string()), value.clone());
        Ok(())
    }

    async fn add_property(
        &mut self,
        _perms: PermissionsContext,
        _definer: Objid,
        obj: Objid,
        pname: &str,
        _owner: Objid,
        _prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), ObjectError> {
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
        _code: Binary,
    ) -> Result<(), ObjectError> {
        todo!()
    }

    async fn remove_verb(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _vname: &str,
    ) -> Result<(), ObjectError> {
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
    ) -> Result<(), ObjectError> {
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
    ) -> Result<(), ObjectError> {
        todo!()
    }

    async fn get_verb(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _vname: &str,
    ) -> Result<VerbInfo, ObjectError> {
        todo!()
    }

    async fn get_verb_at_index(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
        _vidx: usize,
    ) -> Result<VerbInfo, ObjectError> {
        todo!()
    }

    async fn find_method_verb_on(
        &mut self,
        _perms: PermissionsContext,
        obj: Objid,
        vname: &str,
    ) -> Result<VerbInfo, ObjectError> {
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
        _oid: Objid,
        _pc: &ParsedCommand,
    ) -> Result<Option<VerbInfo>, ObjectError> {
        todo!()
    }

    async fn parent_of(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<Objid, ObjectError> {
        todo!()
    }

    async fn children_of(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<Vec<Objid>, ObjectError> {
        todo!()
    }

    async fn valid(&mut self, _obj: Objid) -> Result<bool, ObjectError> {
        Ok(true)
    }

    async fn names_of(
        &mut self,
        _perms: PermissionsContext,
        _obj: Objid,
    ) -> Result<(String, Vec<String>), ObjectError> {
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

    async fn add_verb(
        &self,
        _obj: Objid,
        _names: Vec<&str>,
        _owner: Objid,
        _flags: BitEnum<VerbFlag>,
        _args: VerbArgsSpec,
        _binary: Binary,
    ) -> Result<(), Error> {
        todo!()
    }

    async fn get_property(&self, _obj: Objid, _pname: &str) -> Result<Option<u128>, Error> {
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
        _is_clear: bool,
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

    pub fn new_with_verb(name: &str, binary: &Binary) -> Self {
        let mut store = MockStore {
            verbs: Default::default(),
            properties: Default::default(),
        };
        store.set_verb(Objid(0), name, binary);
        Self(Arc::new(Mutex::new(store)))
    }

    pub fn new_with_verbs(verbs: Vec<(&str, &Binary)>) -> Self {
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
    async fn new_world_state(
        &mut self,
        player: Objid,
    ) -> Result<(Box<dyn WorldState>, PermissionsContext), Error> {
        let permissions_context = PermissionsContext::root_for(
            player,
            BitEnum::new() | ObjFlag::Wizard | ObjFlag::Programmer,
        );
        Ok((Box::new(MockState(self.0.clone())), permissions_context))
    }
}
