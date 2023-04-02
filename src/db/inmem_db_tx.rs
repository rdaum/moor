use std::sync::{Arc, Mutex};
use anyhow::{anyhow, Error};
use bincode::config;
use bincode::error::DecodeError;
use enumset::EnumSet;
use crate::db::inmem_db::ImDB;
use crate::db::state::{WorldState, WorldStateSource, StateError};
use crate::model::ObjDB;
use crate::model::objects::{ObjAttr, Objects, ObjFlag};
use crate::model::permissions::Permissions;
use crate::model::props::{PropAttr, PropDefs, Properties, PropFlag};
use crate::model::var::{Objid, Var};
use crate::model::verbs::{VerbAttr, VerbInfo, Verbs};
use crate::vm::opcode::Binary;

pub struct ImDbTxSource {
    db: Arc<Mutex<ImDB>>,
    next_tx_id : u64
}

impl ImDbTxSource {
    pub fn new(db: ImDB) -> ImDbTxSource {
        ImDbTxSource { db: Arc::new(Mutex::new(db)), next_tx_id: 0 }
    }
}

impl WorldStateSource for ImDbTxSource {
    fn new_transaction(&mut self) -> Result<Arc<Mutex<dyn WorldState>>, Error> {
        let tx_id = self.next_tx_id;
        self.next_tx_id += 1;
        Ok(ImDBTx::new(self.db.clone(), tx_id))
    }
}

pub struct ImDBTx {
    db: Arc<Mutex<ImDB>>,
    tx_id : u64,
}

impl ImDBTx {
    pub fn new(db: Arc<Mutex<ImDB>>, tx_id: u64) -> Arc<Mutex<dyn WorldState>> {
        Arc::new(Mutex::new(ImDBTx { db, tx_id }))
    }
}

impl WorldState for ImDBTx {
    fn location_of(&mut self, obj: Objid) -> Result<Objid, Error> {
        let mut db = self.db.lock().unwrap();
        db.object_get_attrs(obj, EnumSet::from(ObjAttr::Location))
            .map(|attrs| attrs.location.unwrap())
    }

    fn contents_of(&mut self, obj: Objid) -> Result<Vec<Objid>, Error> {
        let db = self.db.lock().unwrap();
        db.object_contents(obj)
    }

    fn retrieve_verb(&self, obj: Objid, vname: &str) -> Result<(Binary, VerbInfo), Error> {
        let db = self.db.lock().unwrap();
        let h = db.find_callable_verb(
            obj,
            vname,
            VerbAttr::Program | VerbAttr::Flags | VerbAttr::Owner | VerbAttr::Definer,
        )?;
        let Some(vi) = h else {
            return Err(anyhow!(StateError::VerbNotFound(obj, vname.to_string())));
        };

        let program = vi.clone().attrs.program.unwrap();
        let slc = &program.0[..];
        let result: Result<(Binary, usize), DecodeError> =
            bincode::serde::decode_from_slice(slc, config::standard());
        let Ok((binary, _size)) = result else {
            return Err(anyhow!(StateError::VerbDecodeError(obj, vname.to_string())));
        };

        Ok((binary, vi))
    }

    fn retrieve_property(
        &self,
        obj: Objid,
        property_name: &str,
        player_flags: EnumSet<ObjFlag>,
    ) -> Result<Var, Error> {
        // TODO builtin properties!
        let db = self.db.lock().unwrap();
        let find = db
            .find_property(
                obj,
                property_name,
                PropAttr::Owner | PropAttr::Flags | PropAttr::Location | PropAttr::Value,
            )
            .expect("db fail");
        match find {
            None => Err(anyhow!(StateError::PropertyNotFound(
                obj,
                property_name.to_string()
            ))),
            Some(p) => {
                if !db.property_allows(
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
        player_flags: EnumSet<ObjFlag>,
        value: &Var,
    ) -> Result<(), Error> {
        let mut db = self.db.lock().unwrap();
        let h = db
            .find_property(obj, property_name, PropAttr::Owner | PropAttr::Flags)
            .expect("Unable to perform property lookup");

        // TODO handle built-in properties
        let Some(p) = h else {
            return Err(anyhow!(StateError::PropertyNotFound(obj, property_name.to_string())));
        };

        if db.property_allows(
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
        db.set_property(
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
        prop_flags: EnumSet<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), anyhow::Error> {
        let mut db = self.db.lock().unwrap();
        db.add_propdef(obj, pname, obj, prop_flags, initial_value)?;
        Ok(())
    }

    fn parent_of(&mut self, obj: Objid) -> Result<Objid, Error> {
        let mut db = self.db.lock().unwrap();
        let attrs = db.object_get_attrs(obj, ObjAttr::Parent | ObjAttr::Owner)?;
        // TODO: this masks other (internal?) errors..
        attrs
            .parent
            .ok_or(anyhow!(StateError::ObjectNotFoundError(obj)))
    }

    fn valid(&mut self, obj: Objid) -> Result<bool, Error> {
        let mut db = self.db.lock().unwrap();
        // TODO: this masks other (internal?) errors..
        Ok(db
            .object_get_attrs(obj, ObjAttr::Parent | ObjAttr::Owner)
            .is_ok())
    }

    fn names_of(&mut self, obj: Objid) -> Result<(String, Vec<String>), Error> {
        let mut db = self.db.lock().unwrap();

        // TODO implement support for aliases.
        let name = db
            .object_get_attrs(obj, EnumSet::from(ObjAttr::Name))?
            .name
            .unwrap();
        Ok((name, vec![]))
    }

    fn commit(self) -> Result<(), anyhow::Error> {
        let mut db = self.db.lock().unwrap();
        db.commit()
    }

    fn rollback(self) -> Result<(), anyhow::Error> {
        let mut db = self.db.lock().unwrap();
        db.rollback()
    }
}
