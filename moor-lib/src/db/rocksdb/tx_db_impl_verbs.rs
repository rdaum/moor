use anyhow::{bail, Context};
use moor_value::AsByteBuffer;
use rocksdb::ErrorKind;
use tracing::trace;
use uuid::Uuid;

use crate::db::rocksdb::tx_db_impl::{composite_key, get_oid_value, oid_key, RocksDbTx};
use crate::db::rocksdb::ColumnFamilies;
use crate::db::{VerbDef, VerbDefs};
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verbs::{BinaryType, VerbFlag};
use moor_value::model::{CommitResult, WorldStateError};
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::{Objid, NOTHING};

impl<'a> RocksDbTx<'a> {
    #[tracing::instrument(skip(self))]
    pub fn get_object_verbs(&self, o: Objid) -> Result<VerbDefs, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok)?;
        let verbs = match verbs_bytes {
            None => VerbDefs::empty(),
            Some(verbs_bytes) => VerbDefs::from_byte_vector(verbs_bytes)
        };
        Ok(verbs)
    }
    #[tracing::instrument(skip(self))]
    pub fn add_object_verb(
        &self,
        oid: Objid,
        owner: Objid,
        names: Vec<String>,
        binary: Vec<u8>,
        binary_type: BinaryType,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), anyhow::Error> {
        // Get the old vector, add the new verb, put the new vector.
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(oid);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let mut verbs: VerbDefs = match verbs_bytes {
            None => VerbDefs::empty(),
            Some(verbs_bytes) => VerbDefs::from_byte_vector(verbs_bytes)
        };

        // Generate a new verb ID.
        let vid = Uuid::new_v4();
        let verb = VerbDef {
            uuid: *vid.as_bytes(),
            location: oid,
            owner,
            names: names.clone(),
            flags,
            binary_type,
            args,
        };
        verbs.push(verb);
        self.tx
            .put_cf(cf, ok, verbs.as_byte_buffer())
            .with_context(|| format!("failure to write verbdef: {}:{:?}", oid, names.clone()))?;

        // Now set the program.
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key(oid, vid.as_bytes());
        self.tx
            .put_cf(cf, vk, binary)
            .with_context(|| format!("failure to write verb program: {}:{:?}", oid, names))?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    pub fn delete_object_verb(&self, o: Objid, v: Uuid) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let verbs: VerbDefs = match verbs_bytes {
            None => VerbDefs::empty(),
            Some(verbs_bytes) => VerbDefs::from_byte_vector(verbs_bytes)
        };
        let Some(verbs) = verbs.with_removed(v) else {
            let v_uuid_str = v.to_string();
            return Err(WorldStateError::VerbNotFound(o, v_uuid_str).into());
        };
        self.tx.put_cf(cf, ok, verbs.as_byte_buffer())?;

        // Delete the program.
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key(o, v.as_bytes());
        self.tx.delete_cf(cf, vk)?;

        Ok(())
    }
    #[tracing::instrument(skip(self))]
    pub fn get_verb(&self, o: Objid, v: Uuid) -> Result<VerbDef, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let verbs: VerbDefs = match verbs_bytes {
            None => VerbDefs::empty(),
            Some(verbs_bytes) => VerbDefs::from_byte_vector(verbs_bytes)
        };
        let verb = verbs.iter().find(|vh| &vh.uuid == v.as_bytes());
        let Some(verb) = verb else {
            let v_uuid_str = v.to_string();
            return Err(WorldStateError::VerbNotFound(o, v_uuid_str).into());
        };
        Ok(verb.clone())
    }
    #[tracing::instrument(skip(self))]
    pub fn get_verb_by_name(&self, o: Objid, n: String) -> Result<VerbDef, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let Some(verbs_bytes) = self.tx.get_cf(cf, ok.clone())? else {
            return Err(WorldStateError::VerbNotFound(o, n).into());
        };
        let verbs = VerbDefs::from_byte_vector(verbs_bytes);
        let Some(verb) = verbs.find_named(n.as_str()) else {
            return Err(WorldStateError::VerbNotFound(o, n).into());
        };
        Ok(verb.clone())
    }
    #[tracing::instrument(skip(self))]
    pub fn get_verb_by_index(&self, o: Objid, i: usize) -> Result<VerbDef, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let verbs: VerbDefs = match verbs_bytes {
            None => VerbDefs::empty(),
            Some(verbs_bytes) => VerbDefs::from_byte_vector(verbs_bytes)
        };
        if i >= verbs.len() {
            return Err(WorldStateError::VerbNotFound(o, format!("{}", i)).into());
        }
        Ok(verbs[i].clone())
    }
    #[tracing::instrument(skip(self))]
    pub fn get_binary(&self, o: Objid, v: Uuid) -> Result<Vec<u8>, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let ok = composite_key(o, v.as_bytes());
        let prg_bytes = self.tx.get_cf(cf, ok)?;
        let Some(prg_bytes) = prg_bytes else {
            let v_uuid_str = v.to_string();
            return Err(WorldStateError::VerbNotFound(o, v_uuid_str).into());
        };
        Ok(prg_bytes)
    }
    #[tracing::instrument(skip(self))]
    pub fn resolve_verb(
        &self,
        o: Objid,
        n: String,
        a: Option<VerbArgsSpec>,
    ) -> Result<VerbDef, anyhow::Error> {
        trace!(object = ?o, verb = %n, args = ?a, "Resolving verb");
        let op_cf = self.cf_handles[(ColumnFamilies::ObjectParent as u8) as usize];
        let ov_cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let mut search_o = o;
        loop {
            let ok = oid_key(search_o);

            let Some(verbs_bytes) = self.tx.get_cf(ov_cf, ok.clone())? else {
                return Err(WorldStateError::VerbNotFound(search_o, n).into());
            };
            let verbs = VerbDefs::from_byte_vector(verbs_bytes);

            // If we found the verb, return it.
            if let Some(verb) = verbs.find_named(n.as_str()) {
                return Ok(verb.clone());
            };

            // Otherwise, find our parent.  If it's, then set o to it and continue unless we've
            // hit the end of the chain.
            let Ok(parent) = get_oid_value(op_cf, &self.tx, search_o) else {
                break;
            };
            if parent == NOTHING {
                break;
            }
            search_o = parent;
        }
        trace!(termination_object = ?search_o, verb = %n, "no verb found");
        Err(WorldStateError::VerbNotFound(o, n).into())
    }
    #[tracing::instrument(skip(self))]
    pub fn retrieve_verb(&self, o: Objid, v: String) -> Result<(Vec<u8>, VerbDef), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let Some(verbs_bytes) = self.tx.get_cf(cf, ok.clone())? else {
            return Err(WorldStateError::VerbNotFound(o, v.clone()).into())
        };
        let verbs = VerbDefs::from_byte_vector(verbs_bytes);
        let Some(verb) = verbs.find_named(v.as_str()) else {
            return Err(WorldStateError::VerbNotFound(o, v.clone()).into())
        };
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key(o, &verb.uuid);
        let prg_bytes = self.tx.get_cf(cf, vk)?;
        let Some(prg_bytes) = prg_bytes else {
            return Err(WorldStateError::VerbNotFound(o, v.clone()).into())
        };
        Ok((prg_bytes, verb.clone()))
    }
    #[tracing::instrument(skip(self))]
    pub fn set_verb_info(
        &self,
        o: Objid,
        v: Uuid,
        new_owner: Option<Objid>,
        new_perms: Option<BitEnum<VerbFlag>>,
        new_names: Option<Vec<String>>,
        new_args: Option<VerbArgsSpec>,
    ) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let verbs: VerbDefs = match verbs_bytes {
            None => VerbDefs::empty(),
            Some(verbs_bytes) => VerbDefs::from_byte_vector(verbs_bytes)
        };
        let Some(new_verbs) = verbs.with_updated(v, |ov| {
            let mut nv = ov.clone();
            if let Some(new_owner) = &new_owner {
                nv.owner = *new_owner;
            }
            if let Some(new_perms) = &new_perms {
                nv.flags = *new_perms;
            }
            if let Some(new_names) = &new_names {
                nv.names = new_names.clone();
            }
            if let Some(new_args) = &new_args {
                nv.args = *new_args;
            }
            nv
        }) else {
            let v_uuid_str = v.to_string();
            return Err(WorldStateError::VerbNotFound(o, v_uuid_str).into());
        };

        self.tx.put_cf(cf, ok, new_verbs.as_byte_buffer())?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    pub fn commit(self) -> Result<CommitResult, anyhow::Error> {
        match self.tx.commit() {
            Ok(()) => Ok(CommitResult::Success),
            Err(e) if e.kind() == ErrorKind::Busy || e.kind() == ErrorKind::TryAgain => {
                Ok(CommitResult::ConflictRetry)
            }
            Err(e) => bail!(e),
        }
    }
    #[tracing::instrument(skip(self))]
    pub fn rollback(&self) -> Result<(), anyhow::Error> {
        self.tx.rollback()?;
        Ok(())
    }
}
