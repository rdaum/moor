use tracing::trace;
use uuid::Uuid;

use moor_value::model::defset::{HasUuid, Named};
use moor_value::model::objset::ObjSet;
use moor_value::model::propdef::{PropDef, PropDefs};
use moor_value::model::props::PropFlag;
use moor_value::model::WorldStateError;
use moor_value::util::bitenum::BitEnum;
use moor_value::util::slice_ref::SliceRef;
use moor_value::var::objid::Objid;
use moor_value::var::{v_none, Var};
use moor_value::{AsByteBuffer, NOTHING};

use crate::db::rocksdb::tx_db_impl::{
    composite_key_uuid, get_oid_or_nothing, get_oid_value, oid_key, write_cf, RocksDbTx,
};
use crate::db::rocksdb::ColumnFamilies;

// Methods related to properties; definitions and values.
impl<'a> RocksDbTx<'a> {
    #[tracing::instrument(skip(self))]
    pub fn get_propdefs(&self, o: Objid) -> Result<PropDefs, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectPropDefs as u8) as usize];
        let ok = oid_key(o);
        let props_bytes = self.tx.get_cf(cf, ok)?;
        let props = match props_bytes {
            None => PropDefs::empty(),
            Some(props_bytes) => PropDefs::from_sliceref(SliceRef::from_vec(props_bytes)),
        };
        Ok(props)
    }
    #[tracing::instrument(skip(self))]
    pub fn retrieve_property(&self, o: Objid, u: Uuid) -> Result<Var, anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let ok = composite_key_uuid(o, &u);
        let var_bytes = self.tx.get_cf(cf, ok)?;
        let Some(var_bytes) = var_bytes else {
            let u_uuid_str = u.to_string();
            return Err(WorldStateError::PropertyNotFound(o, u_uuid_str).into());
        };
        let var = Var::from_sliceref(SliceRef::from_bytes(&var_bytes));
        Ok(var)
    }
    #[tracing::instrument(skip(self))]
    pub fn set_property_value(&self, o: Objid, u: Uuid, v: Var) -> Result<(), anyhow::Error> {
        let cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let ok = composite_key_uuid(o, &u);
        write_cf(&self.tx, cf, &ok, &v)?;
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
        let props = PropDefs::from_sliceref(SliceRef::from_bytes(&props_bytes));
        let Some(new_props) = props.with_updated(u, |p| {
            let name = match &new_name {
                None => p.name(),
                Some(s) => s.as_str(),
            };
            PropDef::new(
                u,
                p.definer(),
                p.location(),
                name,
                new_perms.unwrap_or(p.flags()),
                new_owner.unwrap_or(p.owner()),
            )
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
        let props = PropDefs::from_sliceref(SliceRef::from_bytes(&props_bytes));
        let Some(new_props) = props.with_removed(u) else {
            let u_uuid_str = u.to_string();
            return Err(WorldStateError::PropertyNotFound(o, u_uuid_str).into());
        };
        self.update_propdefs(o, new_props)?;

        // Need to also delete the property value.
        let pv_cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let uk = composite_key_uuid(o, &u);
        self.tx.delete_cf(pv_cf, uk)?;

        Ok(())
    }
    #[tracing::instrument(skip(self))]
    pub fn clear_property(&self, o: Objid, u: Uuid) -> Result<(), anyhow::Error> {
        // Just delete the property value.
        let pv_cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
        let uk = composite_key_uuid(o, &u);
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
        let descendants = self.descendants(location)?;
        let locations = ObjSet::from(&[location]).with_concatenated(descendants);

        // Generate a new property ID. This will get shared all the way down the pipe.
        // But the key for the actual value is always composite of oid,uuid
        let u = Uuid::new_v4();

        for location in locations.iter() {
            let ok = oid_key(location);
            let props_bytes = self.tx.get_cf(p_cf, ok.clone())?;
            let props: PropDefs = match props_bytes {
                None => PropDefs::empty(),
                Some(props_bytes) => PropDefs::from_sliceref(SliceRef::from_bytes(&props_bytes)),
            };

            // Verify we don't already have a property with this name. If we do, return an error.
            if props.iter().any(|prop| prop.matches_name(name.as_str())) {
                return Err(WorldStateError::DuplicatePropertyDefinition(location, name).into());
            }

            let prop = PropDef::new(u, definer, location, name.as_str(), perms, owner);
            self.update_propdefs(location, props.with_added(prop))?;
        }
        // If we have an initial value, set it (NOTE: if propagate_to_children is set, this does not
        // go down the inheritance tree, the value is left "clear" on all children)
        if let Some(value) = value {
            let value_cf = self.cf_handles[(ColumnFamilies::ObjectPropertyValue as u8) as usize];
            let propkey = composite_key_uuid(definer, &u);
            write_cf(&self.tx, value_cf, &propkey, &value)?;
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
            if let Ok(found) = self.retrieve_property(search_obj, propdef.uuid()) {
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
        write_cf(&self.tx, propdefs_cf, &oid_key(obj), &new_props)?;
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
                Some(props_bytes) => PropDefs::from_sliceref(SliceRef::from_bytes(&props_bytes)),
            };
            if let Some(prop) = props.find_named(n.as_str()) {
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
