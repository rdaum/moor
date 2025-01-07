// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use crate::fjall_provider::FjallProvider;
use crate::tx::{TransactionalCache, TransactionalTable, Tx};
use crate::worldstate_db::WorkingSets;
use crate::worldstate_transaction::WorldStateTransaction;
use crate::{BytesHolder, ObjAndUUIDHolder, StringHolder};
use bytes::Bytes;
use crossbeam_channel::Sender;
use moor_values::model::{
    BinaryType, CommitResult, HasUuid, Named, ObjAttrs, ObjFlag, ObjSet, ObjectRef, PropDef,
    PropDefs, PropFlag, PropPerms, ValSet, VerbArgsSpec, VerbAttrs, VerbDef, VerbDefs, VerbFlag,
    WorldStateError,
};
use moor_values::util::BitEnum;
use moor_values::{v_none, AsByteBuffer, Obj, Symbol, Var, NOTHING};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::AtomicI64;
use std::sync::Arc;
use uuid::Uuid;

type LC<Domain, Codomain> = TransactionalTable<
    Domain,
    Codomain,
    TransactionalCache<Domain, Codomain, FjallProvider<Domain, Codomain>>,
>;

pub const SEQUENCE_MAX_OBJECT: usize = 0;

pub struct DbTransaction {
    #[allow(dead_code)]
    pub(crate) tx: Tx,

    /// Channel to send our working set to the main thread for commit.
    /// Reply channel is used to send back the result of the commit.
    pub(crate) commit_channel: Sender<(WorkingSets, oneshot::Sender<CommitResult>)>,

    /// Channel to request the current disk usage of the database.
    /// Note that for now the usage doesn't include the current pending transaction.
    pub(crate) usage_channel: Sender<oneshot::Sender<usize>>,

    pub(crate) object_location: LC<Obj, Obj>,
    pub(crate) object_contents: LC<Obj, ObjSet>,
    pub(crate) object_flags: LC<Obj, BitEnum<ObjFlag>>,
    pub(crate) object_parent: LC<Obj, Obj>,
    pub(crate) object_children: LC<Obj, ObjSet>,
    pub(crate) object_owner: LC<Obj, Obj>,
    pub(crate) object_name: LC<Obj, StringHolder>,

    pub(crate) object_verbdefs: LC<Obj, VerbDefs>,
    pub(crate) object_verbs: LC<ObjAndUUIDHolder, BytesHolder>,
    pub(crate) object_propdefs: LC<Obj, PropDefs>,
    pub(crate) object_propvalues: LC<ObjAndUUIDHolder, Var>,
    pub(crate) object_propflags: LC<ObjAndUUIDHolder, PropPerms>,

    pub(crate) sequences: [Arc<AtomicI64>; 16],
}

impl WorldStateTransaction for DbTransaction {
    fn object_valid(&self, obj: &Obj) -> Result<bool, WorldStateError> {
        match self.object_flags.get(obj) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(WorldStateError::DatabaseError(format!(
                "Error getting object flags: {:?}",
                e
            ))),
        }
    }

    fn ancestors(&self, obj: &Obj) -> Result<ObjSet, WorldStateError> {
        let mut ancestors = vec![];
        let mut current = obj.clone();
        loop {
            ancestors.push(current.clone());
            match self.object_parent.get(&current) {
                Ok(Some(parent)) => current = parent,
                Ok(None) => break,
                Err(e) => {
                    panic!("Error getting parent: {:?}", e);
                }
            }
        }

        Ok(ancestors.into_iter().collect())
    }

    fn get_objects(&self) -> Result<ObjSet, WorldStateError> {
        let objects = self.object_flags.scan(&|_, _| true).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting objects: {:?}", e))
        })?;
        Ok(ObjSet::from_iter(objects.iter().map(|(k, _)| k.clone())))
    }

    fn get_object_flags(&self, obj: &Obj) -> Result<BitEnum<ObjFlag>, WorldStateError> {
        let r = self.object_flags.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object flags: {:?}", e))
        })?;
        Ok(r.unwrap_or_default())
    }

    fn get_players(&self) -> Result<ObjSet, WorldStateError> {
        let players = self
            .object_flags
            .scan(&|_, flags| flags.contains(ObjFlag::User))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting players: {:?}", e))
            })?;
        Ok(ObjSet::from_iter(players.iter().map(|(k, _)| k.clone())))
    }

    fn get_max_object(&self) -> Result<Obj, WorldStateError> {
        let seq_max = self.get_sequence(SEQUENCE_MAX_OBJECT);

        // Turn to i32, but check bounds against MAX_INT
        let seq_max = if seq_max < i32::MIN as i64 || seq_max > i32::MAX as i64 {
            return Err(WorldStateError::DatabaseError(format!(
                "Maximum object sequence number out of bounds: {}",
                seq_max
            )));
        } else {
            seq_max as i32
        };

        Ok(Obj::mk_id(seq_max))
    }

    fn get_object_owner(&self, obj: &Obj) -> Result<Obj, WorldStateError> {
        let r = self.object_owner.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object owner: {:?}", e))
        })?;

        Ok(r.unwrap_or(NOTHING))
    }

    fn set_object_owner(&self, obj: &Obj, owner: &Obj) -> Result<(), WorldStateError> {
        self.object_owner
            .upsert(obj.clone(), owner.clone())
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting object owner: {:?}", e))
            })?;
        Ok(())
    }

    fn set_object_flags(&self, obj: &Obj, flags: BitEnum<ObjFlag>) -> Result<(), WorldStateError> {
        self.object_flags.upsert(obj.clone(), flags).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error setting object flags: {:?}", e))
        })?;
        Ok(())
    }

    fn get_object_name(&self, obj: &Obj) -> Result<String, WorldStateError> {
        let r = self.object_name.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object name: {:?}", e))
        })?;
        let Some(r) = r else {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(obj.clone())));
        };
        Ok(r.0)
    }

    fn set_object_name(&self, obj: &Obj, name: String) -> Result<(), WorldStateError> {
        self.object_name
            .upsert(obj.clone(), StringHolder(name))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting object name: {:?}", e))
            })?;
        Ok(())
    }

    fn create_object(&self, id: Option<Obj>, attrs: ObjAttrs) -> Result<Obj, WorldStateError> {
        let id = match id {
            Some(id) => id,
            None => {
                let max = self.increment_sequence(SEQUENCE_MAX_OBJECT);
                let max = if max < i32::MIN as i64 || max > i32::MAX as i64 {
                    return Err(WorldStateError::DatabaseError(format!(
                        "Maximum object sequence number out of bounds: {}",
                        max
                    )));
                } else {
                    max as i32
                };
                Obj::mk_id(max)
            }
        };

        let owner = attrs.owner().unwrap_or(id.clone());
        self.object_owner
            .upsert(id.clone(), owner)
            .expect("Unable to insert initial owner");

        // Set initial name
        let name = attrs.name().unwrap_or_default();
        self.object_name
            .upsert(id.clone(), StringHolder(name))
            .expect("Unable to insert initial name");

        // We use our own setters for these, since there's biz-logic attached here...
        if let Some(parent) = attrs.parent() {
            self.set_object_parent(&id, &parent)
                .expect("Unable to set parent");
        }
        if let Some(location) = attrs.location() {
            self.set_object_location(&id, &location)
                .expect("Unable to set location");
        }

        self.object_flags
            .upsert(id.clone(), attrs.flags())
            .expect("Unable to insert initial flags");

        // Update the maximum object number if ours is higher than the current one. This is for the
        // textdump case, where our numbers are coming in arbitrarily.
        self.update_sequence_max(SEQUENCE_MAX_OBJECT, id.id().0 as i64);

        Ok(id)
    }

    fn recycle_object(&self, obj: &Obj) -> Result<(), WorldStateError> {
        // First go through and move all objects that are in this object's contents to the
        // to #-1.  It's up to the caller here to execute :exitfunc on all of them before invoking
        // this method.

        let contents = self.get_object_contents(obj)?;
        for c in contents.iter() {
            self.set_object_location(&c, &NOTHING)?;
        }

        // Now reparent all our immediate children to our parent.
        // This should properly move all properties all the way down the chain.
        let parent = self.get_object_parent(obj)?;
        let children = self.get_object_children(obj)?;
        for c in children.iter() {
            self.set_object_parent(&c, &parent)?;
        }

        // Make sure we are removed from the parent's children list.
        let parent_children = self.get_object_children(&parent)?;
        let parent_children = parent_children.with_removed(obj.clone());
        self.object_children
            .upsert(parent.clone(), parent_children)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error updating parent children: {:?}", e))
            })?;

        // Now we can remove this object from all relevant relations
        // First the simple ones which are keyed on the object id.
        self.object_flags.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object flags: {:?}", e))
        })?;
        self.object_name.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object name: {:?}", e))
        })?;
        self.object_children.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object children: {:?}", e))
        })?;
        self.object_owner.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object owner: {:?}", e))
        })?;
        self.object_parent.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object parent: {:?}", e))
        })?;
        self.object_location.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object location: {:?}", e))
        })?;
        self.object_verbdefs.delete(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error deleting object verbdefs: {:?}", e))
        })?;

        let propdefs = self.get_properties(obj)?;
        for p in propdefs.iter() {
            self.object_propvalues
                .delete(&ObjAndUUIDHolder::new(obj, p.uuid()))
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!(
                        "Error deleting property value: {:?}",
                        e
                    ))
                })?;
        }

        // We may or may not have propdefs yet...
        self.object_propdefs.delete(obj).ok();

        Ok(())
    }

    fn get_object_parent(&self, obj: &Obj) -> Result<Obj, WorldStateError> {
        let r = self.object_parent.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object parent: {:?}", e))
        })?;
        Ok(r.unwrap_or(NOTHING))
    }

    fn set_object_parent(&self, o: &Obj, new_parent: &Obj) -> Result<(), WorldStateError> {
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
            self.closest_common_ancestor_with_ancestors(new_parent, o)?;

        // Remove from _me_ any of the properties defined by any of my ancestors
        let old_props = self.get_properties(o)?;
        if !old_props.is_empty() {
            let mut delort_props = vec![];
            for p in old_props.iter() {
                if old_ancestors.contains(&p.definer()) {
                    delort_props.push(p.uuid());

                    self.object_propvalues
                        .delete(&ObjAndUUIDHolder::new(o, p.uuid()))
                        .expect("Unable to delete property value");
                }
            }
            let new_props = old_props.with_all_removed(&delort_props);
            self.object_propdefs
                .upsert(o.clone(), new_props)
                .expect("Unable to update propdefs");
        }

        // Now walk all-my-children and destroy all the properties whose definer is me or any
        // of my ancestors not shared by the new parent.
        let descendants = self.descendants(o)?;

        let mut descendant_props = HashMap::new();
        for c in descendants.iter() {
            let mut inherited_props = vec![];
            // Remove the set common.
            let old_props = self.get_properties(o)?;
            if !old_props.is_empty() {
                for p in old_props.iter() {
                    if old_ancestors.contains(&p.definer()) {
                        inherited_props.push(p.uuid());
                        self.object_propvalues
                            .delete(&ObjAndUUIDHolder::new(&c.clone(), p.uuid()))
                            .expect("Unable to delete property value");
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
        let old_parent = self.get_object_parent(o)?;
        if !old_parent.is_nothing() && old_parent.eq(new_parent) {
            return Ok(());
        };

        self.object_parent
            .upsert(o.clone(), new_parent.clone())
            .expect("Unable to update parent");

        // Make sure the old_parent's children now have use removed.
        let old_parent_children = self.get_object_children(&old_parent)?;
        let old_parent_children = old_parent_children.with_removed(o.clone());
        self.object_children
            .upsert(old_parent.clone(), old_parent_children)
            .expect("Unable to update children");

        if new_parent.is_nothing() {
            return Ok(());
        }

        // And add to the new parent's children.
        let new_parent_children = self.get_object_children(new_parent)?;
        let new_parent_children = new_parent_children.with_appended(&[o.clone()]);
        self.object_children
            .upsert(new_parent.clone(), new_parent_children)
            .expect("Unable to update children");

        // Now walk all my new descendants and give them the properties that derive from any
        // ancestors they don't already share.

        // Now collect properties defined on the new ancestors so we can define the owners on
        // the new descendants.
        let mut new_props = vec![];
        for a in new_ancestors {
            let props = self.get_properties(&a)?;
            if !props.is_empty() {
                for p in props.iter() {
                    if p.definer().eq(&a) {
                        if let Some(propperms) = self
                            .object_propflags
                            .get(&ObjAndUUIDHolder::new(&a, p.uuid()))
                            .map_err(|e| {
                                WorldStateError::DatabaseError(format!(
                                    "Error getting object flags: {:?}",
                                    e
                                ))
                            })?
                        {
                            new_props.push((p.clone(), propperms));
                        }
                    }
                }
            }
        }
        // Then put clear copies on each of the descendants ... and me.
        // This really just means defining the property with no value, which is what we do.
        let descendants = self.descendants(o).expect("Unable to get descendants");
        for c in descendants.iter().chain(std::iter::once(o.clone())) {
            for (p, propperms) in new_props.iter() {
                self.object_propflags
                    .upsert(ObjAndUUIDHolder::new(&c, p.uuid()), propperms.clone())
                    .expect("Unable to update property flags");
            }
        }
        Ok(())
    }

    fn get_object_children(&self, obj: &Obj) -> Result<ObjSet, WorldStateError> {
        let r = self.object_children.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object children: {:?}", e))
        })?;
        Ok(r.unwrap_or_default())
    }

    fn get_object_location(&self, obj: &Obj) -> Result<Obj, WorldStateError> {
        let r = self.object_location.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object location: {:?}", e))
        })?;
        Ok(r.unwrap_or(NOTHING))
    }

    fn get_object_contents(&self, obj: &Obj) -> Result<ObjSet, WorldStateError> {
        let r = self.object_contents.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object contents: {:?}", e))
        })?;
        Ok(r.unwrap_or_default())
    }

    fn get_object_size_bytes(&self, obj: &Obj) -> Result<usize, WorldStateError> {
        // Means retrieving the common for all of the objects attributes, and then summing their sizes.
        // This is remarkably inefficient.

        let flags = self.get_object_flags(obj)?;
        let name = self.object_name.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object name: {:?}", e))
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
        let verbs = verbdefs.iter().map(|v| self.get_verb_binary(obj, v.uuid()));

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
            size += v.map(|v| v.len()).unwrap_or_default();
        }

        Ok(size)
    }

    fn set_object_location(&self, what: &Obj, new_location: &Obj) -> Result<(), WorldStateError> {
        // Detect recursive move
        let mut oid = new_location.clone();
        loop {
            if oid.is_nothing() {
                break;
            }
            if oid.eq(what) {
                return Err(WorldStateError::RecursiveMove(
                    what.clone(),
                    new_location.clone(),
                ));
            }
            let location = self.object_location.get(&oid).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting object location: {:?}", e))
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
            WorldStateError::DatabaseError(format!("Error getting object location: {:?}", e))
        })?;

        if let Some(old_location) = &old_location {
            if old_location.eq(new_location) {
                return Ok(());
            }
        }

        // Set new location.
        self.object_location
            .upsert(what.clone(), new_location.clone())
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting object location: {:?}", e))
            })?;

        // Now need to update contents in both.
        if let Some(old_location) = old_location {
            let old_contents = self.object_contents.get(&old_location).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting object contents: {:?}", e))
            })?;

            let old_contents = old_contents.unwrap_or_default().with_removed(what.clone());

            self.object_contents
                .upsert(old_location.clone(), old_contents)
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!(
                        "Error setting object contents: {:?}",
                        e
                    ))
                })?;
        }

        let new_contents = self.object_contents.get(new_location).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting object contents: {:?}", e))
        })?;
        let new_contents = new_contents
            .unwrap_or_default()
            .with_appended(&[what.clone()]);
        self.object_contents
            .upsert(new_location.clone(), new_contents)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting object contents: {:?}", e))
            })?;

        if new_location.is_nothing() {
            return Ok(());
        }

        Ok(())
    }

    fn get_verbs(&self, obj: &Obj) -> Result<VerbDefs, WorldStateError> {
        let r = self
            .object_verbdefs
            .get(obj)
            .map_err(|e| WorldStateError::DatabaseError(format!("Error getting verbs: {:?}", e)))?;
        Ok(r.unwrap_or_else(VerbDefs::empty))
    }

    fn get_verb_binary(&self, obj: &Obj, uuid: Uuid) -> Result<Bytes, WorldStateError> {
        let r = self
            .object_verbs
            .get(&ObjAndUUIDHolder::new(obj, uuid))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting verb binary: {:?}", e))
            })?;
        let Some(binary) = r else {
            return Err(WorldStateError::VerbNotFound(
                obj.clone(),
                format!("{}", uuid),
            ));
        };
        Ok(Bytes::from(binary.0))
    }

    fn get_verb_by_name(&self, obj: &Obj, name: Symbol) -> Result<VerbDef, WorldStateError> {
        let verbdefs = self.get_verbs(obj)?;
        let named = verbdefs.find_named(name);
        let verb = named
            .first()
            .ok_or_else(|| WorldStateError::VerbNotFound(obj.clone(), name.to_string()))?;
        Ok(verb.clone())
    }

    fn get_verb_by_index(&self, obj: &Obj, index: usize) -> Result<VerbDef, WorldStateError> {
        let verbs = self.get_verbs(obj)?;
        if index >= verbs.len() {
            return Err(WorldStateError::VerbNotFound(
                obj.clone(),
                format!("{}", index),
            ));
        }
        let verb = verbs
            .iter()
            .nth(index)
            .ok_or_else(|| WorldStateError::VerbNotFound(obj.clone(), format!("{}", index)))?;
        Ok(verb.clone())
    }

    fn resolve_verb(
        &self,
        obj: &Obj,
        name: Symbol,
        argspec: Option<VerbArgsSpec>,
    ) -> Result<VerbDef, WorldStateError> {
        let mut search_o = obj.clone();
        loop {
            let verbdefs = self.object_verbdefs.get(&search_o).map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting verbs: {:?}", e))
            })?;
            if let Some(verbdefs) = verbdefs {
                let named = verbdefs.find_named(name);
                let verb = named.first();
                if let Some(verb) = verb {
                    let Some(argspec) = argspec else {
                        return Ok(verb.clone());
                    };

                    if verb.args().matches(&argspec) {
                        return Ok(verb.clone());
                    }
                }
            }
            search_o = self.get_object_parent(&search_o)?;
            if search_o.is_nothing() {
                break;
            }
        }

        Err(WorldStateError::VerbNotFound(obj.clone(), name.to_string()))
    }

    fn update_verb(
        &self,
        obj: &Obj,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let verbdefs = self.get_verbs(obj)?;

        let Some(verbdefs) = verbdefs.with_updated(uuid, |ov| {
            let names = match &verb_attrs.names {
                None => ov.names(),
                Some(new_names) => new_names.iter().map(|n| n.as_str()).collect::<Vec<&str>>(),
            };
            VerbDef::new(
                ov.uuid(),
                ov.location(),
                verb_attrs.owner.clone().unwrap_or(ov.owner()),
                &names,
                verb_attrs.flags.unwrap_or(ov.flags()),
                verb_attrs.binary_type.unwrap_or(ov.binary_type()),
                verb_attrs.args_spec.unwrap_or(ov.args()),
            )
        }) else {
            return Err(WorldStateError::VerbNotFound(
                obj.clone(),
                format!("{}", uuid),
            ));
        };

        self.object_verbdefs
            .upsert(obj.clone(), verbdefs)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting verb definition: {:?}", e))
            })?;

        if verb_attrs.binary.is_some() {
            self.object_verbs
                .upsert(
                    ObjAndUUIDHolder::new(obj, uuid),
                    BytesHolder(verb_attrs.binary.unwrap()),
                )
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!("Error setting verb binary: {:?}", e))
                })?;
        }
        Ok(())
    }

    fn add_object_verb(
        &self,
        oid: &Obj,
        owner: &Obj,
        names: Vec<Symbol>,
        binary: Vec<u8>,
        binary_type: BinaryType,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), WorldStateError> {
        let verbdefs = self.get_verbs(oid)?;

        let uuid = Uuid::new_v4();
        let verbdef = VerbDef::new(
            uuid,
            oid.clone(),
            owner.clone(),
            &names.iter().map(|n| n.as_str()).collect::<Vec<&str>>(),
            flags,
            binary_type,
            args,
        );

        let verbdefs = verbdefs.with_added(verbdef);

        self.object_verbdefs
            .upsert(oid.clone(), verbdefs)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting verb definition: {:?}", e))
            })?;

        self.object_verbs
            .upsert(ObjAndUUIDHolder::new(oid, uuid), BytesHolder(binary))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting verb binary: {:?}", e))
            })?;

        Ok(())
    }

    fn delete_verb(&self, location: &Obj, uuid: Uuid) -> Result<(), WorldStateError> {
        let verbdefs = self.get_verbs(location)?;
        let verbdefs = verbdefs
            .with_removed(uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(location.clone(), format!("{}", uuid)))?;

        self.object_verbdefs
            .upsert(location.clone(), verbdefs)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting verb definition: {:?}", e))
            })?;

        self.object_verbs
            .delete(&ObjAndUUIDHolder::new(location, uuid))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error deleting verb binary: {:?}", e))
            })?;
        Ok(())
    }

    fn get_properties(&self, obj: &Obj) -> Result<PropDefs, WorldStateError> {
        let r = self.object_propdefs.get(obj).map_err(|e| {
            WorldStateError::DatabaseError(format!("Error getting properties: {:?}", e))
        })?;
        Ok(r.unwrap_or_else(PropDefs::empty))
    }

    fn set_property(&self, obj: &Obj, uuid: Uuid, value: Var) -> Result<(), WorldStateError> {
        self.object_propvalues
            .upsert(ObjAndUUIDHolder::new(obj, uuid), value)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error setting property value: {:?}", e))
            })?;
        Ok(())
    }

    fn define_property(
        &self,
        definer: &Obj,
        location: &Obj,
        name: Symbol,
        owner: &Obj,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<Uuid, WorldStateError> {
        let descendants = self.descendants(location)?;

        // If the property is already defined at us or above or below us, that's a failure.
        let props = self.get_properties(location)?;
        if props.find_first_named(name).is_some() {
            return Err(WorldStateError::DuplicatePropertyDefinition(
                location.clone(),
                name.to_string(),
            ));
        }
        let ancestors = self.ancestors(location)?;
        let check_locations = ObjSet::from_items(&[location.clone()]).with_concatenated(ancestors);
        for location in check_locations.iter() {
            let descendant_props = self.get_properties(&location)?;

            // Verify we don't already have a property with this name. If we do, return an error.
            if descendant_props.find_first_named(name).is_some() {
                return Err(WorldStateError::DuplicatePropertyDefinition(
                    location,
                    name.to_string(),
                ));
            }
        }

        // Generate a new property ID. This will get shared all the way down the pipe.
        // But the key for the actual value is always composite of oid,uuid
        let u = Uuid::new_v4();

        let prop = PropDef::new(u, definer.clone(), location.clone(), name.as_str());
        self.object_propdefs
            .upsert(location.clone(), props.with_added(prop))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!(
                    "Error setting property definition: {:?}",
                    e
                ))
            })?;

        // If we have an initial value, set it, but just on ourselves. Descendants start out clear.
        if let Some(value) = value {
            self.set_property(location, u, value)?;
        }

        // Put the initial object owner on ourselves and all our descendants.
        let value_locations =
            ObjSet::from_items(&[location.clone()]).with_concatenated(descendants);
        for location in value_locations.iter() {
            self.object_propflags
                .upsert(
                    ObjAndUUIDHolder::new(&location, u),
                    PropPerms::new(owner.clone(), perms),
                )
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!("Error setting property owner: {:?}", e))
                })?;
        }

        Ok(u)
    }

    fn update_property_info(
        &self,
        obj: &Obj,
        uuid: Uuid,
        new_owner: Option<Obj>,
        new_flags: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
    ) -> Result<(), WorldStateError> {
        if new_owner.is_none() && new_flags.is_none() && new_name.is_none() {
            return Ok(());
        }

        // We only need to update the propdef if there's a new name.
        if let Some(new_name) = new_name {
            let props = self.get_properties(obj)?;

            let Some(props) = props.with_updated(uuid, |p| {
                PropDef::new(p.uuid(), p.definer(), p.location(), &new_name)
            }) else {
                return Err(WorldStateError::PropertyNotFound(
                    obj.clone(),
                    format!("{}", uuid),
                ));
            };

            self.object_propdefs
                .upsert(obj.clone(), props)
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!("Error updating property: {:?}", e))
                })?;
        }

        // If flags or perms updated, do that.
        if new_flags.is_some() || new_owner.is_some() {
            let mut perms = self.retrieve_property_permissions(obj, uuid)?;

            if let Some(new_flags) = new_flags {
                perms = perms.with_flags(new_flags);
            }

            if let Some(new_owner) = new_owner {
                perms = perms.with_owner(new_owner);
            }

            self.object_propflags
                .upsert(ObjAndUUIDHolder::new(obj, uuid), perms)
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!("Error updating property: {:?}", e))
                })?;
        }

        Ok(())
    }

    fn clear_property(&self, obj: &Obj, uuid: Uuid) -> Result<(), WorldStateError> {
        // remove property value
        self.object_propvalues
            .delete(&ObjAndUUIDHolder::new(obj, uuid))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error clearing property value: {:?}", e))
            })?;
        Ok(())
    }

    fn delete_property(&self, obj: &Obj, uuid: Uuid) -> Result<(), WorldStateError> {
        // delete propdef from self and all descendants
        let descendants = self.descendants(obj)?;
        let locations = ObjSet::from_items(&[obj.clone()]).with_concatenated(descendants);
        for location in locations.iter() {
            let props: PropDefs = self.get_properties(&location)?;
            let props = props
                .with_removed(uuid)
                .expect("Unable to remove property definition");

            self.object_propdefs
                .upsert(location.clone(), props)
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!("Error deleting property: {:?}", e))
                })?;
        }
        Ok(())
    }

    fn retrieve_property(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError> {
        let r = self
            .object_propvalues
            .get(&ObjAndUUIDHolder::new(obj, uuid))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting property value: {:?}", e))
            })?;
        let value = r;

        let perms = self.retrieve_property_permissions(obj, uuid)?;

        Ok((value, perms))
    }

    fn retrieve_property_permissions(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<PropPerms, WorldStateError> {
        let r = self
            .object_propflags
            .get(&ObjAndUUIDHolder::new(obj, uuid))
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting property flags: {:?}", e))
            })?;
        let Some(perms) = r else {
            return Err(WorldStateError::DatabaseError(
                "Property missing perms ".to_string(),
            ));
        };
        Ok(perms)
    }

    fn resolve_property(
        &self,
        obj: &Obj,
        name: Symbol,
    ) -> Result<(PropDef, Var, PropPerms, bool), WorldStateError> {
        // Walk up the inheritance tree looking for the property definition.
        let mut search_obj = obj.clone();
        let propdef = loop {
            let propdef = self.get_properties(&search_obj)?.find_first_named(name);

            if let Some(propdef) = propdef {
                break propdef;
            }

            let parent = self.get_object_parent(&search_obj)?;
            if !parent.is_nothing() {
                search_obj = parent;
                continue;
            };

            return Err(WorldStateError::PropertyNotFound(
                obj.clone(),
                name.to_string(),
            ));
        };

        // Now that we have the propdef, we can look for the value & owner.
        // We should *always* have the owner.
        // But value could be 'clear' in which case we need to look in the parent.
        let (pvalue, perms) = self.retrieve_property(obj, propdef.uuid())?;
        match pvalue {
            Some(value) => Ok((propdef, value, perms, false)),
            None => {
                let mut search_obj = obj.clone();
                loop {
                    let parent = self.get_object_parent(&search_obj)?;
                    if parent.is_nothing() {
                        break Ok((propdef, v_none(), perms, true));
                    }
                    search_obj = parent;

                    let value = self
                        .object_propvalues
                        .get(&ObjAndUUIDHolder::new(&search_obj, propdef.uuid()))
                        .map_err(|e| {
                            WorldStateError::DatabaseError(format!(
                                "Error getting property value: {:?}",
                                e
                            ))
                        })?;
                    if let Some(value) = value {
                        break Ok((propdef, value, perms, true));
                    }
                }
            }
        }
    }

    fn db_usage(&self) -> Result<usize, WorldStateError> {
        let (send, receive) = oneshot::channel();
        self.usage_channel
            .send(send)
            .expect("Unable to send usage request");
        Ok(receive.recv().expect("Unable to receive usage response"))
    }

    fn commit(self) -> Result<CommitResult, WorldStateError> {
        // Pull out the working sets
        let object_location = self.object_location.working_set();
        let object_contents = self.object_contents.working_set();
        let object_parent = self.object_parent.working_set();
        let object_children = self.object_children.working_set();
        let object_owner = self.object_owner.working_set();
        let object_flags = self.object_flags.working_set();
        let object_name = self.object_name.working_set();
        let object_verbdefs = self.object_verbdefs.working_set();
        let object_verbs = self.object_verbs.working_set();
        let object_propdefs = self.object_propdefs.working_set();
        let object_propvalues = self.object_propvalues.working_set();
        let object_propflags = self.object_propflags.working_set();

        let ws = WorkingSets {
            tx: self.tx,
            object_location,
            object_contents,
            object_flags,
            object_parent,
            object_children,
            object_owner,
            object_name,
            object_verbdefs,
            object_verbs,
            object_propdefs,
            object_propvalues,
            object_propflags,
        };

        // Send the working sets to the commit processing thread
        let (send, reply) = oneshot::channel();
        self.commit_channel.send((ws, send)).unwrap();

        // Wait for the reply
        Ok(reply.recv().expect("Error waiting for commit reply"))
    }

    fn rollback(self) -> Result<(), WorldStateError> {
        // Just drop the transaction, it will be cleaned up by the drop impl.
        Ok(())
    }

    fn descendants(&self, obj: &Obj) -> Result<ObjSet, WorldStateError> {
        let children = self
            .object_children
            .get(obj)
            .map_err(|e| {
                WorldStateError::DatabaseError(format!("Error getting object children: {:?}", e))
            })?
            .unwrap_or_else(ObjSet::empty);

        let mut descendants = vec![];
        let mut queue: VecDeque<_> = children.iter().collect();
        while let Some(o) = queue.pop_front() {
            descendants.push(o.clone());
            let children = self
                .object_children
                .get(&o)
                .map_err(|e| {
                    WorldStateError::DatabaseError(format!(
                        "Error getting object children: {:?}",
                        e
                    ))
                })?
                .unwrap_or_else(ObjSet::empty);
            queue.extend(children.iter());
        }

        Ok(ObjSet::from_items(&descendants))
    }
}

impl DbTransaction {
    /// Increment the given sequence, return the new value.
    fn increment_sequence(&self, seq: usize) -> i64 {
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

    #[allow(clippy::type_complexity)]
    fn closest_common_ancestor_with_ancestors(
        &self,
        a: &Obj,
        b: &Obj,
    ) -> Result<(Option<Obj>, HashSet<Obj>, HashSet<Obj>), WorldStateError> {
        let mut ancestors_a = HashSet::new();
        let mut search_a = a.clone();

        let mut ancestors_b = HashSet::new();
        let mut search_b = b.clone();

        loop {
            if search_a.is_nothing() && search_b.is_nothing() {
                return Ok((None, ancestors_a, ancestors_b)); // No common ancestor found
            }

            if ancestors_b.contains(&search_a) {
                return Ok((Some(search_a.clone()), ancestors_a, ancestors_b)); // Common ancestor found
            }

            if ancestors_a.contains(&search_b) {
                return Ok((Some(search_b.clone()), ancestors_a, ancestors_b)); // Common ancestor found
            }

            if !search_a.is_nothing() {
                ancestors_a.insert(search_a.clone());
                let parent = self.get_object_parent(&search_a)?;
                search_a = parent.clone();
            }

            if !search_b.is_nothing() {
                ancestors_b.insert(search_b.clone());
                let parent = self.get_object_parent(&search_b)?;
                search_b = parent.clone();
            }
        }
    }
}
