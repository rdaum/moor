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

use crate::worldstate_transaction::WorldStateTransaction;
use crate::{
    BytesHolder, RelationalError, RelationalTransaction, StringHolder, UUIDHolder,
    WorldStateSequence, WorldStateTable,
};
use bytes::Bytes;
use moor_values::model::{
    BinaryType, CommitResult, HasUuid, Named, ObjAttrs, ObjFlag, ObjSet, ObjectRef, PropDef,
    PropDefs, PropFlag, PropPerms, ValSet, VerbArgsSpec, VerbAttrs, VerbDef, VerbDefs, VerbFlag,
    WorldStateError,
};
use moor_values::util::BitEnum;
use moor_values::Symbol;
use moor_values::NOTHING;
use moor_values::{v_none, Objid, Var};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

fn err_map(e: RelationalError) -> WorldStateError {
    match e {
        RelationalError::ConflictRetry => WorldStateError::RollbackRetry,
        _ => WorldStateError::DatabaseError(format!("{:?}", e)),
    }
}

pub struct RelationalWorldStateTransaction<RTX: RelationalTransaction<WorldStateTable>> {
    pub tx: Option<RTX>,
}

impl<RTX: RelationalTransaction<WorldStateTable>> Drop for RelationalWorldStateTransaction<RTX> {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            tx.rollback();
        }
    }
}

impl<RTX: RelationalTransaction<WorldStateTable>> WorldStateTransaction
    for RelationalWorldStateTransaction<RTX>
{
    fn object_valid(&self, obj: &Objid) -> Result<bool, WorldStateError> {
        let ov: Option<Objid> = self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain(WorldStateTable::ObjectOwner, obj)
            .map_err(err_map)?;
        Ok(ov.is_some())
    }

    fn ancestors(&self, obj: &Objid) -> Result<ObjSet, WorldStateError> {
        let mut ancestors = vec![];
        let mut search = obj.clone();
        loop {
            if search.is_nothing() {
                break;
            }
            ancestors.push(search.clone());
            let parent = self
                .tx
                .as_ref()
                .unwrap()
                .seek_unique_by_domain(WorldStateTable::ObjectParent, &search)
                .map_err(err_map)?
                .unwrap_or(NOTHING);
            search = parent;
        }
        Ok(ObjSet::from_items(&ancestors))
    }

    fn get_objects(&self) -> Result<ObjSet, WorldStateError> {
        let objs = self
            .tx
            .as_ref()
            .unwrap()
            .scan_with_predicate(
                WorldStateTable::ObjectFlags,
                |&_: &Objid, _: &BitEnum<ObjFlag>| true,
            )
            .map_err(err_map)?;
        Ok(ObjSet::from_iter(objs.iter().map(|(o, _)| o.clone())))
    }

    fn get_object_flags(&self, obj: &Objid) -> Result<BitEnum<ObjFlag>, WorldStateError> {
        self.tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain(WorldStateTable::ObjectFlags, obj)
            .map_err(err_map)?
            .ok_or(WorldStateError::ObjectNotFound(ObjectRef::Id(obj.clone())))
    }

    fn get_players(&self) -> Result<ObjSet, WorldStateError> {
        // TODO: Improve get_players retrieval in world state
        //   this is going to be not-at-all performant in the long run, and we'll need a way to
        //   cache this or index it better
        let players = self
            .tx
            .as_ref()
            .unwrap()
            .scan_with_predicate(
                WorldStateTable::ObjectFlags,
                |_: &Objid, flags: &BitEnum<ObjFlag>| flags.contains(ObjFlag::User),
            )
            .map_err(err_map)?;
        Ok(ObjSet::from_iter(players.iter().map(|(o, _)| o.clone())))
    }

    fn get_max_object(&self) -> Result<Objid, WorldStateError> {
        Ok(Objid(
            self.tx
                .as_ref()
                .unwrap()
                .get_sequence(WorldStateSequence::MaximumObject)
                .unwrap_or(-1),
        ))
    }

    fn get_object_owner(&self, obj: &Objid) -> Result<Objid, WorldStateError> {
        self.tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain(WorldStateTable::ObjectOwner, obj)
            .map_err(err_map)?
            .ok_or(WorldStateError::ObjectNotFound(ObjectRef::Id(obj.clone())))
    }

    fn set_object_owner(&self, obj: &Objid, owner: &Objid) -> Result<(), WorldStateError> {
        self.tx
            .as_ref()
            .unwrap()
            .upsert(WorldStateTable::ObjectOwner, obj, owner)
            .map_err(err_map)
    }

    fn set_object_flags(
        &self,
        obj: &Objid,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError> {
        self.tx
            .as_ref()
            .unwrap()
            .upsert(WorldStateTable::ObjectFlags, obj, &flags)
            .map_err(err_map)
    }

    fn get_object_name(&self, obj: &Objid) -> Result<String, WorldStateError> {
        let sh: StringHolder = self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain(WorldStateTable::ObjectName, obj)
            .map_err(err_map)?
            .ok_or(WorldStateError::ObjectNotFound(ObjectRef::Id(obj.clone())))?;
        Ok(sh.0)
    }

    fn set_object_name(&self, obj: &Objid, name: String) -> Result<(), WorldStateError> {
        self.tx
            .as_ref()
            .unwrap()
            .upsert(WorldStateTable::ObjectName, obj, &StringHolder(name))
            .map_err(err_map)
    }

    fn create_object(&self, id: Option<Objid>, attrs: ObjAttrs) -> Result<Objid, WorldStateError> {
        let id = match id {
            Some(id) => id,
            None => {
                let max = self
                    .tx
                    .as_ref()
                    .unwrap()
                    .increment_sequence(WorldStateSequence::MaximumObject);
                Objid(max)
            }
        };

        let owner = attrs.owner().unwrap_or(id.clone());
        self.tx
            .as_ref()
            .unwrap()
            .upsert(WorldStateTable::ObjectOwner, &id, &owner)
            .expect("Unable to insert initial owner");

        // Set initial name
        let name = attrs.name().unwrap_or_default();
        self.tx
            .as_ref()
            .unwrap()
            .upsert(WorldStateTable::ObjectName, &id, &StringHolder(name))
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

        self.tx
            .as_ref()
            .unwrap()
            .upsert(WorldStateTable::ObjectFlags, &id, &attrs.flags())
            .expect("Unable to insert initial flags");

        // Update the maximum object number if ours is higher than the current one. This is for the
        // textdump case, where our numbers are coming in arbitrarily.
        self.tx
            .as_ref()
            .unwrap()
            .update_sequence_max(WorldStateSequence::MaximumObject, id.0);

        Ok(id)
    }

    fn recycle_object(&self, obj: &Objid) -> Result<(), WorldStateError> {
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

        // Now we can remove this object from all relevant column relations
        // First the simple ones which are keyed on the object id.
        let oid_relations = [
            WorldStateTable::ObjectFlags,
            WorldStateTable::ObjectName,
            WorldStateTable::ObjectOwner,
            WorldStateTable::ObjectParent,
            WorldStateTable::ObjectLocation,
            WorldStateTable::ObjectVerbs,
        ];
        for rel in oid_relations.iter() {
            // It's ok to get NotFound here, since we're deleting anyways.
            // In particular for ObjectParent, we may not have a tuple.
            match self.tx.as_ref().unwrap().remove_by_domain(*rel, obj) {
                Ok(_) => {}
                Err(RelationalError::NotFound) => {}
                Err(e) => return Err(err_map(e)),
            }
        }

        let propdefs = self.get_properties(obj)?;
        for p in propdefs.iter() {
            self.tx
                .as_ref()
                .unwrap()
                .delete_composite_if_exists(
                    WorldStateTable::ObjectPropertyValue,
                    obj,
                    &UUIDHolder(p.uuid()),
                )
                .unwrap_or(());
        }

        // We may or may not have propdefs yet...
        match self
            .tx
            .as_ref()
            .unwrap()
            .remove_by_domain(WorldStateTable::ObjectPropDefs, obj)
        {
            Ok(_) => {}
            Err(RelationalError::NotFound) => {}
            Err(e) => return Err(err_map(e)),
        }

        Ok(())
    }

    fn get_object_parent(&self, obj: &Objid) -> Result<Objid, WorldStateError> {
        Ok(self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain(WorldStateTable::ObjectParent, obj)
            .map_err(err_map)?
            .unwrap_or(NOTHING))
    }

    // TODO: wiredtiger has joins. we should add join&transitive join to the interface and use it
    fn set_object_parent(&self, o: &Objid, new_parent: &Objid) -> Result<(), WorldStateError> {
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
        if let Some(old_props) = self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain::<Objid, PropDefs>(WorldStateTable::ObjectPropDefs, o)
            .map_err(err_map)?
        {
            let mut delort_props = vec![];
            for p in old_props.iter() {
                if old_ancestors.contains(&p.definer()) {
                    delort_props.push(p.uuid());

                    self.tx
                        .as_ref()
                        .unwrap()
                        .delete_composite_if_exists(
                            WorldStateTable::ObjectPropertyValue,
                            o,
                            &UUIDHolder(p.uuid()),
                        )
                        .expect("Unable to delete property");
                }
            }
            let new_props = old_props.with_all_removed(&delort_props);
            self.tx
                .as_ref()
                .unwrap()
                .upsert(WorldStateTable::ObjectPropDefs, o, &new_props)
                .expect("Unable to update propdefs");
        }

        // Now walk all-my-children and destroy all the properties whose definer is me or any
        // of my ancestors not shared by the new parent.
        let descendants = self.descendants(o)?;

        let mut descendant_props = HashMap::new();
        for c in descendants.iter() {
            let mut inherited_props = vec![];
            // Remove the set values.
            if let Some(old_props) = self
                .tx
                .as_ref()
                .unwrap()
                .seek_unique_by_domain::<Objid, PropDefs>(WorldStateTable::ObjectPropDefs, o)
                .map_err(err_map)?
            {
                for p in old_props.iter() {
                    if old_ancestors.contains(&p.definer()) {
                        inherited_props.push(p.uuid());
                        self.tx
                            .as_ref()
                            .unwrap()
                            .delete_composite_if_exists(
                                WorldStateTable::ObjectPropertyValue,
                                &c,
                                &UUIDHolder(p.uuid()),
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
        if let Some(old_parent) = self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain::<Objid, Objid>(WorldStateTable::ObjectParent, o)
            .map_err(err_map)?
        {
            if old_parent.eq(new_parent) {
                return Ok(());
            }
        };

        self.tx
            .as_ref()
            .unwrap()
            .upsert(WorldStateTable::ObjectParent, o, new_parent)
            .expect("Unable to update parent");

        if new_parent.is_nothing() {
            return Ok(());
        }

        // Now walk all my new descendants and give them the properties that derive from any
        // ancestors they don't already share.

        // Now collect properties defined on the new ancestors so we can define the owners on
        // the new descendants.
        let mut new_props = vec![];
        for a in new_ancestors {
            if let Some(props) = self
                .tx
                .as_ref()
                .unwrap()
                .seek_unique_by_domain::<Objid, PropDefs>(WorldStateTable::ObjectPropDefs, &a)
                .map_err(err_map)?
            {
                for p in props.iter() {
                    if p.definer().eq(&a) {
                        let propperms = self
                            .tx
                            .as_ref()
                            .unwrap()
                            .seek_by_unique_composite_domain::<_, _, PropPerms>(
                                WorldStateTable::ObjectPropertyPermissions,
                                &a,
                                &UUIDHolder(p.uuid()),
                            )
                            .map_err(err_map)?
                            .expect("Unable to get property permissions");
                        new_props.push((p.clone(), propperms));
                    }
                }
            }
        }
        // Then put clear copies on each of the descendants ... and me.
        // This really just means defining the property with no value, which is what we do.
        let descendants = self.descendants(o).expect("Unable to get descendants");
        for c in descendants.iter().chain(std::iter::once(o.clone())) {
            for (p, propperms) in new_props.iter() {
                self.tx
                    .as_ref()
                    .unwrap()
                    .upsert_composite(
                        WorldStateTable::ObjectPropertyPermissions,
                        &c,
                        &UUIDHolder(p.uuid()),
                        propperms,
                    )
                    .expect("Unable to update property permissions");
            }
        }
        Ok(())
    }

    fn get_object_children(&self, obj: &Objid) -> Result<ObjSet, WorldStateError> {
        self.tx
            .as_ref()
            .unwrap()
            .seek_by_codomain::<Objid, Objid, ObjSet>(WorldStateTable::ObjectParent, obj)
            .map_err(err_map)
    }

    fn get_object_location(&self, obj: &Objid) -> Result<Objid, WorldStateError> {
        Ok(self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain(WorldStateTable::ObjectLocation, obj)
            .map_err(err_map)?
            .unwrap_or(NOTHING))
    }

    fn get_object_contents(&self, obj: &Objid) -> Result<ObjSet, WorldStateError> {
        self.tx
            .as_ref()
            .unwrap()
            .seek_by_codomain::<Objid, Objid, ObjSet>(WorldStateTable::ObjectLocation, obj)
            .map_err(err_map)
    }

    fn get_object_size_bytes(&self, obj: &Objid) -> Result<usize, WorldStateError> {
        let mut size = 0;
        size += self
            .tx
            .as_ref()
            .unwrap()
            .tuple_size_for_unique_domain(WorldStateTable::ObjectOwner, obj)
            .map_err(err_map)?
            .unwrap_or(0);
        size += self
            .tx
            .as_ref()
            .unwrap()
            .tuple_size_for_unique_domain(WorldStateTable::ObjectFlags, obj)
            .map_err(err_map)?
            .unwrap_or(0);
        size += self
            .tx
            .as_ref()
            .unwrap()
            .tuple_size_for_unique_domain(WorldStateTable::ObjectName, obj)
            .map_err(err_map)?
            .unwrap_or(0);
        size += self
            .tx
            .as_ref()
            .unwrap()
            .tuple_size_for_unique_domain(WorldStateTable::ObjectParent, obj)
            .map_err(err_map)?
            .unwrap_or(0);
        size += self
            .tx
            .as_ref()
            .unwrap()
            .tuple_size_for_unique_domain(WorldStateTable::ObjectLocation, obj)
            .map_err(err_map)?
            .unwrap_or(0);

        if let Some(verbs) = self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain::<Objid, VerbDefs>(WorldStateTable::ObjectVerbs, obj)
            .map_err(err_map)?
        {
            size += self
                .tx
                .as_ref()
                .unwrap()
                .tuple_size_for_unique_domain(WorldStateTable::ObjectVerbs, obj)
                .map_err(err_map)?
                .unwrap_or(0);
            for v in verbs.iter() {
                size += self
                    .tx
                    .as_ref()
                    .unwrap()
                    .tuple_size_by_composite_domain(
                        WorldStateTable::VerbProgram,
                        obj,
                        &UUIDHolder(v.uuid()),
                    )
                    .map_err(err_map)?
                    .unwrap_or(0);
            }
        }

        if let Some(props) = self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain::<Objid, PropDefs>(WorldStateTable::ObjectPropDefs, obj)
            .map_err(err_map)?
        {
            size += self
                .tx
                .as_ref()
                .unwrap()
                .tuple_size_for_unique_domain(WorldStateTable::ObjectPropDefs, obj)
                .map_err(err_map)?
                .unwrap_or(0);
            for p in props.iter() {
                size += self
                    .tx
                    .as_ref()
                    .unwrap()
                    .tuple_size_by_composite_domain(
                        WorldStateTable::ObjectPropertyValue,
                        obj,
                        &UUIDHolder(p.uuid()),
                    )
                    .map_err(err_map)?
                    .unwrap_or(0);
            }
        }

        Ok(size)
    }

    fn set_object_location(
        &self,
        what: &Objid,
        new_location: &Objid,
    ) -> Result<(), WorldStateError> {
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
            let Some(location) = self
                .tx
                .as_ref()
                .unwrap()
                .seek_unique_by_domain(WorldStateTable::ObjectLocation, &oid)
                .map_err(err_map)?
            else {
                break;
            };
            oid = location
        }

        // Get o's location, get its contents, remove o from old contents, put contents back
        // without it. Set new location, get its contents, add o to contents, put contents
        // back with it. Then update the location of o.
        // Get and remove from contents of old location, if we had any.
        if let Some(old_location) = self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain::<Objid, Objid>(WorldStateTable::ObjectLocation, what)
            .map_err(err_map)?
        {
            if old_location.eq(new_location) {
                return Ok(());
            }
        }

        // Set new location.
        self.tx
            .as_ref()
            .unwrap()
            .upsert(WorldStateTable::ObjectLocation, what, new_location)
            .map_err(err_map)?;

        if new_location.is_nothing() {
            return Ok(());
        }

        Ok(())
    }

    fn get_verbs(&self, obj: &Objid) -> Result<VerbDefs, WorldStateError> {
        Ok(self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain(WorldStateTable::ObjectVerbs, obj)
            .map_err(err_map)?
            .unwrap_or(VerbDefs::empty()))
    }

    fn get_verb_binary(&self, obj: &Objid, uuid: Uuid) -> Result<Bytes, WorldStateError> {
        let bh: BytesHolder = self
            .tx
            .as_ref()
            .unwrap()
            .seek_by_unique_composite_domain(WorldStateTable::VerbProgram, obj, &UUIDHolder(uuid))
            .map_err(err_map)?
            .ok_or_else(|| WorldStateError::VerbNotFound(obj.clone(), format!("{}", uuid)))?;
        Ok(Bytes::from(bh.0))
    }

    fn get_verb_by_name(&self, obj: &Objid, name: Symbol) -> Result<VerbDef, WorldStateError> {
        let verbdefs: VerbDefs = self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain(WorldStateTable::ObjectVerbs, obj)
            .map_err(err_map)?
            .ok_or_else(|| WorldStateError::VerbNotFound(obj.clone(), name.to_string()))?;
        Ok(verbdefs
            .find_named(name)
            .first()
            .ok_or(WorldStateError::VerbNotFound(obj.clone(), name.to_string()))?
            .clone())
    }

    fn get_verb_by_index(&self, obj: &Objid, index: usize) -> Result<VerbDef, WorldStateError> {
        let verbs = self.get_verbs(obj)?;
        if index >= verbs.len() {
            return Err(WorldStateError::VerbNotFound(
                obj.clone(),
                format!("{}", index),
            ));
        }
        let verbs = verbs
            .iter()
            .nth(index)
            .ok_or_else(|| WorldStateError::VerbNotFound(obj.clone(), format!("{}", index)));
        verbs
    }

    fn resolve_verb(
        &self,
        obj: &Objid,
        name: Symbol,
        argspec: Option<VerbArgsSpec>,
    ) -> Result<VerbDef, WorldStateError> {
        let mut search_o = obj.clone();
        loop {
            if let Some(verbdefs) = self
                .tx
                .as_ref()
                .unwrap()
                .seek_unique_by_domain::<Objid, VerbDefs>(WorldStateTable::ObjectVerbs, &search_o)
                .map_err(err_map)?
            {
                // If we found the verb, return it.
                let name_matches = verbdefs.find_named(name);
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
            search_o = match self
                .tx
                .as_ref()
                .unwrap()
                .seek_unique_by_domain::<Objid, Objid>(WorldStateTable::ObjectParent, &search_o)
                .map_err(err_map)?
            {
                Some(NOTHING) | None => {
                    break;
                }
                Some(parent) => parent.clone(),
            };
        }
        Err(WorldStateError::VerbNotFound(obj.clone(), name.to_string()))
    }

    fn update_verb(
        &self,
        obj: &Objid,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let Some(verbdefs): Option<VerbDefs> = self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain(WorldStateTable::ObjectVerbs, obj)
            .map_err(err_map)?
        else {
            return Err(WorldStateError::VerbNotFound(
                obj.clone(),
                format!("{}", uuid),
            ));
        };

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

        self.tx
            .as_ref()
            .unwrap()
            .upsert(WorldStateTable::ObjectVerbs, obj, &verbdefs)
            .map_err(err_map)?;

        if verb_attrs.binary.is_some() {
            self.tx
                .as_ref()
                .unwrap()
                .upsert_composite(
                    WorldStateTable::VerbProgram,
                    obj,
                    &UUIDHolder(uuid),
                    &BytesHolder(verb_attrs.binary.unwrap()),
                )
                .map_err(err_map)?;
        }
        Ok(())
    }

    fn add_object_verb(
        &self,
        oid: &Objid,
        owner: &Objid,
        names: Vec<Symbol>,
        binary: Vec<u8>,
        binary_type: BinaryType,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), WorldStateError> {
        let verbdefs = self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain(WorldStateTable::ObjectVerbs, oid)
            .map_err(err_map)?
            .unwrap_or(VerbDefs::empty());

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

        self.tx
            .as_ref()
            .unwrap()
            .upsert(WorldStateTable::ObjectVerbs, oid, &verbdefs)
            .map_err(err_map)?;

        self.tx
            .as_ref()
            .unwrap()
            .upsert_composite(
                WorldStateTable::VerbProgram,
                oid,
                &UUIDHolder(uuid),
                &BytesHolder(binary),
            )
            .map_err(err_map)?;

        Ok(())
    }

    fn delete_verb(&self, location: &Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let verbdefs: VerbDefs = self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain(WorldStateTable::ObjectVerbs, location)
            .map_err(err_map)?
            .ok_or_else(|| WorldStateError::VerbNotFound(location.clone(), format!("{}", uuid)))?;

        let verbdefs = verbdefs
            .with_removed(uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(location.clone(), format!("{}", uuid)))?;

        self.tx
            .as_ref()
            .unwrap()
            .upsert(WorldStateTable::ObjectVerbs, location, &verbdefs)
            .map_err(err_map)?;

        self.tx
            .as_ref()
            .unwrap()
            .remove_by_composite_domain(WorldStateTable::VerbProgram, location, &UUIDHolder(uuid))
            .map_err(err_map)?;

        Ok(())
    }

    fn get_properties(&self, obj: &Objid) -> Result<PropDefs, WorldStateError> {
        Ok(self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain(WorldStateTable::ObjectPropDefs, obj)
            .map_err(err_map)?
            .unwrap_or(PropDefs::empty()))
    }

    fn set_property(&self, obj: &Objid, uuid: Uuid, value: Var) -> Result<(), WorldStateError> {
        self.tx
            .as_ref()
            .unwrap()
            .upsert_composite(
                WorldStateTable::ObjectPropertyValue,
                obj,
                &UUIDHolder(uuid),
                &value,
            )
            .map_err(err_map)
    }

    fn define_property(
        &self,
        definer: &Objid,
        location: &Objid,
        name: Symbol,
        owner: &Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<Uuid, WorldStateError> {
        let descendants = self.descendants(location)?;

        // If the property is already defined at us or above or below us, that's a failure.
        let props = match self
            .tx
            .as_ref()
            .unwrap()
            .seek_unique_by_domain::<Objid, PropDefs>(WorldStateTable::ObjectPropDefs, location)
            .map_err(err_map)?
        {
            None => PropDefs::empty(),
            Some(propdefs) => {
                if propdefs.find_first_named(name).is_some() {
                    return Err(WorldStateError::DuplicatePropertyDefinition(
                        location.clone(),
                        name.to_string(),
                    ));
                }
                propdefs
            }
        };
        let ancestors = self.ancestors(location)?;
        let check_locations = ObjSet::from_items(&[location.clone()]).with_concatenated(ancestors);
        for location in check_locations.iter() {
            if let Some(descendant_props) = self
                .tx
                .as_ref()
                .unwrap()
                .seek_unique_by_domain::<Objid, PropDefs>(
                    WorldStateTable::ObjectPropDefs,
                    &location,
                )
                .map_err(err_map)?
            {
                // Verify we don't already have a property with this name. If we do, return an error.
                if descendant_props.find_first_named(name).is_some() {
                    return Err(WorldStateError::DuplicatePropertyDefinition(
                        location,
                        name.to_string(),
                    ));
                }
            }
        }

        // Generate a new property ID. This will get shared all the way down the pipe.
        // But the key for the actual value is always composite of oid,uuid
        let u = Uuid::new_v4();

        let prop = PropDef::new(u, definer.clone(), location.clone(), name.as_str());
        self.tx
            .as_ref()
            .unwrap()
            .upsert(
                WorldStateTable::ObjectPropDefs,
                location,
                &props.with_added(prop),
            )
            .expect("Unable to set property definition");

        // If we have an initial value, set it, but just on ourselves. Descendants start out clear.
        if let Some(value) = value {
            self.tx
                .as_ref()
                .unwrap()
                .upsert_composite(
                    WorldStateTable::ObjectPropertyValue,
                    location,
                    &UUIDHolder(u),
                    &value,
                )
                .expect("Unable to set property value");
        }

        // Put the initial object owner on ourselves and all our descendants.
        let value_locations =
            ObjSet::from_items(&[location.clone()]).with_concatenated(descendants);
        for location in value_locations.iter() {
            self.tx
                .as_ref()
                .unwrap()
                .upsert_composite(
                    WorldStateTable::ObjectPropertyPermissions,
                    &location,
                    &UUIDHolder(u),
                    &PropPerms::new(owner.clone(), perms),
                )
                .expect("Unable to set property owner");
        }

        Ok(u)
    }

    fn update_property_info(
        &self,
        obj: &Objid,
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
            let props = self
                .tx
                .as_ref()
                .unwrap()
                .seek_unique_by_domain(WorldStateTable::ObjectPropDefs, obj)
                .map_err(err_map)?
                .unwrap_or(PropDefs::empty());

            let Some(props) = props.with_updated(uuid, |p| {
                PropDef::new(p.uuid(), p.definer(), p.location(), &new_name)
            }) else {
                return Err(WorldStateError::PropertyNotFound(
                    obj.clone(),
                    format!("{}", uuid),
                ));
            };

            self.tx
                .as_ref()
                .unwrap()
                .upsert(WorldStateTable::ObjectPropDefs, obj, &props)
                .map_err(err_map)?;
        }

        // If flags or perms updated, do that.
        if new_flags.is_some() || new_owner.is_some() {
            let mut perms: PropPerms = self
                .tx
                .as_ref()
                .unwrap()
                .seek_by_unique_composite_domain(
                    WorldStateTable::ObjectPropertyPermissions,
                    obj,
                    &UUIDHolder(uuid),
                )
                .unwrap()
                .expect("Unable to get property permissions for update. Integrity error");

            if let Some(new_flags) = new_flags {
                perms = perms.with_flags(new_flags);
            }

            if let Some(new_owner) = new_owner {
                perms = perms.with_owner(new_owner);
            }

            self.tx
                .as_ref()
                .unwrap()
                .upsert_composite(
                    WorldStateTable::ObjectPropertyPermissions,
                    obj,
                    &UUIDHolder(uuid),
                    &perms,
                )
                .map_err(err_map)?;
        }

        Ok(())
    }

    fn clear_property(&self, obj: &Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        self.tx
            .as_ref()
            .unwrap()
            .delete_composite_if_exists(
                WorldStateTable::ObjectPropertyValue,
                obj,
                &UUIDHolder(uuid),
            )
            .unwrap_or(());
        Ok(())
    }

    fn delete_property(&self, obj: &Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        // delete propdef from self and all descendants
        let descendants = self.descendants(obj)?;
        let locations = ObjSet::from_items(&[obj.clone()]).with_concatenated(descendants);
        for location in locations.iter() {
            let props: PropDefs = self
                .tx
                .as_ref()
                .unwrap()
                .seek_unique_by_domain(WorldStateTable::ObjectPropDefs, &location)
                .map_err(err_map)?
                .expect("Unable to find property for object, invalid object");

            let props = props
                .with_removed(uuid)
                .expect("Unable to remove property definition");

            self.tx
                .as_ref()
                .unwrap()
                .upsert(WorldStateTable::ObjectPropDefs, &location, &props)
                .map_err(err_map)?;
        }
        Ok(())
    }

    fn retrieve_property(
        &self,
        obj: &Objid,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError> {
        let value = self
            .tx
            .as_ref()
            .unwrap()
            .seek_by_unique_composite_domain(
                WorldStateTable::ObjectPropertyValue,
                obj,
                &UUIDHolder(uuid),
            )
            .map_err(err_map)?;
        let perms = self.retrieve_property_permissions(obj, uuid)?;
        Ok((value, perms))
    }

    fn retrieve_property_permissions(
        &self,
        obj: &Objid,
        uuid: Uuid,
    ) -> Result<PropPerms, WorldStateError> {
        self.tx
            .as_ref()
            .unwrap()
            .seek_by_unique_composite_domain::<_, _, PropPerms>(
                WorldStateTable::ObjectPropertyPermissions,
                obj,
                &UUIDHolder(uuid),
            )
            .map_err(err_map)?
            .ok_or(WorldStateError::PropertyNotFound(
                obj.clone(),
                format!("{}", uuid),
            ))
    }

    fn resolve_property(
        &self,
        obj: &Objid,
        name: Symbol,
    ) -> Result<(PropDef, Var, PropPerms, bool), WorldStateError> {
        // Walk up the inheritance tree looking for the property definition.
        let mut search_obj = obj.clone();
        let propdef = loop {
            let propdef = self.get_properties(&search_obj)?.find_first_named(name);

            if let Some(propdef) = propdef {
                break propdef;
            }

            if let Some(parent) = self
                .tx
                .as_ref()
                .unwrap()
                .seek_unique_by_domain(WorldStateTable::ObjectParent, &search_obj)
                .map_err(err_map)?
            {
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
        let perms = self
            .tx
            .as_ref()
            .unwrap()
            .seek_by_unique_composite_domain::<_, _, PropPerms>(
                WorldStateTable::ObjectPropertyPermissions,
                obj,
                &UUIDHolder(propdef.uuid()),
            )
            .map_err(err_map)?
            .expect("Unable to get property permissions, coherence problem");

        match self
            .tx
            .as_ref()
            .unwrap()
            .seek_by_unique_composite_domain::<_, _, Var>(
                WorldStateTable::ObjectPropertyValue,
                obj,
                &UUIDHolder(propdef.uuid()),
            )
            .map_err(err_map)?
        {
            Some(value) => Ok((propdef, value, perms, false)),
            None => {
                let mut search_obj = obj.clone();
                loop {
                    let Some(parent): Option<Objid> = self
                        .tx
                        .as_ref()
                        .unwrap()
                        .seek_unique_by_domain(WorldStateTable::ObjectParent, &search_obj)
                        .map_err(err_map)?
                    else {
                        break Ok((propdef, v_none(), perms, true));
                    };
                    if parent.is_nothing() {
                        break Ok((propdef, v_none(), perms, true));
                    }
                    search_obj = parent;

                    let value = self
                        .tx
                        .as_ref()
                        .unwrap()
                        .seek_by_unique_composite_domain(
                            WorldStateTable::ObjectPropertyValue,
                            &search_obj,
                            &UUIDHolder(propdef.uuid()),
                        )
                        .map_err(err_map)?;
                    if let Some(value) = value {
                        break Ok((propdef, value, perms, true));
                    }
                }
            }
        }
    }

    fn db_usage(&self) -> Result<usize, WorldStateError> {
        todo!("Implement db_usage")
    }

    fn commit(&mut self) -> Result<CommitResult, WorldStateError> {
        Ok(self.tx.take().unwrap().commit())
    }

    fn rollback(&mut self) -> Result<(), WorldStateError> {
        self.tx.take().unwrap().rollback();
        Ok(())
    }
}

impl<RTX: RelationalTransaction<WorldStateTable>> RelationalWorldStateTransaction<RTX> {
    pub fn descendants(&self, obj: &Objid) -> Result<ObjSet, WorldStateError> {
        let children = self
            .tx
            .as_ref()
            .unwrap()
            .seek_by_codomain::<Objid, Objid, ObjSet>(WorldStateTable::ObjectParent, obj)
            .map_err(err_map)?;

        let mut descendants = vec![];
        let mut queue: VecDeque<_> = children.iter().collect();
        while let Some(o) = queue.pop_front() {
            descendants.push(o.clone());
            let children = self
                .tx
                .as_ref()
                .unwrap()
                .seek_by_codomain::<Objid, Objid, ObjSet>(WorldStateTable::ObjectParent, &o)
                .map_err(err_map)?;
            queue.extend(children.iter());
        }

        Ok(ObjSet::from_items(&descendants))
    }

    #[allow(clippy::type_complexity)]
    fn closest_common_ancestor_with_ancestors(
        &self,
        a: &Objid,
        b: &Objid,
    ) -> Result<(Option<Objid>, HashSet<Objid>, HashSet<Objid>), WorldStateError> {
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
                let parent = self
                    .tx
                    .as_ref()
                    .unwrap()
                    .seek_unique_by_domain(WorldStateTable::ObjectParent, &search_a)
                    .map_err(err_map)?
                    .unwrap_or(NOTHING);
                search_a = parent.clone();
            }

            if !search_b.is_nothing() {
                ancestors_b.insert(search_b.clone());
                let parent = self
                    .tx
                    .as_ref()
                    .unwrap()
                    .seek_unique_by_domain(WorldStateTable::ObjectParent, &search_b)
                    .map_err(err_map)?
                    .unwrap_or(NOTHING);
                search_b = parent.clone();
            }
        }
    }
}
