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

use strum::{AsRefStr, Display, EnumCount, EnumIter, EnumProperty};
use tempfile::TempDir;
use uuid::Uuid;

use moor_db::db_worldstate::DbTxWorldState;
use moor_db::loader::LoaderInterface;
use moor_db::worldstate_transaction::WorldStateTransaction;
use moor_db::{Database, RelationalError, RelationalTransaction};
use moor_values::model::VerbArgsSpec;
use moor_values::model::{BinaryType, VerbAttrs, VerbFlag};
use moor_values::model::{CommitResult, WorldStateError};
use moor_values::model::{HasUuid, Named};
use moor_values::model::{ObjAttrs, ObjFlag};
use moor_values::model::{ObjSet, ValSet};
use moor_values::model::{PropDef, PropDefs};
use moor_values::model::{PropFlag, PropPerms};
use moor_values::model::{VerbDef, VerbDefs};
use moor_values::model::{WorldState, WorldStateSource};
use moor_values::util::BitEnum;
use moor_values::var::Objid;
use moor_values::var::{v_none, Var};
use moor_values::NOTHING;

use crate::wtrel::db::WiredTigerRelDb;
use crate::wtrel::rel_transaction::WiredTigerRelTransaction;
use crate::wtrel::relation::WiredTigerRelation;

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, EnumIter, EnumCount)]
pub enum WtWorldStateSequence {
    MaximumObject = 0,
}

impl From<WtWorldStateSequence> for u8 {
    fn from(val: WtWorldStateSequence) -> Self {
        val as u8
    }
}

/// The set of binary relations that are used to represent the world state in the moor system.
#[repr(usize)]
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, EnumIter, EnumCount, Display, EnumProperty, AsRefStr,
)]
pub enum WtWorldStateTable {
    /// Object<->Parent
    #[strum(props(SecondaryIndexed = "true"))]
    ObjectParent = 0,
    /// Object<->Location
    #[strum(props(SecondaryIndexed = "true"))]
    ObjectLocation = 1,
    /// Object->Flags (BitEnum<ObjFlag>)
    ObjectFlags = 2,
    /// Object->Name
    ObjectName = 3,
    /// Object->Owner
    ObjectOwner = 4,
    /// Object->Verbs (Verbdefs)
    ObjectVerbs = 5,
    /// (Object, UUID)->VerbProgram (Binary)
    #[strum(props(CompositeDomain = "true", Domain_A_Size = "8", Domain_B_Size = "16"))]
    VerbProgram = 6,
    /// Object->Properties (Propdefs)
    ObjectPropDefs = 7,
    /// (Object, UUID)->PropertyValue (Var)
    #[strum(props(CompositeDomain = "true", Domain_A_Size = "8", Domain_B_Size = "16"))]
    ObjectPropertyValue = 8,
    /// Object->PropertyPermissions (PropPerms)
    #[strum(props(CompositeDomain = "true", Domain_A_Size = "8", Domain_B_Size = "16"))]
    ObjectPropertyPermissions = 9,
    /// Set of sequences sequence_id -> current_value
    Sequences = 10,
}

fn err_map(e: RelationalError) -> WorldStateError {
    match e {
        RelationalError::ConflictRetry => WorldStateError::RollbackRetry,
        _ => WorldStateError::DatabaseError(format!("{:?}", e)),
    }
}

impl WiredTigerRelation for WtWorldStateTable {}

/// An implementation of `WorldState` / `WorldStateSource` that uses the relbox as its backing
pub struct WireTigerWorldState {
    db: Arc<WiredTigerRelDb<WtWorldStateTable>>,
    // If this is a temporary database, since it seems WiredTiger wants a path no matter what,
    // we'll create a temporary directory and use that as the path.
    // We hold it here so RAII can clean it up when we're done.
    _tmpdir: Option<TempDir>,
}

impl WireTigerWorldState {
    pub fn open(path: Option<&PathBuf>) -> (Self, bool) {
        let tmpdir = match path {
            Some(_path) => None,
            None => {
                let tmpdir = tempfile::tempdir().expect("Unable to create temporary directory");
                Some(tmpdir)
            }
        };
        let db_path = match path {
            Some(path) => path,
            None => {
                let path = tmpdir.as_ref().unwrap().path();
                path
            }
        };
        let db = Arc::new(WiredTigerRelDb::new(
            db_path,
            WtWorldStateTable::Sequences,
            path.is_none(),
        ));

        // Check for presence of our relations
        let fresh_db = {
            let tx = db.start_tx();
            let is_fresh = !WtWorldStateTable::has_tables(tx.session());
            tx.rollback();
            is_fresh
        };

        // If fresh, create the tables.
        if fresh_db {
            let tx = db.start_tx();
            db.create_tables();
            tx.commit();
        }

        db.load_sequences();

        (
            Self {
                db,
                _tmpdir: tmpdir,
            },
            fresh_db,
        )
    }
}

impl WorldStateSource for WireTigerWorldState {
    fn new_world_state(&self) -> Result<Box<dyn WorldState>, WorldStateError> {
        let tx = WtWorldStateTransaction::new(self.db.clone());
        Ok(Box::new(DbTxWorldState { tx: Box::new(tx) }))
    }
}

pub struct WtWorldStateTransaction {
    tx: WiredTigerRelTransaction<WtWorldStateTable>,
}

impl WorldStateTransaction for WtWorldStateTransaction {
    fn object_valid(&self, obj: Objid) -> Result<bool, WorldStateError> {
        let ov: Option<Objid> = self
            .tx
            .seek_unique_by_domain(WtWorldStateTable::ObjectOwner, obj)
            .map_err(err_map)?;
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
            let parent = self
                .tx
                .seek_unique_by_domain(WtWorldStateTable::ObjectParent, search)
                .map_err(err_map)?
                .unwrap_or(NOTHING);
            search = parent;
        }
        Ok(ObjSet::from_items(&ancestors))
    }

    fn get_objects(&self) -> Result<ObjSet, WorldStateError> {
        let objs = self
            .tx
            .scan_with_predicate(
                WtWorldStateTable::ObjectFlags,
                |&_: &Objid, _: &BitEnum<ObjFlag>| true,
            )
            .map_err(err_map)?;
        Ok(ObjSet::from_iter(objs.iter().map(|(o, _)| *o)))
    }

    fn get_object_flags(&self, obj: Objid) -> Result<BitEnum<ObjFlag>, WorldStateError> {
        self.tx
            .seek_unique_by_domain(WtWorldStateTable::ObjectFlags, obj)
            .map_err(err_map)?
            .ok_or(WorldStateError::ObjectNotFound(obj))
    }

    fn get_players(&self) -> Result<ObjSet, WorldStateError> {
        // TODO: Improve get_players retrieval in world state
        //   this is going to be not-at-all performant in the long run, and we'll need a way to
        //   cache this or index it better
        let players = self
            .tx
            .scan_with_predicate(
                WtWorldStateTable::ObjectFlags,
                |&_, flags: &BitEnum<ObjFlag>| flags.contains(ObjFlag::User),
            )
            .map_err(err_map)?;
        Ok(ObjSet::from_iter(players.iter().map(|(o, _)| *o)))
    }

    fn get_max_object(&self) -> Result<Objid, WorldStateError> {
        Ok(Objid(
            self.tx.get_sequence(WtWorldStateSequence::MaximumObject),
        ))
    }

    fn get_object_owner(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        self.tx
            .seek_unique_by_domain(WtWorldStateTable::ObjectOwner, obj)
            .map_err(err_map)?
            .ok_or(WorldStateError::ObjectNotFound(obj))
    }

    fn set_object_owner(&self, obj: Objid, owner: Objid) -> Result<(), WorldStateError> {
        self.tx
            .upsert(WtWorldStateTable::ObjectOwner, obj, owner)
            .map_err(err_map)
    }

    fn set_object_flags(&self, obj: Objid, flags: BitEnum<ObjFlag>) -> Result<(), WorldStateError> {
        self.tx
            .upsert(WtWorldStateTable::ObjectFlags, obj, flags)
            .map_err(err_map)
    }

    fn get_object_name(&self, obj: Objid) -> Result<String, WorldStateError> {
        self.tx
            .seek_unique_by_domain(WtWorldStateTable::ObjectName, obj)
            .map_err(err_map)?
            .ok_or(WorldStateError::ObjectNotFound(obj))
    }

    fn set_object_name(&self, obj: Objid, name: String) -> Result<(), WorldStateError> {
        self.tx
            .upsert(WtWorldStateTable::ObjectName, obj, name)
            .map_err(err_map)
    }

    fn create_object(&self, id: Option<Objid>, attrs: ObjAttrs) -> Result<Objid, WorldStateError> {
        let id = match id {
            Some(id) => id,
            None => {
                let max = self
                    .tx
                    .increment_sequence(WtWorldStateSequence::MaximumObject);
                Objid(max)
            }
        };

        let owner = attrs.owner().unwrap_or(NOTHING);
        self.tx
            .upsert(WtWorldStateTable::ObjectOwner, id, owner)
            .expect("Unable to insert initial owner");

        // Set initial name
        let name = attrs.name().unwrap_or_else(|| format!("Object {}", id));
        self.tx
            .upsert(WtWorldStateTable::ObjectName, id, name)
            .expect("Unable to insert initial name");

        // We use our own setters for these, since there's biz-logic attached here...
        if let Some(parent) = attrs.parent() {
            self.set_object_parent(id, parent)
                .expect("Unable to set parent");
        }
        if let Some(location) = attrs.location() {
            self.set_object_location(id, location)
                .expect("Unable to set location");
        }

        self.tx
            .upsert(WtWorldStateTable::ObjectFlags, id, attrs.flags())
            .expect("Unable to insert initial flags");

        // Update the maximum object number if ours is higher than the current one. This is for the
        // textdump case, where our numbers are coming in arbitrarily.
        self.tx
            .update_sequence_max(WtWorldStateSequence::MaximumObject, id.0 + 1);

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
            WtWorldStateTable::ObjectFlags,
            WtWorldStateTable::ObjectName,
            WtWorldStateTable::ObjectOwner,
            WtWorldStateTable::ObjectParent,
            WtWorldStateTable::ObjectLocation,
            WtWorldStateTable::ObjectVerbs,
        ];
        for rel in oid_relations.iter() {
            self.tx.remove_by_domain(*rel, obj).map_err(err_map)?;
        }

        let propdefs = self.get_properties(obj)?;
        for p in propdefs.iter() {
            self.tx
                .delete_composite_if_exists(
                    WtWorldStateTable::ObjectPropertyValue,
                    obj,
                    p.uuid().to_bytes_le(),
                )
                .unwrap_or(());
        }

        self.tx
            .remove_by_domain(WtWorldStateTable::ObjectPropDefs, obj)
            .expect("Unable to delete propdefs");

        Ok(())
    }

    fn get_object_parent(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        Ok(self
            .tx
            .seek_unique_by_domain(WtWorldStateTable::ObjectParent, obj)
            .map_err(err_map)?
            .unwrap_or(NOTHING))
    }

    // TODO: wildtiger has joins. we should add join&transitive join to the interface and use it
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
            self.closest_common_ancestor_with_ancestors(new_parent, o)?;

        // Remove from _me_ any of the properties defined by any of my ancestors
        if let Some(old_props) = self
            .tx
            .seek_unique_by_domain::<Objid, PropDefs>(WtWorldStateTable::ObjectPropDefs, o)
            .map_err(err_map)?
        {
            let mut delort_props = vec![];
            for p in old_props.iter() {
                if old_ancestors.contains(&p.definer()) {
                    delort_props.push(p.uuid());

                    self.tx
                        .delete_composite_if_exists(
                            WtWorldStateTable::ObjectPropertyValue,
                            o,
                            p.uuid().to_bytes_le(),
                        )
                        .expect("Unable to delete property");
                }
            }
            let new_props = old_props.with_all_removed(&delort_props);
            self.tx
                .upsert(WtWorldStateTable::ObjectPropDefs, o, new_props)
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
                .seek_unique_by_domain::<Objid, PropDefs>(WtWorldStateTable::ObjectPropDefs, o)
                .map_err(err_map)?
            {
                for p in old_props.iter() {
                    if old_ancestors.contains(&p.definer()) {
                        inherited_props.push(p.uuid());
                        self.tx
                            .delete_composite_if_exists(
                                WtWorldStateTable::ObjectPropertyValue,
                                c,
                                p.uuid().to_bytes_le(),
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
            .seek_unique_by_domain::<Objid, Objid>(WtWorldStateTable::ObjectParent, o)
            .map_err(err_map)?
        {
            if old_parent == new_parent {
                return Ok(());
            }
        };

        self.tx
            .upsert(WtWorldStateTable::ObjectParent, o, new_parent)
            .expect("Unable to update parent");

        if new_parent == NOTHING {
            return Ok(());
        }

        // Now walk all my new descendants and give them the properties that derive from any
        // ancestors they don't already share.

        // Now collect properties defined on the new ancestors.
        let mut new_props = vec![];
        for a in new_ancestors {
            if let Some(props) = self
                .tx
                .seek_unique_by_domain::<Objid, PropDefs>(WtWorldStateTable::ObjectPropDefs, a)
                .map_err(err_map)?
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
        let descendants = self.descendants(o).expect("Unable to get descendants");
        for c in descendants.iter().chain(std::iter::once(o)) {
            // Check if we have a cached/modified copy from above in descendant_props
            let c_props = match descendant_props.remove(&c) {
                None => self
                    .tx
                    .seek_unique_by_domain(WtWorldStateTable::ObjectPropDefs, c)
                    .map_err(err_map)?
                    .unwrap_or_else(PropDefs::empty),
                Some(props) => props,
            };
            let c_props = c_props.with_all_added(&new_props);
            self.tx
                .upsert(WtWorldStateTable::ObjectPropDefs, c, c_props)
                .expect("Unable to update propdefs");
        }
        Ok(())
    }

    fn get_object_children(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        self.tx
            .seek_by_codomain::<Objid, Objid, ObjSet>(WtWorldStateTable::ObjectParent, obj)
            .map_err(err_map)
    }

    fn get_object_location(&self, obj: Objid) -> Result<Objid, WorldStateError> {
        Ok(self
            .tx
            .seek_unique_by_domain(WtWorldStateTable::ObjectLocation, obj)
            .map_err(err_map)?
            .unwrap_or(NOTHING))
    }

    fn get_object_contents(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        self.tx
            .seek_by_codomain::<Objid, Objid, ObjSet>(WtWorldStateTable::ObjectLocation, obj)
            .map_err(err_map)
    }

    fn get_object_size_bytes(&self, obj: Objid) -> Result<usize, WorldStateError> {
        let mut size = 0;
        size += self
            .tx
            .tuple_size_for_unique_domain(WtWorldStateTable::ObjectOwner, obj)
            .map_err(err_map)?
            .unwrap_or(0);
        size += self
            .tx
            .tuple_size_for_unique_domain(WtWorldStateTable::ObjectFlags, obj)
            .map_err(err_map)?
            .unwrap_or(0);
        size += self
            .tx
            .tuple_size_for_unique_domain(WtWorldStateTable::ObjectName, obj)
            .map_err(err_map)?
            .unwrap_or(0);
        size += self
            .tx
            .tuple_size_for_unique_domain(WtWorldStateTable::ObjectParent, obj)
            .map_err(err_map)?
            .unwrap_or(0);
        size += self
            .tx
            .tuple_size_for_unique_domain(WtWorldStateTable::ObjectLocation, obj)
            .map_err(err_map)?
            .unwrap_or(0);

        if let Some(verbs) = self
            .tx
            .seek_unique_by_domain::<Objid, VerbDefs>(WtWorldStateTable::ObjectVerbs, obj)
            .map_err(err_map)?
        {
            size += self
                .tx
                .tuple_size_for_unique_domain(WtWorldStateTable::ObjectVerbs, obj)
                .map_err(err_map)?
                .unwrap_or(0);
            for v in verbs.iter() {
                size += self
                    .tx
                    .tuple_size_by_composite_domain(
                        WtWorldStateTable::VerbProgram,
                        obj,
                        v.uuid().to_bytes_le(),
                    )
                    .map_err(err_map)?
                    .unwrap_or(0);
            }
        }

        if let Some(props) = self
            .tx
            .seek_unique_by_domain::<Objid, PropDefs>(WtWorldStateTable::ObjectPropDefs, obj)
            .map_err(err_map)?
        {
            size += self
                .tx
                .tuple_size_for_unique_domain(WtWorldStateTable::ObjectPropDefs, obj)
                .map_err(err_map)?
                .unwrap_or(0);
            for p in props.iter() {
                size += self
                    .tx
                    .tuple_size_by_composite_domain(
                        WtWorldStateTable::ObjectPropertyValue,
                        obj,
                        p.uuid().to_bytes_le(),
                    )
                    .map_err(err_map)?
                    .unwrap_or(0);
            }
        }

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
            let Some(location) = self
                .tx
                .seek_unique_by_domain(WtWorldStateTable::ObjectLocation, oid)
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
            .seek_unique_by_domain::<Objid, Objid>(WtWorldStateTable::ObjectLocation, what)
            .map_err(err_map)?
        {
            if old_location == new_location {
                return Ok(());
            }
        }

        // Set new location.
        self.tx
            .upsert(WtWorldStateTable::ObjectLocation, what, new_location)
            .map_err(err_map)?;

        if new_location == NOTHING {
            return Ok(());
        }

        Ok(())
    }

    fn get_verbs(&self, obj: Objid) -> Result<VerbDefs, WorldStateError> {
        Ok(self
            .tx
            .seek_unique_by_domain(WtWorldStateTable::ObjectVerbs, obj)
            .map_err(err_map)?
            .unwrap_or(VerbDefs::empty()))
    }

    fn get_verb_binary(&self, obj: Objid, uuid: Uuid) -> Result<Vec<u8>, WorldStateError> {
        self.tx
            .seek_by_unique_composite_domain(
                WtWorldStateTable::VerbProgram,
                obj,
                uuid.to_bytes_le(),
            )
            .map_err(err_map)?
            .ok_or_else(|| WorldStateError::VerbNotFound(obj, format!("{}", uuid)))
    }

    fn get_verb_by_name(&self, obj: Objid, name: String) -> Result<VerbDef, WorldStateError> {
        let verbdefs: VerbDefs = self
            .tx
            .seek_unique_by_domain(WtWorldStateTable::ObjectVerbs, obj)
            .map_err(err_map)?
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
        let verbs = verbs
            .iter()
            .nth(index)
            .ok_or_else(|| WorldStateError::VerbNotFound(obj, format!("{}", index)));
        verbs
    }

    fn resolve_verb(
        &self,
        obj: Objid,
        name: String,
        argspec: Option<VerbArgsSpec>,
    ) -> Result<VerbDef, WorldStateError> {
        let mut search_o = obj;
        loop {
            if let Some(verbdefs) = self
                .tx
                .seek_unique_by_domain::<Objid, VerbDefs>(WtWorldStateTable::ObjectVerbs, search_o)
                .map_err(err_map)?
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
            search_o = match self
                .tx
                .seek_unique_by_domain::<Objid, Objid>(WtWorldStateTable::ObjectParent, search_o)
                .map_err(err_map)?
            {
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
        let Some(verbdefs): Option<VerbDefs> = self
            .tx
            .seek_unique_by_domain(WtWorldStateTable::ObjectVerbs, obj)
            .map_err(err_map)?
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

        self.tx
            .upsert(WtWorldStateTable::ObjectVerbs, obj, verbdefs)
            .map_err(err_map)?;

        if verb_attrs.binary.is_some() {
            self.tx
                .upsert_composite(
                    WtWorldStateTable::VerbProgram,
                    obj,
                    uuid.to_bytes_le(),
                    verb_attrs.binary.unwrap(),
                )
                .map_err(err_map)?;
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
        let verbdefs = self
            .tx
            .seek_unique_by_domain(WtWorldStateTable::ObjectVerbs, oid)
            .map_err(err_map)?
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

        self.tx
            .upsert(WtWorldStateTable::ObjectVerbs, oid, verbdefs)
            .map_err(err_map)?;

        self.tx
            .upsert_composite(
                WtWorldStateTable::VerbProgram,
                oid,
                uuid.to_bytes_le(),
                binary,
            )
            .map_err(err_map)?;

        Ok(())
    }

    fn delete_verb(&self, location: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let verbdefs: VerbDefs = self
            .tx
            .seek_unique_by_domain(WtWorldStateTable::ObjectVerbs, location)
            .map_err(err_map)?
            .ok_or_else(|| WorldStateError::VerbNotFound(location, format!("{}", uuid)))?;

        let verbdefs = verbdefs
            .with_removed(uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(location, format!("{}", uuid)))?;

        self.tx
            .upsert(WtWorldStateTable::ObjectVerbs, location, verbdefs)
            .map_err(err_map)?;

        self.tx
            .remove_by_composite_domain(
                WtWorldStateTable::VerbProgram,
                location,
                uuid.to_bytes_le(),
            )
            .map_err(err_map)?;

        Ok(())
    }

    fn get_properties(&self, obj: Objid) -> Result<PropDefs, WorldStateError> {
        Ok(self
            .tx
            .seek_unique_by_domain(WtWorldStateTable::ObjectPropDefs, obj)
            .map_err(err_map)?
            .unwrap_or(PropDefs::empty()))
    }

    fn set_property(&self, obj: Objid, uuid: Uuid, value: Var) -> Result<(), WorldStateError> {
        self.tx
            .upsert_composite(
                WtWorldStateTable::ObjectPropertyValue,
                obj,
                uuid.to_bytes_le(),
                value,
            )
            .map_err(err_map)
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
        let props = match self
            .tx
            .seek_unique_by_domain::<Objid, PropDefs>(WtWorldStateTable::ObjectPropDefs, location)
            .map_err(err_map)?
        {
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
            if let Some(descendant_props) = self
                .tx
                .seek_unique_by_domain::<Objid, PropDefs>(
                    WtWorldStateTable::ObjectPropDefs,
                    location,
                )
                .map_err(err_map)?
            {
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
        self.tx
            .upsert(
                WtWorldStateTable::ObjectPropDefs,
                location,
                props.with_added(prop),
            )
            .expect("Unable to set property definition");

        // If we have an initial value, set it, but just on ourselves. Descendants start out clear.
        if let Some(value) = value {
            self.tx
                .upsert_composite(
                    WtWorldStateTable::ObjectPropertyValue,
                    location,
                    u.to_bytes_le(),
                    value,
                )
                .expect("Unable to set property value");
        }

        // Put the initial object owner on ourselves and all our descendants.
        let value_locations = ObjSet::from_items(&[location]).with_concatenated(descendants);
        for location in value_locations.iter() {
            self.tx
                .upsert_composite(
                    WtWorldStateTable::ObjectPropertyPermissions,
                    location,
                    u.to_bytes_le(),
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
            let props = self
                .tx
                .seek_unique_by_domain(WtWorldStateTable::ObjectPropDefs, obj)
                .map_err(err_map)?
                .unwrap_or(PropDefs::empty());

            let Some(props) = props.with_updated(uuid, |p| {
                PropDef::new(p.uuid(), p.definer(), p.location(), &new_name)
            }) else {
                return Err(WorldStateError::PropertyNotFound(obj, format!("{}", uuid)));
            };

            self.tx
                .upsert(WtWorldStateTable::ObjectPropDefs, obj, props)
                .map_err(err_map)?;
        }

        // If flags or perms updated, do that.
        if new_flags.is_some() || new_owner.is_some() {
            let mut perms: PropPerms = self
                .tx
                .seek_by_unique_composite_domain(
                    WtWorldStateTable::ObjectPropertyPermissions,
                    obj,
                    uuid.to_bytes_le(),
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
                .upsert_composite(
                    WtWorldStateTable::ObjectPropertyPermissions,
                    obj,
                    uuid.to_bytes_le(),
                    perms,
                )
                .map_err(err_map)?;
        }

        Ok(())
    }

    fn clear_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        self.tx
            .delete_composite_if_exists(
                WtWorldStateTable::ObjectPropertyValue,
                obj,
                uuid.to_bytes_le(),
            )
            .unwrap_or(());
        Ok(())
    }

    fn delete_property(&self, obj: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        // delete propdef from self and all descendants
        let descendants = self.descendants(obj)?;
        let locations = ObjSet::from_items(&[obj]).with_concatenated(descendants);
        for location in locations.iter() {
            let props: PropDefs = self
                .tx
                .seek_unique_by_domain(WtWorldStateTable::ObjectPropDefs, location)
                .map_err(err_map)?
                .expect("Unable to find property for object, invalid object");

            let props = props
                .with_removed(uuid)
                .expect("Unable to remove property definition");

            self.tx
                .upsert(WtWorldStateTable::ObjectPropDefs, location, props)
                .map_err(err_map)?;
        }
        Ok(())
    }

    fn retrieve_property(
        &self,
        obj: Objid,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError> {
        let value = self
            .tx
            .seek_by_unique_composite_domain(
                WtWorldStateTable::ObjectPropertyValue,
                obj,
                uuid.to_bytes_le(),
            )
            .map_err(err_map)?;
        let perms = self.retrieve_property_permissions(obj, uuid)?;
        Ok((value, perms))
    }

    fn retrieve_property_permissions(
        &self,
        obj: Objid,
        uuid: Uuid,
    ) -> Result<PropPerms, WorldStateError> {
        self.tx
            .seek_by_unique_composite_domain::<_, _, PropPerms>(
                WtWorldStateTable::ObjectPropertyPermissions,
                obj,
                uuid.to_bytes_le(),
            )
            .map_err(err_map)?
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

            if let Some(parent) = self
                .tx
                .seek_unique_by_domain(WtWorldStateTable::ObjectParent, search_obj)
                .map_err(err_map)?
            {
                search_obj = parent;
                continue;
            };

            return Err(WorldStateError::PropertyNotFound(obj, name));
        };

        // Now that we have the propdef, we can look for the value & owner.
        // We should *always* have the owner.
        // But value could be 'clear' in which case we need to look in the parent.
        let perms = self
            .tx
            .seek_by_unique_composite_domain::<_, _, PropPerms>(
                WtWorldStateTable::ObjectPropertyPermissions,
                obj,
                propdef.uuid().to_bytes_le(),
            )
            .map_err(err_map)?
            .expect("Unable to get property permissions, coherence problem");

        match self
            .tx
            .seek_by_unique_composite_domain::<_, _, Var>(
                WtWorldStateTable::ObjectPropertyValue,
                obj,
                propdef.uuid().to_bytes_le(),
            )
            .map_err(err_map)?
        {
            Some(value) => Ok((propdef, value, perms, false)),
            None => {
                let mut search_obj = obj;
                loop {
                    let Some(parent) = self
                        .tx
                        .seek_unique_by_domain(WtWorldStateTable::ObjectParent, search_obj)
                        .map_err(err_map)?
                    else {
                        break Ok((propdef, v_none(), perms, true));
                    };
                    if parent == NOTHING {
                        break Ok((propdef, v_none(), perms, true));
                    }
                    search_obj = parent;

                    let value = self
                        .tx
                        .seek_by_unique_composite_domain(
                            WtWorldStateTable::ObjectPropertyValue,
                            search_obj,
                            propdef.uuid().to_bytes_le(),
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

    fn commit(&self) -> Result<CommitResult, WorldStateError> {
        Ok(self.tx.commit())
    }

    fn rollback(&self) -> Result<(), WorldStateError> {
        self.tx.rollback();
        Ok(())
    }
}

impl Database for WireTigerWorldState {
    fn loader_client(self: Arc<Self>) -> Result<Rc<dyn LoaderInterface>, WorldStateError> {
        let tx = WtWorldStateTransaction::new(self.db.clone());
        Ok(Rc::new(DbTxWorldState { tx: Box::new(tx) }))
    }

    fn world_state_source(self: Arc<Self>) -> Result<Arc<dyn WorldStateSource>, WorldStateError> {
        Ok(self)
    }
}

impl WtWorldStateTransaction {
    pub fn new(db: Arc<WiredTigerRelDb<WtWorldStateTable>>) -> Self {
        let tx = db.start_tx();
        Self { tx }
    }

    pub(crate) fn descendants(&self, obj: Objid) -> Result<ObjSet, WorldStateError> {
        let children = self
            .tx
            .seek_by_codomain::<Objid, Objid, ObjSet>(WtWorldStateTable::ObjectParent, obj)
            .map_err(err_map)?;

        let mut descendants = vec![];
        let mut queue: VecDeque<_> = children.iter().collect();
        while let Some(o) = queue.pop_front() {
            descendants.push(o);
            let children = self
                .tx
                .seek_by_codomain::<Objid, Objid, ObjSet>(WtWorldStateTable::ObjectParent, o)
                .map_err(err_map)?;
            queue.extend(children.iter());
        }

        Ok(ObjSet::from_items(&descendants))
    }

    fn closest_common_ancestor_with_ancestors(
        &self,
        a: Objid,
        b: Objid,
    ) -> Result<(Option<Objid>, HashSet<Objid>, HashSet<Objid>), WorldStateError> {
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
                let parent = self
                    .tx
                    .seek_unique_by_domain(WtWorldStateTable::ObjectParent, search_a)
                    .map_err(err_map)?
                    .unwrap_or(NOTHING);
                search_a = parent;
            }

            if search_b != NOTHING {
                ancestors_b.insert(search_b);
                let parent = self
                    .tx
                    .seek_unique_by_domain(WtWorldStateTable::ObjectParent, search_b)
                    .map_err(err_map)?
                    .unwrap_or(NOTHING);
                search_b = parent;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use moor_db::worldstate_transaction::WorldStateTransaction;
    use moor_values::model::VerbArgsSpec;
    use moor_values::model::{BinaryType, VerbAttrs};
    use moor_values::model::{CommitResult, WorldStateError};
    use moor_values::model::{HasUuid, Named};
    use moor_values::model::{ObjAttrs, PropFlag};
    use moor_values::model::{ObjSet, ValSet};
    use moor_values::util::BitEnum;
    use moor_values::var::Objid;
    use moor_values::var::{v_int, v_str};
    use moor_values::NOTHING;

    use crate::worldstate::wt_worldstate::{WireTigerWorldState, WtWorldStateTransaction};

    fn test_db() -> WireTigerWorldState {
        let db = WireTigerWorldState::open(None);
        db.0.db.create_tables();
        db.0.db.load_sequences();

        db.0
    }

    #[test]
    fn test_create_object() {
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());
        let oid = tx
            .create_object(
                None,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "test"),
            )
            .unwrap();
        assert_eq!(oid, Objid(0));
        assert!(tx.object_valid(oid).unwrap());
        assert_eq!(tx.get_object_owner(oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_parent(oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_location(oid).unwrap(), NOTHING);
        assert_eq!(tx.get_object_name(oid).unwrap(), "test");
        assert_eq!(tx.commit(), Ok(CommitResult::Success));

        // Verify existence in a new transaction.
        let tx = WtWorldStateTransaction::new(ws.db.clone());
        assert!(tx.object_valid(oid).unwrap());
        assert_eq!(tx.get_object_owner(oid).unwrap(), NOTHING);
    }

    #[test]
    fn test_create_object_fixed_id() {
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());
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
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());
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
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());

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
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());

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
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());
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

        let tx = WtWorldStateTransaction::new(ws.db.clone());
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

        let tx = WtWorldStateTransaction::new(ws.db.clone());
        assert_eq!(tx.get_object_location(c).unwrap(), a);
        assert_eq!(tx.get_object_location(b).unwrap(), c);
        assert_eq!(tx.get_object_contents(a).unwrap(), ObjSet::from_items(&[c]));
        assert_eq!(tx.get_object_contents(b).unwrap(), ObjSet::empty());
        assert_eq!(tx.get_object_contents(c).unwrap(), ObjSet::from_items(&[b]));
    }

    #[test]
    fn test_simple_property() {
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());

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
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());
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
        let tx = WtWorldStateTransaction::new(ws.db.clone());
        let vh = tx.resolve_verb(oid, "test2".into(), None).unwrap();
        assert_eq!(vh.names(), vec!["test2"]);
        assert_eq!(tx.commit(), Ok(CommitResult::Success));
    }

    #[test]
    fn test_transitive_property_resolution() {
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());

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
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());

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
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());
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
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());

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
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());

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
        let tx = WtWorldStateTransaction::new(ws.db.clone());
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
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());

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
        let ws = test_db();
        let tx = WtWorldStateTransaction::new(ws.db.clone());
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
