// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Cache cleanup owns exclusive borrows for the lifetime of a relation edit.

use super::PropertyPermMemo;
use crate::{
    ObjAndUUIDHolder,
    cache::{
        ancestry_cache::AncestryCache, prop_cache::PropResolutionCache,
        verb_cache::VerbResolutionCache,
    },
};
use moor_var::Obj;
use uuid::Uuid;

pub(crate) enum EditScope {
    Verbs(Vec<Obj>),
    Properties {
        objects: Vec<Obj>,
        removed: Option<Uuid>,
    },
    PropertyValue(Obj),
    Permissions(ObjAndUUIDHolder),
    Owner(Obj),
    Hierarchy(Vec<Obj>),
    Remove {
        objects: Vec<Obj>,
        removed: Vec<Obj>,
    },
    Renumber {
        objects: Vec<Obj>,
        old: Obj,
        new: Obj,
    },
}

pub(crate) struct EditInvalidation<'a> {
    scope: EditScope,
    verbs: &'a mut VerbResolutionCache,
    properties: &'a mut PropResolutionCache,
    ancestry: &'a mut AncestryCache,
    permissions: &'a mut PropertyPermMemo,
}

impl<'a> EditInvalidation<'a> {
    pub(crate) fn new(
        scope: EditScope,
        verbs: &'a mut VerbResolutionCache,
        properties: &'a mut PropResolutionCache,
        ancestry: &'a mut AncestryCache,
        permissions: &'a mut PropertyPermMemo,
    ) -> Self {
        Self {
            scope,
            verbs,
            properties,
            ancestry,
            permissions,
        }
    }
}

impl Drop for EditInvalidation<'_> {
    fn drop(&mut self) {
        match &self.scope {
            EditScope::Verbs(objects) => self.verbs.invalidate_objects(objects),
            EditScope::Properties { objects, removed } => {
                self.properties.invalidate_objects(objects);
                for object in objects {
                    self.permissions.invalidate_cached_for_obj(object);
                    if let Some(uuid) = removed {
                        self.permissions
                            .invalidate_known_for_holder(&ObjAndUUIDHolder::new(object, *uuid));
                    }
                }
            }
            EditScope::PropertyValue(object) => self.properties.invalidate_objects(&[*object]),
            EditScope::Permissions(holder) => self.permissions.invalidate_cached_for_holder(holder),
            EditScope::Owner(object) => self.permissions.invalidate_cached_for_obj(object),
            EditScope::Hierarchy(objects)
            | EditScope::Remove { objects, .. }
            | EditScope::Renumber { objects, .. } => {
                self.verbs.invalidate_objects(objects);
                self.properties.invalidate_objects(objects);
                self.ancestry.invalidate_objects(objects);
                for object in objects {
                    self.permissions.invalidate_cached_for_obj(object);
                }
                match &self.scope {
                    EditScope::Remove { removed, .. } => {
                        for object in removed {
                            self.permissions.invalidate_known_for_obj(object);
                        }
                    }
                    EditScope::Renumber { old, new, .. } => {
                        self.permissions.invalidate_known_for_obj(old);
                        self.permissions.invalidate_known_for_obj(new);
                        // Renumbering also changes owners outside the inheritance branch.
                        self.permissions.clear_cached();
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DatabaseConfig, engine::moor_db::MoorDB};
    use moor_common::{
        model::{CommitResult, HasUuid, ObjAttrs, ObjectKind, PropFlag, WorldStateError},
        util::BitEnum,
    };
    use moor_var::{NOTHING, Symbol, v_int, v_none};

    fn create(tx: &mut crate::engine::moor_db::WorldStateTransaction, parent: Obj) -> Obj {
        tx.create_object(
            ObjectKind::NextObjid,
            ObjAttrs::new(NOTHING, parent, NOTHING, BitEnum::new(), "test"),
        )
        .unwrap()
    }

    #[test]
    fn failed_initial_value_leaves_definition_visible_to_warm_descendant_cache() {
        let db = MoorDB::try_open(
            crate::StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            crate::PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let mut tx = db.start_transaction();
        let parent = create(&mut tx, NOTHING);
        let child = create(&mut tx, parent);
        // An existing property makes the first-parent cache point at this definer.
        tx.define_property(
            &parent,
            &parent,
            Symbol::mk("existing"),
            &parent,
            BitEnum::new(),
            Some(v_int(1)),
        )
        .unwrap();
        tx.commit().unwrap();
        let mut tx = db.start_transaction();
        let name = Symbol::mk("partial");
        assert!(tx.resolve_property(&child, name).is_err());
        let version = tx.prop_resolution_cache_version();
        assert_eq!(
            tx.define_property(
                &parent,
                &parent,
                name,
                &parent,
                BitEnum::new(),
                Some(v_none())
            ),
            Err(WorldStateError::PropertyTypeMismatch)
        );
        assert!(tx.prop_resolution_cache_version() > version);
        let (_, value, _, clear) = tx.resolve_property(&child, name).unwrap();
        assert_eq!(value, v_none());
        assert!(clear);
        assert!(matches!(
            tx.commit().unwrap(),
            CommitResult::Success {
                mutations_made: true,
                ..
            }
        ));
        assert!(
            db.start_transaction()
                .resolve_property(&child, name)
                .is_ok()
        );
    }

    #[test]
    fn failed_property_permissions_do_not_hide_an_earlier_value_write() {
        let db = MoorDB::try_open(
            crate::StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            crate::PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let mut tx = db.start_transaction();
        let object = create(&mut tx, NOTHING);
        tx.commit().unwrap();
        let mut tx = db.start_transaction();
        let uuid = Uuid::new_v4();
        assert!(!tx.has_mutations());
        assert!(tx.set_property(&object, uuid, v_int(7)).is_err());
        assert_eq!(
            tx.object_propvalues
                .get(&ObjAndUUIDHolder::new(&object, uuid))
                .unwrap(),
            Some(v_int(7))
        );
        assert!(tx.has_mutations());
        assert!(tx.into_working_sets().unwrap().has_mutations);
    }

    #[test]
    fn rename_stays_visible_after_a_later_permissions_error() {
        let db = MoorDB::try_open(
            crate::StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            crate::PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let mut tx = db.start_transaction();
        let parent = create(&mut tx, NOTHING);
        let child = create(&mut tx, parent);
        let old = Symbol::mk("before");
        let new = Symbol::mk("after");
        let uuid = tx
            .define_property(
                &parent,
                &parent,
                old,
                &parent,
                BitEnum::new(),
                Some(v_int(1)),
            )
            .unwrap();
        assert!(tx.resolve_property(&child, old).is_ok());
        assert!(tx.resolve_property(&child, new).is_err());
        // Remove canonical policy to force an error after the definition update.
        tx.object_propflags
            .delete(&ObjAndUUIDHolder::new(&parent, uuid))
            .unwrap();
        tx.prop_perm_memo.clear_cached();
        assert!(
            tx.update_property_info(&parent, uuid, Some(child), None, Some(new))
                .is_err()
        );
        assert!(tx.find_property_by_name(&child, old).is_none());
        assert_eq!(tx.find_property_by_name(&child, new).unwrap().uuid(), uuid);
    }

    #[test]
    fn permissions_follow_canonical_edits_and_new_ancestry() {
        let db = MoorDB::try_open(
            crate::StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            crate::PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let mut tx = db.start_transaction();
        let parent = create(&mut tx, NOTHING);
        let other = create(&mut tx, NOTHING);
        let child = create(&mut tx, parent);
        let name = Symbol::mk("value");
        let uuid = tx
            .define_property(
                &parent,
                &parent,
                name,
                &parent,
                BitEnum::new(),
                Some(v_int(1)),
            )
            .unwrap();
        let other_uuid = tx
            .define_property(&other, &other, name, &other, BitEnum::new(), Some(v_int(2)))
            .unwrap();
        assert_eq!(
            tx.retrieve_property_permissions(&child, uuid)
                .unwrap()
                .owner(),
            parent
        );
        tx.update_property_info(
            &parent,
            uuid,
            Some(other),
            Some(BitEnum::new_with(PropFlag::Read)),
            None,
        )
        .unwrap();
        let perms = tx.retrieve_property_permissions(&child, uuid).unwrap();
        assert_eq!(perms.owner(), other);
        assert!(perms.flags().contains(PropFlag::Read));
        tx.set_object_parent(&child, &other).unwrap();
        assert!(tx.retrieve_property_permissions(&child, uuid).is_err());
        let (prop, value, _, _) = tx.resolve_property(&child, name).unwrap();
        assert_eq!(prop.uuid(), other_uuid);
        assert_eq!(value, v_int(2));
    }

    #[test]
    fn no_op_setters_preserve_read_only_commit_and_cache_generations() {
        let db = MoorDB::try_open(
            crate::StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            crate::PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let mut tx = db.start_transaction();
        let parent = create(&mut tx, NOTHING);
        let child = create(&mut tx, parent);
        let name = Symbol::mk("value");
        let uuid = tx
            .define_property(
                &parent,
                &parent,
                name,
                &parent,
                BitEnum::new(),
                Some(v_int(1)),
            )
            .unwrap();
        tx.set_object_location(&child, &parent).unwrap();
        tx.commit().unwrap();
        let mut tx = db.start_transaction();
        tx.resolve_property(&child, name).unwrap();
        assert!(
            tx.resolve_verb(&child, Symbol::mk("absent"), None, None)
                .is_err()
        );
        let versions = (
            tx.prop_resolution_cache_version(),
            tx.verb_resolution_cache_version(),
        );
        tx.set_object_parent(&child, &parent).unwrap();
        let owner = tx.get_object_owner(&child).unwrap();
        tx.set_object_owner(&child, &owner).unwrap();
        tx.set_object_flags(&child, BitEnum::new()).unwrap();
        tx.set_object_name(&child, "test".to_string()).unwrap();
        tx.set_object_location(&child, &parent).unwrap();
        tx.update_property_info(&parent, uuid, None, None, None)
            .unwrap();
        assert_eq!(
            (
                tx.prop_resolution_cache_version(),
                tx.verb_resolution_cache_version()
            ),
            versions
        );
        assert!(!tx.has_mutations());
        assert!(matches!(
            tx.commit().unwrap(),
            CommitResult::Success {
                mutations_made: false,
                ..
            }
        ));
    }

    #[test]
    fn renumber_and_removal_invalidate_old_new_and_descendant_entries() {
        let db = MoorDB::try_open(
            crate::StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            crate::PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let mut tx = db.start_transaction();
        let parent = create(&mut tx, NOTHING);
        let child = create(&mut tx, parent);
        let name = Symbol::mk("value");
        let uuid = tx
            .define_property(
                &parent,
                &parent,
                name,
                &parent,
                BitEnum::new(),
                Some(v_int(1)),
            )
            .unwrap();
        tx.resolve_property(&parent, name).unwrap();
        tx.resolve_property(&child, name).unwrap();
        let new = Obj::mk_id(100);
        assert!(tx.resolve_property(&new, name).is_err());
        assert_eq!(
            tx.renumber_object(&parent, Some(ObjectKind::Objid(new)))
                .unwrap(),
            new
        );
        assert!(tx.resolve_property(&parent, name).is_err());
        assert_eq!(tx.resolve_property(&child, name).unwrap().0.definer(), new);
        assert_eq!(tx.resolve_property(&new, name).unwrap().0.uuid(), uuid);
        tx.recycle_objects(&std::collections::HashSet::from([new]))
            .unwrap();
        assert!(tx.resolve_property(&child, name).is_err());
        assert!(tx.resolve_property(&new, name).is_err());
        assert!(tx.retrieve_property_permissions(&child, uuid).is_err());
    }

    #[test]
    fn unwind_after_relation_edit_invalidates_cached_ancestry() {
        let db = MoorDB::try_open(
            crate::StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            crate::PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let mut tx = db.start_transaction();
        let parent = create(&mut tx, NOTHING);
        let other = create(&mut tx, NOTHING);
        let child = create(&mut tx, parent);
        assert!(tx.ancestors(&child, false).unwrap().contains(parent));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let edit = tx.edit(EditScope::Hierarchy(vec![child]));
            edit.object_parent.upsert(child, other).unwrap();
            panic!("unwind during edit");
        }));
        assert!(result.is_err());
        let ancestors = tx.ancestors(&child, false).unwrap();
        assert!(ancestors.contains(other));
        assert!(!ancestors.contains(parent));
    }
}
