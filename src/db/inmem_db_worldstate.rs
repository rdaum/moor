use std::sync::atomic::AtomicPtr;

use anyhow::{anyhow, Error};

use crate::db::inmem_db::ImDB;
use crate::db::state::{StateError, WorldState, WorldStateSource};
use crate::db::tx::Tx;
use crate::db::{relations, CommitResult};
use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag, Objects};
use crate::model::permissions::Permissions;
use crate::model::props::{
    Pid, PropAttr, PropAttrs, PropDefs, PropFlag, Propdef, Properties, PropertyInfo,
};
use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
use crate::model::var::{Objid, Var, NOTHING};
use crate::model::verbs::{VerbAttr, VerbAttrs, VerbFlag, VerbInfo, Verbs, Vid};

use crate::server::parse_cmd::ParsedCommand;
use crate::util::bitenum::BitEnum;
use crate::vm::opcode::Binary;

pub struct ImDbWorldStateSource {
    db: ImDB,
}
unsafe impl Send for ImDbWorldStateSource {}
unsafe impl Sync for ImDbWorldStateSource {}

impl ImDbWorldStateSource {
    pub fn new(db: ImDB) -> Self {
        ImDbWorldStateSource { db }
    }
}

impl WorldStateSource for ImDbWorldStateSource {
    fn new_world_state(&mut self) -> Result<Box<dyn WorldState>, Error> {
        let tx = self.db.do_begin_tx()?;
        let odb = ImDBTx::new(AtomicPtr::new(&mut self.db), tx);
        Ok(odb)
    }
}

pub struct ImDBTx {
    db: AtomicPtr<ImDB>,
    tx: Tx,
}

impl ImDBTx {
    pub fn new(db: AtomicPtr<ImDB>, tx: Tx) -> Box<dyn WorldState> {
        Box::new(ImDBTx { db, tx })
    }

    pub fn get_db<'a>(&mut self) -> &'a mut ImDB {
        unsafe { return self.db.get_mut().as_mut().unwrap() }
    }
}

// The DB itself attempts to be thread-safe, there's no need to hold it in an Arc/Mutex.
// We can the tx handle here send and sync-able, and it can be passed around and traded.
unsafe impl Send for ImDBTx {}
unsafe impl Sync for ImDBTx {}

impl Objects for ImDBTx {
    fn create_object(&mut self, oid: Option<Objid>, attrs: &ObjAttrs) -> Result<Objid, Error> {
        let db = self.get_db();
        db.create_object(&mut self.tx, oid, attrs)
    }

    fn destroy_object(&mut self, oid: Objid) -> Result<(), Error> {
        self.get_db().destroy_object(&mut self.tx, oid)
    }

    fn object_valid(&mut self, oid: Objid) -> Result<bool, Error> {
        self.get_db().object_valid(&mut self.tx, oid)
    }

    fn object_get_attrs(
        &mut self,
        oid: Objid,
        attributes: BitEnum<ObjAttr>,
    ) -> Result<ObjAttrs, Error> {
        self.get_db()
            .object_get_attrs(&mut self.tx, oid, attributes)
    }

    fn object_set_attrs(&mut self, oid: Objid, attributes: ObjAttrs) -> Result<(), Error> {
        self.get_db()
            .object_set_attrs(&mut self.tx, oid, attributes)
    }

    fn object_children(&mut self, oid: Objid) -> Result<Vec<Objid>, Error> {
        self.get_db().object_children(&mut self.tx, oid)
    }

    fn object_contents(&mut self, oid: Objid) -> Result<Vec<Objid>, Error> {
        self.get_db().object_contents(&mut self.tx, oid)
    }
}

impl Properties for ImDBTx {
    fn find_property(
        &mut self,
        oid: Objid,
        name: &str,
        attrs: BitEnum<PropAttr>,
    ) -> Result<Option<PropertyInfo>, Error> {
        self.get_db().find_property(&mut self.tx, oid, name, attrs)
    }

    fn get_property(
        &mut self,
        oid: Objid,
        handle: Pid,
        attrs: BitEnum<PropAttr>,
    ) -> Result<Option<PropAttrs>, Error> {
        self.get_db().get_property(&mut self.tx, oid, handle, attrs)
    }

    fn set_property(
        &mut self,
        handle: Pid,
        location: Objid,
        value: Var,
        owner: Objid,
        flags: BitEnum<PropFlag>,
    ) -> Result<(), Error> {
        self.get_db()
            .set_property(&mut self.tx, handle, location, value, owner, flags)
    }
}

impl Verbs for ImDBTx {
    fn add_verb(
        &mut self,
        oid: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        arg_spec: VerbArgsSpec,
        program: Binary,
    ) -> Result<VerbInfo, Error> {
        self.get_db()
            .add_verb(&mut self.tx, oid, names, owner, flags, arg_spec, program)
    }

    fn get_verbs(&mut self, oid: Objid, attrs: BitEnum<VerbAttr>) -> Result<Vec<VerbInfo>, Error> {
        self.get_db().get_verbs(&mut self.tx, oid, attrs)
    }

    fn get_verb(&mut self, vid: Vid, attrs: BitEnum<VerbAttr>) -> Result<VerbInfo, Error> {
        self.get_db().get_verb(&mut self.tx, vid, attrs)
    }

    fn update_verb(&mut self, vid: Vid, attrs: VerbAttrs) -> Result<(), Error> {
        self.get_db().update_verb(&mut self.tx, vid, attrs)
    }

    fn find_command_verb(
        &mut self,
        obj: Objid,
        verb: &str,
        dobj: ArgSpec,
        prep: PrepSpec,
        iobj: ArgSpec,
    ) -> Result<Option<VerbInfo>, anyhow::Error> {
        self.get_db()
            .find_command_verb(&mut self.tx, obj, verb, dobj, prep, iobj)
    }

    fn find_callable_verb(
        &mut self,
        oid: Objid,
        verb: &str,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        self.get_db()
            .find_callable_verb(&mut self.tx, oid, verb, attrs)
    }

    fn find_indexed_verb(
        &mut self,
        oid: Objid,
        index: usize,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        self.get_db()
            .find_indexed_verb(&mut self.tx, oid, index, attrs)
    }
}

impl Permissions for ImDBTx {
    fn property_allows(
        &mut self,
        check_flags: BitEnum<PropFlag>,
        player: Objid,
        player_flags: BitEnum<ObjFlag>,
        prop_flags: BitEnum<PropFlag>,
        prop_owner: Objid,
    ) -> bool {
        self.get_db().property_allows(
            &mut self.tx,
            check_flags,
            player,
            player_flags,
            prop_flags,
            prop_owner,
        )
    }
}

impl PropDefs for ImDBTx {
    fn get_propdef(&mut self, definer: Objid, pname: &str) -> Result<Propdef, Error> {
        self.get_db().get_propdef(&mut self.tx, definer, pname)
    }

    fn add_propdef(
        &mut self,
        definer: Objid,
        name: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<Pid, Error> {
        self.get_db()
            .add_propdef(&mut self.tx, definer, name, owner, flags, initial_value)
    }

    fn rename_propdef(&mut self, definer: Objid, old: &str, new: &str) -> Result<(), Error> {
        self.get_db()
            .rename_propdef(&mut self.tx, definer, old, new)
    }

    fn delete_propdef(&mut self, definer: Objid, pname: &str) -> Result<(), Error> {
        self.get_db().delete_propdef(&mut self.tx, definer, pname)
    }

    fn count_propdefs(&mut self, definer: Objid) -> Result<usize, Error> {
        self.get_db().count_propdefs(&mut self.tx, definer)
    }

    fn get_propdefs(&mut self, definer: Objid) -> Result<Vec<Propdef>, Error> {
        self.get_db().get_propdefs(&mut self.tx, definer)
    }
}

impl WorldState for ImDBTx {
    fn location_of(&mut self, obj: Objid) -> Result<Objid, Error> {
        self.object_get_attrs(obj, BitEnum::new_with(ObjAttr::Location))
            .map(|attrs| attrs.location.unwrap())
    }

    fn contents_of(&mut self, obj: Objid) -> Result<Vec<Objid>, Error> {
        self.object_contents(obj)
    }

    fn retrieve_verb(&mut self, obj: Objid, vname: &str) -> Result<(Binary, VerbInfo), Error> {
        let h = self.find_callable_verb(
            obj,
            vname,
            BitEnum::new_with(VerbAttr::Program)
                | VerbAttr::Flags
                | VerbAttr::Owner
                | VerbAttr::Definer,
        )?;
        let Some(vi) = h else {
            return Err(anyhow!(StateError::VerbNotFound(obj, vname.to_string())));
        };

        let Some(binary) = &vi.attrs.program else {
            return Err(anyhow!(StateError::VerbDecodeError(obj, vname.to_string())));
        };

        Ok((binary.clone(), vi))
    }

    fn retrieve_property(
        &mut self,
        obj: Objid,
        property_name: &str,
        player_flags: BitEnum<ObjFlag>,
    ) -> Result<Var, Error> {
        // TODO builtin properties!
        let find = self
            .find_property(
                obj,
                property_name,
                BitEnum::new_with(PropAttr::Owner)
                    | PropAttr::Flags
                    | PropAttr::Location
                    | PropAttr::Value,
            )
            .expect("db fail");
        match find {
            None => Err(anyhow!(StateError::PropertyNotFound(
                obj,
                property_name.to_string()
            ))),
            Some(p) => {
                if !self.property_allows(
                    PropFlag::Read.into(),
                    obj,
                    player_flags,
                    p.attrs.flags.unwrap(),
                    p.attrs.owner.unwrap(),
                ) {
                    Err(anyhow!(StateError::PropertyPermissionDenied(
                        obj,
                        property_name.to_string()
                    )))
                } else {
                    match p.attrs.value {
                        None => Err(anyhow!(StateError::PropertyNotFound(
                            obj,
                            property_name.to_string()
                        ))),
                        Some(p) => Ok(p),
                    }
                }
            }
        }
    }

    fn update_property(
        &mut self,
        obj: Objid,
        property_name: &str,
        player_flags: BitEnum<ObjFlag>,
        value: &Var,
    ) -> Result<(), Error> {
        let h = self
            .find_property(
                obj,
                property_name,
                BitEnum::new_with(PropAttr::Owner) | PropAttr::Flags,
            )
            .expect("Unable to perform property lookup");

        // TODO handle built-in properties
        let Some(p) = h else {
            return Err(anyhow!(StateError::PropertyNotFound(obj, property_name.to_string())));
        };

        if self.property_allows(
            PropFlag::Write.into(),
            obj,
            player_flags,
            p.attrs.flags.unwrap(),
            p.attrs.owner.unwrap(),
        ) {
            return Err(anyhow!(StateError::PropertyPermissionDenied(
                obj,
                property_name.to_string()
            )));
        }

        // Failure on this is a panic.
        self.set_property(
            p.pid,
            obj,
            value.clone(),
            p.attrs.owner.unwrap(),
            p.attrs.flags.unwrap(),
        )
        .expect("could not set property");
        Ok(())
    }

    fn add_property(
        &mut self,
        obj: Objid,
        pname: &str,
        _owner: Objid,
        prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), anyhow::Error> {
        self.add_propdef(obj, pname, obj, prop_flags, initial_value)?;
        Ok(())
    }

    fn parent_of(&mut self, obj: Objid) -> Result<Objid, Error> {
        let attrs =
            self.object_get_attrs(obj, BitEnum::new_with(ObjAttr::Parent) | ObjAttr::Owner)?;
        // TODO: this masks other (internal?) errors..
        attrs
            .parent
            .ok_or(anyhow!(StateError::ObjectNotFoundError(obj)))
    }

    fn find_command_verb_on(
        &mut self,
        oid: Objid,
        pc: &ParsedCommand,
    ) -> Result<Option<VerbInfo>, anyhow::Error> {
        if !self.valid(oid)? {
            return Ok(None);
        }

        let spec_for_fn = |oid, pco| -> ArgSpec {
            if pco == oid {
                ArgSpec::This
            } else if pco == NOTHING {
                ArgSpec::None
            } else {
                ArgSpec::Any
            }
        };

        let dobj = spec_for_fn(oid, pc.dobj);
        let iobj = spec_for_fn(oid, pc.iobj);

        self.find_command_verb(oid, pc.verb.as_str(), dobj, pc.prep, iobj)
    }

    fn valid(&mut self, obj: Objid) -> Result<bool, Error> {
        // TODO: this masks other (internal?) errors..
        Ok(self
            .object_get_attrs(obj, BitEnum::new_with(ObjAttr::Parent) | ObjAttr::Owner)
            .is_ok())
    }

    fn names_of(&mut self, obj: Objid) -> Result<(String, Vec<String>), Error> {
        // TODO implement support for aliases.
        let name = self
            .object_get_attrs(obj, BitEnum::from(ObjAttr::Name))?
            .name
            .unwrap();
        Ok((name, vec![]))
    }

    fn commit(&mut self) -> Result<CommitResult, anyhow::Error> {
        match self.get_db().do_commit_tx(&mut self.tx) {
            Ok(_) => Ok(CommitResult::Success),
            Err(relations::Error::Conflict) => Ok(CommitResult::ConflictRetry),
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn rollback(&mut self) -> Result<(), anyhow::Error> {
        self.get_db().do_rollback_tx(&mut self.tx)?;
        Ok(())
    }
}
