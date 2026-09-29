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

use crate::{Database, DatabaseConfig, TxDB};
use moor_common::{
    model::{ObjAttrs, ObjFlag, ObjectKind, TaskPermissions, WorldStateError, WorldStateSource},
    util::BitEnum,
};
use moor_var::{NOTHING, Obj, SYSTEM_OBJECT};

const WIZARD: Obj = Obj::mk_id(2);
const OWNER: Obj = Obj::mk_id(3);
const OTHER: Obj = Obj::mk_id(4);

fn permissions(principal: Obj) -> TaskPermissions {
    TaskPermissions::new(principal, BitEnum::new())
}

fn test_db() -> TxDB {
    let db = TxDB::try_open_temporary(DatabaseConfig::default())
        .unwrap()
        .0;
    let mut loader = db.loader_client().unwrap();
    loader
        .create_object(
            ObjectKind::Objid(WIZARD),
            &ObjAttrs::new(WIZARD, NOTHING, NOTHING, ObjFlag::all_flags(), "Wizard"),
        )
        .unwrap();
    loader
        .create_object(
            ObjectKind::Objid(OWNER),
            &ObjAttrs::new(OWNER, NOTHING, NOTHING, BitEnum::new(), "Owner"),
        )
        .unwrap();
    loader
        .create_object(
            ObjectKind::Objid(OTHER),
            &ObjAttrs::new(OTHER, NOTHING, NOTHING, BitEnum::new(), "Other"),
        )
        .unwrap();
    loader
        .create_object(
            ObjectKind::Objid(SYSTEM_OBJECT),
            &ObjAttrs::new(OWNER, NOTHING, NOTHING, BitEnum::new(), "System"),
        )
        .unwrap();
    loader.commit().unwrap();
    db
}

#[test]
fn recycle_object_requires_owner_or_wizard_not_public_write() {
    let db = test_db();
    let mut tx = db.new_world_state().unwrap();
    let obj = tx
        .create_object(
            &permissions(WIZARD),
            &NOTHING,
            &OWNER,
            BitEnum::new_with(ObjFlag::Write),
            ObjectKind::NextObjid,
        )
        .unwrap();

    let err = tx.recycle_object(&permissions(OTHER), &obj).unwrap_err();
    assert!(matches!(err, WorldStateError::ObjectPermissionDenied));
    assert!(tx.valid(&obj).unwrap());

    tx.recycle_object(&permissions(OWNER), &obj).unwrap();
    assert!(!tx.valid(&obj).unwrap());
}

#[test]
fn renumber_object_requires_wizard_not_control_of_system_object() {
    let db = test_db();
    let mut tx = db.new_world_state().unwrap();
    let obj = tx
        .create_object(
            &permissions(WIZARD),
            &NOTHING,
            &OWNER,
            BitEnum::new(),
            ObjectKind::NextObjid,
        )
        .unwrap();

    let err = tx
        .renumber_object(
            &permissions(OWNER),
            &obj,
            Some(ObjectKind::Objid(Obj::mk_id(100))),
        )
        .unwrap_err();
    assert!(matches!(err, WorldStateError::ObjectPermissionDenied));
    assert!(tx.valid(&obj).unwrap());

    let new_obj = tx
        .renumber_object(
            &permissions(WIZARD),
            &obj,
            Some(ObjectKind::Objid(Obj::mk_id(100))),
        )
        .unwrap();
    assert_eq!(new_obj, Obj::mk_id(100));
}

fn property_snapshot_fixture() -> (TxDB, Obj, uuid::Uuid) {
    use moor_common::model::{HasUuid, PropFlag};
    use moor_var::{Symbol, v_str};

    let db = test_db();
    let mut tx = db.new_world_state().unwrap();
    let child = tx
        .create_object(
            &permissions(WIZARD),
            &SYSTEM_OBJECT,
            &OWNER,
            BitEnum::new(),
            ObjectKind::NextObjid,
        )
        .unwrap();
    tx.define_property(
        &permissions(WIZARD),
        &SYSTEM_OBJECT,
        &SYSTEM_OBJECT,
        Symbol::mk("title"),
        &WIZARD,
        BitEnum::new_with(PropFlag::Chown),
        Some(v_str("Hello")),
    )
    .unwrap();
    tx.set_property_metadata(
        &permissions(WIZARD),
        &SYSTEM_OBJECT,
        Symbol::mk("title"),
        Symbol::mk("source"),
        v_str("parent"),
    )
    .unwrap();
    let id = tx
        .get_property_info(&permissions(WIZARD), &child, Symbol::mk("title"))
        .unwrap()
        .0
        .uuid();
    tx.commit().unwrap();
    (db, child, id)
}

#[test]
fn property_snapshot_preserves_local_rows_and_holder_metadata() {
    use moor_var::{Symbol, v_str};

    let (db, child, id) = property_snapshot_fixture();
    let mut tx = db.new_world_state().unwrap();
    let before = tx
        .snapshot_property(&permissions(OWNER), &child, id)
        .unwrap();
    assert!(before.value.is_none());
    assert!(before.permissions.is_none());
    assert!(before.metadata.is_empty());
    tx.update_property(
        &permissions(OWNER),
        &child,
        Symbol::mk("title"),
        &v_str("Hello"),
    )
    .unwrap();
    tx.set_property_metadata(
        &permissions(OWNER),
        &child,
        Symbol::mk("title"),
        Symbol::mk("source"),
        v_str("child"),
    )
    .unwrap();
    let explicit = tx
        .snapshot_property(&permissions(OWNER), &child, id)
        .unwrap();
    assert_eq!(explicit.value.unwrap().as_string(), Some("Hello"));
    assert_eq!(explicit.permissions.unwrap().owner(), OWNER);
    assert_eq!(explicit.metadata, [(Symbol::mk("source"), v_str("child"))]);
    tx.clear_property(&permissions(OWNER), &child, Symbol::mk("title"))
        .unwrap();
    let clear = tx
        .snapshot_property(&permissions(OWNER), &child, id)
        .unwrap();
    assert!(clear.value.is_none());
    assert!(clear.permissions.is_some());
    assert_eq!(
        tx.retrieve_property(&permissions(OWNER), &child, Symbol::mk("title"))
            .unwrap()
            .as_string(),
        Some("Hello")
    );
}

#[test]
fn property_snapshot_checks_effective_permissions_without_materializing_them() {
    use moor_common::model::CommitResult;

    let (db, child, id) = property_snapshot_fixture();
    let tx = db.new_world_state().unwrap();
    assert!(matches!(
        tx.snapshot_property(&permissions(OTHER), &child, id),
        Err(WorldStateError::PropertyPermissionDenied)
    ));
    let snapshot = tx
        .snapshot_property(&permissions(OWNER), &child, id)
        .unwrap();
    assert!(snapshot.value.is_none());
    assert!(snapshot.permissions.is_none());
    assert!(matches!(
        tx.commit().unwrap(),
        CommitResult::Success {
            mutations_made: false,
            ..
        }
    ));
}

#[test]
fn property_snapshot_rejects_uuids_outside_current_ancestry() {
    use moor_var::Symbol;

    let (db, child, id) = property_snapshot_fixture();
    let mut tx = db.new_world_state().unwrap();
    tx.update_property(
        &permissions(OWNER),
        &child,
        Symbol::mk("title"),
        &moor_var::v_str("Local"),
    )
    .unwrap();
    tx.change_parent(&permissions(WIZARD), &child, &NOTHING)
        .unwrap();
    assert!(matches!(
        tx.snapshot_property(&permissions(WIZARD), &child, id),
        Err(WorldStateError::PropertyNotFound(_, _))
    ));
    tx.change_parent(&permissions(WIZARD), &child, &SYSTEM_OBJECT)
        .unwrap();
    tx.snapshot_property(&permissions(WIZARD), &child, id)
        .unwrap();
    tx.delete_property(&permissions(WIZARD), &SYSTEM_OBJECT, Symbol::mk("title"))
        .unwrap();
    assert!(matches!(
        tx.snapshot_property(&permissions(WIZARD), &child, id),
        Err(WorldStateError::PropertyNotFound(_, _))
    ));
}

#[test]
fn property_snapshot_observes_only_its_transaction() {
    use moor_var::{Symbol, v_str};

    let (db, child, id) = property_snapshot_fixture();
    let reader = db.new_world_state().unwrap();
    let mut writer = db.new_world_state().unwrap();
    writer
        .update_property(
            &permissions(OWNER),
            &child,
            Symbol::mk("title"),
            &v_str("local"),
        )
        .unwrap();
    assert_eq!(
        writer
            .snapshot_property(&permissions(OWNER), &child, id)
            .unwrap()
            .value
            .unwrap()
            .as_string(),
        Some("local")
    );
    assert!(
        reader
            .snapshot_property(&permissions(OWNER), &child, id)
            .unwrap()
            .value
            .is_none()
    );
    writer.commit().unwrap();
    assert!(
        reader
            .snapshot_property(&permissions(OWNER), &child, id)
            .unwrap()
            .value
            .is_none()
    );
    let fresh = db.new_world_state().unwrap();
    assert_eq!(
        fresh
            .snapshot_property(&permissions(OWNER), &child, id)
            .unwrap()
            .value
            .unwrap()
            .as_string(),
        Some("local")
    );
}
