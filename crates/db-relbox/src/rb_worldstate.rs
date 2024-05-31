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
use std::rc::Rc;
use std::sync::Arc;

use moor_db::Database;
use strum::{EnumCount, IntoEnumIterator};
use tracing::warn;
use uuid::Uuid;

use moor_values::model::PropFlag;
use moor_values::model::VerbArgsSpec;
use moor_values::model::{BinaryType, VerbAttrs, VerbFlag};
use moor_values::model::{CommitResult, WorldStateError};
use moor_values::model::{HasUuid, Named};
use moor_values::model::{ObjAttrs, ObjFlag};
use moor_values::model::{ObjSet, PropPerms, ValSet};
use moor_values::model::{PropDef, PropDefs};
use moor_values::model::{VerbDef, VerbDefs};
use moor_values::model::{WorldState, WorldStateSource};
use moor_values::util::BitEnum;
use moor_values::var::Var;
use moor_values::var::{v_none, Objid};
use moor_values::{AsByteBuffer, NOTHING, SYSTEM_OBJECT};

use crate::object_relations;
use crate::object_relations::{
    encode_oid, get_all_object_keys_matching, WorldStateRelation, WorldStateSequences,
};
use moor_db::db_worldstate::DbTxWorldState;
use moor_db::loader::LoaderInterface;
use moor_db::worldstate_transaction::WorldStateTransaction;
use relbox::{relation_info_for, RelationError};
use relbox::{CommitError, Transaction};
use relbox::{RelBox, RelationInfo};

/// An implementation of `WorldState` / `WorldStateSource` that uses the relbox as its backing
pub struct RelBoxWorldState {
    db: Arc<RelBox>,
}

impl RelBoxWorldState {
    pub fn open(path: Option<PathBuf>, memory_size: usize) -> (Self, bool) {
        let relations: Vec<RelationInfo> =
            WorldStateRelation::iter().map(relation_info_for).collect();

        let db = RelBox::new(memory_size, path, &relations, WorldStateSequences::COUNT);

        // Check the db for sys (#0) object to see if this is a fresh DB or not.
        let fresh_db = {
            let canonical = db.copy_canonical();
            canonical[WorldStateRelation::ObjectParent as usize]
                .seek_by_domain(encode_oid(SYSTEM_OBJECT))
                .expect("Could not seek for freshness check on DB")
                .is_empty()
        };
        (Self { db }, fresh_db)
    }
}

impl WorldStateSource for RelBoxWorldState {
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let tx = RelBoxTransaction::new(self.db.clone());
        Ok(Box::new(DbTxWorldState { tx: Box::new(tx) }))
    }

    fn checkpoint(&self) -> Result<(), WorldStateError> {
        // noop
        Ok(())
    }
}

pub struct RelBoxTransaction {
    tx: Transaction,
}

impl WorldStateTransaction for RelBoxTransaction {
    fn object_valid(&self, obj: Objid) -> Result<bool, WorldStateError> {
        let ov: Option<Objid> =
            object_relations::get_object_object(&self.tx, WorldStateRelation::ObjectOwner, obj);
        Ok(ov.is_some())
    }

    fn ancestors(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        let mut ancestors = vec![];
        let mut search = obj;
        loop {
            if search == NOTHING {
                break;
            }
            ancestors.push(search);
            let parent = object_relations::get_object_object(
                &self.tx,
                WorldStateRelation::ObjectParent,
                search,
            )
            .unwrap_or(NOTHING);
            search = parent;
        }
        Ok(ObjSet::from_items(&ancestors))
    }

    fn get_objects(&self) -> Result<ObjSet, WorldStateError> {
        get_all_object_keys_matching(
            &self.tx,
            WorldStateRelation::ObjectFlags,
            |_, _: BitEnum<ObjFlag>| true,
        )
    }

    fn get_object_flags(&self, obj: Objid) -> Result<BitEnum<ObjFlag>, WorldStateError> {
        object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectFlags, obj)
            .ok_or(WorldStateError::ObjectNotFound(obj))
    }

    fn get_players(&self) -> Result<ObjSet, WorldStateError> {
        // TODO: Improve get_players retrieval in world state
        //   this is going to be not-at-all performant in the long run, and we'll need a way to
        //   cache this or index it better
        get_all_object_keys_matching(
            &self.tx,
            WorldStateRelation::ObjectFlags,
            |_, flags: BitEnum<ObjFlag>| flags.contains(ObjFlag::User),
        )
    }

    fn get_max_object(&self) -> Result<Objid, WorldStateError> {
        Ok(Objid(
            self.tx
                .sequence_current(WorldStateSequences::MaximumObject as usize) as i64
                - 1,
        ))
    }

    fn get_object_owner(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        object_relations::get_object_object(&self.tx, WorldStateRelation::ObjectOwner, obj)
            .ok_or(WorldStateError::ObjectNotFound(obj))
    }

    fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), WorldStateError> {
        object_relations::upsert_object_object(
            &self.tx,
            WorldStateRelation::ObjectOwner,
            obj,
            owner,
        )
    }

    fn set_object_flags(&self, obj: Objid, flags: BitEnum<ObjFlag>) -> Result<(), WorldStateError> {
        object_relations::upsert_object_value(&self.tx, WorldStateRelation::ObjectFlags, obj, flags)
    }

    fn get_object_name(&self, obj: Objid) -> Result<String, WorldStateError> {
        Ok(
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectName, obj)
                .unwrap_or("".to_string()),
        )
    }

    fn set_object_name(&self, obj: Objid, name: String) -> Result<(), WorldStateError> {
        object_relations::upsert_object_value(&self.tx, WorldStateRelation::ObjectName, obj, name)
    }

    fn create_object(&self, id: Option<Objid>, attrs: ObjAttrs) -> Result<Objid, WorldStateError> {
        let id = match id {
            Some(id) => id,
            None => {
                let max = self
                    .tx
                    .increment_sequence(WorldStateSequences::MaximumObject as usize);
                Objid(max as i64)
            }
        };

        let owner = attrs.owner().unwrap_or(id);
        object_relations::upsert_object_object(
            &self.tx,
            WorldStateRelation::ObjectOwner,
            id,
            owner,
        )
        .expect("Unable to insert initial owner");

        // Set initial name if provided.
        if let Some(name) = attrs.name() {
            object_relations::upsert_object_value(
                &self.tx,
                WorldStateRelation::ObjectName,
                id,
                name,
            )
            .expect("Unable to insert initial name");
        }

        // We use our own setters for these, since there's biz-logic attached here...
        if let Some(parent) = attrs.parent() {
            self.set_object_parent(id, parent)
                .expect("Unable to set parent");
        }
        if let Some(location) = attrs.location() {
            self.set_object_location(id, location)
                .expect("Unable to set location");
        }

        object_relations::upsert_object_value(
            &self.tx,
            WorldStateRelation::ObjectFlags,
            id,
            attrs.flags(),
        )
        .expect("Unable to insert initial flags");

        // Update the maximum object number if ours is higher than the current one. This is for the
        // textdump case, where our numbers are coming in arbitrarily.
        self.tx.update_sequence_max(
            WorldStateSequences::MaximumObject as usize,
            (id.0 + 1) as u64,
        );

        Ok(id)
    }

    fn recycle_object(&self, obj: Objid) -> Result<(), WorldStateError> {
        // First go through and move all objects that are in this object's contents to the
        // to #-1.  It's up to the caller here to execute :exitfunc on all of them before invoking
        // this method.

        let contents = self.get_object_contents(obj)?;
        for c in contents.iter() {
            self.set_object_location(c, NOTHING)?;
        }

        // Now reparent all our immediate children to our parent.
        // This should properly move all properties all the way down the chain.
        let parent = self.get_object_parent(obj)?;
        let children = self.get_object_children(obj)?;
        for c in children.iter() {
            self.set_object_parent(c, parent)?;
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
            let relation = self.tx.relation((*rel).into());
            relation
                .remove_by_domain(obj.0.as_sliceref().expect("Unable to encode object id"))
                .map_err(|e| WorldStateError::DatabaseError(e.to_string()))?;
        }

        let propdefs = self.get_properties(obj)?;
        for p in propdefs.iter() {
            let key = object_relations::composite_key_for(obj, &p.uuid());
            let relation = self
                .tx
                .relation(WorldStateRelation::ObjectPropertyValue.into());
            relation.remove_by_domain(key).unwrap_or(());
        }

        let obj_propdefs_rel = self.tx.relation(WorldStateRelation::ObjectPropDefs.into());
        obj_propdefs_rel
            .remove_by_domain(obj.0.as_sliceref().expect("Unable to encode object id"))
            .expect("Unable to delete propdefs");

        Ok(())
    }

    fn get_object_parent(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        Ok(
            object_relations::get_object_object(&self.tx, WorldStateRelation::ObjectParent, obj)
                .unwrap_or(NOTHING),
        )
    }

    fn set_object_parent(&self, o: Objid, new_parent: Objid) -> Result<(), WorldStateError> {
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
        let (_shared_ancestor, new_ancestors, old_ancestors) =
            self.closest_common_ancestor_with_ancestors(new_parent, o);

        // Remove from _me_ any of the properties defined by any of my ancestors
        if let Some(old_props) = object_relations::get_object_value::<PropDefs>(
            &self.tx,
            WorldStateRelation::ObjectPropDefs,
            o,
        ) {
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
            .expect("Unable to update propdefs");
        }

        // Now walk all-my-children and destroy all the properties whose definer is me or any
        // of my ancestors not shared by the new parent.
        let descendants = self.descendants(o)?;

        let mut descendant_props = HashMap::new();
        for c in descendants.iter() {
            let mut inherited_props = vec![];
            // Remove the set values.
            if let Some(old_props) = object_relations::get_object_value::<PropDefs>(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                o,
            ) {
                for p in old_props.iter() {
                    if old_ancestors.contains(&p.definer()) {
                        inherited_props.push(p.uuid());
                        object_relations::delete_composite_if_exists(
                            &self.tx,
                            WorldStateRelation::ObjectPropertyValue,
                            c,
                            p.uuid(),
                        )
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
        if let Some(old_parent) =
            object_relations::get_object_object(&self.tx, WorldStateRelation::ObjectParent, o)
        {
            if old_parent == new_parent {
                return Ok(());
            }
        };
        object_relations::upsert_object_object(
            &self.tx,
            WorldStateRelation::ObjectParent,
            o,
            new_parent,
        )
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
            ) {
                for p in props.iter() {
                    if p.definer() == a {
                        new_props.push(p.clone())
                    }
                }
            }
        }
        // Then put clear copies on each of the descendants ... and me.
        // This really just means defining the property with no value, which is what we do.
        let descendants = self.descendants(o).expect("Unable to get descendants");
        for c in descendants.iter().chain(std::iter::once(o)) {
            // Check if we have a cached/modified copy from above in descendant_props
            let c_props = match descendant_props.remove(&c) {
                None => object_relations::get_object_value(
                    &self.tx,
                    WorldStateRelation::ObjectPropDefs,
                    c,
                )
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
            .expect("Unable to update propdefs");
        }
        Ok(())
    }

    fn get_object_children(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        Ok(object_relations::get_objects_by_object_codomain(
            &self.tx,
            WorldStateRelation::ObjectParent,
            obj,
        ))
    }

    fn get_object_location(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        Ok(
            object_relations::get_object_object(&self.tx, WorldStateRelation::ObjectLocation, obj)
                .unwrap_or(NOTHING),
        )
    }

    fn get_object_contents(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        Ok(object_relations::get_objects_by_object_codomain(
            &self.tx,
            WorldStateRelation::ObjectLocation,
            obj,
        ))
    }

    fn get_object_size_bytes(&self, obj: Objid) -> Result<usize, WorldStateError> {
        let mut size = 0;
        size += object_relations::tuple_size_for_object_domain(
            &self.tx,
            WorldStateRelation::ObjectFlags,
            obj,
        )
        .unwrap_or(0);

        size += object_relations::tuple_size_for_object_domain(
            &self.tx,
            WorldStateRelation::ObjectName,
            obj,
        )
        .unwrap_or(0);

        size += object_relations::tuple_size_for_object_domain(
            &self.tx,
            WorldStateRelation::ObjectOwner,
            obj,
        )
        .unwrap_or(0);

        size += object_relations::tuple_size_for_object_domain(
            &self.tx,
            WorldStateRelation::ObjectParent,
            obj,
        )
        .unwrap_or(0);

        size += object_relations::tuple_size_for_object_domain(
            &self.tx,
            WorldStateRelation::ObjectLocation,
            obj,
        )
        .unwrap_or(0);

        if let Some(verbs) = object_relations::get_object_value::<VerbDefs>(
            &self.tx,
            WorldStateRelation::ObjectVerbs,
            obj,
        ) {
            size += object_relations::tuple_size_for_object_domain(
                &self.tx,
                WorldStateRelation::ObjectVerbs,
                obj,
            )
            .unwrap_or(0);

            for v in verbs.iter() {
                size += object_relations::tuple_size_composite(
                    &self.tx,
                    WorldStateRelation::VerbProgram,
                    obj,
                    v.uuid(),
                )
                .unwrap_or(0)
            }
        };

        if let Some(props) = object_relations::get_object_value::<PropDefs>(
            &self.tx,
            WorldStateRelation::ObjectPropDefs,
            obj,
        ) {
            size += object_relations::tuple_size_for_object_domain(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                obj,
            )
            .unwrap_or(0);

            for p in props.iter() {
                size += object_relations::tuple_size_composite(
                    &self.tx,
                    WorldStateRelation::ObjectPropertyValue,
                    obj,
                    p.uuid(),
                )
                .unwrap_or(0)
            }
        };

        size += object_relations::tuple_size_for_object_codomain(
            &self.tx,
            WorldStateRelation::ObjectLocation,
            obj,
        )
        .unwrap_or(0);

        Ok(size)
    }

    fn set_object_location(&self, what: Objid, new_location: Objid) -> Result<(), WorldStateError> {
        // Detect recursive move
        let mut oid = new_location;
        loop {
            if oid == NOTHING {
                break;
            }
            if oid == what {
                return Err(WorldStateError::RecursiveMove(what, new_location));
            }
            let Some(location) = object_relations::get_object_object(
                &self.tx,
                WorldStateRelation::ObjectLocation,
                oid,
            ) else {
                break;
            };
            oid = location
        }

        // Get o's location, get its contents, remove o from old contents, put contents back
        // without it. Set new location, get its contents, add o to contents, put contents
        // back with it. Then update the location of o.
        // Get and remove from contents of old location, if we had any.
        if let Some(old_location) =
            object_relations::get_object_object(&self.tx, WorldStateRelation::ObjectLocation, what)
        {
            if old_location == new_location {
                return Ok(());
            }
        }

        // Set new location.
        object_relations::upsert_object_object(
            &self.tx,
            WorldStateRelation::ObjectLocation,
            what,
            new_location,
        )
        .expect("Unable to update location");

        if new_location == NOTHING {
            return Ok(());
        }

        Ok(())
    }

    fn get_verbs(&self, obj: Objid) -> Result<VerbDefs, WorldStateError> {
        Ok(
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectVerbs, obj)
                .unwrap_or(VerbDefs::empty()),
        )
    }

    fn get_verb_binary(&self, obj: Objid, uuid: Uuid) -> Result<Vec<u8>, WorldStateError> {
        object_relations::get_composite_value(&self.tx, WorldStateRelation::VerbProgram, obj, uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(obj, format!("{}", uuid)))
    }

    fn get_verb_by_name(&self, obj: Objid, name: String) -> Result<VerbDef, WorldStateError> {
        let verbdefs: VerbDefs =
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectVerbs, obj)
                .ok_or_else(|| WorldStateError::VerbNotFound(obj, name.clone()))?;
        Ok(verbdefs
            .find_named(name.as_str())
            .first()
            .ok_or(WorldStateError::VerbNotFound(obj, name))?
            .clone())
    }

    fn get_verb_by_index(&self, obj: Objid, index: usize) -> Result<VerbDef, WorldStateError> {
        let verbs = self.get_verbs(obj)?;
        if index >= verbs.len() {
            return Err(WorldStateError::VerbNotFound(obj, format!("{}", index)));
        }
        let verbs = verbs.iter().nth(index);
        verbs.ok_or_else(|| WorldStateError::VerbNotFound(obj, format!("{}", index)))
    }

    fn resolve_verb(
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
            ) {
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
            search_o = match object_relations::get_object_object(
                &self.tx,
                WorldStateRelation::ObjectParent,
                search_o,
            ) {
                Some(NOTHING) | None => {
                    break;
                }
                Some(parent) => parent,
            };
        }
        Err(WorldStateError::VerbNotFound(obj, name))
    }

    fn update_verb(
        &self,
        obj: Objid,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let Some(verbdefs): Option<VerbDefs> =
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectVerbs, obj)
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
        )?;

        if verb_attrs.binary.is_some() {
            object_relations::upsert_obj_uuid_value(
                &self.tx,
                WorldStateRelation::VerbProgram,
                obj,
                uuid,
                verb_attrs.binary.unwrap(),
            )?;
        }
        Ok(())
    }

    fn add_object_verb(
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
        )?;
        object_relations::upsert_obj_uuid_value(
            &self.tx,
            WorldStateRelation::VerbProgram,
            oid,
            uuid,
            binary,
        )?;

        Ok(())
    }

    fn delete_verb(&self, location: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let verbdefs: VerbDefs =
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectVerbs, location)
                .ok_or_else(|| WorldStateError::VerbNotFound(location, format!("{}", uuid)))?;

        let verbdefs = verbdefs
            .with_removed(uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(location, format!("{}", uuid)))?;

        object_relations::upsert_object_value(
            &self.tx,
            WorldStateRelation::ObjectVerbs,
            location,
            verbdefs,
        )?;

        let rel = self.tx.relation(WorldStateRelation::VerbProgram.into());
        rel.remove_by_domain(object_relations::composite_key_for(location, &uuid))
            .expect("Unable to delete verb program");

        Ok(())
    }

    fn get_properties(&self, obj: Objid) -> Result<PropDefs, WorldStateError> {
        Ok(
            object_relations::get_object_value(&self.tx, WorldStateRelation::ObjectPropDefs, obj)
                .unwrap_or(PropDefs::empty()),
        )
    }

    fn set_property(&self, obj: Objid, uuid: Uuid, value: Var) -> Result<(), WorldStateError> {
        object_relations::upsert_obj_uuid_value(
            &self.tx,
            WorldStateRelation::ObjectPropertyValue,
            obj,
            uuid,
            value,
        )?;
        Ok(())
    }

    fn define_property(
        &self,
        definer: Objid,
        location: Objid,
        name: String,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<Uuid, WorldStateError> {
        let descendants = self.descendants(location)?;

        // If the property is already defined at us or above or below us, that's a failure.
        let props = match object_relations::get_object_value::<PropDefs>(
            &self.tx,
            WorldStateRelation::ObjectPropDefs,
            location,
        ) {
            None => PropDefs::empty(),
            Some(propdefs) => {
                if propdefs.find_first_named(name.as_str()).is_some() {
                    return Err(WorldStateError::DuplicatePropertyDefinition(location, name));
                }
                propdefs
            }
        };
        let ancestors = self.ancestors(location)?;
        let check_locations = ObjSet::from_items(&[location]).with_concatenated(ancestors);
        for location in check_locations.iter() {
            if let Some(descendant_props) = object_relations::get_object_value::<PropDefs>(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                location,
            ) {
                // Verify we don't already have a property with this name. If we do, return an error.
                if descendant_props.find_first_named(name.as_str()).is_some() {
                    return Err(WorldStateError::DuplicatePropertyDefinition(location, name));
                }
            }
        }

        // Generate a new property ID. This will get shared all the way down the pipe.
        // But the key for the actual value is always composite of oid,uuid
        let u = Uuid::new_v4();

        let prop = PropDef::new(u, definer, location, name.as_str());
        object_relations::upsert_object_value(
            &self.tx,
            WorldStateRelation::ObjectPropDefs,
            location,
            props.with_added(prop),
        )
        .expect("Unable to set property definition");

        // If we have an initial value, set it, but just on ourselves. Descendants start out clear.
        if let Some(value) = value {
            object_relations::upsert_obj_uuid_value(
                &self.tx,
                WorldStateRelation::ObjectPropertyValue,
                location,
                u,
                value,
            )
            .expect("Unable to set property value");
        }

        // Put the initial object owner on ourselves and all our descendants.
        let value_locations = ObjSet::from_items(&[location]).with_concatenated(descendants);
        for location in value_locations.iter() {
            object_relations::upsert_obj_uuid_value(
                &self.tx,
                WorldStateRelation::ObjectPropertyPermissions,
                location,
                u,
                PropPerms::new(owner, perms),
            )
            .expect("Unable to set property owner");
        }

        Ok(u)
    }

    fn update_property_info(
        &self,
        obj: Objid,
        uuid: Uuid,
        new_owner: Option<Objid>,
        new_flags: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
    ) -> Result<(), WorldStateError> {
        if new_owner.is_none() && new_flags.is_none() && new_name.is_none() {
            return Ok(());
        }

        // We only need to update the propdef if there's a new name.
        if let Some(new_name) = new_name {
            let props = object_relations::get_object_value(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                obj,
            )
            .unwrap_or(PropDefs::empty());

            let Some(props) = props.with_updated(uuid, |p| {
                PropDef::new(p.uuid(), p.definer(), p.location(), &new_name)
            }) else {
                return Err(WorldStateError::PropertyNotFound(obj, format!("{}", uuid)));
            };

            object_relations::upsert_object_value(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                obj,
                props,
            )?;
        }

        // If flags or perms updated, do that.
        if new_flags.is_some() || new_owner.is_some() {
            let mut perms: PropPerms = object_relations::get_composite_value(
                &self.tx,
                WorldStateRelation::ObjectPropertyPermissions,
                obj,
                uuid,
            )
            .expect("Unable to get property permissions for update. Integrity error");

            if let Some(new_flags) = new_flags {
                perms = perms.with_flags(new_flags);
            }

            if let Some(new_owner) = new_owner {
                perms = perms.with_owner(new_owner);
            }

            object_relations::upsert_obj_uuid_value(
                &self.tx,
                WorldStateRelation::ObjectPropertyPermissions,
                obj,
                uuid,
                perms,
            )?;
        }

        Ok(())
    }

    fn clear_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let key = object_relations::composite_key_for(obj, &uuid);
        let rel = self
            .tx
            .relation(WorldStateRelation::ObjectPropertyValue.into());
        match rel.remove_by_domain(key) {
            Ok(_) => Ok(()),
            Err(RelationError::TupleNotFound) => Ok(()),
            Err(e) => {
                panic!("Unexpected error: {:?}", e)
            }
        }
    }

    fn delete_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        // delete propdef from self and all descendants
        let descendants = self.descendants(obj)?;
        let locations = ObjSet::from_items(&[obj]).with_concatenated(descendants);
        for location in locations.iter() {
            let props: PropDefs = object_relations::get_object_value(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                location,
            )
            .expect("Unable to get property definitions");

            let props = props
                .with_removed(uuid)
                .expect("Unable to remove property definition");

            object_relations::upsert_object_value(
                &self.tx,
                WorldStateRelation::ObjectPropDefs,
                location,
                props,
            )?;
        }
        Ok(())
    }

    fn retrieve_property(
        &self,
        obj: Objid,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError> {
        let value = object_relations::get_composite_value(
            &self.tx,
            WorldStateRelation::ObjectPropertyValue,
            obj,
            uuid,
        );
        let owner = object_relations::get_composite_value(
            &self.tx,
            WorldStateRelation::ObjectPropertyPermissions,
            obj,
            uuid,
        )
        .expect("Unable to get property owner, coherence problem");
        Ok((value, owner))
    }

    fn retrieve_property_permissions(
        &self,
        obj: Objid,
        uuid: Uuid,
    ) -> Result<PropPerms, WorldStateError> {
        object_relations::get_composite_value(
            &self.tx,
            WorldStateRelation::ObjectPropertyPermissions,
            obj,
            uuid,
        )
        .ok_or(WorldStateError::PropertyNotFound(obj, format!("{}", uuid)))
    }

    fn resolve_property(
        &self,
        obj: Objid,
        name: String,
    ) -> Result<(PropDef, Var, PropPerms, bool), WorldStateError> {
        // Walk up the inheritance tree looking for the property definition.
        let mut search_obj = obj;
        let propdef = loop {
            let propdef = self
                .get_properties(search_obj)?
                .find_first_named(name.as_str());

            if let Some(propdef) = propdef {
                break propdef;
            }

            if let Some(parent) = object_relations::get_object_object(
                &self.tx,
                WorldStateRelation::ObjectParent,
                search_obj,
            ) {
                search_obj = parent;
                continue;
            };

            return Err(WorldStateError::PropertyNotFound(obj, name));
        };

        // Now that we have the propdef, we can look for the value & owner.
        // We should *always* have the owner.
        // But value could be 'clear' in which case we need to look in the parent.
        let owner = object_relations::get_composite_value(
            &self.tx,
            WorldStateRelation::ObjectPropertyPermissions,
            obj,
            propdef.uuid(),
        )
        .expect("Unable to get property owner, coherence problem");

        match object_relations::get_composite_value::<Var>(
            &self.tx,
            WorldStateRelation::ObjectPropertyValue,
            obj,
            propdef.uuid(),
        ) {
            Some(value) => Ok((propdef, value, owner, false)),
            None => {
                let mut search_obj = obj;
                loop {
                    let Some(parent) = object_relations::get_object_object(
                        &self.tx,
                        WorldStateRelation::ObjectParent,
                        search_obj,
                    ) else {
                        break Ok((propdef, v_none(), owner, true));
                    };
                    if parent == NOTHING {
                        break Ok((propdef, v_none(), owner, true));
                    }
                    search_obj = parent;

                    let value = object_relations::get_composite_value(
                        &self.tx,
                        WorldStateRelation::ObjectPropertyValue,
                        search_obj,
                        propdef.uuid(),
                    );
                    if let Some(value) = value {
                        break Ok((propdef, value, owner, true));
                    }
                }
            }
        }
    }

    fn db_usage(&self) -> Result<usize, WorldStateError> {
        Ok(self.tx.db_usage_bytes())
    }

    fn commit(&self) -> Result<CommitResult, WorldStateError> {
        match self.tx.commit() {
            Ok(_) => Ok(CommitResult::Success),
            Err(CommitError::TupleVersionConflict) => Ok(CommitResult::ConflictRetry),
            Err(CommitError::UniqueConstraintViolation) => Ok(CommitResult::ConflictRetry),
            Err(CommitError::RelationContentionConflict) => {
                warn!("Contention conflict; too many concurrent writes on the same relation(s) after retries.");
                Ok(CommitResult::ConflictRetry)
            }
        }
    }

    fn rollback(&self) -> Result<(), WorldStateError> {
        match self.tx.rollback() {
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

    pub(crate) fn descendants(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        let children = object_relations::get_objects_by_object_codomain(
            &self.tx,
            WorldStateRelation::ObjectParent,
            obj,
        );

        let mut descendants = vec![];
        let mut queue: VecDeque<_> = children.iter().collect();
        while let Some(o) = queue.pop_front() {
            descendants.push(o);
            let children = object_relations::get_objects_by_object_codomain(
                &self.tx,
                WorldStateRelation::ObjectParent,
                o,
            );
            queue.extend(children.iter());
        }

        Ok(ObjSet::from_items(&descendants))
    }
    fn closest_common_ancestor_with_ancestors(
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
                let parent = object_relations::get_object_object(
                    &self.tx,
                    WorldStateRelation::ObjectParent,
                    search_a,
                )
                .unwrap_or(NOTHING);
                search_a = parent;
            }

            if search_b != NOTHING {
                ancestors_b.insert(search_b);
                let parent = object_relations::get_object_object(
                    &self.tx,
                    WorldStateRelation::ObjectParent,
                    search_b,
                )
                .unwrap_or(NOTHING);
                search_b = parent;
            }
        }
    }
}

impl Drop for RelBoxTransaction {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            self.tx.rollback().unwrap();
        }
    }
}

impl Database for RelBoxWorldState {
    fn loader_client(self: Arc<Self>) -> Result<Rc<dyn LoaderInterface>, WorldStateError> {
        let tx = RelBoxTransaction::new(self.db.clone());
        Ok(Rc::new(DbTxWorldState { tx: Box::new(tx) }))
    }

    fn world_state_source(self: Arc<Self>) -> Result<Arc<dyn WorldStateSource>, WorldStateError> {
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use strum::{EnumCount, IntoEnumIterator};

    use moor_values::model::ObjSet;
    use moor_values::model::VerbArgsSpec;
    use moor_values::model::{BinaryType, VerbAttrs};
    use moor_values::model::{CommitResult, WorldStateError};
    use moor_values::model::{HasUuid, Named};
    use moor_values::model::{ObjAttrs, PropFlag, ValSet};
    use moor_values::util::BitEnum;
    use moor_values::var::Objid;
    use moor_values::var::{v_int, v_str};
    use moor_values::NOTHING;

    use crate::object_relations::{WorldStateRelation, WorldStateSequences};
    use crate::rb_worldstate::RelBoxTransaction;
    use moor_db::worldstate_transaction::WorldStateTransaction;
    use relbox::{relation_info_for, RelBox, RelationInfo};

    fn test_db() -> Arc<RelBox> {
        let relations: Vec<RelationInfo> =
            WorldStateRelation::iter().map(relation_info_for).collect();

        RelBox::new(1 << 24, None, &relations, WorldStateSequences::COUNT)
    }

    #[test]
    fn test_create_object() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db.clone());
        let oid = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();
        assert_eq!(oid, Objid(0));
        assert!(tx.object_valid(oid).unwrap());
        assert_eq!(tx.get_object_owner(oid).unwrap(), oid);
        assert_eq!(tx.get_object_parent(oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_location(oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_name(oid).unwrap(), "test");
        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        // Verify existence in a new transaction.
        let tx = RelBoxTransaction::new(db);
        assert!(tx.object_valid(oid).unwrap());
        assert_eq!(tx.get_object_owner(oid).unwrap(), oid);
    }

    #[test]
    fn test_create_object_fixed_id() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db);
        // Force at 1.
        let oid = tx
            .create_object(Some(Objid(1)), ObjAttrs::default())
            .unwrap();
        assert_eq!(oid, Objid(1));
        // Now verify the next will be 2.
        let oid2 = tx.create_object(None, ObjAttrs::default()).unwrap();
        assert_eq!(oid2, Objid(2));
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    fn test_parent_children() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db);

        // Single parent/child relationship.
        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();

        assert_eq!(tx.get_object_parent(b).unwrap(), a);
        assert!(tx
            .get_object_children(a)
            .unwrap()
            .is_same(ObjSet::from_items(&[b])));

        assert_eq!(tx.get_object_parent(a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_children(b).unwrap(), ObjSet::empty());

        // Add a second child
        let c = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test3"),
            )
            .unwrap();

        assert_eq!(tx.get_object_parent(c).unwrap(), a);
        assert!(tx
            .get_object_children(a)
            .unwrap()
            .is_same(ObjSet::from_items(&[b, c])));

        assert_eq!(tx.get_object_parent(a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_children(b).unwrap(), ObjSet::empty());

        // Create new obj and reparent one child
        let d = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test4"),
            )
            .unwrap();

        tx.set_object_parent(b, d).unwrap();
        assert_eq!(tx.get_object_parent(b).unwrap(), d);
        assert!(tx
            .get_object_children(a)
            .unwrap()
            .is_same(ObjSet::from_items(&[c])));
        assert!(tx
            .get_object_children(d)
            .unwrap()
            .is_same(ObjSet::from_items(&[b])));
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    fn test_descendants() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db);

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();
        assert_eq!(a, Objid(0));

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();
        assert_eq!(b, Objid(1));

        let c = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test3"),
            )
            .unwrap();
        assert_eq!(c, Objid(2));

        let d = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, c, NOTHING, BitEnum::new(), "test4"),
            )
            .unwrap();
        assert_eq!(d, Objid(3));

        assert!(tx
            .descendants(a)
            .unwrap()
            .is_same(ObjSet::from_items(&[b, c, d])));
        assert_eq!(tx.descendants(b).unwrap(), ObjSet::empty());
        assert_eq!(tx.descendants(c).unwrap(), ObjSet::from_items(&[d]));

        // Now reparent d to b
        tx.set_object_parent(d, b).unwrap();
        assert!(tx
            .get_object_children(a)
            .unwrap()
            .is_same(ObjSet::from_items(&[b, c])));
        assert_eq!(tx.get_object_children(b).unwrap(), ObjSet::from_items(&[d]));
        assert_eq!(tx.get_object_children(c).unwrap(), ObjSet::empty());
        assert!(tx
            .descendants(a)
            .unwrap()
            .is_same(ObjSet::from_items(&[b, c, d])));
        assert_eq!(tx.descendants(b).unwrap(), ObjSet::from_items(&[d]));
        assert_eq!(tx.descendants(c).unwrap(), ObjSet::empty());
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    fn test_location_contents() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db.clone());

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, a, BitEnum::new(), "test2"),
            )
            .unwrap();

        assert_eq!(tx.get_object_location(b).unwrap(), a);
        assert_eq!(tx.get_object_contents(a).unwrap(), ObjSet::from_items(&[b]));

        assert_eq!(tx.get_object_location(a).unwrap(), NOTHING);
        assert_eq!(tx.get_object_contents(b).unwrap(), ObjSet::empty());

        let c = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test3"),
            )
            .unwrap();

        tx.set_object_location(b, c).unwrap();
        assert_eq!(tx.get_object_location(b).unwrap(), c);
        assert_eq!(tx.get_object_contents(a).unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::from_items(&[b]));

        let d = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test4"),
            )
            .unwrap();
        tx.set_object_location(d, c).unwrap();
        assert!(tx
            .get_object_contents(c)
            .unwrap()
            .is_same(ObjSet::from_items(&[b, d])));
        assert_eq!(tx.get_object_location(d).unwrap(), c);

        tx.set_object_location(a, c).unwrap();
        assert!(tx
            .get_object_contents(c)
            .unwrap()
            .is_same(ObjSet::from_items(&[b, d, a])));
        assert_eq!(tx.get_object_location(a).unwrap(), c);

        // Validate recursive move detection.
        match tx.set_object_location(c, b).err() {
            Some(WorldStateError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // Move b one level deeper, and then check recursive move detection again.
        tx.set_object_location(b, d).unwrap();
        match tx.set_object_location(c, b).err() {
            Some(WorldStateError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // The other way around, d to c should be fine.
        tx.set_object_location(d, c).unwrap();
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    /// Test data integrity of object moves between commits.
    #[test]
    fn test_object_move_commits() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db.clone());

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, a, BitEnum::new(), "test2"),
            )
            .unwrap();

        let c = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test3"),
            )
            .unwrap();

        tx.set_object_location(b, a).unwrap();
        tx.set_object_location(c, a).unwrap();
        assert_eq!(tx.get_object_location(b).unwrap(), a);
        assert_eq!(tx.get_object_location(c).unwrap(), a);
        assert!(tx
            .get_object_contents(a)
            .unwrap()
            .is_same(ObjSet::from_items(&[b, c])));
        assert_eq!(tx.get_object_contents(b).unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::empty());

        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        let tx = RelBoxTransaction::new(db.clone());
        assert_eq!(tx.get_object_location(b).unwrap(), a);
        assert_eq!(tx.get_object_location(c).unwrap(), a);
        let contents = tx.get_object_contents(a).expect("Unable to get contents");
        assert!(
            contents.is_same(ObjSet::from_items(&[b, c])),
            "Contents of a are not as expected: {:?} vs {:?}",
            contents,
            ObjSet::from_items(&[b, c])
        );
        assert_eq!(tx.get_object_contents(b).unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::empty());

        tx.set_object_location(b, c).unwrap();
        assert_eq!(tx.get_object_location(b).unwrap(), c);
        assert_eq!(tx.get_object_location(c).unwrap(), a);
        assert_eq!(tx.get_object_contents(a).unwrap(), ObjSet::from_items(&[c]));
        assert_eq!(tx.get_object_contents(b).unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::from_items(&[b]));
        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        let tx = RelBoxTransaction::new(db.clone());
        assert_eq!(tx.get_object_location(c).unwrap(), a);
        assert_eq!(tx.get_object_location(b).unwrap(), c);
        assert_eq!(tx.get_object_contents(a).unwrap(), ObjSet::from_items(&[c]));
        assert_eq!(tx.get_object_contents(b).unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::from_items(&[b]));
    }

    #[test]
    fn test_simple_property() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db);

        let oid = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
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
        let (prop, v, perms, is_clear) = tx.resolve_property(oid, "test".into()).unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_str("test"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(!is_clear);
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    /// Regression test for updating-verbs failing.
    #[test]
    fn test_verb_add_update() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db.clone());
        let oid = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
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
        .unwrap();
        // resolve the verb to its vh.
        let vh = tx.resolve_verb(oid, "test".into(), None).unwrap();
        assert_eq!(vh.names(), vec!["test"]);
        // Verify it's actually on the object when we get verbs.
        let verbs = tx.get_verbs(oid).unwrap();
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
        .unwrap();
        // resolve with the new name.
        let vh = tx.resolve_verb(oid, "test2".into(), None).unwrap();
        assert_eq!(vh.names(), vec!["test2"]);

        // Now commit, and try to resolve again.
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
        let tx = RelBoxTransaction::new(db);
        let vh = tx.resolve_verb(oid, "test2".into(), None).unwrap();
        assert_eq!(vh.names(), vec!["test2"]);
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    fn test_transitive_property_resolution() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db);

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
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
        let (prop, v, perms, is_clear) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        // Verify we *don't* get this property for an unrelated, unhinged object by reparenting b
        // to new parent c.  This should remove the defs for a's properties from b.
        let c = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test3"),
            )
            .unwrap();

        tx.set_object_parent(b, c).unwrap();

        let result = tx.resolve_property(b, "test".into());
        assert_eq!(
            result.err().unwrap(),
            WorldStateError::PropertyNotFound(b, "test".into())
        );
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    fn test_transitive_property_resolution_clear_property() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db);

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
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
        let (prop, v, perms, is_clear) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        // Set the property on the child to a new value.
        tx.set_property(b, prop.uuid(), v_int(666)).unwrap();

        // Verify the new value is present.
        let (prop, v, perms, is_clear) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_int(666));
        assert_eq!(perms.owner(), NOTHING);
        assert!(!is_clear);

        // Now clear, and we should get the old value, but with clear status.
        tx.clear_property(b, prop.uuid()).unwrap();
        let (prop, v, perms, is_clear) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        // Changing flags or owner should have nothing to do with the clarity of the property value.
        tx.update_property_info(
            b,
            prop.uuid(),
            Some(b),
            Some(BitEnum::new_with(PropFlag::Read)),
            None,
        )
        .unwrap();
        let (prop, v, perms, is_clear) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), b);
        assert_eq!(perms.flags(), BitEnum::new_with(PropFlag::Read));
        assert!(is_clear);

        // Setting the value again makes it not clear
        tx.set_property(b, prop.uuid(), v_int(666)).unwrap();
        let (prop, v, perms, is_clear) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_int(666));
        assert_eq!(perms.owner(), b);
        assert_eq!(perms.flags(), BitEnum::new_with(PropFlag::Read));
        assert!(!is_clear);

        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    fn test_rename_property() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db);
        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();

        let uuid = tx
            .define_property(
                a,
                a,
                "test".into(),
                NOTHING,
                BitEnum::new(),
                Some(v_str("test_value")),
            )
            .unwrap();

        // I can update the name on the parent...
        tx.update_property_info(a, uuid, None, None, Some("a_new_name".to_string()))
            .unwrap();

        // And now resolve that new name on the child.
        let (prop, v, perms, is_clear) = tx.resolve_property(b, "a_new_name".into()).unwrap();
        assert_eq!(prop.name(), "a_new_name");
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        // But it's illegal to try to rename it on the child who doesn't define it.
        assert!(tx
            .update_property_info(b, uuid, None, None, Some("a_new_name".to_string()))
            .is_err())
    }

    /// Test regression where parent properties were present via `properties()` on children.
    #[test]
    fn test_regression_properties() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db);

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
            )
            .unwrap();

        // Define 1 property on parent
        tx.define_property(
            a,
            a,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();
        let (prop, v, perms, is_clear) = tx.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_str("test_value"));
        assert_eq!(perms.owner(), NOTHING);
        assert!(is_clear);

        // And another on child
        let child_prop = tx
            .define_property(
                b,
                b,
                "test2".into(),
                NOTHING,
                BitEnum::new(),
                Some(v_str("test_value2")),
            )
            .unwrap();

        let props = tx.get_properties(b).unwrap();

        // Our prop should be there
        assert!(props.find(&child_prop).is_some());

        // Listing the set of properties on the child should include only the child's properties
        assert_eq!(props.len(), 1);
    }

    #[test]
    fn test_verb_resolve() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db.clone());

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
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
            tx.resolve_verb(a, "test".into(), None).unwrap().names(),
            vec!["test"]
        );

        assert_eq!(
            tx.resolve_verb(a, "test".into(), Some(VerbArgsSpec::this_none_this()))
                .unwrap()
                .names(),
            vec!["test"]
        );

        let v_uuid = tx.resolve_verb(a, "test".into(), None).unwrap().uuid();
        assert_eq!(tx.get_verb_binary(a, v_uuid).unwrap(), vec![]);

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
        .unwrap();

        // Verify we can get it
        assert_eq!(
            tx.resolve_verb(a, "test2".into(), None).unwrap().names(),
            vec!["test2"]
        );
        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        // Verify existence in a new transaction.
        let tx = RelBoxTransaction::new(db);
        assert_eq!(
            tx.resolve_verb(a, "test".into(), None).unwrap().names(),
            vec!["test"]
        );
        assert_eq!(
            tx.resolve_verb(a, "test2".into(), None).unwrap().names(),
            vec!["test2"]
        );
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    fn test_verb_resolve_inherited() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db);

        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();

        let b = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, a, NOTHING, BitEnum::new(), "test2"),
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
            tx.resolve_verb(b, "test".into(), None).unwrap().names(),
            vec!["test"]
        );

        assert_eq!(
            tx.resolve_verb(b, "test".into(), Some(VerbArgsSpec::this_none_this()))
                .unwrap()
                .names(),
            vec!["test"]
        );

        let v_uuid = tx.resolve_verb(b, "test".into(), None).unwrap().uuid();
        assert_eq!(tx.get_verb_binary(a, v_uuid).unwrap(), vec![]);
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    fn test_verb_resolve_wildcard() {
        let db = test_db();
        let tx = RelBoxTransaction::new(db);
        let a = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
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
        .unwrap();

        assert_eq!(
            tx.resolve_verb(a, "dname".into(), None).unwrap().names(),
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(a, "dnamec".into(), None).unwrap().names(),
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(a, "iname".into(), None).unwrap().names(),
            verb_names
        );

        assert_eq!(
            tx.resolve_verb(a, "inamec".into(), None).unwrap().names(),
            verb_names
        );
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }
}
