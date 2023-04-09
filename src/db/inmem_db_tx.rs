use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

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
use crate::model::r#match::VerbArgsSpec;
use crate::model::var::{Objid, Var};
use crate::model::verbs::{VerbAttr, VerbAttrs, VerbFlag, VerbInfo, Verbs, Vid};
use crate::util::bitenum::BitEnum;
use crate::vm::opcode::Binary;

pub struct ImDbTxSource {
    db: Arc<Mutex<ImDB>>,
    next_tx_id: AtomicU64,

    // Global atomic counter for the next transactions start timestamp
    gtls: AtomicU64,
}

impl ImDbTxSource {
    pub fn new(db: ImDB) -> ImDbTxSource {
        ImDbTxSource {
            db: Arc::new(Mutex::new(db)),
            next_tx_id: Default::default(),
            gtls: Default::default(),
        }
    }
}

impl WorldStateSource for ImDbTxSource {
    fn new_transaction(&mut self) -> Result<Arc<Mutex<dyn WorldState>>, Error> {
        let tx_id = self
            .next_tx_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let tx_start_ts = self.gtls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let tx = Tx::new(tx_id, tx_start_ts);
        Ok(ImDBTx::new(self.db.clone(), tx))
    }
}

pub struct ImDBTx {
    db: Arc<Mutex<ImDB>>,
    tx: Tx,
}

impl ImDBTx {
    pub fn new(db: Arc<Mutex<ImDB>>, tx: Tx) -> Arc<Mutex<dyn WorldState>> {
        Arc::new(Mutex::new(ImDBTx { db, tx }))
    }
}

impl Objects for ImDBTx {
    fn create_object(&mut self, oid: Option<Objid>, attrs: &ObjAttrs) -> Result<Objid, Error> {
        self.db
            .lock()
            .unwrap()
            .create_object(&mut self.tx, oid, attrs)
    }

    fn destroy_object(&mut self, oid: Objid) -> Result<(), Error> {
        self.db.lock().unwrap().destroy_object(&mut self.tx, oid)
    }

    fn object_valid(&mut self, oid: Objid) -> Result<bool, Error> {
        self.db.lock().unwrap().object_valid(&mut self.tx, oid)
    }

    fn object_get_attrs(
        &mut self,
        oid: Objid,
        attributes: BitEnum<ObjAttr>,
    ) -> Result<ObjAttrs, Error> {
        self.db
            .lock()
            .unwrap()
            .object_get_attrs(&mut self.tx, oid, attributes)
    }

    fn object_set_attrs(&mut self, oid: Objid, attributes: ObjAttrs) -> Result<(), Error> {
        self.db
            .lock()
            .unwrap()
            .object_set_attrs(&mut self.tx, oid, attributes)
    }

    fn object_children(&mut self, oid: Objid) -> Result<Vec<Objid>, Error> {
        self.db.lock().unwrap().object_children(&mut self.tx, oid)
    }

    fn object_contents(&mut self, oid: Objid) -> Result<Vec<Objid>, Error> {
        self.db.lock().unwrap().object_contents(&mut self.tx, oid)
    }
}

impl Properties for ImDBTx {
    fn find_property(
        &mut self,
        oid: Objid,
        name: &str,
        attrs: BitEnum<PropAttr>,
    ) -> Result<Option<PropertyInfo>, Error> {
        self.db
            .lock()
            .unwrap()
            .find_property(&mut self.tx, oid, name, attrs)
    }

    fn get_property(
        &mut self,
        oid: Objid,
        handle: Pid,
        attrs: BitEnum<PropAttr>,
    ) -> Result<Option<PropAttrs>, Error> {
        self.db
            .lock()
            .unwrap()
            .get_property(&mut self.tx, oid, handle, attrs)
    }

    fn set_property(
        &mut self,
        handle: Pid,
        location: Objid,
        value: Var,
        owner: Objid,
        flags: BitEnum<PropFlag>,
    ) -> Result<(), Error> {
        self.db
            .lock()
            .unwrap()
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
        self.db
            .lock()
            .unwrap()
            .add_verb(&mut self.tx, oid, names, owner, flags, arg_spec, program)
    }

    fn get_verbs(&mut self, oid: Objid, attrs: BitEnum<VerbAttr>) -> Result<Vec<VerbInfo>, Error> {
        self.db.lock().unwrap().get_verbs(&mut self.tx, oid, attrs)
    }

    fn get_verb(&mut self, vid: Vid, attrs: BitEnum<VerbAttr>) -> Result<VerbInfo, Error> {
        self.db.lock().unwrap().get_verb(&mut self.tx, vid, attrs)
    }

    fn update_verb(&mut self, vid: Vid, attrs: VerbAttrs) -> Result<(), Error> {
        self.db
            .lock()
            .unwrap()
            .update_verb(&mut self.tx, vid, attrs)
    }

    fn find_command_verb(
        &mut self,
        oid: Objid,
        verb: &str,
        arg_spec: VerbArgsSpec,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        self.db
            .lock()
            .unwrap()
            .find_command_verb(&mut self.tx, oid, verb, arg_spec, attrs)
    }

    fn find_callable_verb(
        &mut self,
        oid: Objid,
        verb: &str,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        self.db
            .lock()
            .unwrap()
            .find_callable_verb(&mut self.tx, oid, verb, attrs)
    }

    fn find_indexed_verb(
        &mut self,
        oid: Objid,
        index: usize,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        self.db
            .lock()
            .unwrap()
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
        self.db.lock().unwrap().property_allows(
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
        self.db
            .lock()
            .unwrap()
            .get_propdef(&mut self.tx, definer, pname)
    }

    fn add_propdef(
        &mut self,
        definer: Objid,
        name: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<Pid, Error> {
        self.db.lock().unwrap().add_propdef(
            &mut self.tx,
            definer,
            name,
            owner,
            flags,
            initial_value,
        )
    }

    fn rename_propdef(&mut self, definer: Objid, old: &str, new: &str) -> Result<(), Error> {
        self.db
            .lock()
            .unwrap()
            .rename_propdef(&mut self.tx, definer, old, new)
    }

    fn delete_propdef(&mut self, definer: Objid, pname: &str) -> Result<(), Error> {
        self.db
            .lock()
            .unwrap()
            .delete_propdef(&mut self.tx, definer, pname)
    }

    fn count_propdefs(&mut self, definer: Objid) -> Result<usize, Error> {
        self.db
            .lock()
            .unwrap()
            .count_propdefs(&mut self.tx, definer)
    }

    fn get_propdefs(&mut self, definer: Objid) -> Result<Vec<Propdef>, Error> {
        self.db.lock().unwrap().get_propdefs(&mut self.tx, definer)
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

    fn commit(mut self) -> Result<CommitResult, anyhow::Error> {
        match self.db.lock().unwrap().do_commit_tx(&mut self.tx) {
            Ok(_) => Ok(CommitResult::Success),
            Err(relations::Error::Conflict) => Ok(CommitResult::ConflictRetry),
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn rollback(self) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
