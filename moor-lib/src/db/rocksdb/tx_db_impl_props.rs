use crate::db::rocksdb::tx_db_impl::{
    composite_key, get_oid_or_nothing, get_oid_value, oid_key, RocksDbTx,
};
use crate::db::rocksdb::ColumnFamilies;
use crate::db::{PropDef, PropDefs};
use moor_value::model::props::PropFlag;
use moor_value::model::WorldStateError;
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::{ObjSet, Objid, NOTHING};
use moor_value::var::{v_none, Var};
use moor_value::BINCODE_CONFIG;
use tracing::{info, trace};
use uuid::Uuid;

// Methods related to properties; definitions and values.
impl<'a> RocksDbTx<'a> {
    #[tracing::instrument(skip(self))]
    pub fn get_propdefs(&self, o: Objid) -> Result<PropDefs, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectPropDefs as u8) as usize];
        let ok = oid_key(o);
        let props_bytes = self.tx.get_cf(cf, ok)?;
        let props = match props_bytes {
            None => PropDefs::empty(),
            Some(prop_bytes) => {
                let (props, _) = bincode::decode_from_slice(&prop_bytes, *BINCODE_CONFIG)?;
                props
            }
        };
        Ok(props)
    }
    #[tracing::instrument(skip(self))]
    pub fn retrieve_property(&self, o: Objid, u: Uuid) -> Result<Var, anyhow::Error> {
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
    pub fn set_property_value(&self, o: Objid, u: Uuid, v: Var) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let ok = composite_key(o, u.as_bytes());
        let var_bytes = bincode::encode_to_vec(v, *BINCODE_CONFIG)?;
        self.tx.put_cf(cf, ok, var_bytes)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    pub fn set_property_info(
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
        let (mut props, _): (PropDefs, _) =
            bincode::decode_from_slice(&props_bytes, *BINCODE_CONFIG)?;

        let Some(new_props) = props.with_updated(u, |p| {
            let mut new_p = p.clone();
            if let Some(new_owner) = new_owner {
                new_p.owner = new_owner;
            }
            if let Some(new_perms) = new_perms {
                new_p.perms = new_perms;
            }
            if let Some(new_name) = &new_name {
                new_p.name = new_name.clone();
            }
            new_p
        }) else {
            let u_uuid_str = u.to_string();
            return Err(WorldStateError::PropertyNotFound(o, u_uuid_str).into());
        };

        self.update_propdefs(o, new_props)?;
        Ok(())
    }
    #[tracing::instrument(skip(self))]
    pub fn delete_property(&self, o: Objid, u: Uuid) -> Result<(), anyhow::Error> {
        let p_cf = self.cf_handles[(ColumnFamilies::ObjectPropDefs as u8) as usize];
        let ok = oid_key(o);
        let props_bytes = self.tx.get_cf(p_cf, ok.clone())?;
        let Some(props_bytes) = props_bytes else {
            return Err(WorldStateError::ObjectNotFound(o).into());
        };
        let (props, _): (PropDefs, _) =
            bincode::decode_from_slice(&props_bytes, *BINCODE_CONFIG)?;
        let Some(new_props) = props.with_removed(u) else {
            let u_uuid_str = u.to_string();
            return Err(WorldStateError::PropertyNotFound(o, u_uuid_str).into());
        };
        self.update_propdefs(o, new_props)?;

        // Need to also delete the property value.
        let pv_cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let uk = composite_key(o, u.as_bytes());
        self.tx.delete_cf(pv_cf, uk)?;

        Ok(())
    }
    #[tracing::instrument(skip(self))]
    pub fn clear_property(&self, o: Objid, u: Uuid) -> Result<(), anyhow::Error> {
        // Just delete the property value.
        let pv_cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let uk = composite_key(o, u.as_bytes());
        self.tx.delete_cf(pv_cf, uk)?;

        Ok(())
    }
    #[tracing::instrument(skip(self))]
    pub fn define_property(
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
        let mut locations = ObjSet::from(vec![location]);
        let descendants = self.descendants(location)?;
        locations.append(descendants);

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

        for location in locations.iter() {
            let ok = oid_key(*location);
            let props_bytes = self.tx.get_cf(p_cf, ok.clone())?;
            let mut props: PropDefs = match props_bytes {
                None => PropDefs::empty(),
                Some(prop_bytes) => {
                    let (props, _) = bincode::decode_from_slice(&prop_bytes, *BINCODE_CONFIG)?;
                    props
                }
            };

            // Verify we don't already have a property with this name. If we do, return an error.
            if props.iter().any(|prop| prop.name == name) {
                return Err(WorldStateError::DuplicatePropertyDefinition(*location, name).into());
            }

            let prop = PropDef {
                uuid: *u.as_bytes(),
                definer,
                location: *location,
                name: name.clone(),
                owner,
                perms,
            };
            props.push(prop.clone());
            self.update_propdefs(*location, props)?;
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
    #[tracing::instrument(skip(self))]
    pub fn resolve_property(&self, obj: Objid, n: String) -> Result<(PropDef, Var), anyhow::Error> {
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
}

impl<'a> RocksDbTx<'a> {
    pub(crate) fn update_propdefs(
        &self,
        obj: Objid,
        new_props: PropDefs,
    ) -> Result<(), anyhow::Error> {
        let propdefs_cf = self.cf_handles[((ColumnFamilies::ObjectPropDefs) as u8) as usize];
        let props_bytes = bincode::encode_to_vec(new_props, *BINCODE_CONFIG)?;
        self.tx.put_cf(propdefs_cf, oid_key(obj), props_bytes)?;
        Ok(())
    }

    pub(crate) fn seek_property_definition(
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

            let props: PropDefs = match self.tx.get_cf(ov_cf, ok.clone())? {
                None => PropDefs::empty(),
                Some(prop_bytes) => {
                    let (props, _) = bincode::decode_from_slice(&prop_bytes, *BINCODE_CONFIG)?;
                    props
                }
            };
            let prop = props
                .iter()
                .find(|vh| vh.name.to_lowercase() == n.to_lowercase());

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
}
