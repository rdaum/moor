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

use super::*;
use crate::config::DatabaseConfig;
use crate::engine::moor_db::WorldStateTransaction;
use crate::model::ObjAndUUIDHolder;
use moor_common::model::{HasUuid, ObjAttrs, ObjFlag, ObjectKind, PropFlag};
use moor_common::util::BitEnum;
use moor_var::{NOTHING, Obj, Symbol, v_int};

fn success(result: CommitResult) {
    assert!(matches!(result, CommitResult::Success { .. }), "{result:?}");
}

fn prepare<'db>(db: &'db MoorDB, tx: WorldStateTransaction) -> PreparedCommit<'db> {
    match PreparedCommit::prepare(db, tx.into_working_sets().unwrap()) {
        Ok(prepared) => prepared,
        Err(PrepareError::Conflict(info)) => panic!("unexpected conflict: {info}"),
        Err(PrepareError::Database(error)) => panic!("unexpected database error: {error}"),
    }
}

fn lose(prepared: PreparedCommit<'_>) -> RebaseRequired<'_> {
    match prepared.try_publish() {
        Ok(_) => panic!("publication should have lost to an intervening commit"),
        Err(retry) => retry,
    }
}

fn publish(prepared: PreparedCommit<'_>) -> PublishedCommit<'_> {
    match prepared.try_publish() {
        Ok(published) => published,
        Err(_) => panic!("publication should succeed without an intervening commit"),
    }
}

fn create_object(tx: &mut WorldStateTransaction, name: &str) -> Obj {
    tx.create_object(
        ObjectKind::NextObjid,
        ObjAttrs::new(NOTHING, NOTHING, NOTHING, BitEnum::new(), name),
    )
    .unwrap()
}

#[test]
fn ownership_transfer_revalidates_an_empty_property_scan_after_cas_loss() {
    let db = MoorDB::try_open(
        crate::StorageConfig::temporary_fjall(),
        DatabaseConfig::default(),
        crate::PersistenceConfig::default(),
    )
    .unwrap()
    .0;
    let mut tx = db.start_transaction();
    let parent = create_object(&mut tx, "parent");
    let child = create_object(&mut tx, "child");
    tx.set_object_parent(&child, &parent).unwrap();
    success(tx.commit().unwrap());

    let mut transfer = db.start_transaction();
    transfer.set_object_owner(&child, &parent).unwrap();
    let prepared = prepare(&db, transfer);
    let mut winner = db.start_transaction();
    let uuid = winner
        .define_property(
            &parent,
            &parent,
            Symbol::mk("new"),
            &parent,
            BitEnum::new_with(PropFlag::Chown),
            Some(v_int(1)),
        )
        .unwrap();
    success(winner.commit().unwrap());
    assert!(lose(prepared).rebase().is_err());

    let mut retry = db.start_transaction();
    retry.set_object_owner(&child, &parent).unwrap();
    success(retry.commit().unwrap());
    assert_eq!(
        db.start_transaction()
            .retrieve_property_permissions(&child, uuid)
            .unwrap()
            .owner(),
        child
    );
}

#[test]
fn repeated_cas_losses_advance_the_checked_root_and_preserve_winner_writes() {
    let db = MoorDB::try_open(
        crate::StorageConfig::temporary_fjall(),
        DatabaseConfig::default(),
        crate::PersistenceConfig::default(),
    )
    .unwrap()
    .0;
    let obj = Obj::mk_id(1);
    let mut tx = db.start_transaction();
    tx.set_object_name(&obj, "ours".into()).unwrap();
    let mut prepared = prepare(&db, tx);
    let timestamp = prepared.data.tx_timestamp;
    assert!(prepared.data.checkers.object_name.is_some());
    assert!(prepared.data.checkers.object_flags.is_none());
    assert!(prepared.data.checkers.object_propvalues.is_none());

    for attempt in 0..3 {
        let old_checked = prepared.checked_root.clone();
        let mut winner = db.start_transaction();
        // The same object key in another relation guarantees a Bloom hit.
        winner
            .set_object_flags(
                &obj,
                if attempt % 2 == 0 {
                    BitEnum::new_with(ObjFlag::Wizard)
                } else {
                    BitEnum::new()
                },
            )
            .unwrap();
        success(winner.commit().unwrap());
        let winner = db.snapshot_planes.load_root();
        let retry = lose(prepared);
        assert!(Arc::ptr_eq(&retry.checked_root, &old_checked));
        assert_eq!(
            retry.data.checkers.rebase_check(
                &retry.data.working_sets,
                &retry.checked_root,
                &winner,
            ),
            RebaseCheck::ExactlyDisjoint
        );
        prepared = retry.rebase().unwrap();
        assert!(Arc::ptr_eq(&prepared.checked_root, &winner));
        assert_eq!(prepared.candidate.version, winner.version + 1);
        assert!(Arc::ptr_eq(
            &prepared.candidate.object_flags,
            &winner.object_flags
        ));
    }
    let expected_version = prepared.candidate.version;
    let published = publish(prepared);
    assert_eq!(published.publication_version, expected_version);
    assert_eq!(published.tx_timestamp, timestamp);
    success(published.enqueue_persistence());
    db.wait_for_persistence().unwrap();
    assert_eq!(
        db.start_transaction().get_object_name(&obj).unwrap(),
        "ours"
    );
}

#[test]
fn rebase_accepts_a_bloom_miss_and_falls_back_when_coverage_expires() {
    for expire_coverage in [false, true] {
        let db = MoorDB::try_open(
            crate::StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            crate::PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let mut tx = db.start_transaction();
        tx.set_object_name(&Obj::mk_id(1), "ours".into()).unwrap();
        let prepared = prepare(&db, tx);
        // Select a key whose Bloom bits do not intersect our write set.
        let winner_obj = (2..1000)
            .map(Obj::mk_id)
            .find(|obj| {
                let mut bloom = CommitBloom::new();
                bloom.insert(obj);
                !prepared.data.bloom.might_intersect(&bloom)
            })
            .unwrap();
        for version in 0..if expire_coverage { 35 } else { 1 } {
            let mut winner = db.start_transaction();
            winner
                .set_object_name(&winner_obj, format!("winner {version}"))
                .unwrap();
            success(winner.commit().unwrap());
        }
        let retry = lose(prepared);
        let winner = db.snapshot_planes.load_root();
        assert_eq!(
            retry.data.checkers.rebase_check(
                &retry.data.working_sets,
                &retry.checked_root,
                &winner,
            ),
            if expire_coverage {
                RebaseCheck::ExactlyDisjoint
            } else {
                RebaseCheck::BloomDisjoint
            }
        );
        success(publish(retry.rebase().unwrap()).enqueue_persistence());
        assert_eq!(
            db.start_transaction().get_object_name(&winner_obj).unwrap(),
            if expire_coverage {
                "winner 34"
            } else {
                "winner 0"
            }
        );
        assert_eq!(
            db.start_transaction()
                .get_object_name(&Obj::mk_id(1))
                .unwrap(),
            "ours"
        );
    }
}

#[test]
fn overlap_after_a_successful_rebase_rejects_and_releases_admission() {
    let db = MoorDB::try_open(
        crate::StorageConfig::temporary_fjall(),
        DatabaseConfig::default(),
        crate::PersistenceConfig::default(),
    )
    .unwrap()
    .0;
    let obj = Obj::mk_id(1);
    let mut tx = db.start_transaction();
    tx.set_object_name(&obj, "ours".into()).unwrap();
    let prepared = prepare(&db, tx);
    let mut unrelated = db.start_transaction();
    unrelated.set_object_flags(&obj, BitEnum::new()).unwrap();
    success(unrelated.commit().unwrap());
    let prepared = lose(prepared).rebase().unwrap();

    let mut winner = db.start_transaction();
    winner.set_object_name(&obj, "winner".into()).unwrap();
    success(winner.commit().unwrap());
    let version = db.snapshot_planes.load_root().version;
    assert!(lose(prepared).rebase().is_err());
    assert_eq!(db.snapshot_planes.load_root().version, version);
    assert_eq!(
        db.start_transaction().get_object_name(&obj).unwrap(),
        "winner"
    );
    db.wait_for_persistence().unwrap();
    db.set_commit_queue_policy(Duration::ZERO, Duration::from_millis(10));
    // Any leaked permit makes the final reservation time out.
    drop(db.coordinator.hold_all_admission());
}

#[test]
fn dropping_an_exhausted_retry_releases_admission_without_publication() {
    let db = MoorDB::try_open(
        crate::StorageConfig::temporary_fjall(),
        DatabaseConfig::default(),
        crate::PersistenceConfig::default(),
    )
    .unwrap()
    .0;
    let obj = Obj::mk_id(1);
    let mut tx = db.start_transaction();
    tx.set_object_name(&obj, "ours".into()).unwrap();
    let prepared = prepare(&db, tx);
    let mut winner = db.start_transaction();
    winner.set_object_name(&obj, "winner".into()).unwrap();
    success(winner.commit().unwrap());
    let version = db.snapshot_planes.load_root().version;
    drop(lose(prepared));
    assert_eq!(db.snapshot_planes.load_root().version, version);
    db.wait_for_persistence().unwrap();
    db.set_commit_queue_policy(Duration::ZERO, Duration::from_millis(10));
    drop(db.coordinator.hold_all_admission());
}

#[test]
fn rebase_checks_clobber_policy_lifetime_and_other_writes() {
    for change in ["value", "policy", "delete", "other"] {
        let db = MoorDB::try_open(
            crate::StorageConfig::temporary_fjall(),
            DatabaseConfig::default(),
            crate::PersistenceConfig::default(),
        )
        .unwrap()
        .0;
        let mut seed = db.start_transaction();
        let obj = create_object(&mut seed, "object");
        let uuid = seed
            .define_property(
                &obj,
                &obj,
                Symbol::mk("value"),
                &obj,
                BitEnum::new_with(PropFlag::Clobber),
                Some(v_int(0)),
            )
            .unwrap();
        success(seed.commit().unwrap());
        let mut ours = db.start_transaction();
        ours.set_property(&obj, uuid, v_int(1)).unwrap();
        if change == "other" {
            ours.set_object_name(&obj, "ours".into()).unwrap();
        }
        let prepared = prepare(&db, ours);
        let mut winner = db.start_transaction();
        match change {
            "value" => winner.set_property(&obj, uuid, v_int(2)).unwrap(),
            "policy" => winner
                .update_property_info(&obj, uuid, None, Some(BitEnum::new()), None)
                .unwrap(),
            "delete" => winner.delete_property(&obj, uuid).unwrap(),
            "other" => winner.set_object_name(&obj, "theirs".into()).unwrap(),
            _ => unreachable!(),
        }
        success(winner.commit().unwrap());
        let result = lose(prepared).rebase();
        if change != "value" {
            assert!(
                result.is_err(),
                "{change} must invalidate the prepared writes"
            );
            continue;
        }
        let prepared = result.unwrap();
        assert_eq!(
            prepared
                .candidate
                .object_propvalues
                .index_lookup(&ObjAndUUIDHolder::new(&obj, uuid),)
                .unwrap()
                .value,
            v_int(1)
        );
        success(publish(prepared).enqueue_persistence());
        assert_eq!(
            db.start_transaction()
                .retrieve_property(&obj, uuid)
                .unwrap()
                .0,
            Some(v_int(1))
        );
    }
}

#[test]
fn rebased_publication_persists_its_version_operations_and_definition_changes() {
    let dir = tempfile::tempdir().unwrap();
    let db = MoorDB::try_open(
        crate::StorageConfig::fjall(dir.path()),
        DatabaseConfig::default(),
        crate::PersistenceConfig::default(),
    )
    .unwrap()
    .0;
    let mut seed = db.start_transaction();
    let ours_obj = create_object(&mut seed, "ours");
    let other_obj = create_object(&mut seed, "other");
    let old_uuid = seed
        .define_property(
            &ours_obj,
            &ours_obj,
            Symbol::mk("old_prop"),
            &ours_obj,
            BitEnum::new(),
            Some(v_int(0)),
        )
        .unwrap();
    success(seed.commit().unwrap());

    let mut ours = db.start_transaction();
    ours.delete_property(&ours_obj, old_uuid).unwrap();
    let ours_uuid = ours
        .define_property(
            &ours_obj,
            &ours_obj,
            Symbol::mk("ours_prop"),
            &ours_obj,
            BitEnum::new(),
            Some(v_int(10)),
        )
        .unwrap();
    let prepared = prepare(&db, ours);
    let timestamp = prepared.data.tx_timestamp;
    let mut winner = db.start_transaction();
    let other_uuid = winner
        .define_property(
            &other_obj,
            &other_obj,
            Symbol::mk("other_prop"),
            &other_obj,
            BitEnum::new(),
            Some(v_int(20)),
        )
        .unwrap();
    success(winner.commit().unwrap());
    let checked = db.snapshot_planes.load_root();
    let prepared = lose(prepared).rebase().unwrap();
    let published = publish(prepared);
    assert_eq!(published.publication_version, checked.version + 1);
    assert_eq!(published.tx_timestamp, timestamp);
    assert!(matches!(
        published.definition_changes.as_slice(),
        [PropertyDefinitionChange::Remove(removed), PropertyDefinitionChange::Upsert(added)]
            if *removed == old_uuid && added.uuid() == ours_uuid
                && added.name() == Symbol::mk("ours_prop")
    ));
    success(published.enqueue_persistence());
    db.wait_for_persistence().unwrap();
    db.stop().unwrap();
    drop(db);

    let reopened = MoorDB::try_open(
        crate::StorageConfig::fjall(dir.path()),
        DatabaseConfig::default(),
        crate::PersistenceConfig::default(),
    )
    .unwrap()
    .0;
    let tx = reopened.start_transaction();
    assert!(
        tx.get_properties(&ours_obj)
            .unwrap()
            .find(&old_uuid)
            .is_none()
    );
    assert_eq!(
        tx.retrieve_property(&ours_obj, ours_uuid).unwrap().0,
        Some(v_int(10))
    );
    assert_eq!(
        tx.retrieve_property(&other_obj, other_uuid).unwrap().0,
        Some(v_int(20))
    );
    assert!(
        tx.get_properties(&ours_obj)
            .unwrap()
            .find(&ours_uuid)
            .is_some()
    );
    assert!(
        tx.get_properties(&other_obj)
            .unwrap()
            .find(&other_uuid)
            .is_some()
    );
}

#[test]
fn rebase_reuses_admission_when_the_queue_has_no_free_permits() {
    let db = MoorDB::try_open(
        crate::StorageConfig::temporary_fjall(),
        DatabaseConfig::default(),
        crate::PersistenceConfig::default(),
    )
    .unwrap()
    .0;
    db.set_commit_queue_policy(Duration::ZERO, Duration::from_millis(10));
    let mut reserved = db.coordinator.hold_all_admission();
    drop(reserved.pop().unwrap());
    drop(reserved.pop().unwrap());
    let obj = Obj::mk_id(1);
    let mut ours = db.start_transaction();
    ours.set_object_name(&obj, "ours".into()).unwrap();
    let prepared = prepare(&db, ours);
    let mut winner = db.start_transaction();
    winner.set_object_flags(&obj, BitEnum::new()).unwrap();
    success(winner.commit().unwrap());
    db.wait_for_persistence().unwrap();
    reserved.push(db.coordinator.admit(Timestamp(999)).unwrap());
    assert!(matches!(
        db.coordinator.admit(Timestamp(1000)),
        Err(CommitAdmissionError::Timeout { .. })
    ));

    // Rebase must retain our original permit, even while all others are held.
    success(publish(lose(prepared).rebase().unwrap()).enqueue_persistence());
    drop(reserved);
    db.wait_for_persistence().unwrap();
    drop(db.coordinator.hold_all_admission());
}
