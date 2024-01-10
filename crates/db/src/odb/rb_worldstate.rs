// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use strum::{EnumCount, IntoEnumIterator};
use tracing::warn;
use uuid::Uuid;

use crate::rdb::TupleError;
use crate::Database;
use moor_values::model::defset::{HasUuid, Named};
use moor_values::model::objects::{ObjAttrs, ObjFlag};
use moor_values::model::objset::ObjSet;
use moor_values::model::propdef::{PropDef, PropDefs};
use moor_values::model::props::PropFlag;
use moor_values::model::r#match::VerbArgsSpec;
use moor_values::model::verbdef::{VerbDef, VerbDefs};
use moor_values::model::verbs::{BinaryType, VerbAttrs, VerbFlag};
use moor_values::model::world_state::{WorldState, WorldStateSource};
use moor_values::model::{CommitResult, WorldStateError};
use moor_values::util::bitenum::BitEnum;
use moor_values::var::objid::Objid;
use moor_values::var::{v_none, Var};
use moor_values::{AsByteBuffer, NOTHING, SYSTEM_OBJECT};

use crate::db_tx::DbTransaction;
use crate::db_worldstate::DbTxWorldState;
use crate::loader::LoaderInterface;
use crate::odb::object_relations;
use crate::odb::object_relations::{
    get_all_object_keys_matching, WorldStateRelation, WorldStateSequences,
};
use crate::rdb::{CommitError, Transaction};
use crate::rdb::{RelBox, RelationInfo};

/// An implementation of `WorldState` / `WorldStateSource` that uses the rdb as its backing
pub struct RelBoxWorldState {
    db: Arc<RelBox>,
}

impl RelBoxWorldState {
    pub async fn open(path: Option<PathBuf>, memory_size: usize) -> (Self, bool) {
        let mut relations: Vec<RelationInfo> = WorldStateRelation::iter()
            .map(|wsr| {
                RelationInfo {
                    name: wsr.to_string(),
                    domain_type_id: 0, /* tbd */
                    codomain_type_id: 0,
                    secondary_indexed: false,
                }
            })
            .collect();

        // "Children" is derived from projection of the secondary index of parents.
        relations[WorldStateRelation::ObjectParent as usize].secondary_indexed = true;
        // Same with "contents".
        relations[WorldStateRelation::ObjectLocation as usize].secondary_indexed = true;
        let db = RelBox::new(memory_size, path, &relations, WorldStateSequences::COUNT).await;

        // Check the db for sys (#0) object to see if this is a fresh DB or not.
        let fresh_db = {
            let rels = db.canonical.read().await;
            rels[WorldStateRelation::ObjectParent as usize]
                .seek_by_domain(SYSTEM_OBJECT.0.as_sliceref())
                .is_none()
        };
        (Self { db }, fresh_db)
    }
}

#[async_trait]
impl WorldStateSource for RelBoxWorldState {
    async fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let tx = RelBoxTransaction::new(self.db.clone());
        return Ok(Box::new(DbTxWorldState { tx: Box::new(tx) }));
    }
}

pub struct RelBoxTransaction {
    tx: Transaction,
}

#[async_trait]
impl DbTransaction for RelBoxTransaction {
    async fn get_objects(&self) -> Result<ObjSet, WorldStateError> {
        get_all_object_keys_matching(
            &self.tx,
            WorldStateRelation::ObjectFlags,
            |_, _: BitEnum<ObjFlag>| true,
        )
        .await
    }

    async fn get_players(&self) -> Result<ObjSet, WorldStateError> {
        // TODO: this is going to be not-at-all performant in the long run, and we'll need a way to
        //   cache this or index it better
        get_all_object_keys_matching(
            &self.tx,
            WorldStateRelation::ObjectFlags,
            |_, flags: BitEnum<ObjFlag>| flags.contains(ObjFlag::User),
        )
        .await
    }

    async fn get_object_owner(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectOwner, obj)
            .await
            .ok_or(WorldStateError::ObjectNotFound(obj))
    }

    async fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), WorldStateError> {
        object_relations::upsert_object_value(&self.tx, WorldStateRelation::ObjectOwner, obj, owner)
            .await
    }

    async fn get_object_flags(&self, obj: Objid) -> Result<BitEnum<ObjFlag>, WorldStateError> {
        object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectFlags, obj)
            .await
            .ok_or(WorldStateError::ObjectNotFound(obj))
    }

    async fn set_object_flags(
        &self,
        obj: Objid,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError> {
        object_relations::upsert_object_value(&self.tx, WorldStateRelation::ObjectFlags, obj, flags)
            .await
    }

    async fn get_object_name(&self, obj: Objid) -> Result<String, WorldStateError> {
        object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectName, obj)
            .await
            .ok_or(WorldStateError::ObjectNotFound(obj))
    }

    async fn create_object(
        &self,
        id: Option<Objid>,
        attrs: ObjAttrs,
    ) -> Result<Objid, WorldStateError> {
        let id = match id {
            Some(id) => id,
            None => {
                let max = self
                    .tx
                    .increment_sequence(WorldStateSequences::MaximumObject as usize)
                    .await;
                Objid(max as i64)
            }
        };

        let owner = attrs.owner.unwrap_or(id);
        object_relations::upsert_object_value(&self.tx, WorldStateRelation::ObjectOwner, id, owner)
            .await
            .expect("Unable to insert initial owner");

        // Set initial name
        let name = attrs.name.unwrap_or_else(|| format!("Object {}", id));
        object_relations::upsert_object_value(&self.tx, WorldStateRelation::ObjectName, id, name)
            .await
            .expect("Unable to insert initial name");

        // We use our own setters for these, since there's biz-logic attached here...
        if let Some(parent) = attrs.parent {
            self.set_object_parent(id, parent)
                .await
                .expect("Unable to set parent");
        }
        if let Some(location) = attrs.location {
            self.set_object_location(id, location)
                .await
                .expect("Unable to set location");
        }

        let default_object_flags = BitEnum::new();
        object_relations::upsert_object_value(
            &self.tx,
            WorldStateRelation::ObjectFlags,
            id,
            attrs.flags.unwrap_or(default_object_flags),
        )
        .await
        .expect("Unable to insert initial flags");

        // Update the maximum object number if ours is higher than the current one. This is for the
        // textdump case, where our numbers are coming in arbitrarily.
        self.tx
            .update_sequence_max(
                WorldStateSequences::MaximumObject as usize,
                (id.0 + 1) as u64,
            )
            .await;

        Ok(id)
    }

    async fn recycle_object(&self, obj: Objid) -> Result<(), WorldStateError> {
        // First go through and move all objects that are in this object's contents to the
        // to #-1.  It's up to the caller here to execute :exitfunc on all of them before invoking
        // this method.

        let contents = self.get_object_contents(obj).await?;
        for c in contents.iter() {
            self.set_object_location(c, NOTHING).await?;
        }

        // Now reparent all our immediate children to our parent.
        // This should properly move all properties all the way down the chain.
        let parent = self.get_object_parent(obj).await?;
        let children = self.get_object_children(obj).await?;
        for c in children.iter() {
            self.set_object_parent(c, parent).await?;
        }

        // Now we can remove this object from all relevant column relations
        // First the simple ones which are keyed on the object id.
        let oid_relations = [
            WorldStateRelation::ObjectFlags,
            WorldStateRelation::ObjectName,
            WorldStateRelation::ObjectOwner,
            WorldStateRelation::ObjectParent,
            WorldStateRelation::ObjectLocation,
            WorldStateRelation::ObjectVerbs,
        ];
        for rel in oid_relations.iter() {
            let relation = self.tx.relation((*rel).into()).await;
            relation
                .remove_by_domain(obj.0.as_sliceref())
                .await
                .map_err(|e| WorldStateError::DatabaseError(e.to_string()))?;
        }

        let propdefs = self.get_properties(obj).await?;
        for p in propdefs.iter() {
            let key = object_relations::composite_key_for(obj, &p.uuid());
            let relation = self
                .tx
                .relation(WorldStateRelation::ObjectPropertyValue.into())
                .await;
            relation.remove_by_domain(key).await.unwrap_or(());
        }

        let obj_propdefs_rel = self
            .tx
            .relation(WorldStateRelation::ObjectPropDefs.into())
            .await;
        obj_propdefs_rel
            .remove_by_domain(obj.0.as_sliceref())
            .await
            .expect("Unable to delete propdefs");

        Ok(())
    }

    async fn set_object_name(&self, obj: Objid, name: String) -> Result<(), WorldStateError> {
        object_relations::upsert_object_value(&self.tx, WorldStateRelation::ObjectName, obj, name)
            .await
    }

    async fn get_object_parent(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        Ok(
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectParent, obj)
                .await
                .unwrap_or(NOTHING),
        )
    }

    async fn set_object_parent(&self, o: Objid, new_parent: Objid) -> Result<(), WorldStateError> {
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
        //   new_parent and o, but I need to get to the bottom of this and fix it properly.

        let (_shared_ancestor, new_ancestors, old_ancestors) = self
            .closest_common_ancestor_with_ancestors(new_parent, o)
            .await;

        // Remove from _me_ any of the properties defined by any of my ancestors
        if let Some(old_props) = object_relations::get_object_value::<PropDefs>(
            &self.tx,
            WorldStateRelation::ObjectPropDefs,
            o,
        )
        .await
        {
            let mut delort_props = vec![];
            for p in old_props.iter() {
                if old_ancestors.contains(&p.definer()) {
                    delort_props.push(p.uuid());
                    object_relations::delete_composite_if_exists(
                        &self.tx,
                        WorldStateRelation::ObjectPropertyValue,
                        o,
                        p.uuid(),
                    )
                    .await
                    .expect("Unable to delete property");
                }
            }
            let new_props = old_props.with_all_removed(&delort_props);
            object_relations::upsert_object_value(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                o,
                new_props,
            )
            .await
            .expect("Unable to update propdefs");
        }

        // Now walk all-my-children and destroy all the properties whose definer is me or any
        // of my ancestors not shared by the new parent.
        let descendants = self.descendants(o).await?;

        let mut descendant_props = HashMap::new();
        for c in descendants.iter() {
            let mut inherited_props = vec![];
            // Remove the set values.
            if let Some(old_props) = object_relations::get_object_value::<PropDefs>(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                o,
            )
            .await
            {
                for p in old_props.iter() {
                    if old_ancestors.contains(&p.definer()) {
                        inherited_props.push(p.uuid());
                        object_relations::delete_composite_if_exists(
                            &self.tx,
                            WorldStateRelation::ObjectPropertyValue,
                            c,
                            p.uuid(),
                        )
                        .await
                        .expect("Unable to delete property");
                    }
                }
                // And update the property list to not include them
                let new_props = old_props.with_all_removed(&inherited_props);

                // We're not actually going to *set* these yet because we are going to add, later.
                descendant_props.insert(c, new_props);
            }
        }

        // If this is a new object it won't have a parent, old parent this will come up not-found,
        // and if that's the case we can ignore that.
        if let Some(old_parent) = object_relations::get_object_value::<Objid>(
            &self.tx,
            WorldStateRelation::ObjectParent,
            o,
        )
        .await
        {
            if old_parent == new_parent {
                return Ok(());
            }
        };
        object_relations::upsert_object_value(
            &self.tx,
            WorldStateRelation::ObjectParent,
            o,
            new_parent,
        )
        .await
        .expect("Unable to update parent");

        if new_parent == NOTHING {
            return Ok(());
        }

        // Now walk all my new descendants and give them the properties that derive from any
        // ancestors they don't already share.

        // Now collect properties defined on the new ancestors.
        let mut new_props = vec![];
        for a in new_ancestors {
            if let Some(props) = object_relations::get_object_value::<PropDefs>(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                a,
            )
            .await
            {
                for p in props.iter() {
                    if p.definer() == a {
                        new_props.push(p.clone())
                    }
                }
            }
        }
        // Then put clear copies on each of the descendants ... and me.
        // This really just means defining the property with no value, which is what we do.
        let descendants = self
            .descendants(o)
            .await
            .expect("Unable to get descendants");
        for c in descendants.iter().chain(std::iter::once(o)) {
            // Check if we have a cached/modified copy from above in descendant_props
            let c_props = match descendant_props.remove(&c) {
                None => object_relations::get_object_value(
                    &self.tx,
                    WorldStateRelation::ObjectPropDefs,
                    c,
                )
                .await
                .unwrap_or_else(PropDefs::empty),
                Some(props) => props,
            };
            let c_props = c_props.with_all_added(&new_props);
            object_relations::upsert_object_value(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                c,
                c_props,
            )
            .await
            .expect("Unable to update propdefs");
        }
        Ok(())
    }

    async fn get_object_children(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        Ok(object_relations::get_object_by_codomain(
            &self.tx,
            WorldStateRelation::ObjectParent,
            obj,
        )
        .await)
    }

    async fn get_object_location(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        Ok(
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectLocation, obj)
                .await
                .unwrap_or(NOTHING),
        )
    }

    async fn set_object_location(
        &self,
        what: Objid,
        new_location: Objid,
    ) -> Result<(), WorldStateError> {
        // Detect recursive move
        let mut oid = new_location;
        loop {
            if oid == NOTHING {
                break;
            }
            if oid == what {
                return Err(WorldStateError::RecursiveMove(what, new_location));
            }
            let Some(location) = object_relations::get_object_value(
                &self.tx,
                WorldStateRelation::ObjectLocation,
                oid,
            )
            .await
            else {
                break;
            };
            oid = location
        }

        // Get o's location, get its contents, remove o from old contents, put contents back
        // without it. Set new location, get its contents, add o to contents, put contents
        // back with it. Then update the location of o.
        // Get and remove from contents of old location, if we had any.
        if let Some(old_location) = object_relations::get_object_value::<Objid>(
            &self.tx,
            WorldStateRelation::ObjectLocation,
            what,
        )
        .await
        {
            if old_location == new_location {
                return Ok(());
            }
        }

        // Set new location.
        object_relations::upsert_object_value(
            &self.tx,
            WorldStateRelation::ObjectLocation,
            what,
            new_location,
        )
        .await
        .expect("Unable to update location");

        if new_location == NOTHING {
            return Ok(());
        }

        Ok(())
    }

    async fn get_object_contents(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        Ok(object_relations::get_object_by_codomain(
            &self.tx,
            WorldStateRelation::ObjectLocation,
            obj,
        )
        .await)
    }

    async fn get_max_object(&self) -> Result<Objid, WorldStateError> {
        Ok(Objid(
            self.tx
                .sequence_current(WorldStateSequences::MaximumObject as usize)
                .await as i64
                - 1,
        ))
    }

    async fn get_verbs(&self, obj: Objid) -> Result<VerbDefs, WorldStateError> {
        Ok(
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectVerbs, obj)
                .await
                .unwrap_or(VerbDefs::empty()),
        )
    }

    async fn get_verb_binary(&self, obj: Objid, uuid: Uuid) -> Result<Vec<u8>, WorldStateError> {
        object_relations::get_composite_value(&self.tx, WorldStateRelation::VerbProgram, obj, uuid)
            .await
            .ok_or_else(|| WorldStateError::VerbNotFound(obj, format!("{}", uuid)))
    }

    async fn get_verb_by_name(&self, obj: Objid, name: String) -> Result<VerbDef, WorldStateError> {
        let verbdefs: VerbDefs =
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectVerbs, obj)
                .await
                .ok_or_else(|| WorldStateError::VerbNotFound(obj, name.clone()))?;
        Ok(verbdefs
            .find_named(name.as_str())
            .first()
            .ok_or(WorldStateError::VerbNotFound(obj, name))?
            .clone())
    }

    async fn get_verb_by_index(
        &self,
        obj: Objid,
        index: usize,
    ) -> Result<VerbDef, WorldStateError> {
        let verbs = self.get_verbs(obj).await?;
        if index >= verbs.len() {
            return Err(WorldStateError::VerbNotFound(obj, format!("{}", index)));
        }
        verbs
            .iter()
            .nth(index)
            .ok_or_else(|| WorldStateError::VerbNotFound(obj, format!("{}", index)))
    }

    async fn resolve_verb(
        &self,
        obj: Objid,
        name: String,
        argspec: Option<VerbArgsSpec>,
    ) -> Result<VerbDef, WorldStateError> {
        let mut search_o = obj;
        loop {
            if let Some(verbdefs) = object_relations::get_object_value::<VerbDefs>(
                &self.tx,
                WorldStateRelation::ObjectVerbs,
                search_o,
            )
            .await
            {
                // If we found the verb, return it.
                let name_matches = verbdefs.find_named(name.as_str());
                for verb in name_matches {
                    match argspec {
                        Some(argspec) => {
                            if verb.args().matches(&argspec) {
                                return Ok(verb.clone());
                            }
                        }
                        None => {
                            return Ok(verb.clone());
                        }
                    }
                }
            }
            // Otherwise, find our parent.  If it's, then set o to it and continue unless we've
            // hit the end of the chain.
            search_o = match object_relations::get_object_value::<Objid>(
                &self.tx,
                WorldStateRelation::ObjectParent,
                search_o,
            )
            .await
            {
                Some(NOTHING) | None => {
                    break;
                }
                Some(parent) => parent,
            };
        }
        Err(WorldStateError::VerbNotFound(obj, name))
    }

    async fn update_verb(
        &self,
        obj: Objid,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let Some(verbdefs): Option<VerbDefs> =
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectVerbs, obj)
                .await
        else {
            return Err(WorldStateError::VerbNotFound(obj, format!("{}", uuid)));
        };

        let Some(verbdefs) = verbdefs.with_updated(uuid, |ov| {
            let names = match &verb_attrs.names {
                None => ov.names(),
                Some(new_names) => new_names.iter().map(|n| n.as_str()).collect::<Vec<&str>>(),
            };
            VerbDef::new(
                ov.uuid(),
                ov.location(),
                verb_attrs.owner.unwrap_or(ov.owner()),
                &names,
                verb_attrs.flags.unwrap_or(ov.flags()),
                verb_attrs.binary_type.unwrap_or(ov.binary_type()),
                verb_attrs.args_spec.unwrap_or(ov.args()),
            )
        }) else {
            return Err(WorldStateError::VerbNotFound(obj, format!("{}", uuid)));
        };

        object_relations::upsert_object_value(
            &self.tx,
            WorldStateRelation::ObjectVerbs,
            obj,
            verbdefs,
        )
        .await?;

        if verb_attrs.binary.is_some() {
            object_relations::upsert_obj_uuid_value(
                &self.tx,
                WorldStateRelation::VerbProgram,
                obj,
                uuid,
                verb_attrs.binary.unwrap(),
            )
            .await?;
        }
        Ok(())
    }

    async fn add_object_verb(
        &self,
        oid: Objid,
        owner: Objid,
        names: Vec<String>,
        binary: Vec<u8>,
        binary_type: BinaryType,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), WorldStateError> {
        let verbdefs =
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectVerbs, oid)
                .await
                .unwrap_or(VerbDefs::empty());

        let uuid = Uuid::new_v4();
        let verbdef = VerbDef::new(
            uuid,
            oid,
            owner,
            &names.iter().map(|n| n.as_str()).collect::<Vec<&str>>(),
            flags,
            binary_type,
            args,
        );

        let verbdefs = verbdefs.with_added(verbdef);

        object_relations::upsert_object_value(
            &self.tx,
            WorldStateRelation::ObjectVerbs,
            oid,
            verbdefs,
        )
        .await?;
        object_relations::upsert_obj_uuid_value(
            &self.tx,
            WorldStateRelation::VerbProgram,
            oid,
            uuid,
            binary,
        )
        .await?;

        Ok(())
    }

    async fn delete_verb(&self, location: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let verbdefs: VerbDefs =
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectVerbs, location)
                .await
                .ok_or_else(|| WorldStateError::VerbNotFound(location, format!("{}", uuid)))?;

        let verbdefs = verbdefs
            .with_removed(uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(location, format!("{}", uuid)))?;

        object_relations::upsert_object_value(
            &self.tx,
            WorldStateRelation::ObjectVerbs,
            location,
            verbdefs,
        )
        .await?;

        let rel = self
            .tx
            .relation(WorldStateRelation::VerbProgram.into())
            .await;
        rel.remove_by_domain(object_relations::composite_key_for(location, &uuid))
            .await
            .expect("Unable to delete verb program");

        Ok(())
    }

    async fn get_properties(&self, obj: Objid) -> Result<PropDefs, WorldStateError> {
        Ok(
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectPropDefs, obj)
                .await
                .unwrap_or(PropDefs::empty()),
        )
    }

    async fn set_property(
        &self,
        obj: Objid,
        uuid: Uuid,
        value: Var,
    ) -> Result<(), WorldStateError> {
        object_relations::upsert_obj_uuid_value(
            &self.tx,
            WorldStateRelation::ObjectPropertyValue,
            obj,
            uuid,
            value,
        )
        .await?;
        Ok(())
    }

    async fn define_property(
        &self,
        definer: Objid,
        location: Objid,
        name: String,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<Uuid, WorldStateError> {
        let descendants = self.descendants(location).await?;
        let locations = ObjSet::from(&[location]).with_concatenated(descendants);

        // Generate a new property ID. This will get shared all the way down the pipe.
        // But the key for the actual value is always composite of oid,uuid
        let u = Uuid::new_v4();

        for location in locations.iter() {
            let props = object_relations::get_object_value(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                location,
            )
            .await
            .unwrap_or(PropDefs::empty());

            // Verify we don't already have a property with this name. If we do, return an error.
            if props.find_first_named(name.as_str()).is_some() {
                return Err(WorldStateError::DuplicatePropertyDefinition(location, name));
            }

            let prop = PropDef::new(u, definer, location, name.as_str(), perms, owner);
            object_relations::upsert_object_value(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                location,
                props.with_added(prop),
            )
            .await
            .expect("Unable to set property definition")
        }
        // If we have an initial value, set it.
        if let Some(value) = value {
            object_relations::upsert_obj_uuid_value(
                &self.tx,
                WorldStateRelation::ObjectPropertyValue,
                definer,
                u,
                value,
            )
            .await
            .expect("Unable to set initial property value")
        }

        Ok(u)
    }

    async fn update_property_definition(
        &self,
        obj: Objid,
        uuid: Uuid,
        new_owner: Option<Objid>,
        new_flags: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
    ) -> Result<(), WorldStateError> {
        let props =
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectPropDefs, obj)
                .await
                .unwrap_or(PropDefs::empty());

        let Some(props) = props.with_updated(uuid, |p| {
            let name = match &new_name {
                None => p.name(),
                Some(s) => s.as_str(),
            };

            PropDef::new(
                p.uuid(),
                p.definer(),
                p.location(),
                name,
                new_flags.unwrap_or_else(|| p.flags()),
                new_owner.unwrap_or_else(|| p.owner()),
            )
        }) else {
            return Err(WorldStateError::PropertyNotFound(obj, format!("{}", uuid)));
        };

        object_relations::upsert_object_value(
            &self.tx,
            WorldStateRelation::ObjectPropDefs,
            obj,
            props,
        )
        .await?;

        Ok(())
    }

    async fn clear_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let key = object_relations::composite_key_for(obj, &uuid);
        let rel = self
            .tx
            .relation(WorldStateRelation::ObjectPropertyValue.into())
            .await;
        match rel.remove_by_domain(key).await {
            Ok(_) => return Ok(()),
            Err(TupleError::NotFound) => return Ok(()),
            Err(e) => {
                panic!("Unexpected error: {:?}", e)
            }
        }
    }

    async fn delete_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        // delete propdef from self and all descendants
        let descendants = self.descendants(obj).await?;
        let locations = ObjSet::from(&[obj]).with_concatenated(descendants);
        for location in locations.iter() {
            let props: PropDefs = object_relations::get_object_value(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                location,
            )
            .await
            .expect("Unable to get property definitions");

            let props = props
                .with_removed(uuid)
                .expect("Unable to remove property definition");

            object_relations::upsert_object_value(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                location,
                props,
            )
            .await?;
        }
        Ok(())
    }

    async fn retrieve_property(&self, obj: Objid, uuid: Uuid) -> Result<Var, WorldStateError> {
        object_relations::get_composite_value(
            &self.tx,
            WorldStateRelation::ObjectPropertyValue,
            obj,
            uuid,
        )
        .await
        .ok_or_else(|| WorldStateError::PropertyNotFound(obj, format!("{}", uuid)))
    }

    async fn resolve_property(
        &self,
        obj: Objid,
        name: String,
    ) -> Result<(PropDef, Var), WorldStateError> {
        let propdef = self
            .get_properties(obj)
            .await?
            .find_first_named(name.as_str())
            .ok_or_else(|| WorldStateError::PropertyNotFound(obj, name.clone()))?;

        // Then we're going to resolve the value up the tree, skipping 'clear' (un-found) until we
        // get a value.
        let mut search_obj = obj;
        loop {
            // Look for the value. If we're not 'clear', we can return straight away. that's our thing.
            if let Some(found) = object_relations::get_composite_value::<Var>(
                &self.tx,
                WorldStateRelation::ObjectPropertyValue,
                search_obj,
                propdef.uuid(),
            )
            .await
            {
                return Ok((propdef, found));
            }

            // But if it was clear, we have to continue up the inheritance hierarchy. (But we return
            // the of handle we got, because this is what we want to return for information
            // about permissions, etc.)
            let Some(parent) = object_relations::get_object_value(
                &self.tx,
                WorldStateRelation::ObjectParent,
                search_obj,
            )
            .await
            else {
                // If we hit the end of the chain, we're done.
                break;
            };

            if parent == NOTHING {
                // This is an odd one, clear all the way up. so our value will end up being
                // NONE, I guess.
                break;
            }
            search_obj = parent;
        }
        Ok((propdef, v_none()))
    }

    async fn object_valid(&self, obj: Objid) -> Result<bool, WorldStateError> {
        let ov: Option<Objid> =
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectOwner, obj)
                .await;
        Ok(ov.is_some())
    }

    async fn commit(&self) -> Result<CommitResult, WorldStateError> {
        match self.tx.commit().await {
            Ok(_) => Ok(CommitResult::Success),
            Err(CommitError::TupleVersionConflict) => Ok(CommitResult::ConflictRetry),
            Err(CommitError::RelationContentionConflict) => {
                warn!("Contention conflict; too many concurrent writes on the same relation(s) after retries.");
                Ok(CommitResult::ConflictRetry)
            }
        }
    }

    async fn rollback(&self) -> Result<(), WorldStateError> {
        match self.tx.rollback().await {
            Ok(_) => Ok(()),
            Err(e) => Err(WorldStateError::DatabaseError(e.to_string())),
        }
    }
}

impl RelBoxTransaction {
    pub fn new(db: Arc<RelBox>) -> Self {
        let tx = db.start_tx();
        Self { tx }
    }
    pub(crate) async fn descendants(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        let children = object_relations::get_object_by_codomain(
            &self.tx,
            WorldStateRelation::ObjectParent,
            obj,
        )
        .await;

        let mut descendants = vec![];
        let mut queue: VecDeque<_> = children.iter().collect();
        while let Some(o) = queue.pop_front() {
            descendants.push(o);
            let children = object_relations::get_object_by_codomain(
                &self.tx,
                WorldStateRelation::ObjectParent,
                o,
            )
            .await;
            queue.extend(children.iter());
        }

        Ok(ObjSet::from(&descendants))
    }
    async fn closest_common_ancestor_with_ancestors(
        &self,
        a: Objid,
        b: Objid,
    ) -> (Option<Objid>, HashSet<Objid>, HashSet<Objid>) {
        let mut ancestors_a = HashSet::new();
        let mut search_a = a;

        let mut ancestors_b = HashSet::new();
        let mut search_b = b;

        loop {
            if search_a == NOTHING && search_b == NOTHING {
                return (None, ancestors_a, ancestors_b); // No common ancestor found
            }

            if ancestors_b.contains(&search_a) {
                return (Some(search_a), ancestors_a, ancestors_b); // Common ancestor found
            }

            if ancestors_a.contains(&search_b) {
                return (Some(search_b), ancestors_a, ancestors_b); // Common ancestor found
            }

            if search_a != NOTHING {
                ancestors_a.insert(search_a);
                let parent = object_relations::get_object_value(
                    &self.tx,
                    WorldStateRelation::ObjectParent,
                    search_a,
                )
                .await
                .unwrap_or(NOTHING);
                search_a = parent;
            }

            if search_b != NOTHING {
                ancestors_b.insert(search_b);
                let parent = object_relations::get_object_value(
                    &self.tx,
                    WorldStateRelation::ObjectParent,
                    search_b,
                )
                .await
                .unwrap_or(NOTHING);
                search_b = parent;
            }
        }
    }
}

impl Database for RelBoxWorldState {
    fn loader_client(&mut self) -> Result<Box<dyn LoaderInterface>, WorldStateError> {
        let tx = RelBoxTransaction::new(self.db.clone());
        Ok(Box::new(DbTxWorldState { tx: Box::new(tx) }))
    }

    fn world_state_source(self: Box<Self>) -> Result<Arc<dyn WorldStateSource>, WorldStateError> {
        Ok(Arc::new(*self))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use strum::{EnumCount, IntoEnumIterator};

    use moor_values::model::defset::{HasUuid, Named};
    use moor_values::model::objects::ObjAttrs;
    use moor_values::model::objset::ObjSet;
    use moor_values::model::r#match::VerbArgsSpec;
    use moor_values::model::verbs::{BinaryType, VerbAttrs};
    use moor_values::model::{CommitResult, WorldStateError};
    use moor_values::util::bitenum::BitEnum;
    use moor_values::var::objid::Objid;
    use moor_values::var::v_str;
    use moor_values::NOTHING;

    use crate::db_tx::DbTransaction;
    use crate::odb::object_relations::{WorldStateRelation, WorldStateSequences};
    use crate::odb::rb_worldstate::RelBoxTransaction;
    use crate::rdb::{RelBox, RelationInfo};

    async fn test_db() -> Arc<RelBox> {
        let mut relations: Vec<RelationInfo> = WorldStateRelation::iter()
            .map(|wsr| {
                RelationInfo {
                    name: wsr.to_string(),
                    domain_type_id: 0, /* tbd */
                    codomain_type_id: 0,
                    secondary_indexed: false,
                }
            })
            .collect();
        relations[WorldStateRelation::ObjectParent as usize].secondary_indexed = true;
        relations[WorldStateRelation::ObjectLocation as usize].secondary_indexed = true;

        RelBox::new(1 << 24, None, &relations, WorldStateSequences::COUNT).await
    }

    #[tokio::test]
    async fn test_create_object() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db.clone());
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
            .await
            .unwrap();
        assert_eq!(oid, Objid(0));
        assert!(tx.object_valid(oid).await.unwrap());
        assert_eq!(tx.get_object_owner(oid).await.unwrap(), NOTHING);
        assert_eq!(tx.get_object_parent(oid).await.unwrap(), NOTHING);
        assert_eq!(tx.get_object_location(oid).await.unwrap(), NOTHING);
        assert_eq!(tx.get_object_name(oid).await.unwrap(), "test");
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));

        // Verify existence in a new transaction.
        let tx = RelBoxTransaction::new(db);
        assert!(tx.object_valid(oid).await.unwrap());
        assert_eq!(tx.get_object_owner(oid).await.unwrap(), NOTHING);
    }

    #[tokio::test]
    async fn test_create_object_fixed_id() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db);
        // Force at 1.
        let oid = tx
            .create_object(Some(Objid(1)), ObjAttrs::default())
            .await
            .unwrap();
        assert_eq!(oid, Objid(1));
        // Now verify the next will be 2.
        let oid2 = tx.create_object(None, ObjAttrs::default()).await.unwrap();
        assert_eq!(oid2, Objid(2));
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));
    }

    #[tokio::test]
    async fn test_parent_children() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db);

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
            .await
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
            .await
            .unwrap();

        assert_eq!(tx.get_object_parent(b).await.unwrap(), a);
        assert!(tx
            .get_object_children(a)
            .await
            .unwrap()
            .is_same(ObjSet::from(&[b])));

        assert_eq!(tx.get_object_parent(a).await.unwrap(), NOTHING);
        assert_eq!(tx.get_object_children(b).await.unwrap(), ObjSet::empty());

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
            .await
            .unwrap();

        assert_eq!(tx.get_object_parent(c).await.unwrap(), a);
        assert!(tx
            .get_object_children(a)
            .await
            .unwrap()
            .is_same(ObjSet::from(&[b, c])));

        assert_eq!(tx.get_object_parent(a).await.unwrap(), NOTHING);
        assert_eq!(tx.get_object_children(b).await.unwrap(), ObjSet::empty());

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
            .await
            .unwrap();

        tx.set_object_parent(b, d).await.unwrap();
        assert_eq!(tx.get_object_parent(b).await.unwrap(), d);
        assert!(tx
            .get_object_children(a)
            .await
            .unwrap()
            .is_same(ObjSet::from(&[c])));
        assert!(tx
            .get_object_children(d)
            .await
            .unwrap()
            .is_same(ObjSet::from(&[b])));
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));
    }

    #[tokio::test]
    async fn test_descendants() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db);

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
            .await
            .unwrap();
        assert_eq!(a, Objid(0));

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
            .await
            .unwrap();
        assert_eq!(b, Objid(1));

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
            .await
            .unwrap();
        assert_eq!(c, Objid(2));

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
            .await
            .unwrap();
        assert_eq!(d, Objid(3));

        assert!(tx
            .descendants(a)
            .await
            .unwrap()
            .is_same(ObjSet::from(&[b, c, d])));
        assert_eq!(tx.descendants(b).await.unwrap(), ObjSet::empty());
        assert_eq!(tx.descendants(c).await.unwrap(), ObjSet::from(&[d]));

        // Now reparent d to b
        tx.set_object_parent(d, b).await.unwrap();
        assert!(tx
            .get_object_children(a)
            .await
            .unwrap()
            .is_same(ObjSet::from(&[b, c])));
        assert_eq!(tx.get_object_children(b).await.unwrap(), ObjSet::from(&[d]));
        assert_eq!(tx.get_object_children(c).await.unwrap(), ObjSet::empty());
        assert!(tx
            .descendants(a)
            .await
            .unwrap()
            .is_same(ObjSet::from(&[b, c, d])));
        assert_eq!(tx.descendants(b).await.unwrap(), ObjSet::from(&[d]));
        assert_eq!(tx.descendants(c).await.unwrap(), ObjSet::empty());
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));
    }

    #[tokio::test]
    async fn test_location_contents() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db.clone());

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
            .await
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
            .await
            .unwrap();

        assert_eq!(tx.get_object_location(b).await.unwrap(), a);
        assert_eq!(tx.get_object_contents(a).await.unwrap(), ObjSet::from(&[b]));

        assert_eq!(tx.get_object_location(a).await.unwrap(), NOTHING);
        assert_eq!(tx.get_object_contents(b).await.unwrap(), ObjSet::empty());

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
            .await
            .unwrap();

        tx.set_object_location(b, c).await.unwrap();
        assert_eq!(tx.get_object_location(b).await.unwrap(), c);
        assert_eq!(tx.get_object_contents(a).await.unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(c).await.unwrap(), ObjSet::from(&[b]));

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
            .await
            .unwrap();
        tx.set_object_location(d, c).await.unwrap();
        assert!(tx
            .get_object_contents(c)
            .await
            .unwrap()
            .is_same(ObjSet::from(&[b, d])));
        assert_eq!(tx.get_object_location(d).await.unwrap(), c);

        tx.set_object_location(a, c).await.unwrap();
        assert!(tx
            .get_object_contents(c)
            .await
            .unwrap()
            .is_same(ObjSet::from(&[b, d, a])));
        assert_eq!(tx.get_object_location(a).await.unwrap(), c);

        // Validate recursive move detection.
        match tx.set_object_location(c, b).await.err() {
            Some(WorldStateError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // Move b one level deeper, and then check recursive move detection again.
        tx.set_object_location(b, d).await.unwrap();
        match tx.set_object_location(c, b).await.err() {
            Some(WorldStateError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // The other way around, d to c should be fine.
        tx.set_object_location(d, c).await.unwrap();
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));
    }

    /// Test data integrity of object moves between commits.
    #[tokio::test]
    async fn test_object_move_commits() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db.clone());

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
            .await
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
            .await
            .unwrap();

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
            .await
            .unwrap();

        tx.set_object_location(b, a).await.unwrap();
        tx.set_object_location(c, a).await.unwrap();
        assert_eq!(tx.get_object_location(b).await.unwrap(), a);
        assert_eq!(tx.get_object_location(c).await.unwrap(), a);
        assert!(tx
            .get_object_contents(a)
            .await
            .unwrap()
            .is_same(ObjSet::from(&[b, c])));
        assert_eq!(tx.get_object_contents(b).await.unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(c).await.unwrap(), ObjSet::empty());

        assert_eq!(tx.commit().await, Ok(CommitResult::Success));

        let tx = RelBoxTransaction::new(db.clone());
        assert_eq!(tx.get_object_location(b).await.unwrap(), a);
        assert_eq!(tx.get_object_location(c).await.unwrap(), a);
        let contents = tx
            .get_object_contents(a)
            .await
            .expect("Unable to get contents");
        assert!(
            contents.is_same(ObjSet::from(&[b, c])),
            "Contents of a are not as expected: {:?} vs {:?}",
            contents,
            ObjSet::from(&[b, c])
        );
        assert_eq!(tx.get_object_contents(b).await.unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(c).await.unwrap(), ObjSet::empty());

        tx.set_object_location(b, c).await.unwrap();
        assert_eq!(tx.get_object_location(b).await.unwrap(), c);
        assert_eq!(tx.get_object_location(c).await.unwrap(), a);
        assert_eq!(tx.get_object_contents(a).await.unwrap(), ObjSet::from(&[c]));
        assert_eq!(tx.get_object_contents(b).await.unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(c).await.unwrap(), ObjSet::from(&[b]));
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));

        let tx = RelBoxTransaction::new(db.clone());
        assert_eq!(tx.get_object_location(b).await.unwrap(), c);
        assert_eq!(tx.get_object_location(c).await.unwrap(), a);
        assert_eq!(tx.get_object_contents(a).await.unwrap(), ObjSet::from(&[c]));
        assert_eq!(tx.get_object_contents(b).await.unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(c).await.unwrap(), ObjSet::from(&[b]));
    }

    #[tokio::test]
    async fn test_simple_property() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db);

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
            .await
            .unwrap();

        tx.define_property(
            oid,
            oid,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test")),
        )
        .await
        .unwrap();
        let (prop, v) = tx.resolve_property(oid, "test".into()).await.unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_str("test"));
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));
    }

    /// Regression test for updating-verbs failing.
    #[tokio::test]
    async fn test_verb_add_update() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db.clone());
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
            .await
            .unwrap();
        tx.add_object_verb(
            oid,
            oid,
            vec!["test".into()],
            vec![],
            BinaryType::LambdaMoo18X,
            BitEnum::new(),
            VerbArgsSpec::this_none_this(),
        )
        .await
        .unwrap();
        // resolve the verb to its vh.
        let vh = tx.resolve_verb(oid, "test".into(), None).await.unwrap();
        assert_eq!(vh.names(), vec!["test"]);
        // Verify it's actually on the object when we get verbs.
        let verbs = tx.get_verbs(oid).await.unwrap();
        assert_eq!(verbs.len(), 1);
        assert!(verbs.contains(vh.uuid()));
        // update the verb using its uuid, renaming it.
        tx.update_verb(
            oid,
            vh.uuid(),
            VerbAttrs {
                definer: None,
                owner: None,
                names: Some(vec!["test2".into()]),
                flags: None,
                args_spec: None,
                binary_type: None,
                binary: None,
            },
        )
        .await
        .unwrap();
        // resolve with the new name.
        let vh = tx.resolve_verb(oid, "test2".into(), None).await.unwrap();
        assert_eq!(vh.names(), vec!["test2"]);

        // Now commit, and try to resolve again.
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));
        let tx = RelBoxTransaction::new(db);
        let vh = tx.resolve_verb(oid, "test2".into(), None).await.unwrap();
        assert_eq!(vh.names(), vec!["test2"]);
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));
    }

    #[tokio::test]
    async fn test_transitive_property_resolution() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db);

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
            .await
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
            .await
            .unwrap();

        tx.define_property(
            a,
            a,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .await
        .unwrap();
        let (prop, v) = tx.resolve_property(b, "test".into()).await.unwrap();
        assert_eq!(prop.name(), "test");
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
            .await
            .unwrap();

        tx.set_object_parent(b, c).await.unwrap();

        let result = tx.resolve_property(b, "test".into()).await;
        assert_eq!(
            result.err().unwrap(),
            WorldStateError::PropertyNotFound(b, "test".into())
        );
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));
    }

    #[tokio::test]
    async fn test_transitive_property_resolution_clear_property() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db);

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
            .await
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
            .await
            .unwrap();

        tx.define_property(
            a,
            a,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .await
        .unwrap();
        let (prop, v) = tx.resolve_property(b, "test".into()).await.unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_str("test_value"));

        // Define the property again, but on the object 'b',
        // This should raise an error because the child already *has* this property.
        // MOO will not let this happen. The right way to handle overloading is to set the value
        // on the child.
        let result = tx
            .define_property(a, b, "test".into(), NOTHING, BitEnum::new(), None)
            .await;
        assert!(matches!(
            result,
            Err(WorldStateError::DuplicatePropertyDefinition(_, _))
        ));
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));
    }

    #[tokio::test]
    async fn test_verb_resolve() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db.clone());

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
            .await
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
        .await
        .unwrap();

        assert_eq!(
            tx.resolve_verb(a, "test".into(), None)
                .await
                .unwrap()
                .names(),
            vec!["test"]
        );

        assert_eq!(
            tx.resolve_verb(a, "test".into(), Some(VerbArgsSpec::this_none_this()))
                .await
                .unwrap()
                .names(),
            vec!["test"]
        );

        let v_uuid = tx
            .resolve_verb(a, "test".into(), None)
            .await
            .unwrap()
            .uuid();
        assert_eq!(tx.get_verb_binary(a, v_uuid).await.unwrap(), vec![]);

        // Add a second verb with a different name
        tx.add_object_verb(
            a,
            a,
            vec!["test2".into()],
            vec![],
            BinaryType::LambdaMoo18X,
            BitEnum::new(),
            VerbArgsSpec::this_none_this(),
        )
        .await
        .unwrap();

        // Verify we can get it
        assert_eq!(
            tx.resolve_verb(a, "test2".into(), None)
                .await
                .unwrap()
                .names(),
            vec!["test2"]
        );
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));

        // Verify existence in a new transaction.
        let tx = RelBoxTransaction::new(db);
        assert_eq!(
            tx.resolve_verb(a, "test".into(), None)
                .await
                .unwrap()
                .names(),
            vec!["test"]
        );
        assert_eq!(
            tx.resolve_verb(a, "test2".into(), None)
                .await
                .unwrap()
                .names(),
            vec!["test2"]
        );
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));
    }

    #[tokio::test]
    async fn test_verb_resolve_inherited() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db);

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
            .await
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
            .await
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
        .await
        .unwrap();

        assert_eq!(
            tx.resolve_verb(b, "test".into(), None)
                .await
                .unwrap()
                .names(),
            vec!["test"]
        );

        assert_eq!(
            tx.resolve_verb(b, "test".into(), Some(VerbArgsSpec::this_none_this()))
                .await
                .unwrap()
                .names(),
            vec!["test"]
        );

        let v_uuid = tx
            .resolve_verb(b, "test".into(), None)
            .await
            .unwrap()
            .uuid();
        assert_eq!(tx.get_verb_binary(a, v_uuid).await.unwrap(), vec![]);
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));
    }

    #[tokio::test]
    async fn test_verb_resolve_wildcard() {
        let db = test_db().await;
        let tx = RelBoxTransaction::new(db);
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
            .await
            .unwrap();

        let verb_names = vec!["dname*c", "iname*c"];
        tx.add_object_verb(
            a,
            a,
            verb_names.iter().map(|s| s.to_string()).collect(),
            vec![],
            BinaryType::LambdaMoo18X,
            BitEnum::new(),
            VerbArgsSpec::this_none_this(),
        )
        .await
        .unwrap();

        assert_eq!(
            tx.resolve_verb(a, "dname".into(), None)
                .await
                .unwrap()
                .names(),
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(a, "dnamec".into(), None)
                .await
                .unwrap()
                .names(),
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(a, "iname".into(), None)
                .await
                .unwrap()
                .names(),
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(a, "inamec".into(), None)
                .await
                .unwrap()
                .names(),
            verb_names
        );
        assert_eq!(tx.commit().await, Ok(CommitResult::Success));
    }
}
