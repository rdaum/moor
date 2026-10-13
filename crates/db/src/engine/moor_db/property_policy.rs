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

//! Property-local opt-outs from snapshot isolation's write-conflict rule.

use super::{RelationWorkingSets, WorldStateSnapshot};
use crate::{
    engine::relation_defs::relation_key_unchanged,
    model::ObjAndUUIDHolder,
    tx::{OpType, WorkingSet, make_conflict_info},
};
use moor_common::model::{ConflictInfo, ConflictType, PropFlag, PropPerms};
use moor_var::{NOTHING, Symbol};

/// Use the transaction's final permissions; deleting permissions never grants clobber.
pub(super) fn property_can_clobber(
    flags: &WorkingSet<ObjAndUUIDHolder, PropPerms>,
    key: &ObjAndUUIDHolder,
) -> bool {
    let perms = match flags.tuples_ref().get(key).map(|op| &op.operation) {
        Some(OpType::Insert(perms) | OpType::Update(perms)) => Some(perms),
        Some(OpType::Delete) => None,
        None => flags
            .base_index()
            .index_lookup(key)
            .map(|entry| &entry.value),
    };
    perms.is_some_and(|perms| perms.flags().contains(PropFlag::Clobber))
}

impl RelationWorkingSets {
    /// Revalidate policy and property lifetime, including changes made during CAS rebase.
    pub(super) fn check_property_policies(
        &self,
        current: &WorldStateSnapshot,
    ) -> Result<(), ConflictInfo> {
        // An ownership transfer must see every inherited c property, including
        // properties added to a previously empty ancestor before publication.
        for object in &self.owner_change_reads {
            macro_rules! validate {
                ($field:ident) => {
                    if !relation_key_unchanged(self.$field.base_index(), &*current.$field, object) {
                        return Err(make_conflict_info(
                            Symbol::mk(stringify!($field)),
                            object,
                            ConflictType::ConcurrentWrite,
                        ));
                    }
                };
            }
            validate!(object_flags);
            validate!(object_parent);
            validate!(object_propdefs);
        }
        for key in &self.inherited_policy_reads {
            let unchanged = relation_key_unchanged(
                self.object_propflags.base_index(),
                &*current.object_propflags,
                key,
            ) && relation_key_unchanged(
                self.object_propdefs.base_index(),
                &*current.object_propdefs,
                &key.obj(),
            ) && relation_key_unchanged(
                self.object_flags.base_index(),
                &*current.object_flags,
                &key.obj(),
            );
            if !unchanged {
                return Err(make_conflict_info(
                    Symbol::mk("object_propflags"),
                    key,
                    ConflictType::ConcurrentWrite,
                ));
            }
        }
        for key in self.object_propvalues.tuples_ref().keys().chain(
            self.object_propflags
                .tuples_ref()
                .keys()
                .filter(|key| !self.object_propvalues.tuples_ref().contains_key(*key)),
        ) {
            if !relation_key_unchanged(
                self.object_propflags.base_index(),
                &*current.object_propflags,
                key,
            ) {
                return Err(make_conflict_info(
                    Symbol::mk("object_propflags"),
                    key,
                    ConflictType::ConcurrentWrite,
                ));
            }
            let local_perms = self.object_propflags.base_index().index_lookup(key);
            if local_perms.is_some() && !property_can_clobber(&self.object_propflags, key) {
                continue;
            }

            // Lazy permissions depend on the definer. Clobber also depends on
            // the object and definition surviving, even when values do not overlap.
            let mut object = key.obj();
            while object != NOTHING {
                macro_rules! validate {
                    ($field:ident, $key:expr) => {
                        if !relation_key_unchanged(
                            self.$field.base_index(),
                            &*current.$field,
                            &$key,
                        ) {
                            return Err(make_conflict_info(
                                Symbol::mk(stringify!($field)),
                                &$key,
                                ConflictType::ConcurrentWrite,
                            ));
                        }
                    };
                }
                validate!(object_flags, object);
                validate!(object_parent, object);
                validate!(object_propdefs, object);
                if let Some(defs) = final_value(&self.object_propdefs, &object)
                    && let Some(def) = defs.find(&key.uuid())
                {
                    if local_perms.is_none() {
                        let canonical = ObjAndUUIDHolder::new(&def.definer(), key.uuid());
                        validate!(object_propflags, canonical);
                    }
                    break;
                }
                let Some(parent) = final_value(&self.object_parent, &object) else {
                    break;
                };
                object = *parent;
            }
        }
        Ok(())
    }

    /// A clobber value replaces the complete persisted value, including lists.
    pub(super) fn clear_clobber_hints(&mut self) {
        for (key, op) in self.object_propvalues.tuples_mut() {
            if !property_can_clobber(&self.object_propflags, key) {
                continue;
            }
            if let OpType::Insert(value) | OpType::Update(value) = &mut op.operation
                && value.op_hint() != moor_var::OP_HINT_NONE
            {
                *value = value.clone().with_cleared_hint();
            }
        }
    }
}

fn final_value<'a, D: crate::tx::RelationDomain, C: crate::tx::RelationCodomain>(
    ws: &'a WorkingSet<D, C>,
    key: &D,
) -> Option<&'a C> {
    match ws.tuples_ref().get(key).map(|op| &op.operation) {
        Some(OpType::Insert(value) | OpType::Update(value)) => Some(value),
        Some(OpType::Delete) => None,
        None => ws.base_index().index_lookup(key).map(|entry| &entry.value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::DatabaseConfig, engine::moor_db::MoorDB};
    use moor_common::{
        model::{CommitResult, ObjAttrs, ObjectKind},
        util::BitEnum,
    };
    use moor_var::{Obj, Var, v_int, v_list, v_map, v_str};
    use std::sync::Arc;
    use uuid::Uuid;

    fn fixture(db: &Arc<MoorDB>, flags: BitEnum<PropFlag>, value: Var) -> (Obj, Uuid) {
        let mut tx = db.start_transaction();
        let object = tx
            .create_object(
                ObjectKind::NextObjid,
                ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), "policy test"),
            )
            .unwrap();
        let uuid = tx
            .define_property(
                &object,
                &object,
                Symbol::mk("value"),
                &object,
                flags,
                Some(value),
            )
            .unwrap();
        success(tx.commit().unwrap());
        (object, uuid)
    }

    fn success(result: CommitResult) {
        assert!(matches!(result, CommitResult::Success { .. }), "{result:?}");
    }
    fn conflict(result: CommitResult) {
        assert!(
            matches!(result, CommitResult::ConflictRetry { .. }),
            "{result:?}"
        );
    }
    fn value(db: &Arc<MoorDB>, object: Obj, uuid: Uuid) -> Var {
        db.start_transaction()
            .retrieve_property(&object, uuid)
            .unwrap()
            .0
            .unwrap()
    }
    fn db() -> Arc<MoorDB> {
        MoorDB::try_open(None, DatabaseConfig::default()).unwrap().0
    }

    #[test]
    fn strict_identical_writes_retry_and_retry_observes_winner() {
        let db = db();
        let (obj, uuid) = fixture(&db, BitEnum::new(), v_int(0));
        let mut first = db.start_transaction();
        let mut second = db.start_transaction();
        for tx in [&mut first, &mut second] {
            let old = tx
                .retrieve_property(&obj, uuid)
                .unwrap()
                .0
                .unwrap()
                .as_integer()
                .unwrap();
            tx.set_property(&obj, uuid, v_int(old + 1)).unwrap();
        }
        success(first.commit().unwrap());
        conflict(second.commit().unwrap());
        let mut retry = db.start_transaction();
        let old = retry
            .retrieve_property(&obj, uuid)
            .unwrap()
            .0
            .unwrap()
            .as_integer()
            .unwrap();
        retry.set_property(&obj, uuid, v_int(old + 1)).unwrap();
        success(retry.commit().unwrap());
        assert_eq!(value(&db, obj, uuid), v_int(2));
    }

    #[test]
    fn strict_maps_do_not_merge_and_case_insensitive_equality_does_not_accept() {
        let db = db();
        for (base, mine, theirs) in [
            (v_str("base"), v_str("ABC"), v_str("abc")),
            (
                v_map(&[(v_str("x"), v_int(0))]),
                v_map(&[(v_str("x"), v_int(1)), (v_str("a"), v_int(2))]),
                v_map(&[(v_str("x"), v_int(0)), (v_str("b"), v_int(3))]),
            ),
        ] {
            let (obj, uuid) = fixture(&db, BitEnum::new(), base);
            let mut first = db.start_transaction();
            let mut second = db.start_transaction();
            first.set_property(&obj, uuid, mine).unwrap();
            second.set_property(&obj, uuid, theirs).unwrap();
            success(first.commit().unwrap());
            conflict(second.commit().unwrap());
        }
    }

    #[test]
    fn clobber_follows_publication_order_and_policy_changes_retry() {
        let db = db();
        let (obj, uuid) = fixture(&db, BitEnum::new_with(PropFlag::Clobber), v_int(0));
        let mut older = db.start_transaction();
        let mut newer = db.start_transaction();
        older.set_property(&obj, uuid, v_int(1)).unwrap();
        newer.set_property(&obj, uuid, v_int(2)).unwrap();
        success(newer.commit().unwrap());
        let mut strict = db.start_transaction();
        strict
            .update_property_info(&obj, uuid, None, Some(BitEnum::new()), None)
            .unwrap();
        strict.set_property(&obj, uuid, v_int(3)).unwrap();
        success(older.commit().unwrap());
        assert_eq!(value(&db, obj, uuid), v_int(1));
        conflict(strict.commit().unwrap()); // The current timestamp is now smaller.

        let mut stale = db.start_transaction();
        stale.set_property(&obj, uuid, v_int(4)).unwrap();
        let mut revoke = db.start_transaction();
        revoke
            .update_property_info(&obj, uuid, None, Some(BitEnum::new()), None)
            .unwrap();
        success(revoke.commit().unwrap());
        conflict(stale.commit().unwrap());
        assert_eq!(value(&db, obj, uuid), v_int(1));
    }

    #[test]
    fn clobber_does_not_override_other_conflicts_or_publish_partial_writes() {
        let db = db();
        let (obj, uuid) = fixture(&db, BitEnum::new_with(PropFlag::Clobber), v_int(0));
        let mut first = db.start_transaction();
        let mut second = db.start_transaction();
        first.set_property(&obj, uuid, v_int(1)).unwrap();
        second.set_property(&obj, uuid, v_int(2)).unwrap();
        first.set_object_name(&obj, "first".into()).unwrap();
        second.set_object_name(&obj, "second".into()).unwrap();
        success(first.commit().unwrap());
        conflict(second.commit().unwrap());
        assert_eq!(value(&db, obj, uuid), v_int(1));
        assert_eq!(
            db.start_transaction().get_object_name(&obj).unwrap(),
            "first"
        );
    }

    #[test]
    fn clobber_revalidates_lazy_inheritance_and_definition_lifetime() {
        for metadata_only in [false, true] {
            let db = db();
            let (parent, uuid) = fixture(&db, BitEnum::new_with(PropFlag::Clobber), v_int(0));
            let mut create = db.start_transaction();
            let child = create
                .create_object(
                    ObjectKind::NextObjid,
                    ObjAttrs::new(parent, parent, NOTHING, BitEnum::new(), "child"),
                )
                .unwrap();
            success(create.commit().unwrap());
            let mut stale = db.start_transaction();
            if metadata_only {
                stale
                    .update_property_info(&child, uuid, Some(child), None, None)
                    .unwrap();
            } else {
                stale.set_property(&child, uuid, v_int(1)).unwrap();
            }
            let mut revoke = db.start_transaction();
            revoke
                .update_property_info(&parent, uuid, None, Some(BitEnum::new()), None)
                .unwrap();
            success(revoke.commit().unwrap());
            conflict(stale.commit().unwrap());
        }
        for recycle in [false, true] {
            let db = db();
            let (obj, uuid) = fixture(&db, BitEnum::new_with(PropFlag::Clobber), v_int(0));
            let mut stale = db.start_transaction();
            stale.set_property(&obj, uuid, v_int(1)).unwrap();
            let mut delete = db.start_transaction();
            if recycle {
                delete.recycle_object(&obj).unwrap();
            } else {
                delete.delete_property(&obj, uuid).unwrap();
            }
            success(delete.commit().unwrap());
            conflict(stale.commit().unwrap());
        }
    }

    #[test]
    fn inherited_policy_dependency_survives_intermediate_reparenting() {
        let db = db();
        let (parent, uuid) = fixture(&db, BitEnum::new_with(PropFlag::Clobber), v_int(0));
        let mut create = db.start_transaction();
        let child = create
            .create_object(
                ObjectKind::NextObjid,
                ObjAttrs::new(parent, NOTHING, NOTHING, BitEnum::new(), "child"),
            )
            .unwrap();
        success(create.commit().unwrap());
        let mut stale = db.start_transaction();
        stale.set_object_parent(&child, &parent).unwrap();
        stale
            .update_property_info(&child, uuid, Some(child), None, None)
            .unwrap();
        stale.set_object_parent(&child, &NOTHING).unwrap();

        let mut revoke = db.start_transaction();
        revoke
            .update_property_info(&parent, uuid, None, Some(BitEnum::new()), None)
            .unwrap();
        success(revoke.commit().unwrap());
        conflict(stale.commit().unwrap());
    }

    #[test]
    fn clobber_lists_persist_as_replacements_including_after_clear() {
        for clear in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let db = MoorDB::try_open(Some(dir.path()), DatabaseConfig::default())
                .unwrap()
                .0;
            let (obj, uuid) = fixture(
                &db,
                BitEnum::new_with(PropFlag::Clobber),
                v_list(&[v_int(0)]),
            );
            let mut older = db.start_transaction();
            let mut newer = db.start_transaction();
            let base = value(&db, obj, uuid);
            let expected = base.push(&v_int(1)).unwrap();
            older.set_property(&obj, uuid, expected.clone()).unwrap();
            if clear {
                newer.clear_property(&obj, uuid).unwrap();
            } else {
                newer
                    .set_property(&obj, uuid, base.push(&v_int(2)).unwrap())
                    .unwrap();
            }
            success(newer.commit().unwrap());
            success(older.commit().unwrap());
            assert_eq!(value(&db, obj, uuid), expected);
            db.wait_for_persistence().unwrap();
            db.stop().unwrap();
            drop(db);
            let reopened = MoorDB::try_open(Some(dir.path()), DatabaseConfig::default())
                .unwrap()
                .0;
            assert_eq!(value(&reopened, obj, uuid), expected);
            assert!(
                reopened
                    .start_transaction()
                    .retrieve_property(&obj, uuid)
                    .unwrap()
                    .1
                    .flags()
                    .contains(PropFlag::Clobber)
            );
        }
    }
}
