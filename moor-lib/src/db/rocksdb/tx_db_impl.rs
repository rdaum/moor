use anyhow::{bail, Error};
use lazy_static::lazy_static;
use rocksdb::{ColumnFamily, ErrorKind};
use tracing::trace;
use uuid::Uuid;

use moor_value::util::bitenum::BitEnum;
use moor_value::util::verbname_cmp;
use moor_value::var::objid::{Objid, NOTHING};
use moor_value::var::{v_none, Var};

use crate::db::rocksdb::tx_server::{PropHandle, VerbHandle};
use crate::db::rocksdb::{ColumnFamilies, DbStorage};
use crate::db::CommitResult;
use crate::model::objects::{ObjAttrs, ObjFlag};
use crate::model::props::PropFlag;
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::VerbFlag;
use crate::model::ObjectError;
use crate::vm::opcode::Binary;

lazy_static! {
    static ref BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();
}

fn oid_key(o: Objid) -> Vec<u8> {
    o.0.to_be_bytes().to_vec()
}

fn composite_key(o: Objid, uuid: u128) -> Vec<u8> {
    let mut key = oid_key(o);
    key.extend_from_slice(&uuid.to_be_bytes());
    key
}

fn oid_vec(o: Vec<Objid>) -> Result<Vec<u8>, anyhow::Error> {
    let ov = bincode::encode_to_vec(o, *BINCODE_CONFIG)?;
    Ok(ov)
}

fn get_oid_value<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
) -> Result<Objid, anyhow::Error> {
    let ok = oid_key(o);
    let ov = tx.get_cf(cf, ok).unwrap();
    let ov = ov.ok_or(ObjectError::ObjectNotFound(o))?;
    let ov = u64::from_be_bytes(ov.try_into().unwrap());
    let ov = Objid(ov as i64);
    Ok(ov)
}

fn set_oid_value<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
    v: Objid,
) -> Result<(), anyhow::Error> {
    let ok = oid_key(o);
    let ov = oid_key(v);
    tx.put_cf(cf, ok, ov).unwrap();
    Ok(())
}

fn get_oid_vec<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
) -> Result<Vec<Objid>, anyhow::Error> {
    let ok = oid_key(o);
    let ov = tx.get_cf(cf, ok).unwrap();
    let ov = ov.ok_or(ObjectError::ObjectNotFound(o))?;
    let (ov, _) = bincode::decode_from_slice(&ov, *BINCODE_CONFIG).unwrap();
    Ok(ov)
}

fn set_oid_vec<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
    v: Vec<Objid>,
) -> Result<(), ObjectError> {
    let ok = oid_key(o);
    let ov = oid_vec(v).unwrap();
    tx.put_cf(cf, ok, ov).unwrap();
    Ok(())
}

fn cf_for<'a>(cf_handles: &[&'a ColumnFamily], cf: ColumnFamilies) -> &'a ColumnFamily {
    cf_handles[(cf as u8) as usize]
}

fn err_is_objnjf(e: &anyhow::Error) -> bool {
    if let Some(ObjectError::ObjectNotFound(_)) = e.downcast_ref::<ObjectError>() {
        return true;
    }
    false
}

pub(crate) struct RocksDbTx<'a> {
    pub(crate) tx: rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    pub(crate) cf_handles: Vec<&'a ColumnFamily>,
}

fn match_in_verb_names<'a>(verb_names: &'a [String], word: &str) -> Option<&'a String> {
    verb_names.iter().find(|&verb| verbname_cmp(verb, word))
}

impl<'a> RocksDbTx<'a> {
    // TODO sucks to do this transactionally, but we need to make sure we don't create a duplicate
    // we could do this an atomic increment on the whole DB, but in the long run we actually want to
    // get rid of object ids entirely.
    fn next_object_id(&self) -> Result<Objid, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectIds as u8) as usize];
        let key = "OBJECT_ID_COUNTER".as_bytes();
        let id_bytes = self.tx.get_cf(cf, key)?;
        let id = match id_bytes {
            None => {
                let id = Objid(0);
                let id_bytes = id.0.to_be_bytes().to_vec();
                self.tx.put_cf(cf, key, id_bytes)?;
                id
            }
            Some(id_bytes) => {
                let id_bytes = id_bytes.as_slice();
                let id_bytes: [u8; 8] = id_bytes.try_into().unwrap();
                let id = Objid(i64::from_be_bytes(id_bytes) + 1);
                let id_bytes = id.0.to_be_bytes().to_vec();
                self.tx.put_cf(cf, key, id_bytes)?;
                id
            }
        };
        Ok(id)
    }

    /// Update the highest object ID if the given ID is higher than the current highest.
    fn update_highest_object_id(&self, oid: Objid) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectIds as u8) as usize];
        let key = "OBJECT_ID_COUNTER".as_bytes();
        let id_bytes = self.tx.get_cf(cf, key)?;
        match id_bytes {
            None => {
                let id_bytes = oid.0.to_be_bytes().to_vec();
                self.tx.put_cf(cf, key, id_bytes)?;
            }
            Some(id_bytes) => {
                let id_bytes = id_bytes.as_slice();
                let id_bytes: [u8; 8] = id_bytes.try_into().unwrap();
                let id = Objid(i64::from_be_bytes(id_bytes));
                if oid > id {
                    let id_bytes = oid.0.to_be_bytes().to_vec();
                    self.tx.put_cf(cf, key, id_bytes)?;
                }
            }
        };
        Ok(())
    }

    fn seek_property_handle(
        &self,
        obj: Objid,
        n: String,
    ) -> Result<Option<PropHandle>, anyhow::Error> {
        trace!(?obj, name = ?n, "resolving property in inheritance hierarchy");
        let op_cf = self.cf_handles[(ColumnFamilies::ObjectParent as u8) as usize];
        let ov_cf = self.cf_handles[(ColumnFamilies::ObjectProperties as u8) as usize];
        let mut search_o = obj;
        loop {
            let ok = oid_key(search_o);

            let props: Vec<PropHandle> = match self.tx.get_cf(ov_cf, ok.clone())? {
                None => vec![],
                Some(prop_bytes) => {
                    let (props, _) = bincode::decode_from_slice(&prop_bytes, *BINCODE_CONFIG)?;
                    props
                }
            };
            let prop = props.iter().find(|vh| vh.name == n);

            if let Some(prop) = prop {
                trace!(?prop, parent = ?search_o, "found property");
                return Ok(Some(prop.clone()));
            }

            // Otherwise, find our parent.  If it's, then set o to it and continue.
            let Ok(parent) = get_oid_value(op_cf, &self.tx, search_o) else {
                break;
            };
            if parent == NOTHING {
                break;
            }
            search_o = parent;
        }
        trace!(termination_object= ?obj, property=?n, "property not found");
        Ok(None)
    }
}

impl<'a> DbStorage for RocksDbTx<'a> {
    #[tracing::instrument(skip(self))]
    fn object_valid(&self, o: Objid) -> Result<bool, anyhow::Error> {
        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectFlags);
        let ok = oid_key(o);
        let ov = self.tx.get_cf(cf, ok)?;
        Ok(ov.is_some())
    }
    #[tracing::instrument(skip(self))]
    fn create_object(&self, oid: Option<Objid>, attrs: ObjAttrs) -> Result<Objid, anyhow::Error> {
        let oid = match oid {
            None => self.next_object_id()?,
            Some(oid) => {
                self.update_highest_object_id(oid)?;
                oid
            }
        };

        if let Some(owner) = attrs.owner {
            set_oid_value(
                cf_for(&self.cf_handles, ColumnFamilies::ObjectOwner),
                &self.tx,
                oid,
                owner,
            )?;
        }

        // Set initial name
        let name = attrs.name.unwrap_or_else(|| format!("Object #{}", oid.0));
        self.set_object_name(oid, name.clone())?;

        // Establish initial `contents` and `children` vectors, empty.
        let c_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectContents);
        let ch_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectChildren);
        set_oid_vec(c_cf, &self.tx, oid, vec![])?;
        set_oid_vec(ch_cf, &self.tx, oid, vec![])?;

        if let Some(parent) = attrs.parent {
            self.set_object_parent(oid, parent)?;
        }

        if let Some(location) = attrs.location {
            self.set_object_location(oid, location)?;
        }

        let default_object_flags = BitEnum::new();
        self.set_object_flags(oid, attrs.flags.unwrap_or(default_object_flags))?;

        Ok(oid)
    }
    #[tracing::instrument(skip(self))]
    fn set_object_parent(&self, o: Objid, new_parent: Objid) -> Result<(), anyhow::Error> {
        // Get o's parent, get its children, remove o from children, put children back
        // without it. Set new parent, get its children, add o to children, put children
        // back with it. Then update the parent of o.
        let p_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectParent);
        let c_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectChildren);

        // If this is a new object it won't have a parent, old parent this will come up not-found,
        // and if that's the case we can ignore that.
        match get_oid_value(p_cf, &self.tx, o) {
            Ok(old_parent) => {
                if old_parent == new_parent {
                    return Ok(());
                }
                if old_parent != NOTHING {
                    let old_children = get_oid_vec(c_cf, &self.tx, old_parent)?;
                    let old_children = old_children.into_iter().filter(|&x| x != o).collect();
                    set_oid_vec(c_cf, &self.tx, old_parent, old_children)?;
                }
            }
            Err(e) if !err_is_objnjf(&e) => {
                // Object not found is fine, we just don't have a parent yet.
                return Err(e);
            }
            Err(_) => {}
        }
        set_oid_value(p_cf, &self.tx, o, new_parent)?;

        if new_parent == NOTHING {
            return Ok(());
        }
        let mut new_children = get_oid_vec(c_cf, &self.tx, new_parent).unwrap_or_else(|_| vec![]);
        new_children.push(o);
        set_oid_vec(c_cf, &self.tx, new_parent, new_children)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn get_object_children(&self, o: Objid) -> Result<Vec<Objid>, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectChildren as u8) as usize];
        get_oid_vec(cf, &self.tx, o)
    }
    #[tracing::instrument(skip(self))]
    fn get_object_name(&self, o: Objid) -> Result<String, anyhow::Error> {
        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectName);
        let ok = oid_key(o);
        let name_bytes = self.tx.get_cf(cf, ok)?;
        let Some(name_bytes) = name_bytes else {
            return Err(ObjectError::ObjectNotFound(o).into());
        };
        let (attrs, _) = bincode::decode_from_slice(&name_bytes, *BINCODE_CONFIG)?;
        Ok(attrs)
    }
    #[tracing::instrument(skip(self))]
    fn set_object_name(&self, o: Objid, names: String) -> Result<(), anyhow::Error> {
        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectName);
        let ok = oid_key(o);
        let name_v = bincode::encode_to_vec(names, *BINCODE_CONFIG)?;
        self.tx.put_cf(cf, ok, name_v)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn get_object_flags(&self, o: Objid) -> Result<BitEnum<ObjFlag>, anyhow::Error> {
        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectFlags);
        let ok = oid_key(o);
        let flag_bytes = self.tx.get_cf(cf, ok)?;
        let Some(flag_bytes) = flag_bytes else {
            return Err(ObjectError::ObjectNotFound(o).into());
        };
        let (flags, _) = bincode::decode_from_slice(&flag_bytes, *BINCODE_CONFIG)?;
        Ok(flags)
    }
    #[tracing::instrument(skip(self))]
    fn set_object_flags(&self, o: Objid, flags: BitEnum<ObjFlag>) -> Result<(), anyhow::Error> {
        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectFlags);
        let ok = oid_key(o);
        let flag_v = bincode::encode_to_vec(flags, *BINCODE_CONFIG)?;
        self.tx.put_cf(cf, ok, flag_v)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn get_object_owner(&self, o: Objid) -> Result<Objid, anyhow::Error> {
        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectOwner);
        get_oid_value(cf, &self.tx, o)
    }
    #[tracing::instrument(skip(self))]
    fn set_object_owner(&self, o: Objid, owner: Objid) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectOwner as u8) as usize];
        set_oid_value(cf, &self.tx, o, owner)
    }
    #[tracing::instrument(skip(self))]
    fn get_object_parent(&self, o: Objid) -> Result<Objid, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectParent as u8) as usize];
        get_oid_value(cf, &self.tx, o)
    }
    #[tracing::instrument(skip(self))]
    fn get_object_location(&self, o: Objid) -> Result<Objid, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectLocation as u8) as usize];
        get_oid_value(cf, &self.tx, o)
    }
    #[tracing::instrument(skip(self))]
    fn get_object_contents(&self, o: Objid) -> Result<Vec<Objid>, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectContents as u8) as usize];
        get_oid_vec(cf, &self.tx, o)
    }
    #[tracing::instrument(skip(self))]
    fn set_object_location(&self, what: Objid, new_location: Objid) -> Result<(), anyhow::Error> {
        let mut oid = new_location;
        loop {
            if oid == NOTHING {
                break;
            }
            if oid == what {
                return Err(ObjectError::RecursiveMove(what, new_location).into());
            }
            oid = self.get_object_location(oid)?;
        }

        // Get o's location, get its contents, remove o from old contents, put contents back
        // without it. Set new location, get its contents, add o to contents, put contents
        // back with it. Then update the location of o.

        let l_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectLocation);
        let c_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectContents);

        // Get and remove from contents of old location, if we had any.
        match get_oid_value(l_cf, &self.tx, what) {
            Ok(old_location) => {
                if old_location == new_location {
                    return Ok(());
                }
                if old_location != NOTHING {
                    let c_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectContents);
                    let old_contents = get_oid_vec(c_cf, &self.tx, old_location)?;
                    let old_contents = old_contents.into_iter().filter(|&x| x != what).collect();
                    set_oid_vec(c_cf, &self.tx, old_location, old_contents)?;
                }
            }
            Err(e) if !err_is_objnjf(&e) => {
                // Object not found is fine, we just don't have a location yet.
                return Err(e);
            }
            Err(_) => {}
        }
        // Set new location.
        set_oid_value(l_cf, &self.tx, what, new_location)?;

        if new_location == NOTHING {
            return Ok(());
        }

        // Get and add to contents of new location.
        let mut new_contents = get_oid_vec(c_cf, &self.tx, new_location).unwrap_or_else(|_| vec![]);
        new_contents.push(what);
        set_oid_vec(c_cf, &self.tx, new_location, new_contents)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn get_object_verbs(&self, o: Objid) -> Result<Vec<VerbHandle>, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok)?;
        let verbs = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, *BINCODE_CONFIG)?;
                verbs
            }
        };
        Ok(verbs)
    }
    #[tracing::instrument(skip(self))]
    fn add_object_verb(
        &self,
        oid: Objid,
        owner: Objid,
        names: Vec<String>,
        program: Binary,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), anyhow::Error> {
        // Get the old vector, add the new verb, put the new vector.
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(oid);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let mut verbs: Vec<VerbHandle> = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, *BINCODE_CONFIG)?;
                verbs
            }
        };

        // If there's overlap in the names, we need to fail.
        for verb in &verbs {
            for name in &names {
                if verb.names.contains(name) {
                    return Err(ObjectError::DuplicateVerb(oid, name.clone()).into());
                }
            }
        }

        // Generate a new verb ID.
        let vid = Uuid::new_v4();
        let verb = VerbHandle {
            uuid: vid.as_u128(),
            location: oid,
            owner,
            names,
            flags,
            args,
        };
        verbs.push(verb);
        let verbs_v = bincode::encode_to_vec(&verbs, *BINCODE_CONFIG)?;
        self.tx.put_cf(cf, ok, verbs_v)?;

        // Now set the program.
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key(oid, vid.as_u128());
        let prg_bytes = bincode::encode_to_vec(program, *BINCODE_CONFIG)?;
        self.tx.put_cf(cf, vk, prg_bytes)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn delete_object_verb(&self, o: Objid, v: u128) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let mut verbs: Vec<VerbHandle> = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, *BINCODE_CONFIG)?;
                verbs
            }
        };
        let mut found = false;
        verbs.retain(|vh| {
            if vh.uuid == v {
                found = true;
                false
            } else {
                true
            }
        });
        if !found {
            let v_uuid_str = Uuid::from_u128(v).to_string();
            return Err(ObjectError::VerbNotFound(o, v_uuid_str).into());
        }
        let verbs_v = bincode::encode_to_vec(&verbs, *BINCODE_CONFIG)?;
        self.tx.put_cf(cf, ok, verbs_v)?;

        // Delete the program.
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key(o, v);
        self.tx.delete_cf(cf, vk)?;

        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn get_verb(&self, o: Objid, v: u128) -> Result<VerbHandle, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let verbs: Vec<VerbHandle> = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, *BINCODE_CONFIG)?;
                verbs
            }
        };
        let verb = verbs.iter().find(|vh| vh.uuid == v);
        let Some(verb) = verb else {
            let v_uuid_str = Uuid::from_u128(v).to_string();
            return Err(ObjectError::VerbNotFound(o, v_uuid_str).into());
        };
        Ok(verb.clone())
    }
    #[tracing::instrument(skip(self))]
    fn get_verb_by_name(&self, o: Objid, n: String) -> Result<VerbHandle, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let verbs: Vec<VerbHandle> = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, *BINCODE_CONFIG)?;
                verbs
            }
        };
        let verb = verbs
            .iter()
            .find(|vh| match_in_verb_names(&vh.names, &n).is_some());
        let Some(verb) = verb else {
            return Err(ObjectError::VerbNotFound(o, n).into());
        };
        Ok(verb.clone())
    }
    #[tracing::instrument(skip(self))]
    fn get_verb_by_index(&self, o: Objid, i: usize) -> Result<VerbHandle, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let verbs: Vec<VerbHandle> = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, *BINCODE_CONFIG)?;
                verbs
            }
        };
        if i >= verbs.len() {
            return Err(ObjectError::VerbNotFound(o, format!("{}", i)).into());
        }
        let verb = verbs.get(i);
        let Some(verb) = verb else {
            return Err(ObjectError::VerbNotFound(o, format!("{}", i)).into());
        };
        Ok(verb.clone())
    }
    #[tracing::instrument(skip(self))]
    fn get_program(&self, o: Objid, v: u128) -> Result<Binary, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let ok = composite_key(o, v);
        let prg_bytes = self.tx.get_cf(cf, ok)?;
        let Some(prg_bytes) = prg_bytes else {
            let v_uuid_str = Uuid::from_u128(v).to_string();
            return Err(ObjectError::VerbNotFound(o, v_uuid_str).into());
        };
        let (prg, _) = bincode::decode_from_slice(&prg_bytes, *BINCODE_CONFIG)?;
        Ok(prg)
    }
    #[tracing::instrument(skip(self))]
    fn resolve_verb(
        &self,
        o: Objid,
        n: String,
        a: Option<VerbArgsSpec>,
    ) -> Result<VerbHandle, anyhow::Error> {
        trace!(object = ?o, verb = %n, args = ?a, "Resolving verb");
        let op_cf = self.cf_handles[(ColumnFamilies::ObjectParent as u8) as usize];
        let ov_cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let mut search_o = o;
        loop {
            let ok = oid_key(search_o);

            let verbs: Vec<VerbHandle> = match self.tx.get_cf(ov_cf, ok.clone())? {
                None => vec![],
                Some(verb_bytes) => {
                    let (verbs, _) = bincode::decode_from_slice(&verb_bytes, *BINCODE_CONFIG)?;
                    verbs
                }
            };
            let verb = verbs.iter().find(|vh| {
                if match_in_verb_names(&vh.names, &n).is_some() {
                    return if let Some(a) = a { a.matches(&a) } else { true };
                }
                false
            });
            // If we found the verb, return it.
            if let Some(verb) = verb {
                trace!(?verb, ?search_o, "resolved verb");
                return Ok(verb.clone());
            }

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
        Err(ObjectError::VerbNotFound(o, n).into())
    }
    #[tracing::instrument(skip(self))]
    fn retrieve_verb(&self, o: Objid, v: String) -> Result<(Binary, VerbHandle), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let verbs: Vec<VerbHandle> = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, *BINCODE_CONFIG)?;
                verbs
            }
        };
        let verb = verbs
            .iter()
            .find(|vh| match_in_verb_names(&vh.names, &v).is_some());
        let Some(verb) = verb else {
            return Err(ObjectError::VerbNotFound(o, v.clone()).into())
        };

        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key(o, verb.uuid);
        let prg_bytes = self.tx.get_cf(cf, vk)?;
        let Some(prg_bytes) = prg_bytes else {
            return Err(ObjectError::VerbNotFound(o, v.clone()).into())
        };
        let (program, _) = bincode::decode_from_slice(&prg_bytes, *BINCODE_CONFIG)?;
        Ok((program, verb.clone()))
    }
    #[tracing::instrument(skip(self))]
    fn set_verb_info(
        &self,
        o: Objid,
        v: u128,
        new_owner: Option<Objid>,
        new_perms: Option<BitEnum<VerbFlag>>,
        new_names: Option<Vec<String>>,
        new_args: Option<VerbArgsSpec>,
    ) -> Result<(), Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = oid_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let mut verbs: Vec<VerbHandle> = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, *BINCODE_CONFIG)?;
                verbs
            }
        };
        let mut found = false;
        for verb in verbs.iter_mut() {
            if verb.uuid == v {
                found = true;
                if let Some(new_owner) = new_owner {
                    verb.owner = new_owner;
                }
                if let Some(new_perms) = new_perms {
                    verb.flags = new_perms;
                }
                if let Some(new_names) = new_names {
                    verb.names = new_names;
                }
                if let Some(new_args) = new_args {
                    verb.args = new_args;
                }
                break;
            }
        }
        if !found {
            let v_uuid_str = Uuid::from_u128(v).to_string();
            return Err(ObjectError::VerbNotFound(o, v_uuid_str).into());
        }

        let verbs_v = bincode::encode_to_vec(&verbs, *BINCODE_CONFIG)?;

        self.tx.put_cf(cf, ok, verbs_v)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn get_properties(&self, o: Objid) -> Result<Vec<PropHandle>, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectProperties as u8) as usize];
        let ok = oid_key(o);
        let props_bytes = self.tx.get_cf(cf, ok)?;
        let props = match props_bytes {
            None => vec![],
            Some(prop_bytes) => {
                let (props, _) = bincode::decode_from_slice(&prop_bytes, *BINCODE_CONFIG)?;
                props
            }
        };
        Ok(props)
    }
    #[tracing::instrument(skip(self))]
    fn retrieve_property(&self, o: Objid, u: u128) -> Result<Var, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let ok = composite_key(o, u);
        let var_bytes = self.tx.get_cf(cf, ok)?;
        let Some(var_bytes) = var_bytes else {
            let u_uuid_str = Uuid::from_u128(u).to_string();
            return Err(ObjectError::PropertyNotFound(o, u_uuid_str).into());
        };
        let (var, _) = bincode::decode_from_slice(&var_bytes, *BINCODE_CONFIG)?;
        Ok(var)
    }
    #[tracing::instrument(skip(self))]
    fn set_property_value(&self, o: Objid, u: u128, v: Var) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let ok = composite_key(o, u);
        let var_bytes = bincode::encode_to_vec(v, *BINCODE_CONFIG)?;
        self.tx.put_cf(cf, ok, var_bytes)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn set_property_info(
        &self,
        o: Objid,
        u: u128,
        new_owner: Option<Objid>,
        new_perms: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
        is_clear: Option<bool>,
    ) -> Result<(), anyhow::Error> {
        let p_cf = self.cf_handles[(ColumnFamilies::ObjectProperties as u8) as usize];
        let ok = oid_key(o);
        let props_bytes = self.tx.get_cf(p_cf, ok.clone())?;
        let Some(props_bytes) = props_bytes else {
            let u_uuid_str = Uuid::from_u128(u).to_string();
            return Err(ObjectError::PropertyNotFound(o, u_uuid_str).into());
        };
        let (mut props, _): (Vec<PropHandle>, _) =
            bincode::decode_from_slice(&props_bytes, *BINCODE_CONFIG)?;
        let mut found = false;
        for prop in props.iter_mut() {
            if prop.uuid == u {
                found = true;
                if let Some(new_owner) = new_owner {
                    prop.owner = new_owner;
                }
                if let Some(new_perms) = new_perms {
                    prop.perms = new_perms;
                }
                if let Some(new_name) = &new_name {
                    prop.name = new_name.clone();
                }
                if let Some(is_clear) = is_clear {
                    prop.is_clear = is_clear;
                }
            }
        }
        if !found {
            let u_uuid_str = Uuid::from_u128(u).to_string();
            return Err(ObjectError::PropertyNotFound(o, u_uuid_str).into());
        }
        let props_bytes = bincode::encode_to_vec(&props, *BINCODE_CONFIG)?;
        self.tx.put_cf(p_cf, ok, props_bytes)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn delete_property(&self, o: Objid, u: u128) -> Result<(), anyhow::Error> {
        let p_cf = self.cf_handles[(ColumnFamilies::ObjectProperties as u8) as usize];
        let ok = oid_key(o);
        let props_bytes = self.tx.get_cf(p_cf, ok.clone())?;
        let Some(props_bytes) = props_bytes else {
            return Err(ObjectError::ObjectNotFound(o).into());
        };
        let (mut props, _): (Vec<PropHandle>, _) =
            bincode::decode_from_slice(&props_bytes, *BINCODE_CONFIG)?;
        let mut found = false;
        props.retain(|prop| {
            if prop.uuid == u {
                found = true;
                false
            } else {
                true
            }
        });
        if !found {
            let u_uuid_str = Uuid::from_u128(u).to_string();
            return Err(ObjectError::PropertyNotFound(o, u_uuid_str).into());
        }
        let props_bytes = bincode::encode_to_vec(&props, *BINCODE_CONFIG)?;
        self.tx.put_cf(p_cf, ok, props_bytes)?;

        // Need to also delete the property value.
        let pv_cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let uk = composite_key(o, u);
        self.tx.delete_cf(pv_cf, uk)?;

        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn add_property(
        &self,
        location: Objid,
        name: String,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
        is_clear: bool,
    ) -> Result<PropHandle, anyhow::Error> {
        let p_cf = self.cf_handles[(ColumnFamilies::ObjectProperties as u8) as usize];
        let ok = oid_key(location);
        let props_bytes = self.tx.get_cf(p_cf, ok.clone())?;
        let mut props: Vec<PropHandle> = match props_bytes {
            None => vec![],
            Some(prop_bytes) => {
                let (props, _) = bincode::decode_from_slice(&prop_bytes, *BINCODE_CONFIG)?;
                props
            }
        };

        // Verify we don't already have a property with this name. If we do, return an error.
        if props.iter().any(|prop| prop.name == name) {
            return Err(ObjectError::DuplicatePropertyDefinition(location, name).into());
        }

        // Generate a new property ID.
        let u = Uuid::new_v4();
        let prop = PropHandle {
            uuid: u.as_u128(),
            location,
            name,
            owner,
            perms,
            is_clear,
        };
        props.push(prop.clone());
        let props_bytes = bincode::encode_to_vec(&props, *BINCODE_CONFIG)?;
        self.tx.put_cf(p_cf, ok, props_bytes)?;

        // If we have an initial value, set it.
        if let Some(value) = value {
            let value_cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
            let propkey = composite_key(location, u.as_u128());
            let prop_bytes = bincode::encode_to_vec(value, *BINCODE_CONFIG)?;
            self.tx.put_cf(value_cf, propkey, prop_bytes)?;
        }

        Ok(prop)
    }
    #[tracing::instrument(skip(self))]
    fn resolve_property(&self, obj: Objid, n: String) -> Result<(PropHandle, Var), anyhow::Error> {
        trace!(?obj, name = ?n, "resolving property");
        let op_cf = self.cf_handles[(ColumnFamilies::ObjectParent as u8) as usize];

        let mut search_obj = obj;
        let mut og_handle = None;
        let mut found_value = None;
        loop {
            let property_handle = self.seek_property_handle(search_obj, n.clone())?;
            let Some(property_handle) = property_handle else {
                return Err(ObjectError::PropertyNotFound(obj, n).into());
            };
            if og_handle.is_none() {
                og_handle = Some(property_handle.clone());
            }

            // Typical case: property is not marked 'clear', so we can return straight away.
            if !property_handle.is_clear {
                // Decode the value out of ObjectPropertyValue, but from property_handle, not og_handle
                found_value =
                    Some(self.retrieve_property(property_handle.location, property_handle.uuid)?);
                break;
            }

            // But if it was clear, we have to continue up the inheritance hierarchy. (But we return
            // the og handle we got, because this is what we want to return for information
            // about permissions, etc.)
            let Ok(parent) = get_oid_value(op_cf, &self.tx, search_obj) else {
                break;
            };
            if parent == NOTHING {
                // This is an odd one, clear all the way up. so our value will end up being
                // NONE, I guess.
                break;
            }
            search_obj = parent;
        }
        let Some(prop_handle) = og_handle else {
            return Err(ObjectError::PropertyNotFound(obj, n).into());
        };
        let found_value = found_value.unwrap_or_else(v_none);
        Ok((prop_handle, found_value))
    }

    #[tracing::instrument(skip(self))]
    fn commit(self) -> Result<CommitResult, anyhow::Error> {
        match self.tx.commit() {
            Ok(()) => Ok(CommitResult::Success),
            Err(e) if e.kind() == ErrorKind::Busy || e.kind() == ErrorKind::TryAgain => {
                Ok(CommitResult::ConflictRetry)
            }
            Err(e) => bail!(e),
        }
    }
    #[tracing::instrument(skip(self))]
    fn rollback(&self) -> Result<(), anyhow::Error> {
        self.tx.rollback()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rocksdb::OptimisticTransactionDB;
    use strum::VariantNames;
    use tempdir::TempDir;

    use moor_value::util::bitenum::BitEnum;
    use moor_value::var::objid::{Objid, NOTHING};
    use moor_value::var::v_str;

    use crate::compiler::codegen::compile;
    use crate::db::rocksdb::tx_db_impl::RocksDbTx;
    use crate::db::rocksdb::{ColumnFamilies, DbStorage};
    use crate::model::objects::ObjAttrs;
    use crate::model::r#match::VerbArgsSpec;
    use crate::model::ObjectError;

    struct TestDb {
        db: Arc<OptimisticTransactionDB>,
    }

    impl TestDb {
        fn tx(&self) -> RocksDbTx {
            let cf_handles = ColumnFamilies::VARIANTS
                .iter()
                .enumerate()
                .map(|cf| self.db.cf_handle(cf.1).unwrap())
                .collect();
            let rtx = self.db.transaction();

            RocksDbTx {
                tx: rtx,
                cf_handles,
            }
        }
    }

    fn mk_test_db() -> TestDb {
        let tmp_dir = TempDir::new("test_db").unwrap();
        let db_path = tmp_dir.path().join("test_db");
        let mut options = rocksdb::Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let column_families = ColumnFamilies::VARIANTS;
        let db: Arc<OptimisticTransactionDB> =
            Arc::new(OptimisticTransactionDB::open_cf(&options, db_path, column_families).unwrap());

        TestDb { db: db.clone() }
    }

    #[test]
    fn test_create_object() {
        let db = mk_test_db();
        let tx = db.tx();
        let oid = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();
        assert_eq!(oid, Objid(0));
        assert!(tx.object_valid(oid).unwrap());
        assert_eq!(tx.get_object_owner(oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_parent(oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_location(oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_name(oid).unwrap(), "test");
    }

    #[test]
    fn test_parent_children() {
        let db = mk_test_db();
        let tx = db.tx();

        // Single parent/child relationship.
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        assert_eq!(tx.get_object_parent(b).unwrap(), a);
        assert_eq!(tx.get_object_children(a).unwrap(), vec![b]);

        assert_eq!(tx.get_object_parent(a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_children(b).unwrap(), vec![]);

        // Add a second child
        let c = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        assert_eq!(tx.get_object_parent(c).unwrap(), a);
        assert_eq!(tx.get_object_children(a).unwrap(), vec![b, c]);

        assert_eq!(tx.get_object_parent(a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_children(b).unwrap(), vec![]);

        // Reparent one child
        let d = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test3".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.set_object_parent(b, d).unwrap();
        assert_eq!(tx.get_object_parent(b).unwrap(), d);
        assert_eq!(tx.get_object_children(a).unwrap(), vec![c]);
        assert_eq!(tx.get_object_children(d).unwrap(), vec![b]);
    }

    #[test]
    fn test_location_contents() {
        let db = mk_test_db();
        let tx = db.tx();
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(NOTHING),
                    location: Some(a),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        assert_eq!(tx.get_object_location(b).unwrap(), a);
        assert_eq!(tx.get_object_contents(a).unwrap(), vec![b]);

        assert_eq!(tx.get_object_location(a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_contents(b).unwrap(), vec![]);

        let c = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test3".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.set_object_location(b, c).unwrap();
        assert_eq!(tx.get_object_location(b).unwrap(), c);
        assert_eq!(tx.get_object_contents(a).unwrap(), vec![]);
        assert_eq!(tx.get_object_contents(c).unwrap(), vec![b]);

        let d = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test4".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();
        tx.set_object_location(d, c).unwrap();
        assert_eq!(tx.get_object_contents(c).unwrap(), vec![b, d]);
        assert_eq!(tx.get_object_location(d).unwrap(), c);

        tx.set_object_location(a, c).unwrap();
        assert_eq!(tx.get_object_contents(c).unwrap(), vec![b, d, a]);
        assert_eq!(tx.get_object_location(a).unwrap(), c);

        // Validate recursive move detection.
        match tx.set_object_location(c, b).err().unwrap().downcast_ref::<ObjectError>() {
            Some(ObjectError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // Move b one level deeper, and then check recursive move detection again.
        tx.set_object_location(b, d).unwrap();
        match tx.set_object_location(c, b).err().unwrap().downcast_ref::<ObjectError>() {
            Some(ObjectError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // The other way around, d to c should be fine.
        tx.set_object_location(d, c).unwrap();
    }

    #[test]
    fn test_simple_property() {
        let db = mk_test_db();
        let tx = db.tx();
        let oid = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.add_property(
            oid,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test")),
            false,
        )
        .unwrap();
        let (prop, v) = tx.resolve_property(oid, "test".into()).unwrap();
        assert_eq!(prop.name, "test");
        assert_eq!(v, v_str("test"));
    }

    #[test]
    fn test_transitive_property_resolution() {
        let db = mk_test_db();
        let tx = db.tx();
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.add_property(
            a,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
            false,
        )
        .unwrap();
        let (prop, v) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name, "test");
        assert_eq!(v, v_str("test_value"));

        // Verify we *don't* get this property for an unrelated, unhinged object.
        let c = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test3".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.set_object_parent(b, c).unwrap();
        let result = tx.resolve_property(b, "test".into());
        assert_eq!(
            result.err().unwrap().downcast_ref::<ObjectError>().unwrap(),
            &ObjectError::PropertyNotFound(b, "test".into())
        );
    }

    #[test]
    fn test_transitive_property_resolution_clear_property() {
        let db = mk_test_db();
        let tx = db.tx();
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        tx.add_property(
            a,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
            false,
        )
        .unwrap();
        let (prop, v) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name, "test");
        assert_eq!(v, v_str("test_value"));

        // Define the property again, but on the object 'b', but set it to clear.
        // We should then get b's *handle* but a's value when we query on b.
        tx.add_property(b, "test".into(), NOTHING, BitEnum::new(), None, true)
            .unwrap();
        let (prop, v) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name, "test");
        assert_eq!(v, v_str("test_value"));
    }

    #[test]
    fn test_verb_resolve() {
        let db = mk_test_db();
        let tx = db.tx();
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let program = compile("return 5;").unwrap();
        tx.add_object_verb(
            a,
            a,
            vec!["test".into()],
            program.clone(),
            BitEnum::new(),
            VerbArgsSpec::this_none_this(),
        )
        .unwrap();

        assert_eq!(
            tx.resolve_verb(a, "test".into(), None).unwrap().names,
            vec!["test"]
        );

        assert_eq!(
            tx.resolve_verb(a, "test".into(), Some(VerbArgsSpec::this_none_this()))
                .unwrap()
                .names,
            vec!["test"]
        );

        assert_eq!(
            tx.get_program(a, tx.resolve_verb(a, "test".into(), None).unwrap().uuid)
                .unwrap(),
            program
        );
    }

    #[test]
    fn test_verb_resolve_wildcard() {
        let db = mk_test_db();
        let tx = db.tx();
        let a = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let program = compile("return 5;").unwrap();
        let verb_names = vec!["dname*c".into(), "iname*c".into()];
        tx.add_object_verb(
            a,
            a,
            verb_names.clone(),
            program.clone(),
            BitEnum::new(),
            VerbArgsSpec::this_none_this(),
        )
        .unwrap();

        assert_eq!(
            tx.resolve_verb(a, "dname".into(), None).unwrap().names,
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(a, "dnamec".into(), None).unwrap().names,
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(a, "iname".into(), None).unwrap().names,
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(a, "inamec".into(), None).unwrap().names,
            verb_names
        );
    }
}
