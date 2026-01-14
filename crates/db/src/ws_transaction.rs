// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::{
    CommitSet, Error, ObjAndUUIDHolder, StringHolder,
    db_worldstate::db_counters,
    moor_db::{Caches, SEQUENCE_MAX_OBJECT, WorldStateTransaction},
    provider::fjall_provider::FjallProvider,
    tx_management::{EncodeFor, Relation, RelationTransaction},
};
use byteview::ByteView;
use moor_common::{
    model::{
        CommitResult, HasUuid, Named, ObjAttrs, ObjFlag, ObjSet, ObjectKind, ObjectRef, PropDef,
        PropDefs, PropFlag, PropPerms, ValSet, VerbArgsSpec, VerbAttrs, VerbDef, VerbDefs,
        VerbFlag, WorldStateError,
    },
    util::{BitEnum, PerfTimerGuard},
};
use moor_var::{
    ByteSized, NOTHING, Obj, Symbol, Var, program::ProgramType, v_empty_map, v_map, v_none,
};
use std::collections::HashSet;
use std::{
    collections::VecDeque,
    hash::Hash,
    time::{Duration, Instant},
};
use tracing::warn;
use uuid::Uuid;

type RTx<Domain, Codomain> = RelationTransaction<
    Domain,
    Codomain,
    Relation<Domain, Codomain, FjallProvider<Domain, Codomain>>,
>;

fn upsert<Domain, Codomain>(
    table: &mut RTx<Domain, Codomain>,
    d: Domain,
    c: Codomain,
) -> Result<Option<Codomain>, Error>
where
    Domain: Clone + Eq + Hash + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
    FjallProvider<Domain, Codomain>:
        EncodeFor<Domain, Stored = ByteView> + EncodeFor<Codomain, Stored = ByteView>,
{
    table.upsert(d, c)
}

fn insert_guaranteed_unique<Domain, Codomain>(
    table: &mut RTx<Domain, Codomain>,
    d: Domain,
    c: Codomain,
) -> Result<(), Error>
where
    Domain: Clone + Eq + Hash + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
    FjallProvider<Domain, Codomain>:
        EncodeFor<Domain, Stored = ByteView> + EncodeFor<Codomain, Stored = ByteView>,
{
    table.insert_guaranteed_unique(d, c)
}

impl WorldStateTransaction {
    pub fn object_valid(&self, obj: &Obj) -> Result<bool, WorldStateError> {
        match self.object_flags.has_domain(obj) {
            Ok(b) => Ok(b),
            Err(e) => Err(WorldStateError::DatabaseError(format!(
                "Error getting object flags: {e:?}"
            ))),
        }
    }

    /// Bulk validity check for multiple objects
    /// Returns only the objects that are valid
    pub fn valid_objects<T: Iterator<Item = Obj>>(
        &self,
        objs: T,
    ) -> Result<HashSet<Obj>, WorldStateError> {
        self.object_flags.check_domains(objs).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error checking object flags: {e:?}"))
        })
    }

    pub fn ancestors(&self, obj: &Obj, include_self: bool) -> Result<ObjSet, WorldStateError> {
        // Check ancestry cache first.
        let results_sans_self = match self.ancestry_cache.lookup(obj) {
            Some(hit) => hit,
            None => {
                let mut ancestors = vec![];
                let mut current = *obj;
                loop {
                    match self.object_parent.get(&current) {
                        Ok(Some(parent)) => {
                            current = parent;
                            if current.is_nothing() {
                                break;
                            }
                            ancestors.push(current);
                        }
                        Ok(None) => break,
                        Err(e) => {
                            panic!("Error getting parent: {e:?}");
                        }
                    }
                }
                // Fill in the cache.
                self.ancestry_cache.fill(obj, &ancestors);

                ancestors
            }
        };
        let ancestor_set = if include_self {
            // Chained iter of "obj" + the results
            let chained = std::iter::once(*obj).chain(results_sans_self);
            ObjSet::from_iter(chained)
        } else {
            ObjSet::from_items(&results_sans_self)
        };

        Ok(ancestor_set)
    }

    pub fn get_objects(&self) -> Result<ObjSet, WorldStateError> {
        let objects = self
            .object_flags
            .scan(&|_, _| true)
            .map_err(|e| WorldStateError::DatabaseError(format!("Error getting objects: {e:?}")))?;
        Ok(ObjSet::from_iter(objects.iter().map(|(k, _)| *k)))
    }

    pub fn get_object_flags(&self, obj: &Obj) -> Result<BitEnum<ObjFlag>, WorldStateError> {
        let r = self.object_flags.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object flags: {e:?}"))
        })?;
        Ok(r.unwrap_or_default())
    }

    pub fn get_players(&self) -> Result<ObjSet, WorldStateError> {
        let players = self
            .object_flags
            .scan(&|_, flags| flags.contains(ObjFlag::User))
            .map_err(|e| WorldStateError::DatabaseError(format!("Error getting players: {e:?}")))?;
        Ok(ObjSet::from_iter(players.iter().map(|(k, _)| *k)))
    }

    pub fn get_max_object(&self) -> Result<Obj, WorldStateError> {
        let seq_max = self.get_sequence(SEQUENCE_MAX_OBJECT);

        // Turn to i32, but check bounds against MAX_INT
        let seq_max = if seq_max < i32::MIN as i64 || seq_max > i32::MAX as i64 {
            return Err(WorldStateError::DatabaseError(format!(
                "Maximum object sequence number out of bounds: {seq_max}"
            )));
        } else {
            seq_max as i32
        };

        Ok(Obj::mk_id(seq_max))
    }

    pub fn get_object_owner(&self, obj: &Obj) -> Result<Obj, WorldStateError> {
        let r = self.object_owner.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object owner: {e:?}"))
        })?;

        Ok(r.unwrap_or(NOTHING))
    }

    pub fn set_object_owner(&mut self, obj: &Obj, owner: &Obj) -> Result<(), WorldStateError> {
        self.object_owner.upsert(*obj, *owner).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting object owner: {e:?}"))
        })?;
        self.has_mutations = true;
        Ok(())
    }

    pub fn set_object_flags(
        &mut self,
        obj: &Obj,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError> {
        upsert(&mut self.object_flags, *obj, flags).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting object flags: {e:?}"))
        })?;
        self.has_mutations = true;
        Ok(())
    }

    pub fn get_object_name(&self, obj: &Obj) -> Result<String, WorldStateError> {
        let r = self.object_name.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object name: {e:?}"))
        })?;
        let Some(r) = r else {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(*obj)));
        };
        Ok(r.0)
    }

    pub fn set_object_name(&mut self, obj: &Obj, name: String) -> Result<(), WorldStateError> {
        upsert(&mut self.object_name, *obj, StringHolder(name)).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting object name: {e:?}"))
        })?;
        self.has_mutations = true;
        Ok(())
    }

    pub fn create_object(
        &mut self,
        id_kind: ObjectKind,
        attrs: ObjAttrs,
    ) -> Result<Obj, WorldStateError> {
        let id = match id_kind {
            ObjectKind::Objid(id) => id,
            ObjectKind::NextObjid => {
                let max = self.increment_sequence(SEQUENCE_MAX_OBJECT);
                let max = if max < i32::MIN as i64 || max > i32::MAX as i64 {
                    return Err(WorldStateError::DatabaseError(format!(
                        "Maximum object sequence number out of bounds: {max}"
                    )));
                } else {
                    max as i32
                };
                Obj::mk_id(max)
            }
            ObjectKind::UuObjId => Obj::mk_uuobjid_generated(),
            ObjectKind::Anonymous => Obj::mk_anonymous_generated(),
        };

        let owner = attrs.owner().unwrap_or(id);
        insert_guaranteed_unique(&mut self.object_owner, id, owner)
            .expect("Unable to insert initial owner");

        self.has_mutations = true;

        // Set initial name
        let name = attrs.name().unwrap_or_default();
        insert_guaranteed_unique(&mut self.object_name, id, StringHolder(name))
            .expect("Unable to insert initial name");

        // Set initial parent using optimized method for new objects
        if let Some(parent) = attrs.parent() {
            self.set_initial_object_parent(&id, &parent)
                .expect("Unable to set parent");
        }
        if let Some(location) = attrs.location() {
            self.set_initial_object_location(&id, &location)
                .expect("Unable to set location");
        }

        insert_guaranteed_unique(&mut self.object_flags, id, attrs.flags())
            .expect("Unable to insert initial flags");

        // Update the maximum object number if ours is higher than the current one. This is for the
        // textdump case, where our numbers are coming in arbitrarily.
        // Only do this for objids, not uuobjids or anonymous objects
        if !id.is_uuobjid() && !id.is_anonymous() {
            self.update_sequence_max(SEQUENCE_MAX_OBJECT, id.id().0 as i64);
        }

        Ok(id)
    }

    pub fn recycle_object(&mut self, obj: &Obj) -> Result<(), WorldStateError> {
        // First go through and move all objects that are in this object's contents to the
        // to #-1.  It's up to the caller here to execute :exitfunc on all of them before invoking
        // this method.

        // Get both contents and children BEFORE making any modifications to avoid
        // secondary index confusion during transaction
        let contents = self.get_object_contents(obj)?;
        let parent = self.get_object_parent(obj)?;
        let children = self.get_object_children(obj)?;

        // Move contents to NOTHING
        for c in contents.iter() {
            self.set_object_location(&c, &NOTHING)?;
        }
        self.has_mutations = true;

        // Reparent all children to our parent
        for c in children.iter() {
            self.set_object_parent(&c, &parent)?;
        }

        // Remove parent relationship (children list is automatically updated via secondary index)
        self.object_parent.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error removing parent relationship: {e:?}"))
        })?;

        // Remove location relationship (contents list is automatically updated via secondary index)
        self.object_location.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error removing location relationship: {e:?}"))
        })?;

        // Now we can remove this object from all relevant relations
        // First the simple ones which are keyed on the object id.
        self.object_flags.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object flags: {e:?}"))
        })?;
        self.object_name.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object name: {e:?}"))
        })?;
        // object_children is now derived from object_parent secondary index, no need to delete
        self.object_owner.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object owner: {e:?}"))
        })?;
        self.object_parent.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object parent: {e:?}"))
        })?;
        self.object_location.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object location: {e:?}"))
        })?;
        self.object_verbdefs.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object verbdefs: {e:?}"))
        })?;

        let propdefs = self.get_properties(obj)?;
        for p in propdefs.iter() {
            self.object_propvalues
                .delete(&ObjAndUUIDHolder::new(obj, p.uuid()))
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!("Error deleting property value: {e:?}"))
                })?;
        }

        // We may or may not have propdefs yet...
        self.object_propdefs.delete(obj).ok();

        self.invalidate_verb_cache_for_objects(&[*obj]);
        self.invalidate_prop_cache_for_objects(&[*obj]);
        self.invalidate_ancestry_cache_for_objects(&[*obj]);

        Ok(())
    }

    /// Optimized batch recycling for garbage collection sweep phase.
    /// Reduces transaction overhead and cache flushes compared to individual recycle_object calls.
    pub fn recycle_objects(&mut self, objects: &HashSet<Obj>) -> Result<(), WorldStateError> {
        if objects.is_empty() {
            return Ok(());
        }

        // Pre-collect all relationship data to minimize individual queries
        let mut contents_to_move = Vec::new();
        let mut children_to_reparent = Vec::new();
        let mut properties_to_delete = Vec::new();

        for obj in objects {
            // Get both contents and children BEFORE making any modifications to avoid
            // secondary index confusion during transaction
            let contents = self.get_object_contents(obj)?;
            let parent = self.get_object_parent(obj)?;
            let children = self.get_object_children(obj)?;
            let propdefs = self.get_properties(obj)?;

            // Collect contents that need to be moved to NOTHING
            contents_to_move.extend(contents.iter());

            // Collect children that need to be reparented to this object's parent
            for c in children.iter() {
                children_to_reparent.push((c, parent));
            }

            // Collect property UUIDs for deletion
            for p in propdefs.iter() {
                properties_to_delete.push((*obj, p.uuid()));
            }
        }

        // Bulk update location relationships directly on the relation
        for content in contents_to_move {
            upsert(&mut self.object_location, content, NOTHING).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error updating object location: {e:?}"))
            })?;
        }

        // Bulk update parent relationships directly on the relation
        for (child, new_parent) in &children_to_reparent {
            upsert(&mut self.object_parent, *child, *new_parent).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error updating object parent: {e:?}"))
            })?;
        }

        // Batch delete all core object data
        for obj in objects {
            // Remove parent relationship (children list is automatically updated via secondary index)
            self.object_parent.delete(obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error removing parent relationship: {e:?}"))
            })?;

            // Remove location relationship (contents list is automatically updated via secondary index)
            self.object_location.delete(obj).map_err(|e| {
                WorldStateError::DatabaseError(format!(
                    "Error removing location relationship: {e:?}"
                ))
            })?;

            // Delete core object attributes
            self.object_flags.delete(obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting object flags: {e:?}"))
            })?;
            self.object_name.delete(obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting object name: {e:?}"))
            })?;
            self.object_owner.delete(obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting object owner: {e:?}"))
            })?;
            self.object_verbdefs.delete(obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting object verbdefs: {e:?}"))
            })?;

            // We may or may not have propdefs yet...
            self.object_propdefs.delete(obj).ok();
        }

        // Batch delete property values
        for (obj, prop_uuid) in properties_to_delete {
            self.object_propvalues
                .delete(&ObjAndUUIDHolder::new(&obj, prop_uuid))
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!("Error deleting property value: {e:?}"))
                })?;
        }

        self.has_mutations = true;

        // Update caches for reparented children and removed objects
        {
            let reparented_children: HashSet<Obj> = children_to_reparent
                .iter()
                .map(|(child, _)| *child)
                .collect();
            for child in reparented_children {
                self.invalidate_all_caches_for_branch(&child)?;
            }
        }

        let removed: Vec<Obj> = objects.iter().copied().collect();
        self.invalidate_verb_cache_for_objects(&removed);
        self.invalidate_prop_cache_for_objects(&removed);
        self.invalidate_ancestry_cache_for_objects(&removed);

        Ok(())
    }

    pub fn get_object_parent(&self, obj: &Obj) -> Result<Obj, WorldStateError> {
        let r = self.object_parent.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object parent: {e:?}"))
        })?;
        Ok(r.unwrap_or(NOTHING))
    }

    pub fn set_object_parent(&mut self, o: &Obj, new_parent: &Obj) -> Result<(), WorldStateError> {
        // Check if we're setting the same parent (no-op)
        let old_parent = self.get_object_parent(o)?;
        if old_parent.eq(new_parent) {
            return Ok(());
        };

        // Update the parent relationship and invalidate caches for the affected subtree.
        // Property and verb resolution will rebuild lazily against the new ancestry.

        self.has_mutations = true;

        // Update the parent relationship
        upsert(&mut self.object_parent, *o, *new_parent).expect("Unable to update parent");

        self.invalidate_all_caches_for_branch(o)?;

        Ok(())
    }

    /// Optimized version of set_object_parent for new object creation.
    /// Skips the no-op check since we know this is a new object.
    /// Uses guaranteed unique insertion for anonymous objects.
    fn set_initial_object_parent(&mut self, o: &Obj, parent: &Obj) -> Result<(), WorldStateError> {
        self.has_mutations = true;

        // Use optimized insertion for anonymous and UUID objects, regular insert for new traditional objects
        if o.is_anonymous() || o.is_uuobjid() {
            insert_guaranteed_unique(&mut self.object_parent, *o, *parent)
                .expect("Unable to set parent");
        } else {
            // For new traditional objects, we can use regular insert since we know the ID doesn't exist
            // in our transaction, but we can't guarantee no conflict with another transaction.
            self.object_parent
                .insert(*o, *parent)
                .expect("Unable to set parent");
        }

        Ok(())
    }

    pub fn get_object_children(&self, obj: &Obj) -> Result<ObjSet, WorldStateError> {
        // Use object_parent secondary index to get children of a parent
        let children_vec = self.object_parent.get_by_codomain(obj);
        Ok(ObjSet::from_items(&children_vec))
    }

    pub fn get_owned_objects(&self, owner: &Obj) -> Result<ObjSet, WorldStateError> {
        // Use object_owner secondary index to get objects owned by an owner
        let owned_vec = self.object_owner.get_by_codomain(owner);
        Ok(ObjSet::from_items(&owned_vec))
    }

    pub fn get_object_location(&self, obj: &Obj) -> Result<Obj, WorldStateError> {
        let r = self.object_location.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object location: {e:?}"))
        })?;
        Ok(r.unwrap_or(NOTHING))
    }

    pub fn get_object_contents(&self, obj: &Obj) -> Result<ObjSet, WorldStateError> {
        // Use object_location secondary index to get contents of a location
        let contents_vec = self.object_location.get_by_codomain(obj);
        Ok(ObjSet::from_items(&contents_vec))
    }

    // This is more of a guestimate.
    pub fn get_object_size_bytes(&self, obj: &Obj) -> Result<usize, WorldStateError> {
        let flags = self.get_object_flags(obj)?;
        let name = self.object_name.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object name: {e:?}"))
        })?;
        let owner = self.get_object_owner(obj)?;
        let parent = self.get_object_parent(obj)?;
        let location = self.get_object_location(obj)?;
        let contents = self.get_object_contents(obj)?;
        let children = self.get_object_children(obj)?;
        let verbdefs = self.get_verbs(obj)?;
        let propdefs = self.get_properties(obj)?;
        let propvalues = propdefs
            .iter()
            .map(|p| self.retrieve_property(obj, p.uuid()));
        let verbs = verbdefs
            .iter()
            .map(|v| self.get_verb_program(obj, v.uuid()));

        let mut size = flags.size_bytes();
        size += name.map(|n| n.size_bytes()).unwrap_or_default();
        size += owner.size_bytes();
        size += parent.size_bytes();
        size += location.size_bytes();
        size += contents.size_bytes();
        size += children.size_bytes();
        size += verbdefs.size_bytes();
        size += propdefs.size_bytes();
        for pv in propvalues {
            size += pv
                .map(|(v, p)| v.map(|v| v.size_bytes()).unwrap_or_default() + p.size_bytes())
                .unwrap_or_default();
        }
        for v in verbs {
            let p = v?;
            size += size_of_val(&p);
        }

        Ok(size)
    }

    pub fn set_object_location(
        &mut self,
        what: &Obj,
        new_location: &Obj,
    ) -> Result<(), WorldStateError> {
        // Detect recursive move
        let mut oid = *new_location;
        loop {
            if oid.is_nothing() {
                break;
            }
            if oid.eq(what) {
                return Err(WorldStateError::RecursiveMove(*what, *new_location));
            }
            let location = self.object_location.get(&oid).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting object location: {e:?}"))
            })?;
            let Some(location) = location else {
                break;
            };
            oid = location
        }

        // Get o's location, get its contents, remove o from old contents, put contents back
        // without it. Set new location, get its contents, add o to contents, put contents
        // back with it. Then update the location of o.
        // Get and remove from contents of old location, if we had any.
        let old_location = self.object_location.get(what).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object location: {e:?}"))
        })?;

        if let Some(old_location) = &old_location
            && old_location.eq(new_location)
        {
            return Ok(());
        }

        // Set new location.
        upsert(&mut self.object_location, *what, *new_location).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting object location: {e:?}"))
        })?;
        self.has_mutations = true;

        // Now need to update contents in both.
        // Contents lists are automatically updated via object_location secondary index
        // Just update the core object_location relation

        if new_location.is_nothing() {
            return Ok(());
        }

        Ok(())
    }

    fn set_initial_object_location(
        &mut self,
        what: &Obj,
        new_location: &Obj,
    ) -> Result<(), WorldStateError> {
        upsert(&mut self.object_location, *what, *new_location).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting initial object location: {e:?}"))
        })?;
        self.has_mutations = true;
        Ok(())
    }

    pub fn get_verbs(&self, obj: &Obj) -> Result<VerbDefs, WorldStateError> {
        let r = self
            .object_verbdefs
            .get(obj)
            .map_err(|e| WorldStateError::DatabaseError(format!("Error getting verbs: {e:?}")))?;
        Ok(r.unwrap_or_else(VerbDefs::empty))
    }

    pub fn get_verb_program(&self, obj: &Obj, uuid: Uuid) -> Result<ProgramType, WorldStateError> {
        let r = self
            .object_verbs
            .get(&ObjAndUUIDHolder::new(obj, uuid))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting verb binary: {e:?}"))
            })?;
        let Some(program) = r else {
            return Err(WorldStateError::VerbNotFound(*obj, format!("{uuid}")));
        };
        Ok(program)
    }

    pub fn get_verb_by_name(&self, obj: &Obj, name: Symbol) -> Result<VerbDef, WorldStateError> {
        // Check verb cache first, and then if we get a hit and definer == obj, we've got one,
        // otherwise, go hunting.
        match self.verb_resolution_cache.lookup(obj, &name) {
            Some(Some(verbdef)) if verbdef.location().eq(obj) => Ok(verbdef),
            Some(None) => Err(WorldStateError::VerbNotFound(*obj, name.to_string())),
            Some(Some(_verbdef)) => {
                // Found cached verb but it's not directly on this object (it's inherited).
                // get_verb_by_name only returns verbs directly on the object, so we need
                // to look directly on this object. But we should NOT record a miss since
                // the cached entry is valid for resolve_verb inheritance lookups.
                let verbdefs = self.get_verbs(obj)?;
                let named = verbdefs.find_named(name);
                let Some(verb) = named.first() else {
                    // Don't record miss - preserve the inherited verb cache entry
                    return Err(WorldStateError::VerbNotFound(*obj, name.to_string()));
                };

                // Fill cache with the direct verb (this might replace the inherited one,
                // but that's okay since this is more specific)
                self.verb_resolution_cache.fill_hit(obj, &name, verb);
                Ok(verb.clone())
            }
            None => {
                // No cache entry at all, proceed with normal lookup
                let verbdefs = self.get_verbs(obj)?;
                let named = verbdefs.find_named(name);
                let Some(verb) = named.first() else {
                    // Don't record a miss - get_verb_by_name only looks directly on object,
                    // so not finding it here doesn't mean the verb doesn't exist via inheritance
                    return Err(WorldStateError::VerbNotFound(*obj, name.to_string()));
                };

                // Fill cache
                self.verb_resolution_cache.fill_hit(obj, &name, verb);
                Ok(verb.clone())
            }
        }
    }

    pub fn get_verb_by_index(&self, obj: &Obj, index: usize) -> Result<VerbDef, WorldStateError> {
        let verbs = self.get_verbs(obj)?;
        if index >= verbs.len() {
            return Err(WorldStateError::VerbNotFound(*obj, format!("{index}")));
        }
        let verb = verbs
            .iter()
            .nth(index)
            .ok_or_else(|| WorldStateError::VerbNotFound(*obj, format!("{index}")))?;
        Ok(verb.clone())
    }

    pub fn resolve_verb(
        &self,
        obj: &Obj,
        name: Symbol,
        argspec: Option<VerbArgsSpec>,
        flagspec: Option<BitEnum<VerbFlag>>,
    ) -> Result<VerbDef, WorldStateError> {
        // Check the cache first.
        if let Some(cache_result) = self.verb_resolution_cache.lookup(obj, &name) {
            // We recorded a miss here before..
            let Some(verbdef) = cache_result else {
                return Err(WorldStateError::VerbNotFound(*obj, name.to_string()));
            };
            if verbdef.matches_spec(&argspec, &flagspec) {
                return Ok(verbdef.clone());
            }
        }

        // Check to see if we have a hit for this object for first ancestor with verbdefs...
        // If we do, we can jump straight to that as our search_o
        let mut first_parent_hit = false;
        let mut search_o = {
            match self
                .verb_resolution_cache
                .lookup_first_parent_with_verbs(obj)
            {
                Some(Some(o)) => {
                    first_parent_hit = true;
                    o
                }
                Some(None) => {
                    // No ancestors with verbs, verbnf
                    return Err(WorldStateError::VerbNotFound(*obj, name.to_string()));
                }
                None => *obj,
            }
        };
        let mut found = false;
        loop {
            let verbdefs = self.object_verbdefs.get(&search_o).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting verbs: {e:?}"))
            })?;
            if let Some(verbdefs) = verbdefs {
                if !first_parent_hit {
                    self.verb_resolution_cache
                        .fill_first_parent_with_verbs(obj, Some(search_o));
                    first_parent_hit = true;
                }

                // Find the named verb (which may be empty if the verb is not defined on this
                // object, but is defined on an ancestor
                let named = verbdefs.find_named(name);

                // Fill the verb cache.
                let verb = named.first();
                if let Some(verb) = verb {
                    self.verb_resolution_cache.fill_hit(obj, &name, verb);

                    found = true;
                    if verb.matches_spec(&argspec, &flagspec) {
                        return Ok(verb.clone());
                    }
                }
            }
            search_o = self.get_object_parent(&search_o)?;
            if search_o.is_nothing() {
                break;
            }
        }

        // Record the miss, but only if we actually didn't find anything, otherwise we can end up
        // recording a miss for things where the argspec didn't match
        if !found {
            self.verb_resolution_cache.fill_miss(obj, &name);
        }
        Err(WorldStateError::VerbNotFound(*obj, name.to_string()))
    }

    pub fn update_verb(
        &mut self,
        obj: &Obj,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let verbdefs = self.get_verbs(obj)?;

        let Some(verbdefs) = verbdefs.with_updated(uuid, |ov| {
            let names = match &verb_attrs.names {
                None => ov.names(),
                Some(new_names) => new_names.as_slice(),
            };
            VerbDef::new(
                ov.uuid(),
                ov.location(),
                verb_attrs.owner.unwrap_or(ov.owner()),
                names,
                verb_attrs.flags.unwrap_or(ov.flags()),
                verb_attrs.args_spec.unwrap_or(ov.args()),
            )
        }) else {
            return Err(WorldStateError::VerbNotFound(*obj, format!("{uuid}")));
        };

        upsert(&mut self.object_verbdefs, *obj, verbdefs).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting verb definition: {e:?}"))
        })?;
        self.has_mutations = true;

        if verb_attrs.program.is_some() {
            upsert(
                &mut self.object_verbs,
                ObjAndUUIDHolder::new(obj, uuid),
                verb_attrs.program.unwrap(),
            )
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting verb binary: {e:?}"))
            })?;
        }
        self.invalidate_verb_cache_for_branch(obj)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_object_verb(
        &mut self,
        oid: &Obj,
        owner: &Obj,
        names: &[Symbol],
        program: ProgramType,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), WorldStateError> {
        let verbdefs = self.get_verbs(oid)?;

        let uuid = Uuid::new_v4();
        let verbdef = VerbDef::new(uuid, *oid, *owner, names, flags, args);

        let verbdefs = verbdefs.with_added(verbdef);
        upsert(&mut self.object_verbdefs, *oid, verbdefs).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting verb definition: {e:?}"))
        })?;
        self.has_mutations = true;

        upsert(
            &mut self.object_verbs,
            ObjAndUUIDHolder::new(oid, uuid),
            program,
        )
        .map_err(|e| WorldStateError::DatabaseError(format!("Error setting verb binary: {e:?}")))?;

        self.invalidate_verb_cache_for_branch(oid)?;
        Ok(())
    }

    pub fn delete_verb(&mut self, location: &Obj, uuid: Uuid) -> Result<(), WorldStateError> {
        let verbdefs = self.get_verbs(location)?;
        let verbdefs = verbdefs
            .with_removed(uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(*location, format!("{uuid}")))?;
        upsert(&mut self.object_verbdefs, *location, verbdefs).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting verb definition: {e:?}"))
        })?;
        self.has_mutations = true;

        self.object_verbs
            .delete(&ObjAndUUIDHolder::new(location, uuid))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting verb binary: {e:?}"))
            })?;
        self.invalidate_verb_cache_for_branch(location)?;
        Ok(())
    }

    pub fn get_properties(&self, obj: &Obj) -> Result<PropDefs, WorldStateError> {
        let r = self.object_propdefs.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting properties: {e:?}"))
        })?;
        Ok(r.unwrap_or_else(PropDefs::empty))
    }

    pub fn set_property(
        &mut self,
        obj: &Obj,
        uuid: Uuid,
        value: Var,
    ) -> Result<(), WorldStateError> {
        // Set the property value
        upsert(
            &mut self.object_propvalues,
            ObjAndUUIDHolder::new(obj, uuid),
            value,
        )
        .map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting property value: {e:?}"))
        })?;

        // In lazy mode, ensure we have a local propflags entry when setting a value locally.
        // If we don't have one, create it by inheriting from the canonical permissions.
        let holder = ObjAndUUIDHolder::new(obj, uuid);
        if self
            .object_propflags
            .get(&holder)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error checking property flags: {e:?}"))
            })?
            .is_none()
        {
            // No local propflags entry - create one based on inherited permissions
            let inherited_perms = self.retrieve_property_permissions(obj, uuid)?;
            upsert(&mut self.object_propflags, holder, inherited_perms).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting property flags: {e:?}"))
            })?;
        }

        self.has_mutations = true;
        Ok(())
    }

    pub fn define_property(
        &mut self,
        definer: &Obj,
        location: &Obj,
        name: Symbol,
        owner: &Obj,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<Uuid, WorldStateError> {
        // If the property is already defined at us or above or below us, that's a failure.
        let props = self.get_properties(location)?;
        if props.find_first_named(name).is_some() {
            return Err(WorldStateError::DuplicatePropertyDefinition(
                *location,
                name.to_string(),
            ));
        }

        // Check ancestors - can't define a property that already exists on a parent
        let ancestors = self.ancestors(location, false)?;
        for ancestor in ancestors.iter() {
            let ancestor_props = self.get_properties(&ancestor)?;
            if ancestor_props.find_first_named(name).is_some() {
                return Err(WorldStateError::DuplicatePropertyDefinition(
                    ancestor,
                    name.to_string(),
                ));
            }
        }

        // Check descendants - can't define a property if a child already has one with that name
        let descendants = self.descendants(location, false)?;
        for descendant in descendants.iter() {
            let descendant_props = self.get_properties(&descendant)?;
            if descendant_props.find_first_named(name).is_some() {
                return Err(WorldStateError::DuplicatePropertyDefinition(
                    descendant,
                    name.to_string(),
                ));
            }
        }

        // Generate a new property ID. This will get shared all the way down the pipe.
        // But the key for the actual value is always composite of oid,uuid
        let u = Uuid::new_v4();

        let prop = PropDef::new(u, *definer, *location, name);
        upsert(&mut self.object_propdefs, *location, props.with_added(prop)).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting property definition: {e:?}"))
        })?;
        self.has_mutations = true;

        // Always create propflags entry for the defining location (canonical permissions)
        upsert(
            &mut self.object_propflags,
            ObjAndUUIDHolder::new(location, u),
            PropPerms::new(*owner, perms),
        )
        .map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting property owner: {e:?}"))
        })?;

        // If we have an initial value, set it, but just on ourselves. Descendants start out clear.
        if let Some(value) = value {
            self.set_property(location, u, value)?;
        }

        self.invalidate_prop_cache_for_branch(location)?;

        Ok(u)
    }

    pub fn update_property_info(
        &mut self,
        obj: &Obj,
        uuid: Uuid,
        new_owner: Option<Obj>,
        new_flags: Option<BitEnum<PropFlag>>,
        new_name: Option<Symbol>,
    ) -> Result<(), WorldStateError> {
        if new_owner.is_none() && new_flags.is_none() && new_name.is_none() {
            return Ok(());
        }

        // We only need to update the propdef if there's a new name.
        if let Some(new_name) = new_name {
            let props = self.get_properties(obj)?;

            let Some(props) = props.with_updated(uuid, |p| {
                PropDef::new(p.uuid(), p.definer(), p.location(), new_name)
            }) else {
                return Err(WorldStateError::PropertyNotFound(*obj, format!("{uuid}")));
            };

            upsert(&mut self.object_propdefs, *obj, props).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error updating property: {e:?}"))
            })?;
        }
        self.has_mutations = true;

        // If flags or perms updated, do that.
        if new_flags.is_some() || new_owner.is_some() {
            let mut perms = self.retrieve_property_permissions(obj, uuid)?;

            if let Some(new_flags) = new_flags {
                perms = perms.with_flags(new_flags);
            }

            if let Some(new_owner) = new_owner {
                perms = perms.with_owner(new_owner);
            }

            upsert(
                &mut self.object_propflags,
                ObjAndUUIDHolder::new(obj, uuid),
                perms,
            )
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error updating property: {e:?}"))
            })?;
        }

        self.invalidate_prop_cache_for_branch(obj)?;
        Ok(())
    }

    pub fn clear_property(&mut self, obj: &Obj, uuid: Uuid) -> Result<(), WorldStateError> {
        // remove property value
        self.object_propvalues
            .delete(&ObjAndUUIDHolder::new(obj, uuid))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error clearing property value: {e:?}"))
            })?;
        self.has_mutations = true;
        self.invalidate_prop_cache_for_objects(&[*obj]);
        Ok(())
    }

    pub fn delete_property(&mut self, obj: &Obj, uuid: Uuid) -> Result<(), WorldStateError> {
        // delete propdef from self and all descendants
        let descendants = self.descendants(obj, false)?;
        let locations = ObjSet::from_items(&[*obj]).with_concatenated(descendants);
        for location in locations.iter() {
            let props: PropDefs = self.get_properties(&location)?;
            if let Some(props) = props.with_removed(uuid) {
                upsert(&mut self.object_propdefs, location, props).map_err(|e| {
                    WorldStateError::DatabaseError(format!("Error deleting property: {e:?}"))
                })?;
            }
        }
        self.has_mutations = true;
        self.invalidate_prop_cache_for_branch(obj)?;
        Ok(())
    }

    pub fn retrieve_property(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError> {
        let r = self
            .object_propvalues
            .get(&ObjAndUUIDHolder::new(obj, uuid))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting property value: {e:?}"))
            })?;
        let value = r;

        let perms = self.retrieve_property_permissions(obj, uuid)?;

        Ok((value, perms))
    }

    pub fn retrieve_property_permissions(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<PropPerms, WorldStateError> {
        // First check if this object has local propflags (set via set_property or update_property_info)
        if let Ok(Some(perms)) = self.object_propflags.get(&ObjAndUUIDHolder::new(obj, uuid)) {
            return Ok(perms);
        }

        // No local propflags entry - need to find the property definition in ancestry chain
        // and compute permissions lazily
        let propdef = self.find_property_by_uuid(obj, uuid)?;
        let defining_obj = propdef.definer();

        // Get the canonical permissions from the defining object
        let canonical_perms = self
            .object_propflags
            .get(&ObjAndUUIDHolder::new(&defining_obj, uuid))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting canonical property flags: {e:?}"))
            })?
            .ok_or_else(|| {
                WorldStateError::DatabaseError(
                    format!("Canonical property permissions not found on definer {defining_obj} for property {uuid}")
                )
            })?;

        // If the property has Chown flag, use the object's owner as the property owner
        let final_perms =
            if canonical_perms.flags().contains(PropFlag::Chown) && *obj != defining_obj {
                let obj_owner = self.get_object_owner(obj)?;
                canonical_perms.with_owner(obj_owner)
            } else {
                canonical_perms
            };

        Ok(final_perms)
    }

    // Helper function to find property definition by UUID instead of name
    fn find_property_by_uuid(&self, obj: &Obj, uuid: Uuid) -> Result<PropDef, WorldStateError> {
        // Walk up the ancestry chain looking for the property definition
        let ancestors = self.ancestors(obj, true)?;
        for ancestor in ancestors.iter() {
            let props = self.get_properties(&ancestor)?;
            if let Some(prop) = props.find(&uuid) {
                return Ok(prop.clone());
            }
        }
        Err(WorldStateError::PropertyNotFound(
            *obj,
            format!("uuid:{uuid}"),
        ))
    }

    fn find_property_by_name(&self, obj: &Obj, name: Symbol) -> Option<PropDef> {
        // Check the cache first.
        if let Some(cache_result) = self.prop_resolution_cache.lookup(obj, &name) {
            return cache_result;
        }

        // Look in the cache for the first parent with non-empty propdefs. If we have no cache entry,
        // then seek upwards until we find one, record that, and then look there.
        let (mut propdefs, mut search_o) = {
            match self
                .prop_resolution_cache
                .lookup_first_parent_with_props(obj)
            {
                Some(Some(o)) => (self.get_properties(&o).ok()?, o),
                Some(None) => {
                    // No ancestors with verbs, verbnf
                    return None;
                }
                None => {
                    let mut search_o = *obj;
                    let propdefs = loop {
                        let propdefs = self.get_properties(&search_o).ok()?;
                        if !propdefs.is_empty() {
                            self.prop_resolution_cache
                                .fill_first_parent_with_props(obj, Some(search_o));

                            break propdefs;
                        }

                        search_o = self.get_object_parent(&search_o).ok()?;
                        if search_o.is_nothing() {
                            return None;
                        }
                    };
                    (propdefs, search_o)
                }
            }
        };

        let mut found_propdef = None;
        loop {
            let propdef = propdefs.find_first_named(name);
            if let Some(propdef) = propdef {
                found_propdef = Some(propdef);
                break;
            }

            search_o = self.get_object_parent(&search_o).ok()?;
            if search_o.is_nothing() {
                break;
            }
            propdefs = self.get_properties(&search_o).ok()?;
        }
        let Some(propdef) = found_propdef else {
            self.prop_resolution_cache.fill_miss(obj, &name);
            return None;
        };

        // Cache it
        self.prop_resolution_cache.fill_hit(obj, &name, &propdef);

        Some(propdef)
    }

    pub fn resolve_property(
        &self,
        obj: &Obj,
        name: Symbol,
    ) -> Result<(PropDef, Var, PropPerms, bool), WorldStateError> {
        let Some(propdef) = self.find_property_by_name(obj, name) else {
            return Err(WorldStateError::PropertyNotFound(*obj, name.to_string()));
        };

        // Now that we have the propdef, we can look for the value & owner.
        // We should *always* have the owner.
        // But value could be 'clear' in which case we need to look in the parent.
        let prop_uuid = propdef.uuid();
        let (pvalue, perms) = self.retrieve_property(obj, prop_uuid)?;
        match pvalue {
            Some(value) => Ok((propdef, value, perms, false)),
            None => {
                let ancestors = self.ancestors(obj, false)?;
                for search_obj in ancestors.iter() {
                    let value = self
                        .object_propvalues
                        .get(&ObjAndUUIDHolder::new(&search_obj, propdef.uuid()))
                        .map_err(|e| {
                            WorldStateError::DatabaseError(format!(
                                "Error getting property value: {e:?}"
                            ))
                        })?;
                    if let Some(value) = value {
                        return Ok((propdef, value, perms, true));
                    }
                }
                Ok((propdef, v_none(), perms, true))
            }
        }
    }

    pub fn db_usage(&self) -> Result<usize, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.usage_channel
            .send(send)
            .expect("Unable to send usage request");
        Ok(receive.recv().expect("Unable to receive usage response"))
    }

    pub fn commit(self) -> Result<CommitResult, WorldStateError> {
        let counters = db_counters();
        let commit_start = Instant::now();
        let record_commit_result = |result: &CommitResult| {
            let elapsed_nanos = commit_start.elapsed().as_nanos() as isize;
            match result {
                CommitResult::Success { mutations_made, .. } => {
                    counters.commit_success.invocations().add(1);
                    counters
                        .commit_success
                        .cumulative_duration_nanos()
                        .add(elapsed_nanos);
                    if *mutations_made {
                        counters.commit_success_write.invocations().add(1);
                        counters
                            .commit_success_write
                            .cumulative_duration_nanos()
                            .add(elapsed_nanos);
                    } else {
                        counters.commit_success_readonly.invocations().add(1);
                        counters
                            .commit_success_readonly
                            .cumulative_duration_nanos()
                            .add(elapsed_nanos);
                    }
                }
                CommitResult::ConflictRetry => {
                    counters.commit_conflict.invocations().add(1);
                    counters
                        .commit_conflict
                        .cumulative_duration_nanos()
                        .add(elapsed_nanos);
                }
            }
        };

        // Did we have any mutations at all?  If not, just fire and forget the verb cache and
        // return immediate success.
        if !self.has_mutations {
            if self.verb_resolution_cache.has_changed() || self.prop_resolution_cache.has_changed()
            {
                self.commit_channel
                    .send(CommitSet::CommitReadOnly(Caches {
                        verb_resolution_cache: self.verb_resolution_cache,
                        prop_resolution_cache: self.prop_resolution_cache,
                        ancestry_cache: self.ancestry_cache,
                    }))
                    .expect("Unable to send commit request for read-only transaction");
            }
            let result = CommitResult::Success {
                mutations_made: false,
                timestamp: 0, // Read-only transactions don't have meaningful timestamps
            };
            record_commit_result(&result);
            return Ok(result);
        }

        // Pull out the working sets
        let _t = PerfTimerGuard::new(&counters.tx_commit_mk_working_set_phase);

        // Extract commit channel before consuming self
        let commit_channel = self.commit_channel.clone();
        let ws = self.into_working_sets()?;

        let tuple_count = ws.total_tuples();

        // Send the working sets to the commit processing thread
        drop(_t);
        let _t = PerfTimerGuard::new(&counters.tx_commit_send_working_set_phase);
        let (send, reply) = oneshot::channel();
        commit_channel
            .send(CommitSet::CommitWrites(ws, send))
            .expect("Could not send commit request -- channel closed?");

        // Wait for the reply.
        drop(_t);
        let _t = PerfTimerGuard::new(&counters.tx_commit_wait_result_phase);
        let mut last_check_time = Instant::now();
        loop {
            match reply.recv_timeout(Duration::from_millis(10)) {
                Ok(reply) => {
                    record_commit_result(&reply);
                    return Ok(reply);
                }
                Err(_) => {
                    if last_check_time.elapsed() > Duration::from_secs(5) {
                        warn!(
                            "Transaction commit (started {}s ago) taking a long time to commit. Contains {tuple_count} total tuples.",
                            commit_start.elapsed().as_secs_f32(),
                        );
                    }
                    last_check_time = Instant::now();
                }
            }
        }
    }

    pub fn rollback(self) -> Result<(), WorldStateError> {
        // Just drop the transaction, it will be cleaned up by the drop impl.
        Ok(())
    }

    pub fn descendants(&self, obj: &Obj, include_self: bool) -> Result<ObjSet, WorldStateError> {
        let children = self.get_object_children(obj)?;

        let mut results_sans_self = vec![];
        let mut queue: VecDeque<_> = children.iter().collect();
        while let Some(o) = queue.pop_front() {
            results_sans_self.push(o);
            let children = self.get_object_children(&o)?;
            queue.extend(children.iter());
        }

        let descendant_set = if include_self {
            // Chained iter of "obj" + the results
            let chained = std::iter::once(*obj).chain(results_sans_self);
            ObjSet::from_iter(chained)
        } else {
            ObjSet::from_items(&results_sans_self)
        };

        Ok(descendant_set)
    }

    /// Flush all internal caches (verb resolution, property resolution, ancestry).
    /// This ensures that subsequent queries will see fresh data.
    pub fn flush_caches(&mut self) {
        self.verb_resolution_cache.flush();
        self.prop_resolution_cache.flush();
        self.ancestry_cache.flush();
    }
}

impl WorldStateTransaction {
    pub fn get_last_move(&self, obj: &Obj) -> Result<Var, WorldStateError> {
        let r = self.object_last_move.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting last_move: {e:?}"))
        })?;
        Ok(r.unwrap_or_else(v_empty_map))
    }

    pub fn set_last_move(&mut self, obj: &Obj, old_location: Obj) -> Result<(), WorldStateError> {
        // Create the last_move map with timestamp and source
        let time_key = Symbol::mk("time");
        let source_key = Symbol::mk("source");
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let last_move_map = v_map(&[
            (Var::from(time_key.to_string()), Var::from(timestamp)),
            (Var::from(source_key.to_string()), Var::from(old_location)),
        ]);

        upsert(&mut self.object_last_move, *obj, last_move_map).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting last_move: {e:?}"))
        })?;
        self.has_mutations = true;
        Ok(())
    }

    /// Increment the given sequence, return the new value.
    pub fn increment_sequence(&self, seq: usize) -> i64 {
        self.sequences[seq].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.sequences[seq].load(std::sync::atomic::Ordering::Relaxed)
    }

    fn get_sequence(&self, seq: usize) -> i64 {
        self.sequences[seq].load(std::sync::atomic::Ordering::Relaxed)
    }

    fn update_sequence_max(&self, seq: usize, value: i64) -> i64 {
        loop {
            let current = self.sequences[seq].load(std::sync::atomic::Ordering::Relaxed);
            let max = std::cmp::max(current, value);
            if max <= current {
                return current;
            }
            if self.sequences[seq]
                .compare_exchange(
                    current,
                    max,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_ok()
            {
                return current;
            }
        }
    }

    /// Renumber an object to a new object number, following LambdaMOO semantics.
    /// Updates structural database relationships but not object references in code/property values.
    pub fn renumber_object(
        &mut self,
        old_obj: &Obj,
        target: Option<&Obj>,
    ) -> Result<Obj, WorldStateError> {
        // Verify old object exists
        if !self.object_valid(old_obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(*old_obj)));
        }

        // Determine new object ID
        let new_obj = if let Some(target) = target {
            // Explicit target - ensure it's not already in use
            if self.object_valid(target)? {
                return Err(WorldStateError::InvalidRenumber(format!(
                    "Target object {target} already exists"
                )));
            }
            *target
        } else {
            // Auto-selection logic
            if old_obj.is_uuobjid() {
                // For UUID objects: scan backwards from max_object, then use max_object + 1 if none found
                let max_obj = self.get_max_object()?;
                let mut candidate_id = max_obj.id().0;

                // Scan backwards from max_object to 0
                loop {
                    let candidate = Obj::mk_id(candidate_id);
                    if !self.object_valid(&candidate)? {
                        break candidate;
                    }
                    if candidate_id == 0 {
                        break Obj::mk_id(max_obj.id().0 + 1);
                    }
                    candidate_id -= 1;
                }
            } else {
                // For numbered objects: LambdaMOO algorithm - scan from 0 to old-1
                let mut found_candidate = None;
                let mut candidate_id = 0;
                while candidate_id < old_obj.id().0 {
                    let candidate = Obj::mk_id(candidate_id);
                    if !self.object_valid(&candidate)? {
                        found_candidate = Some(candidate);
                        break;
                    }
                    candidate_id += 1;
                }
                // Per LambdaMOO spec: if no available numbers found, return original unchanged
                found_candidate.unwrap_or(*old_obj)
            }
        };

        // If no renumbering is needed (same object), return early
        if new_obj == *old_obj {
            return Ok(new_obj);
        }

        // Validate cross-type renumbering restrictions
        match (old_obj.is_uuobjid(), new_obj.is_uuobjid()) {
            (true, true) => {
                // renumber(uuid, uuid) - FAIL
                return Err(WorldStateError::InvalidRenumber(
                    "Cannot renumber UUID object to another UUID".to_string(),
                ));
            }
            (false, true) => {
                // renumber(obj, uuid) - FAIL
                return Err(WorldStateError::InvalidRenumber(
                    "Cannot renumber numbered object to UUID".to_string(),
                ));
            }
            (true, false) => {
                // renumber(uuid) or renumber(uuid, obj) - SUCCEED (UUID → Objid allowed)
            }
            (false, false) => {
                // renumber(obj) or renumber(obj, obj) - SUCCEED (Objid → Objid allowed)
            }
        }

        // Step 1: Update all relations where old_obj appears as a codomain (target)

        // Update parent relationships (children pointing to old_obj as parent)
        let parent_refs = self
            .object_parent
            .scan(&|_domain, codomain| *codomain == *old_obj)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error scanning parent relations: {e:?}"))
            })?;
        for (child, _) in parent_refs {
            self.object_parent.delete(&child).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting parent relation: {e:?}"))
            })?;
            self.object_parent.upsert(child, new_obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error updating parent relation: {e:?}"))
            })?;
        }

        // Update location relationships (contents pointing to old_obj as location)
        let location_refs = self
            .object_location
            .scan(&|_domain, codomain| *codomain == *old_obj)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error scanning location relations: {e:?}"))
            })?;
        for (content, _) in location_refs {
            self.object_location.delete(&content).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting location relation: {e:?}"))
            })?;
            self.object_location.upsert(content, new_obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error updating location relation: {e:?}"))
            })?;
        }

        // Update ownership relationships (objects owned by old_obj)
        let owner_refs = self
            .object_owner
            .scan(&|_domain, codomain| *codomain == *old_obj)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error scanning owner relations: {e:?}"))
            })?;
        for (owned, _) in owner_refs {
            self.object_owner.delete(&owned).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting owner relation: {e:?}"))
            })?;
            self.object_owner.upsert(owned, new_obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error updating owner relation: {e:?}"))
            })?;
        }

        // Step 2: Update relations where old_obj is the domain (source)

        // Update old_obj's parent relationship
        if let Ok(Some(parent)) = self.object_parent.get(old_obj) {
            self.object_parent.delete(old_obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting old object parent: {e:?}"))
            })?;
            self.object_parent.upsert(new_obj, parent).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting new object parent: {e:?}"))
            })?;
        }

        // Update old_obj's location relationship
        if let Ok(Some(location)) = self.object_location.get(old_obj) {
            self.object_location.delete(old_obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting old object location: {e:?}"))
            })?;
            self.object_location
                .upsert(new_obj, location)
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!(
                        "Error setting new object location: {e:?}"
                    ))
                })?;
        }

        // Update old_obj's owner relationship
        if let Ok(Some(owner)) = self.object_owner.get(old_obj) {
            self.object_owner.delete(old_obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting old object owner: {e:?}"))
            })?;
            self.object_owner.upsert(new_obj, owner).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting new object owner: {e:?}"))
            })?;
        }

        // Step 3: Update other object data relations (flags, name, etc.)

        // Move flags
        if let Ok(Some(flags)) = self.object_flags.get(old_obj) {
            self.object_flags.delete(old_obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting old object flags: {e:?}"))
            })?;
            self.object_flags.upsert(new_obj, flags).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting new object flags: {e:?}"))
            })?;
        }

        // Move name
        if let Ok(Some(name)) = self.object_name.get(old_obj) {
            self.object_name.delete(old_obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting old object name: {e:?}"))
            })?;
            self.object_name.upsert(new_obj, name).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting new object name: {e:?}"))
            })?;
        }

        // Move verb definitions
        if let Ok(Some(verbdefs)) = self.object_verbdefs.get(old_obj) {
            self.object_verbdefs.delete(old_obj).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting old object verbs: {e:?}"))
            })?;

            // Create new VerbDefs with updated location for each verb
            let mut updated_verbs = Vec::new();
            for verb in verbdefs.iter() {
                // Create new VerbDef with updated location
                let updated_verb = VerbDef::new(
                    verb.uuid(),
                    new_obj, // Updated location
                    verb.owner(),
                    verb.names(),
                    verb.flags(),
                    verb.args(),
                );
                updated_verbs.push(updated_verb);
            }
            let updated_verbdefs = VerbDefs::from_items(&updated_verbs);

            self.object_verbdefs
                .upsert(new_obj, updated_verbdefs)
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!("Error setting new object verbs: {e:?}"))
                })?;

            // Move verb programs for each verb
            for verb in verbdefs.iter() {
                let old_holder = ObjAndUUIDHolder::new(old_obj, verb.uuid());
                let new_holder = ObjAndUUIDHolder::new(&new_obj, verb.uuid());

                // Move verb program if it exists
                if let Ok(Some(program)) = self.object_verbs.get(&old_holder) {
                    self.object_verbs.delete(&old_holder).map_err(|e| {
                        WorldStateError::DatabaseError(format!(
                            "Error deleting old verb program: {e:?}"
                        ))
                    })?;
                    self.object_verbs.upsert(new_holder, program).map_err(|e| {
                        WorldStateError::DatabaseError(format!(
                            "Error setting new verb program: {e:?}"
                        ))
                    })?;
                }
            }
        }

        // Move property definitions
        if let Ok(Some(propdefs)) = self.object_propdefs.get(old_obj) {
            self.object_propdefs.delete(old_obj).map_err(|e| {
                WorldStateError::DatabaseError(format!(
                    "Error deleting old object properties: {e:?}"
                ))
            })?;
            self.object_propdefs
                .upsert(new_obj, propdefs.clone())
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!(
                        "Error setting new object properties: {e:?}"
                    ))
                })?;

            // Update all property definitions in the inheritance hierarchy that reference old_obj as definer
            let all_propdefs = self.object_propdefs.get_all().map_err(|e| {
                WorldStateError::DatabaseError(format!(
                    "Error scanning property definitions: {e:?}"
                ))
            })?;

            for (obj, props) in all_propdefs {
                let mut needs_update = false;
                let mut updated_props = Vec::new();

                for prop in props {
                    if prop.definer() == *old_obj {
                        // Create new PropDef with updated definer
                        let updated_prop =
                            PropDef::new(prop.uuid(), new_obj, prop.location(), prop.name());
                        updated_props.push(updated_prop);
                        needs_update = true;
                    } else {
                        updated_props.push(prop);
                    }
                }

                if needs_update {
                    let updated_defs = PropDefs::from_items(&updated_props);
                    self.object_propdefs
                        .upsert(obj, updated_defs)
                        .map_err(|e| {
                            WorldStateError::DatabaseError(format!(
                                "Error updating property definer references: {e:?}"
                            ))
                        })?;
                }
            }
        }

        // Move all property values for this object
        let all_prop_values = self
            .object_propvalues
            .scan(&|holder, _value| &holder.obj() == old_obj)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error scanning property values: {e:?}"))
            })?;

        for (old_holder, value) in all_prop_values {
            let new_holder = ObjAndUUIDHolder::new(&new_obj, old_holder.uuid());
            self.object_propvalues.delete(&old_holder).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting old property value: {e:?}"))
            })?;
            self.object_propvalues
                .upsert(new_holder, value)
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!(
                        "Error setting new property value: {e:?}"
                    ))
                })?;
        }

        // Move all property flags for this object
        let all_prop_flags = self
            .object_propflags
            .scan(&|holder, _flags| &holder.obj() == old_obj)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error scanning property flags: {e:?}"))
            })?;

        for (old_holder, flags) in all_prop_flags {
            let new_holder = ObjAndUUIDHolder::new(&new_obj, old_holder.uuid());
            self.object_propflags.delete(&old_holder).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting old property flags: {e:?}"))
            })?;
            self.object_propflags
                .upsert(new_holder, flags)
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!(
                        "Error setting new property flags: {e:?}"
                    ))
                })?;
        }

        self.has_mutations = true;

        // Update max_object if the new object ID is higher
        if !new_obj.is_uuobjid() {
            let current_max = self.get_max_object()?;
            if new_obj.id().0 > current_max.id().0 {
                self.update_sequence_max(SEQUENCE_MAX_OBJECT, new_obj.id().0 as i64);
            }
        }

        // Ensure caches reflect the new object identifier
        self.invalidate_verb_cache_for_objects(&[*old_obj]);
        self.invalidate_prop_cache_for_objects(&[*old_obj]);
        self.invalidate_ancestry_cache_for_objects(&[*old_obj]);
        self.invalidate_all_caches_for_branch(&new_obj)?;

        Ok(new_obj)
    }
}

impl WorldStateTransaction {
    pub(crate) fn scan_anonymous_object_references(
        &mut self,
    ) -> Result<Vec<(Obj, HashSet<Obj>)>, WorldStateError> {
        let mut reference_map = std::collections::HashMap::new();

        // Scan ALL property values directly - this catches inherited property overrides
        // that would be missed by only scanning propdefs per object. When a child object
        // overrides an inherited property, the value is stored in object_propvalues under
        // the child's obj id, but the propdef only exists on the parent.
        let all_propvalues = self
            .object_propvalues
            .get_all()
            .map_err(|e| WorldStateError::DatabaseError(e.to_string()))?;

        for (holder, prop_value) in all_propvalues {
            let obj = holder.obj;
            let obj_refs = reference_map.entry(obj).or_insert_with(HashSet::new);
            crate::extract_anonymous_refs(&prop_value, obj_refs);
        }

        // Get all objects for remaining checks (parent, location, verbdefs, propdefs metadata)
        let all_objects = self
            .object_flags
            .get_all()
            .map_err(|e| WorldStateError::DatabaseError(e.to_string()))?;

        for (obj, _flags) in all_objects {
            let obj_refs = reference_map.entry(obj).or_insert_with(HashSet::new);

            // 1. Check parent relationship
            if let Ok(parent) = self.get_object_parent(&obj)
                && parent.is_anonymous()
            {
                obj_refs.insert(parent);
            }

            // 2. Check location relationship
            if let Ok(location) = self.get_object_location(&obj)
                && location.is_anonymous()
            {
                obj_refs.insert(location);
            }

            // 3. Check contents (objects contained in this object)
            if let Ok(contents) = self.get_object_contents(&obj) {
                for contained in contents.iter() {
                    if contained.is_anonymous() {
                        obj_refs.insert(contained);
                    }
                }
            }

            // 4. Check verb definitions for object references
            if let Ok(verbdefs) = self.get_verbs(&obj) {
                for verbdef in verbdefs.iter() {
                    // Check location field
                    if verbdef.location().is_anonymous() {
                        obj_refs.insert(verbdef.location());
                    }
                    // Check owner field
                    if verbdef.owner().is_anonymous() {
                        obj_refs.insert(verbdef.owner());
                    }
                }
            }

            // 5. Check property definitions for object references
            if let Ok(propdefs) = self.get_properties(&obj) {
                for propdef in propdefs.iter() {
                    // Check definer field
                    if propdef.definer().is_anonymous() {
                        obj_refs.insert(propdef.definer());
                    }
                    // Check location field
                    if propdef.location().is_anonymous() {
                        obj_refs.insert(propdef.location());
                    }
                }
            }
        }

        Ok(reference_map
            .into_iter()
            .filter(|(_, refs)| !refs.is_empty())
            .collect())
    }

    pub(crate) fn get_anonymous_objects(&self) -> Result<HashSet<Obj>, WorldStateError> {
        // Get all objects and filter for anonymous ones
        let all_objects = self.get_objects()?;
        let anonymous_objects = all_objects
            .iter()
            .filter(|obj| obj.is_anonymous())
            .collect();
        Ok(anonymous_objects)
    }

    pub(crate) fn collect_unreachable_anonymous_objects(
        &mut self,
        unreachable_objects: &HashSet<Obj>,
    ) -> Result<usize, WorldStateError> {
        // Filter to only anonymous objects
        let anonymous_objects = unreachable_objects
            .iter()
            .filter(|obj| obj.is_anonymous())
            .cloned();

        // Bulk check validity - only recycle objects that still exist
        let objects_to_recycle = self.valid_objects(anonymous_objects)?;

        let collected = objects_to_recycle.len();

        if !objects_to_recycle.is_empty() {
            // Use batch recycling for better performance
            self.recycle_objects(&objects_to_recycle)?;
        }

        Ok(collected)
    }
}

impl WorldStateTransaction {
    fn branch_objects(&self, root: &Obj) -> Result<Vec<Obj>, WorldStateError> {
        let branch = self.descendants(root, true)?;
        Ok(branch.iter().collect())
    }

    fn invalidate_verb_cache_for_objects(&self, objects: &[Obj]) {
        if objects.is_empty() {
            return;
        }
        self.verb_resolution_cache.invalidate_objects(objects);
    }

    fn invalidate_prop_cache_for_objects(&self, objects: &[Obj]) {
        if objects.is_empty() {
            return;
        }
        self.prop_resolution_cache.invalidate_objects(objects);
    }

    fn invalidate_ancestry_cache_for_objects(&self, objects: &[Obj]) {
        if objects.is_empty() {
            return;
        }
        self.ancestry_cache.invalidate_objects(objects);
    }

    fn invalidate_verb_cache_for_branch(&self, root: &Obj) -> Result<(), WorldStateError> {
        let objects = self.branch_objects(root)?;
        self.invalidate_verb_cache_for_objects(&objects);
        Ok(())
    }

    fn invalidate_prop_cache_for_branch(&self, root: &Obj) -> Result<(), WorldStateError> {
        let objects = self.branch_objects(root)?;
        self.invalidate_prop_cache_for_objects(&objects);
        Ok(())
    }

    fn invalidate_all_caches_for_branch(&self, root: &Obj) -> Result<(), WorldStateError> {
        let objects = self.branch_objects(root)?;
        self.invalidate_verb_cache_for_objects(&objects);
        self.invalidate_prop_cache_for_objects(&objects);
        self.invalidate_ancestry_cache_for_objects(&objects);
        Ok(())
    }
}
