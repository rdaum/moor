use anyhow::Context;
use tracing::trace;
use uuid::Uuid;

use moor_value::model::defset::HasUuid;
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verbdef::{VerbDef, VerbDefs};
use moor_value::model::verbs::{BinaryType, VerbFlag};
use moor_value::model::WorldStateError;
use moor_value::util::bitenum::BitEnum;
use moor_value::util::slice_ref::SliceRef;
use moor_value::var::objid::Objid;
use moor_value::NOTHING;

use crate::db::rocksdb::tx_db_impl::{
    composite_key_for, composite_key_uuid, get_oid_value, oid_key, write_cf, RocksDbTx,
};
use crate::db::rocksdb::ColumnFamilies;

impl<'a> RocksDbTx<'a> {
    #[tracing::instrument(skip(self))]
    pub fn get_object_verbs(&self, o: Objid) -> Result<VerbDefs, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_pinned_cf(cf, ok)?;
        let verbs = match verbs_bytes {
            None => VerbDefs::empty(),
            Some(verbs_bytes) => VerbDefs::from_sliceref(SliceRef::from_bytes(&verbs_bytes)),
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
        let verbs_bytes = self.tx.get_cf(cf, ok)?;
        let verbs: VerbDefs = match verbs_bytes {
            None => VerbDefs::empty(),
            Some(verbs_bytes) => VerbDefs::from_sliceref(SliceRef::from_bytes(&verbs_bytes)),
        };

        // Generate a new verb ID.
        let vid = Uuid::new_v4();
        let names = names.iter().map(|n| n.as_str()).collect::<Vec<&str>>();
        let verb = VerbDef::new(vid, oid, owner, &names, flags, binary_type, args);
        let verbs = verbs.with_added(verb);
        write_cf(&self.tx, cf, &ok, &verbs)
            .with_context(|| format!("failure to write verbdef: {}:{:?}", oid, names.clone()))?;

        // Now set the program.
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key_uuid(oid, &vid);
        self.tx
            .put_cf(cf, vk, binary)
            .with_context(|| format!("failure to write verb program: {}:{:?}", oid, names))?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    pub fn delete_object_verb(&self, o: Objid, v: Uuid) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok)?;
        let verbs: VerbDefs = match verbs_bytes {
            None => VerbDefs::empty(),
            Some(verbs_bytes) => VerbDefs::from_sliceref(SliceRef::from_bytes(&verbs_bytes)),
        };
        let Some(verbs) = verbs.with_removed(v) else {
            let v_uuid_str = v.to_string();
            return Err(WorldStateError::VerbNotFound(o, v_uuid_str).into());
        };
        write_cf(&self.tx, cf, &ok, &verbs)?;

        // Delete the program.
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key_uuid(o, &v);
        self.tx.delete_cf(cf, vk)?;

        Ok(())
    }
    #[tracing::instrument(skip(self))]
    pub fn get_verb(&self, o: Objid, v: Uuid) -> Result<VerbDef, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok)?;
        let verbs: VerbDefs = match verbs_bytes {
            None => VerbDefs::empty(),
            Some(verbs_bytes) => VerbDefs::from_sliceref(SliceRef::from_bytes(&verbs_bytes)),
        };
        let verb = verbs.find(&v);
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
        let Some(verbs_bytes) = self.tx.get_cf(cf, ok)? else {
            return Err(WorldStateError::VerbNotFound(o, n).into());
        };
        let verbs = VerbDefs::from_sliceref(SliceRef::from_bytes(&verbs_bytes));
        let Some(verb) = verbs.find_named(n.as_str()) else {
            return Err(WorldStateError::VerbNotFound(o, n).into());
        };
        Ok(verb.clone())
    }
    #[tracing::instrument(skip(self))]
    pub fn get_verb_by_index(&self, o: Objid, i: usize) -> Result<VerbDef, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok)?;
        let verbs: VerbDefs = match verbs_bytes {
            None => VerbDefs::empty(),
            Some(verbs_bytes) => VerbDefs::from_sliceref(SliceRef::from_bytes(&verbs_bytes)),
        };
        if i >= verbs.len() {
            return Err(WorldStateError::VerbNotFound(o, format!("{}", i)).into());
        }
        verbs
            .iter()
            .nth(i)
            .ok_or_else(|| WorldStateError::VerbNotFound(o, format!("{}", i)).into())
    }
    #[tracing::instrument(skip(self))]
    pub fn get_binary(&self, o: Objid, v: Uuid) -> Result<Vec<u8>, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let ok = composite_key_uuid(o, &v);
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

            let verbs = match self.tx.get_cf(ov_cf, ok)? {
                None => VerbDefs::empty(),
                Some(verbs_bytes) => VerbDefs::from_sliceref(SliceRef::from_bytes(&verbs_bytes)),
            };

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
        let Some(verbs_bytes) = self.tx.get_cf(cf, ok)? else {
            return Err(WorldStateError::VerbNotFound(o, v.clone()).into());
        };
        let verbs = VerbDefs::from_sliceref(SliceRef::from_bytes(&verbs_bytes));
        let Some(verb) = verbs.find_named(v.as_str()) else {
            return Err(WorldStateError::VerbNotFound(o, v.clone()).into());
        };
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key_for(o, &verb);
        let prg_bytes = self.tx.get_cf(cf, vk)?;
        let Some(prg_bytes) = prg_bytes else {
            return Err(WorldStateError::VerbNotFound(o, v.clone()).into());
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
        new_binary_type: Option<BinaryType>,
    ) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok)?;
        let verbs: VerbDefs = match verbs_bytes {
            None => VerbDefs::empty(),
            Some(verbs_bytes) => VerbDefs::from_sliceref(SliceRef::from_bytes(&verbs_bytes)),
        };
        let Some(new_verbs) = verbs.with_updated(v, |ov| {
            let names = match &new_names {
                None => ov.names(),
                Some(new_names) => new_names.iter().map(|n| n.as_str()).collect::<Vec<&str>>(),
            };
            VerbDef::new(
                ov.uuid(),
                ov.location(),
                new_owner.unwrap_or(ov.owner()),
                &names,
                new_perms.unwrap_or(ov.flags()),
                new_binary_type.unwrap_or(ov.binary_type()),
                new_args.unwrap_or(ov.args()),
            )
        }) else {
            let v_uuid_str = v.to_string();
            return Err(WorldStateError::VerbNotFound(o, v_uuid_str).into());
        };

        write_cf(&self.tx, cf, &ok, &new_verbs)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    pub fn set_verb_binary(
        &self,
        o: Objid,
        v: Uuid,
        new_binary: Vec<u8>,
    ) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key_uuid(o, &v);
        self.tx.put_cf(cf, vk, new_binary)?;
        Ok(())
    }
}
