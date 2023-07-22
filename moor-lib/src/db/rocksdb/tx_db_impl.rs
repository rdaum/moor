use anyhow::bail;
use bincode::config::Configuration;
use rocksdb::{ColumnFamily, ErrorKind};

use uuid::Uuid;

use crate::db::rocksdb::tx_server::{PropHandle, VerbHandle};
use crate::db::rocksdb::{ColumnFamilies, DbStorage};
use crate::db::CommitResult;
use crate::model::objects::{ObjAttrs, ObjFlag};
use crate::model::props::PropFlag;
use crate::model::r#match::VerbArgsSpec;
use crate::model::verbs::VerbFlag;
use crate::model::ObjectError;
use crate::util::bitenum::BitEnum;
use crate::util::verbname_cmp;
use crate::var::{Objid, Var, NOTHING};
use crate::vm::opcode::Binary;

fn object_key(o: Objid) -> Vec<u8> {
    o.0.to_be_bytes().to_vec()
}

fn composite_key(o: Objid, uuid: u128) -> Vec<u8> {
    let mut key = object_key(o);
    key.extend_from_slice(&uuid.to_be_bytes());
    key
}

fn object_vec(o: Vec<Objid>) -> Result<Vec<u8>, anyhow::Error> {
    let bincode_cfg = bincode::config::standard();

    let ov = bincode::encode_to_vec(o, bincode_cfg)?;
    Ok(ov)
}

fn get_object_value<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
) -> Result<Objid, anyhow::Error> {
    let ok = object_key(o);
    let ov = tx.get_cf(cf, ok).unwrap();
    let ov = ov.ok_or(ObjectError::ObjectNotFound(o))?;
    let ov = u64::from_be_bytes(ov.try_into().unwrap());
    let ov = Objid(ov as i64);
    Ok(ov)
}

fn set_object_value<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
    v: Objid,
) -> Result<(), anyhow::Error> {
    let ok = object_key(o);
    let ov = object_key(v);
    tx.put_cf(cf, ok, ov).unwrap();
    Ok(())
}

fn get_object_vec<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
) -> Result<Vec<Objid>, anyhow::Error> {
    let bincode_cfg = bincode::config::standard();

    let ok = object_key(o);
    let ov = tx.get_cf(cf, ok).unwrap();
    let ov = ov.ok_or(ObjectError::ObjectNotFound(o))?;
    let (ov, _) = bincode::decode_from_slice(&ov, bincode_cfg).unwrap();
    Ok(ov)
}

fn set_object_vec<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
    v: Vec<Objid>,
) -> Result<(), ObjectError> {
    let ok = object_key(o);
    let ov = object_vec(v).unwrap();
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
    pub(crate) bincode_cfg: Configuration,
}

fn verbname_matches(verb_names: &[String], candidate: &str) -> Option<String> {
    verb_names
        .iter()
        .find(|&v| verbname_cmp(v, candidate))
        .cloned()
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
}

impl<'a> DbStorage for RocksDbTx<'a> {
    fn object_valid(&self, o: Objid) -> Result<bool, anyhow::Error> {
        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectFlags);
        let ok = object_key(o);
        let ov = self.tx.get_cf(cf, ok)?;
        Ok(ov.is_some())
    }

    fn create_object(&self, oid: Option<Objid>, attrs: ObjAttrs) -> Result<Objid, anyhow::Error> {
        let oid = match oid {
            None => self.next_object_id()?,
            Some(oid) => {
                self.update_highest_object_id(oid)?;
                oid
            }
        };

        if let Some(owner) = attrs.owner {
            set_object_value(
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
        set_object_vec(c_cf, &self.tx, oid, vec![])?;
        set_object_vec(ch_cf, &self.tx, oid, vec![])?;

        self.set_object_parent(oid, attrs.parent.unwrap_or(NOTHING))?;
        self.set_object_location(oid, attrs.location.unwrap_or(NOTHING))?;

        let default_object_flags = BitEnum::new();
        self.set_object_flags(oid, attrs.flags.unwrap_or(default_object_flags))?;

        Ok(oid)
    }

    fn set_object_parent(&self, o: Objid, new_parent: Objid) -> Result<(), anyhow::Error> {
        // Get o's parent, get its children, remove o from children, put children back
        // without it. Set new parent, get its children, add o to children, put children
        // back with it. Then update the parent of o.
        let p_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectParent);
        let c_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectChildren);

        // If this is a new object it won't have a parent, old parent this will come up not-found,
        // and if that's the case we can ignore that.
        match get_object_value(p_cf, &self.tx, o) {
            Ok(old_parent) => {
                if old_parent == new_parent {
                    return Ok(());
                }
                if old_parent != NOTHING {
                    let old_children = get_object_vec(c_cf, &self.tx, old_parent)?;
                    let old_children = old_children.into_iter().filter(|&x| x != o).collect();
                    set_object_vec(c_cf, &self.tx, old_parent, old_children)?;
                }
            }
            Err(e) if !err_is_objnjf(&e) => {
                // Object not found is fine, we just don't have a parent yet.
                return Err(e);
            }
            Err(_) => {}
        }
        set_object_value(p_cf, &self.tx, o, new_parent)?;

        if new_parent == NOTHING {
            return Ok(());
        }
        let mut new_children =
            get_object_vec(c_cf, &self.tx, new_parent).unwrap_or_else(|_| vec![]);
        new_children.push(o);
        set_object_vec(c_cf, &self.tx, new_parent, new_children)?;
        Ok(())
    }
    fn get_object_children(&self, o: Objid) -> Result<Vec<Objid>, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectChildren as u8) as usize];
        get_object_vec(cf, &self.tx, o)
    }
    fn get_object_name(&self, o: Objid) -> Result<String, anyhow::Error> {
        let bincode_cfg = bincode::config::standard();

        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectName);
        let ok = object_key(o);
        let name_bytes = self.tx.get_cf(cf, ok)?;
        let Some(name_bytes) = name_bytes else {
            return Err(ObjectError::ObjectNotFound(o).into());
        };
        let (attrs, _) = bincode::decode_from_slice(&name_bytes, bincode_cfg)?;
        Ok(attrs)
    }
    fn set_object_name(&self, o: Objid, names: String) -> Result<(), anyhow::Error> {
        let bincode_cfg = bincode::config::standard();

        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectName);
        let ok = object_key(o);
        let name_v = bincode::encode_to_vec(names, bincode_cfg)?;
        self.tx.put_cf(cf, ok, name_v)?;
        Ok(())
    }
    fn get_object_flags(&self, o: Objid) -> Result<BitEnum<ObjFlag>, anyhow::Error> {
        let bincode_cfg = bincode::config::standard();

        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectFlags);
        let ok = object_key(o);
        let flag_bytes = self.tx.get_cf(cf, ok)?;
        let Some(flag_bytes) = flag_bytes else {
            return Err(ObjectError::ObjectNotFound(o).into());
        };
        let (flags, _) = bincode::decode_from_slice(&flag_bytes, bincode_cfg)?;
        Ok(flags)
    }
    fn set_object_flags(&self, o: Objid, flags: BitEnum<ObjFlag>) -> Result<(), anyhow::Error> {
        let bincode_cfg = bincode::config::standard();

        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectFlags);
        let ok = object_key(o);
        let flag_v = bincode::encode_to_vec(flags, bincode_cfg)?;
        self.tx.put_cf(cf, ok, flag_v)?;
        Ok(())
    }
    fn get_object_owner(&self, o: Objid) -> Result<Objid, anyhow::Error> {
        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectOwner);
        get_object_value(cf, &self.tx, o)
    }
    fn set_object_owner(&self, o: Objid, owner: Objid) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectOwner as u8) as usize];
        set_object_value(cf, &self.tx, o, owner)
    }
    fn get_object_parent(&self, o: Objid) -> Result<Objid, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectParent as u8) as usize];
        get_object_value(cf, &self.tx, o)
    }
    fn get_object_location(&self, o: Objid) -> Result<Objid, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectLocation as u8) as usize];
        get_object_value(cf, &self.tx, o)
    }
    fn get_object_contents(&self, o: Objid) -> Result<Vec<Objid>, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectContents as u8) as usize];
        get_object_vec(cf, &self.tx, o)
    }
    fn set_object_location(&self, o: Objid, new_location: Objid) -> Result<(), anyhow::Error> {
        // Get o's location, get its contents, remove o from old contents, put contents back
        // without it. Set new location, get its contents, add o to contents, put contents
        // back with it. Then update the location of o.

        let l_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectLocation);
        let c_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectContents);

        // Get and remove from contents of old location, if we had any.
        match get_object_value(l_cf, &self.tx, o) {
            Ok(old_location) => {
                if old_location == new_location {
                    return Ok(());
                }
                if old_location != NOTHING {
                    let c_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectContents);
                    let old_contents = get_object_vec(c_cf, &self.tx, old_location)?;
                    let old_contents = old_contents.into_iter().filter(|&x| x != o).collect();
                    set_object_vec(c_cf, &self.tx, old_location, old_contents)?;
                }
            }
            Err(e) if !err_is_objnjf(&e) => {
                // Object not found is fine, we just don't have a location yet.
                return Err(e);
            }
            Err(_) => {}
        }
        // Set new location.
        set_object_value(l_cf, &self.tx, o, new_location)?;

        if new_location == NOTHING {
            return Ok(());
        }

        // Get and add to contents of new location.
        let mut new_contents =
            get_object_vec(c_cf, &self.tx, new_location).unwrap_or_else(|_| vec![]);
        new_contents.push(o);
        set_object_vec(c_cf, &self.tx, new_location, new_contents)?;
        Ok(())
    }
    fn get_object_verbs(&self, o: Objid) -> Result<Vec<VerbHandle>, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = object_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok)?;
        let verbs = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, self.bincode_cfg)?;
                verbs
            }
        };
        Ok(verbs)
    }
    fn add_object_verb(
        &self,
        oid: Objid,
        owner: Objid,
        names: Vec<String>,
        program: Binary,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), anyhow::Error> {
        // TODO: check for duplicate names.

        // Get the old vector, add the new verb, put the new vector.
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = object_key(oid);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let mut verbs = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, self.bincode_cfg)?;
                verbs
            }
        };
        // Generate a new verb ID.
        let vid = Uuid::new_v4();
        let verb = VerbHandle {
            uuid: vid.as_u128(),
            definer: oid,
            owner,
            names,
            flags,
            args,
        };
        verbs.push(verb);
        let verbs_v = bincode::encode_to_vec(&verbs, self.bincode_cfg)?;
        self.tx.put_cf(cf, ok, verbs_v)?;

        // Now set the program.
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key(oid, vid.as_u128());
        let prg_bytes = bincode::encode_to_vec(program, self.bincode_cfg)?;
        self.tx.put_cf(cf, vk, prg_bytes)?;
        Ok(())
    }
    fn delete_object_verb(&self, o: Objid, v: u128) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = object_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let mut verbs: Vec<VerbHandle> = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, self.bincode_cfg)?;
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
        let verbs_v = bincode::encode_to_vec(&verbs, self.bincode_cfg)?;
        self.tx.put_cf(cf, ok, verbs_v)?;

        // Delete the program.
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key(o, v);
        self.tx.delete_cf(cf, vk)?;

        Ok(())
    }
    fn get_verb(&self, o: Objid, v: u128) -> Result<VerbHandle, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = object_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let verbs: Vec<VerbHandle> = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, self.bincode_cfg)?;
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
    fn get_verb_by_name(&self, o: Objid, n: String) -> Result<VerbHandle, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = object_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let verbs: Vec<VerbHandle> = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, self.bincode_cfg)?;
                verbs
            }
        };
        // TODO: wildcard search
        let verb = verbs
            .iter()
            .find(|vh| verbname_matches(&vh.names, &n).is_some());
        let Some(verb) = verb else {
            return Err(ObjectError::VerbNotFound(o, n).into());
        };
        Ok(verb.clone())
    }
    fn get_verb_by_index(&self, o: Objid, i: usize) -> Result<VerbHandle, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = object_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let verbs: Vec<VerbHandle> = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, self.bincode_cfg)?;
                verbs
            }
        };
        let verb = verbs.get(i);
        let Some(verb) = verb else {
            return Err(ObjectError::VerbNotFound(o, format!("{}", i)).into());
        };
        Ok(verb.clone())
    }
    fn get_program(&self, o: Objid, v: u128) -> Result<Binary, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let ok = composite_key(o, v);
        let prg_bytes = self.tx.get_cf(cf, ok)?;
        let Some(prg_bytes) = prg_bytes else {
            let v_uuid_str = Uuid::from_u128(v).to_string();
            return Err(ObjectError::VerbNotFound(o, v_uuid_str).into());
        };
        let (prg, _) = bincode::decode_from_slice(&prg_bytes, self.bincode_cfg)?;
        Ok(prg)
    }
    #[tracing::instrument(skip(self))]
    fn resolve_verb(
        &self,
        o: Objid,
        n: String,
        a: Option<VerbArgsSpec>,
    ) -> Result<VerbHandle, anyhow::Error> {
        let op_cf = self.cf_handles[(ColumnFamilies::ObjectParent as u8) as usize];
        let ov_cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let mut search_o = o;
        loop {
            let ok = object_key(search_o);

            let verbs: Vec<VerbHandle> = match self.tx.get_cf(ov_cf, ok.clone())? {
                None => vec![],
                Some(verb_bytes) => {
                    let (verbs, _) = bincode::decode_from_slice(&verb_bytes, self.bincode_cfg)?;
                    verbs
                }
            };
            let verb = verbs.iter().find(|vh| {
                if verbname_matches(&vh.names, &n).is_some() {
                    return if let Some(a) = a {
                        a.matches(&a)
                    } else {
                        vh.args == VerbArgsSpec::this_none_this()
                    };
                }
                false
            });
            // If we found the verb, return it.
            if let Some(verb) = verb {
                return Ok(verb.clone());
            }

            // Otherwise, find our parent.  If it's, then set o to it and continue unless we've
            // hit the end of the chain.
            let Ok(parent) = get_object_value(op_cf, &self.tx, search_o) else {
                break;
            };
            if parent == NOTHING {
                break;
            }
            search_o = parent;
        }
        Err(ObjectError::VerbNotFound(o, n).into())
    }
    fn retrieve_verb(&self, o: Objid, v: String) -> Result<(Binary, VerbHandle), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectVerbs as u8) as usize];
        let ok = object_key(o);
        let verbs_bytes = self.tx.get_cf(cf, ok.clone())?;
        let verbs: Vec<VerbHandle> = match verbs_bytes {
            None => vec![],
            Some(verb_bytes) => {
                let (verbs, _) = bincode::decode_from_slice(&verb_bytes, self.bincode_cfg)?;
                verbs
            }
        };
        let verb = verbs
            .iter()
            .find(|vh| verbname_matches(&vh.names, &v).is_some());
        let Some(verb) = verb else {
            return Err(ObjectError::VerbNotFound(o, v.clone()).into())
        };

        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key(o, verb.uuid);
        let prg_bytes = self.tx.get_cf(cf, vk)?;
        let Some(prg_bytes) = prg_bytes else {
            return Err(ObjectError::VerbNotFound(o, v.clone()).into())
        };
        let (program, _) = bincode::decode_from_slice(&prg_bytes, self.bincode_cfg)?;
        Ok((program, verb.clone()))
    }
    fn get_properties(&self, o: Objid) -> Result<Vec<PropHandle>, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectProperties as u8) as usize];
        let ok = object_key(o);
        let props_bytes = self.tx.get_cf(cf, ok)?;
        let props = match props_bytes {
            None => vec![],
            Some(prop_bytes) => {
                let (props, _) = bincode::decode_from_slice(&prop_bytes, self.bincode_cfg)?;
                props
            }
        };
        Ok(props)
    }
    fn retrieve_property(&self, o: Objid, u: u128) -> Result<Var, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let ok = composite_key(o, u);
        let var_bytes = self.tx.get_cf(cf, ok)?;
        let Some(var_bytes) = var_bytes else {
            let u_uuid_str = Uuid::from_u128(u).to_string();
            return Err(ObjectError::PropertyNotFound(o, u_uuid_str).into());
        };
        let (var, _) = bincode::decode_from_slice(&var_bytes, self.bincode_cfg)?;
        Ok(var)
    }
    fn set_property_value(&self, o: Objid, u: u128, v: Var) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let ok = composite_key(o, u);
        let var_bytes = bincode::encode_to_vec(v, self.bincode_cfg)?;
        self.tx.put_cf(cf, ok, var_bytes)?;
        Ok(())
    }
    fn set_property_info(
        &self,
        o: Objid,
        u: u128,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        new_name: Option<String>,
    ) -> Result<(), anyhow::Error> {
        let p_cf = self.cf_handles[(ColumnFamilies::ObjectProperties as u8) as usize];
        let ok = object_key(o);
        let props_bytes = self.tx.get_cf(p_cf, ok.clone())?;
        let Some(props_bytes) = props_bytes else {
            let u_uuid_str = Uuid::from_u128(u).to_string();
            return Err(ObjectError::PropertyNotFound(o, u_uuid_str).into());
        };
        let (mut props, _): (Vec<PropHandle>, _) =
            bincode::decode_from_slice(&props_bytes, self.bincode_cfg)?;
        let mut found = false;
        for prop in props.iter_mut() {
            if prop.uuid == u {
                found = true;
                prop.owner = owner;
                prop.perms = perms;
                if let Some(new_name) = new_name {
                    prop.name = new_name;
                }
                break;
            }
        }
        if !found {
            let u_uuid_str = Uuid::from_u128(u).to_string();
            return Err(ObjectError::PropertyNotFound(o, u_uuid_str).into());
        }
        let props_bytes = bincode::encode_to_vec(&props, self.bincode_cfg)?;
        self.tx.put_cf(p_cf, ok, props_bytes)?;
        Ok(())
    }
    fn delete_property(&self, o: Objid, u: u128) -> Result<(), anyhow::Error> {
        let p_cf = self.cf_handles[(ColumnFamilies::ObjectProperties as u8) as usize];
        let ok = object_key(o);
        let props_bytes = self.tx.get_cf(p_cf, ok.clone())?;
        let Some(props_bytes) = props_bytes else {
            return Err(ObjectError::ObjectNotFound(o).into());
        };
        let (mut props, _): (Vec<PropHandle>, _) =
            bincode::decode_from_slice(&props_bytes, self.bincode_cfg)?;
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
        let props_bytes = bincode::encode_to_vec(&props, self.bincode_cfg)?;
        self.tx.put_cf(p_cf, ok, props_bytes)?;

        // Need to also delete the property value.
        let pv_cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let uk = composite_key(o, u);
        self.tx.delete_cf(pv_cf, uk)?;

        Ok(())
    }
    fn add_property(
        &self,
        o: Objid,
        name: String,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<PropHandle, anyhow::Error> {
        // TODO check names for dupes and return error

        let p_cf = self.cf_handles[(ColumnFamilies::ObjectProperties as u8) as usize];
        let ok = object_key(o);
        let props_bytes = self.tx.get_cf(p_cf, ok.clone())?;
        let mut props = match props_bytes {
            None => vec![],
            Some(prop_bytes) => {
                let (props, _) = bincode::decode_from_slice(&prop_bytes, self.bincode_cfg)?;
                props
            }
        };
        // Generate a new property ID.
        let u = Uuid::new_v4();
        let prop = PropHandle {
            uuid: u.as_u128(),
            definer: o,
            name,
            owner,
            perms,
        };
        props.push(prop.clone());
        let props_bytes = bincode::encode_to_vec(&props, self.bincode_cfg)?;
        self.tx.put_cf(p_cf, ok, props_bytes)?;

        // If we have an initial value, set it.
        if let Some(value) = value {
            let value_cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
            let ok = composite_key(o, u.as_u128());
            let prop_bytes = bincode::encode_to_vec(value, self.bincode_cfg)?;
            self.tx.put_cf(value_cf, ok, prop_bytes)?;
        }

        Ok(prop)
    }
    #[tracing::instrument(skip(self))]
    fn resolve_property(&self, obj: Objid, n: String) -> Result<PropHandle, anyhow::Error> {
        let op_cf = self.cf_handles[(ColumnFamilies::ObjectParent as u8) as usize];
        let ov_cf = self.cf_handles[(ColumnFamilies::ObjectProperties as u8) as usize];
        let mut search_o = obj;
        loop {
            let ok = object_key(search_o);

            let props: Vec<PropHandle> = match self.tx.get_cf(ov_cf, ok.clone())? {
                None => vec![],
                Some(prop_bytes) => {
                    let (props, _) = bincode::decode_from_slice(&prop_bytes, self.bincode_cfg)?;
                    props
                }
            };
            let prop = props.iter().find(|vh| vh.name == n);
            // If we found the property, return it.
            if let Some(prop) = prop {
                return Ok(prop.clone());
            }

            // Otherwise, find our parent.  If it's, then set o to it and continue.
            let Ok(parent) = get_object_value(op_cf, &self.tx, search_o) else {
                break;
            };
            if parent == NOTHING {
                break;
            }
            search_o = parent;
        }
        Err(ObjectError::PropertyNotFound(obj, n).into())
    }
    fn commit(self) -> Result<CommitResult, anyhow::Error> {
        match self.tx.commit() {
            Ok(()) => Ok(CommitResult::Success),
            Err(e) if e.kind() == ErrorKind::Busy || e.kind() == ErrorKind::TryAgain => {
                Ok(CommitResult::ConflictRetry)
            }
            Err(e) => bail!(e),
        }
    }
    fn rollback(&self) -> Result<(), anyhow::Error> {
        self.tx.rollback()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::codegen::compile;
    use crate::db::rocksdb::tx_db_impl::RocksDbTx;
    use crate::db::rocksdb::{ColumnFamilies, DbStorage};
    use crate::model::objects::ObjAttrs;
    use crate::model::r#match::VerbArgsSpec;
    use crate::model::ObjectError;
    use crate::util::bitenum::BitEnum;
    use crate::var::{v_str, Objid, NOTHING};
    use rocksdb::OptimisticTransactionDB;
    use std::sync::Arc;
    use strum::VariantNames;
    use tempdir::TempDir;

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
                bincode_cfg: bincode::config::standard(),
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
        )
        .unwrap();
        let prop = tx.resolve_property(oid, "test".into()).unwrap();
        assert_eq!(prop.name, "test");

        let v = tx.retrieve_property(oid, prop.uuid).unwrap();
        assert_eq!(v, v_str("test"));
    }

    #[test]
    fn test_parent_children() {
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

        assert_eq!(tx.get_object_parent(b).unwrap(), a);
        assert_eq!(tx.get_object_children(a).unwrap(), vec![b]);

        assert_eq!(tx.get_object_parent(a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_children(b).unwrap(), vec![]);

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
        assert_eq!(tx.get_object_parent(b).unwrap(), c);
        assert_eq!(tx.get_object_children(a).unwrap(), vec![]);
        assert_eq!(tx.get_object_children(c).unwrap(), vec![b]);
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

        tx.add_property(a, "test".into(), NOTHING, BitEnum::new(), None)
            .unwrap();
        let prop = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name, "test");

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
