use std::collections::{HashMap, HashSet};

use anyhow::{bail, Context, Error};
use rocksdb::{ColumnFamily, ErrorKind};
use tracing::{info, trace};
use uuid::Uuid;

use moor_value::util::bitenum::BitEnum;
use moor_value::util::verbname_cmp;
use moor_value::var::objid::{Objid, NOTHING};
use moor_value::var::{v_none, Var};

use crate::db::rocksdb::tx_server::{PropDef, VerbHandle};
use crate::db::rocksdb::{ColumnFamilies, DbStorage};
use crate::BINCODE_CONFIG;
use moor_value::model::objects::{ObjAttrs, ObjFlag};
use moor_value::model::props::PropFlag;
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verbs::{BinaryType, VerbFlag};
use moor_value::model::CommitResult;
use moor_value::model::WorldStateError;

fn oid_key(o: Objid) -> Vec<u8> {
    o.0.to_be_bytes().to_vec()
}

fn composite_key(o: Objid, uuid: &uuid::Bytes) -> Vec<u8> {
    let mut key = oid_key(o);
    key.extend_from_slice(&uuid[..]);
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
    let ov = ov.ok_or(WorldStateError::ObjectNotFound(o))?;
    let ov = u64::from_be_bytes(ov.try_into().unwrap());
    let ov = Objid(ov as i64);
    Ok(ov)
}

fn get_oid_or_nothing<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
) -> Result<Objid, anyhow::Error> {
    let ok = oid_key(o);
    let ov = tx.get_cf(cf, ok).unwrap();
    let Some(ov) = ov else {
        return Ok(NOTHING);
    };
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
    let ov = ov.ok_or(WorldStateError::ObjectNotFound(o))?;
    let (ov, _) = bincode::decode_from_slice(&ov, *BINCODE_CONFIG).unwrap();
    Ok(ov)
}

fn set_oid_vec<'a>(
    cf: &'a ColumnFamily,
    tx: &rocksdb::Transaction<'a, rocksdb::OptimisticTransactionDB>,
    o: Objid,
    v: Vec<Objid>,
) -> Result<(), WorldStateError> {
    let ok = oid_key(o);
    let ov = oid_vec(v).unwrap();
    tx.put_cf(cf, ok, ov).unwrap();
    Ok(())
}

fn cf_for<'a>(cf_handles: &[&'a ColumnFamily], cf: ColumnFamilies) -> &'a ColumnFamily {
    cf_handles[(cf as u8) as usize]
}

fn err_is_objnjf(e: &anyhow::Error) -> bool {
    if let Some(WorldStateError::ObjectNotFound(_)) = e.downcast_ref::<WorldStateError>() {
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
    // (One thought is to simply make Objid u128 and use UUIDs for object ids and then just handle
    // any totally-theoretical collisions optimistically by relying on commit-time conflicts to
    // suss them out. There's some code in MOO cores that *implies* the concept of monotonically
    // increment OIds, but it is not necessary, I'm pretty sure)
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

    fn seek_property_definition(
        &self,
        obj: Objid,
        n: String,
    ) -> Result<Option<PropDef>, anyhow::Error> {
        trace!(?obj, name = ?n, "resolving property in inheritance hierarchy");
        let op_cf = self.cf_handles[(ColumnFamilies::ObjectParent as u8) as usize];
        let ov_cf = self.cf_handles[(ColumnFamilies::ObjectPropDefs as u8) as usize];
        let mut search_o = obj;
        loop {
            let ok = oid_key(search_o);

            let props: Vec<PropDef> = match self.tx.get_cf(ov_cf, ok.clone())? {
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
            let parent = get_oid_or_nothing(op_cf, &self.tx, search_o)?;
            if parent == NOTHING {
                break;
            }
            search_o = parent;
        }
        trace!(termination_object= ?obj, property=?n, "property not found");
        Ok(None)
    }

    fn closest_common_ancestor_with_ancestors(
        &self,
        a: Objid,
        b: Objid,
    ) -> Result<(Option<Objid>, HashSet<Objid>, HashSet<Objid>), anyhow::Error> {
        let mut ancestors_a = HashSet::new();
        let mut search_a = a;

        let mut ancestors_b = HashSet::new();
        let mut search_b = b;

        loop {
            if search_a == NOTHING && search_b == NOTHING {
                return Ok((None, ancestors_a, ancestors_b)); // No common ancestor found
            }

            if ancestors_b.contains(&search_a) {
                return Ok((Some(search_a), ancestors_a, ancestors_b)); // Common ancestor found
            }

            if ancestors_a.contains(&search_b) {
                return Ok((Some(search_b), ancestors_a, ancestors_b)); // Common ancestor found
            }

            if search_a != NOTHING {
                ancestors_a.insert(search_a);
                let parent_cf = self.cf_handles[((ColumnFamilies::ObjectParent) as u8) as usize];
                let parent = get_oid_or_nothing(parent_cf, &self.tx, search_a)?;
                search_a = parent;
            }

            if search_b != NOTHING {
                ancestors_b.insert(search_b);
                let parent_cf = self.cf_handles[((ColumnFamilies::ObjectParent) as u8) as usize];
                let parent = get_oid_or_nothing(parent_cf, &self.tx, search_b)?;
                search_b = parent;
            }
        }
    }

    fn descendants(&self, obj: Objid) -> Result<Vec<Objid>, anyhow::Error> {
        let mut all_children: Vec<Objid> = vec![];
        let mut search_queue: Vec<Objid> = vec![obj];

        while let Some(search_obj) = search_queue.pop() {
            let new_children = self.get_object_children(search_obj)?;

            // Add new children to the search queue
            search_queue.extend(new_children.iter().cloned());

            // Add new children to the all_children list
            all_children.extend(new_children);
        }
        Ok(all_children)
    }

    fn update_propdefs(&self, obj: Objid, new_props: Vec<PropDef>) -> Result<(), anyhow::Error> {
        let propdefs_cf = self.cf_handles[((ColumnFamilies::ObjectPropDefs) as u8) as usize];
        let props_bytes = bincode::encode_to_vec(new_props, *BINCODE_CONFIG)?;
        self.tx.put_cf(propdefs_cf, oid_key(obj), props_bytes)?;
        Ok(())
    }

    fn update_object_children(
        &self,
        obj: Objid,
        new_cildren: Vec<Objid>,
    ) -> Result<(), WorldStateError> {
        let children_cf = self.cf_handles[((ColumnFamilies::ObjectChildren) as u8) as usize];
        set_oid_vec(children_cf, &self.tx, obj, new_cildren)
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

        // None (#-1) owner becomes
        let owner = attrs.owner.unwrap_or(oid);
        set_oid_value(
            cf_for(&self.cf_handles, ColumnFamilies::ObjectOwner),
            &self.tx,
            oid,
            owner,
        )?;

        // Set initial name
        let name = attrs.name.unwrap_or_else(|| format!("Object #{}", oid.0));
        self.set_object_name(oid, name.clone())?;

        // Establish initial `contents` and `children` vectors, initially empty.
        let c_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectContents);
        set_oid_vec(c_cf, &self.tx, oid, vec![])?;

        self.update_object_children(oid, vec![])?;

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
        let parent_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectParent);
        let property_value_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectPropertyValue);

        if o.0 == 0 {
            info!("Setting parent of #0 to {}", new_parent);
        }
        // TODO this is all very wasteful for net-new objects, which have no children or properties
        // to move around.

        // Steps for object re-parenting:

        // Get o's old-parents's children
        //      remove o from it, and save.
        // Walk existing descendant tree of O and find any props that they inherited from old-parent
        // or any of its ancestors up to the most recent common ancestor, remove them.
        // Get o's new-parent's children list add o to it, and save.
        // Walk same descendant tree, and add props defined by new-parent and *its* ancestors, up to
        // shared one.
        // Set o's parent field.

        // This will find a) our shared ancestor, b) all ancestors not shared with new ancestor,
        // c) all the new ancestors we'd have after the reparenting, all in one go. Hopefully.
        // TODO: the argument order seems backward here. I was able to make it work by flipping
        // new_parent and o, but I need to get to the bottom of this and fix it properly.
        let (_shared_ancestor, old_ancestors, new_ancestors) =
            self.closest_common_ancestor_with_ancestors(new_parent, o)?;

        // Remove from _me_ any of the properties defined by any of my ancestors
        let old_props = self.get_propdefs(o)?;
        let mut delort_props = vec![];
        for p in &old_props {
            if old_ancestors.contains(&p.definer) {
                delort_props.push(p.uuid);
                let vk = composite_key(o, &p.uuid);
                self.tx.delete_cf(property_value_cf, vk)?;
            }
        }
        let new_props: Vec<PropDef> = old_props
            .into_iter()
            .filter(|p| delort_props.contains(&p.uuid))
            .collect();
        self.update_propdefs(o, new_props)?;

        // Now walk all-my-children and destroy all the properties whose definer is me or any
        // of my ancestors not shared by the new parent.
        let descendants = self.descendants(o)?;

        let mut descendant_props = HashMap::new();
        for c in descendants {
            let mut inherited_props = vec![];
            // Remove the set values.
            let old_props = self.get_propdefs(c)?;
            for p in &old_props {
                if old_ancestors.contains(&p.definer) {
                    inherited_props.push(p.uuid);
                    let vk = composite_key(c, &p.uuid);
                    self.tx.delete_cf(property_value_cf, vk)?;
                }
            }
            // And update the property list to not include them
            let new_props: Vec<PropDef> = old_props
                .into_iter()
                .filter(|p| inherited_props.contains(&p.uuid))
                .collect();

            // We're not actually going to *set* these yet because we are going to add, later.
            descendant_props.insert(c, new_props);
        }

        // If this is a new object it won't have a parent, old parent this will come up not-found,
        // and if that's the case we can ignore that.
        match get_oid_value(parent_cf, &self.tx, o) {
            Ok(old_parent) => {
                if old_parent == new_parent {
                    return Ok(());
                }
                if old_parent != NOTHING {
                    // Prune us out of the old parent's children list.
                    let old_children = self.get_object_children(old_parent)?;
                    let new_children = old_children.into_iter().filter(|&x| x != o).collect();
                    self.update_object_children(old_parent, new_children)?;
                }
            }
            Err(e) if !err_is_objnjf(&e) => {
                // Object not found is fine, we just don't have a parent yet.
                return Err(e);
            }
            Err(_) => {}
        };
        set_oid_value(parent_cf, &self.tx, o, new_parent)?;

        if new_parent == NOTHING {
            return Ok(());
        }
        let mut new_children = self.get_object_children(new_parent)?;
        new_children.push(o);
        self.update_object_children(new_parent, new_children)?;

        // Now walk all my new descendants and give them the properties that derive from any
        // ancestors they don't already share.

        // Now collect properties defined on the new ancestors.
        let mut new_props = vec![];
        for a in new_ancestors {
            let props = self.get_propdefs(a)?;
            for p in props {
                if p.definer == a {
                    new_props.push(p.clone())
                }
            }
        }
        // Then put clear copies on each of the descendants
        // This really just means defining the property with no value, which is what we do.
        let descendants = self.descendants(o)?;
        for c in descendants {
            // Check if we have a cached/modified copy from above in descendant_props
            let mut c_props = match descendant_props.remove(&c) {
                None => self.get_propdefs(c)?,
                Some(props) => props,
            };
            for p in &new_props {
                let ph = p.clone();
                c_props.push(ph);
            }
            self.update_propdefs(o, c_props)?;
        }
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn get_object_children(&self, o: Objid) -> Result<Vec<Objid>, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectChildren as u8) as usize];
        Ok(get_oid_vec(cf, &self.tx, o).unwrap_or_else(|_| vec![]))
    }
    #[tracing::instrument(skip(self))]
    fn get_object_name(&self, o: Objid) -> Result<String, anyhow::Error> {
        let cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectName);
        let ok = oid_key(o);
        let name_bytes = self.tx.get_cf(cf, ok)?;
        let Some(name_bytes) = name_bytes else {
            return Err(WorldStateError::ObjectNotFound(o).into());
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
            return Err(WorldStateError::ObjectNotFound(o).into());
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
        get_oid_or_nothing(cf, &self.tx, o)
    }
    #[tracing::instrument(skip(self))]
    fn set_object_owner(&self, o: Objid, owner: Objid) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectOwner as u8) as usize];
        set_oid_value(cf, &self.tx, o, owner)
    }
    #[tracing::instrument(skip(self))]
    fn get_object_parent(&self, o: Objid) -> Result<Objid, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectParent as u8) as usize];
        get_oid_or_nothing(cf, &self.tx, o)
    }
    #[tracing::instrument(skip(self))]
    fn get_object_location(&self, o: Objid) -> Result<Objid, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectLocation as u8) as usize];
        get_oid_or_nothing(cf, &self.tx, o)
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
                return Err(WorldStateError::RecursiveMove(what, new_location).into());
            }
            oid = self.get_object_location(oid).unwrap_or(NOTHING);
        }

        // Get o's location, get its contents, remove o from old contents, put contents back
        // without it. Set new location, get its contents, add o to contents, put contents
        // back with it. Then update the location of o.

        let l_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectLocation);
        let c_cf = cf_for(&self.cf_handles, ColumnFamilies::ObjectContents);

        // Get and remove from contents of old location, if we had any.
        match get_oid_or_nothing(l_cf, &self.tx, what) {
            Ok(NOTHING) => {
                // Object not found is fine, we just don't have a location yet.
            }
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
            Err(e) => {
                return Err(e);
            }
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
        binary: Vec<u8>,
        binary_type: BinaryType,
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

        // Generate a new verb ID.
        let vid = Uuid::new_v4();
        let verb = VerbHandle {
            uuid: *vid.as_bytes(),
            location: oid,
            owner,
            names: names.clone(),
            flags,
            binary_type,
            args,
        };
        verbs.push(verb);
        let verbs_v = bincode::encode_to_vec(&verbs, *BINCODE_CONFIG)?;
        self.tx
            .put_cf(cf, ok, verbs_v)
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
    fn delete_object_verb(&self, o: Objid, v: Uuid) -> Result<(), anyhow::Error> {
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
            if &vh.uuid == v.as_bytes() {
                found = true;
                false
            } else {
                true
            }
        });
        if !found {
            let v_uuid_str = v.to_string();
            return Err(WorldStateError::VerbNotFound(o, v_uuid_str).into());
        }
        let verbs_v = bincode::encode_to_vec(&verbs, *BINCODE_CONFIG)?;
        self.tx.put_cf(cf, ok, verbs_v)?;

        // Delete the program.
        let cf = self.cf_handles[(ColumnFamilies::VerbProgram as u8) as usize];
        let vk = composite_key(o, v.as_bytes());
        self.tx.delete_cf(cf, vk)?;

        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn get_verb(&self, o: Objid, v: Uuid) -> Result<VerbHandle, anyhow::Error> {
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
        let verb = verbs.iter().find(|vh| &vh.uuid == v.as_bytes());
        let Some(verb) = verb else {
            let v_uuid_str = v.to_string();
            return Err(WorldStateError::VerbNotFound(o, v_uuid_str).into());
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
            return Err(WorldStateError::VerbNotFound(o, n).into());
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
            return Err(WorldStateError::VerbNotFound(o, format!("{}", i)).into());
        }
        let verb = verbs.get(i);
        let Some(verb) = verb else {
            return Err(WorldStateError::VerbNotFound(o, format!("{}", i)).into());
        };
        Ok(verb.clone())
    }
    #[tracing::instrument(skip(self))]
    fn get_binary(&self, o: Objid, v: Uuid) -> Result<Vec<u8>, anyhow::Error> {
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
        Err(WorldStateError::VerbNotFound(o, n).into())
    }
    #[tracing::instrument(skip(self))]
    fn retrieve_verb(&self, o: Objid, v: String) -> Result<(Vec<u8>, VerbHandle), anyhow::Error> {
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
    fn set_verb_info(
        &self,
        o: Objid,
        v: Uuid,
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
            if &verb.uuid == v.as_bytes() {
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
            let v_uuid_str = v.to_string();
            return Err(WorldStateError::VerbNotFound(o, v_uuid_str).into());
        }

        let verbs_v = bincode::encode_to_vec(&verbs, *BINCODE_CONFIG)?;

        self.tx.put_cf(cf, ok, verbs_v)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn get_propdefs(&self, o: Objid) -> Result<Vec<PropDef>, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectPropDefs as u8) as usize];
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
    fn retrieve_property(&self, o: Objid, u: Uuid) -> Result<Var, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let ok = composite_key(o, u.as_bytes());
        let var_bytes = self.tx.get_cf(cf, ok)?;
        let Some(var_bytes) = var_bytes else {
            let u_uuid_str = u.to_string();
            return Err(WorldStateError::PropertyNotFound(o, u_uuid_str).into());
        };
        let (var, _) = bincode::decode_from_slice(&var_bytes, *BINCODE_CONFIG)?;
        Ok(var)
    }
    #[tracing::instrument(skip(self))]
    fn set_property_value(&self, o: Objid, u: Uuid, v: Var) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let ok = composite_key(o, u.as_bytes());
        let var_bytes = bincode::encode_to_vec(v, *BINCODE_CONFIG)?;
        self.tx.put_cf(cf, ok, var_bytes)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn set_property_info(
        &self,
        o: Objid,
        u: Uuid,
        new_owner: Option<Objid>,
        new_perms: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
    ) -> Result<(), anyhow::Error> {
        let p_cf = self.cf_handles[(ColumnFamilies::ObjectPropDefs as u8) as usize];
        let ok = oid_key(o);
        let props_bytes = self.tx.get_cf(p_cf, ok.clone())?;
        let Some(props_bytes) = props_bytes else {
            let u_uuid_str = u.to_string();
            return Err(WorldStateError::PropertyNotFound(o, u_uuid_str).into());
        };
        let (mut props, _): (Vec<PropDef>, _) =
            bincode::decode_from_slice(&props_bytes, *BINCODE_CONFIG)?;
        let mut found = false;
        for prop in props.iter_mut() {
            if &prop.uuid == u.as_bytes() {
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
            }
        }
        if !found {
            let u_uuid_str = u.to_string();
            return Err(WorldStateError::PropertyNotFound(o, u_uuid_str).into());
        }
        self.update_propdefs(o, props)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn delete_property(&self, o: Objid, u: Uuid) -> Result<(), anyhow::Error> {
        let p_cf = self.cf_handles[(ColumnFamilies::ObjectPropDefs as u8) as usize];
        let ok = oid_key(o);
        let props_bytes = self.tx.get_cf(p_cf, ok.clone())?;
        let Some(props_bytes) = props_bytes else {
            return Err(WorldStateError::ObjectNotFound(o).into());
        };
        let (mut props, _): (Vec<PropDef>, _) =
            bincode::decode_from_slice(&props_bytes, *BINCODE_CONFIG)?;
        let mut found = false;
        props.retain(|prop| {
            if &prop.uuid == u.as_bytes() {
                found = true;
                false
            } else {
                true
            }
        });
        if !found {
            let u_uuid_str = u.to_string();
            return Err(WorldStateError::PropertyNotFound(o, u_uuid_str).into());
        }
        self.update_propdefs(o, props)?;

        // Need to also delete the property value.
        let pv_cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let uk = composite_key(o, u.as_bytes());
        self.tx.delete_cf(pv_cf, uk)?;

        Ok(())
    }
    #[tracing::instrument(skip(self))]
    fn define_property(
        &self,
        definer: Objid,
        location: Objid,
        name: String,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<Uuid, anyhow::Error> {
        let p_cf = self.cf_handles[(ColumnFamilies::ObjectPropDefs as u8) as usize];

        // We have to propagate the propdef down to all my children
        let mut locations = vec![location];
        let mut descendants = self.descendants(location)?;
        locations.append(&mut descendants);

        if name == "builtins" {
            info!(
                ?location,
                ?definer,
                ?locations,
                "define_property: name is 'builtins'"
            );
        }

        // Generate a new property ID. This will get shared all the way down the pipe.
        // But the key for the actual value is always composite of oid,uuid
        let u = Uuid::new_v4();

        for location in locations {
            let ok = oid_key(location);
            let props_bytes = self.tx.get_cf(p_cf, ok.clone())?;
            let mut props: Vec<PropDef> = match props_bytes {
                None => vec![],
                Some(prop_bytes) => {
                    let (props, _) = bincode::decode_from_slice(&prop_bytes, *BINCODE_CONFIG)?;
                    props
                }
            };

            // Verify we don't already have a property with this name. If we do, return an error.
            if props.iter().any(|prop| prop.name == name) {
                return Err(WorldStateError::DuplicatePropertyDefinition(location, name).into());
            }

            let prop = PropDef {
                uuid: *u.as_bytes(),
                definer,
                location,
                name: name.clone(),
                owner,
                perms,
            };
            props.push(prop.clone());
            self.update_propdefs(location, props)?;
        }
        // If we have an initial value, set it (NOTE: if propagate_to_children is set, this does not
        // go down the inheritance tree, the value is left "clear" on all children)
        if let Some(value) = value {
            let value_cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
            let propkey = composite_key(definer, u.as_bytes());
            let prop_bytes = bincode::encode_to_vec(value, *BINCODE_CONFIG)?;
            self.tx.put_cf(value_cf, propkey, prop_bytes)?;
        }

        Ok(u)
    }

    fn clear_property(&self, o: Objid, u: Uuid) -> Result<(), Error> {
        let pv_cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let uk = composite_key(o, u.as_bytes());
        self.tx.delete_cf(pv_cf, uk)?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn resolve_property(&self, obj: Objid, n: String) -> Result<(PropDef, Var), anyhow::Error> {
        trace!(?obj, name = ?n, "resolving property");
        let op_cf = self.cf_handles[(ColumnFamilies::ObjectParent as u8) as usize];

        let propdef = self.seek_property_definition(obj, n.clone())?;
        let Some(propdef) = propdef else {
            return Err(WorldStateError::PropertyNotFound(obj, n).into());
        };

        // Then we're going to resolve the value up the tree, skipping 'clear' (un-found) until we
        // get a value.
        let mut search_obj = obj;
        loop {
            // Look for the value. If we're not 'clear', we can return straight away. that's our thing.
            if let Ok(found) = self.retrieve_property(search_obj, Uuid::from_bytes(propdef.uuid)) {
                return Ok((propdef, found));
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
        // TODO: is this right? can you have a 'clear' value on a root def of a property?
        Ok((propdef, v_none()))
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
    use uuid::Uuid;

    use moor_value::util::bitenum::BitEnum;
    use moor_value::var::objid::{Objid, NOTHING};
    use moor_value::var::v_str;

    use crate::db::rocksdb::tx_db_impl::RocksDbTx;
    use crate::db::rocksdb::{ColumnFamilies, DbStorage};
    use moor_value::model::objects::ObjAttrs;
    use moor_value::model::r#match::VerbArgsSpec;
    use moor_value::model::verbs::BinaryType;
    use moor_value::model::WorldStateError;

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

        // Create new obj and reparent one child
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
    fn test_descendants() {
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

        let d = tx
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test4".into()),
                    parent: Some(c),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        assert_eq!(tx.descendants(a).unwrap(), vec![b, c, d]);
        assert_eq!(tx.descendants(b).unwrap(), vec![]);
        assert_eq!(tx.descendants(c).unwrap(), vec![d]);

        // Now reparent d to b
        tx.set_object_parent(d, b).unwrap();
        assert_eq!(tx.get_object_children(a).unwrap(), vec![b, c]);
        assert_eq!(tx.get_object_children(b).unwrap(), vec![d]);
        assert_eq!(tx.get_object_children(c).unwrap(), vec![]);
        assert_eq!(tx.descendants(a).unwrap(), vec![b, c, d]);
        assert_eq!(tx.descendants(b).unwrap(), vec![d]);
        assert_eq!(tx.descendants(c).unwrap(), vec![]);
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
        match tx
            .set_object_location(c, b)
            .err()
            .unwrap()
            .downcast_ref::<WorldStateError>()
        {
            Some(WorldStateError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // Move b one level deeper, and then check recursive move detection again.
        tx.set_object_location(b, d).unwrap();
        match tx
            .set_object_location(c, b)
            .err()
            .unwrap()
            .downcast_ref::<WorldStateError>()
        {
            Some(WorldStateError::RecursiveMove(_, _)) => {}
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

        tx.define_property(
            oid,
            oid,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test")),
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

        tx.define_property(
            a,
            a,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();
        let (prop, v) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name, "test");
        assert_eq!(v, v_str("test_value"));

        // Verify we *don't* get this property for an unrelated, unhinged object by reparenting b
        // to new parent c.  This should remove the defs for a's properties from b.
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
            result
                .err()
                .unwrap()
                .downcast_ref::<WorldStateError>()
                .unwrap(),
            &WorldStateError::PropertyNotFound(b, "test".into())
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

        tx.define_property(
            a,
            a,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();
        let (prop, v) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name, "test");
        assert_eq!(v, v_str("test_value"));

        // Define the property again, but on the object 'b',
        // This should raise an error because the child already *has* this property.
        // MOO will not let this happen. The right way to handle overloading is to set the value
        // on the child.
        let result = tx.define_property(a, b, "test".into(), NOTHING, BitEnum::new(), None);
        assert!(
            matches!(result, Err(e) if matches!(e.downcast_ref::<WorldStateError>(),
                Some(WorldStateError::DuplicatePropertyDefinition(_, _))))
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

        tx.add_object_verb(
            a,
            a,
            vec!["test".into()],
            vec![],
            BinaryType::LambdaMoo18X,
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

        let v_uuid = tx.resolve_verb(a, "test".into(), None).unwrap().uuid;
        let v_uuid = Uuid::from_bytes(v_uuid);
        assert_eq!(tx.get_binary(a, v_uuid).unwrap(), vec![]);
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

        let verb_names = vec!["dname*c".into(), "iname*c".into()];
        tx.add_object_verb(
            a,
            a,
            verb_names.clone(),
            vec![],
            BinaryType::LambdaMoo18X,
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
