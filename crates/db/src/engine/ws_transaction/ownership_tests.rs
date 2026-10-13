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
use crate::{DatabaseConfig, engine::moor_db::MoorDB};

fn create(tx: &mut WorldStateTransaction, owner: Obj, parent: Obj) -> Obj {
    tx.create_object(
        ObjectKind::NextObjid,
        ObjAttrs::new(owner, parent, NOTHING, BitEnum::new(), "ownership test"),
    )
    .unwrap()
}

fn success(result: CommitResult) {
    assert!(matches!(result, CommitResult::Success { .. }), "{result:?}");
}

#[test]
fn ownership_transfer_preserves_permissions_values_and_clear_state() {
    for warm in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let db = MoorDB::try_open(Some(dir.path()), DatabaseConfig::default())
            .unwrap()
            .0;
        let mut tx = db.start_transaction();
        let parent = create(&mut tx, NOTHING, NOTHING);
        let old_owner = create(&mut tx, NOTHING, NOTHING);
        let new_owner = create(&mut tx, NOTHING, NOTHING);
        let child = create(&mut tx, old_owner, parent);
        let grandchild = create(&mut tx, new_owner, child);
        let flags = BitEnum::new_with(PropFlag::Chown) | PropFlag::Read | PropFlag::Clobber;
        let mut properties = Vec::new();
        for name in [
            "untouched",
            "written",
            "cleared",
            "override",
            "non_c",
            "direct",
        ] {
            let definer = if name == "direct" { child } else { parent };
            let perms = if name == "non_c" {
                BitEnum::new_with(PropFlag::Read)
            } else {
                flags
            };
            let uuid = tx
                .define_property(
                    &definer,
                    &definer,
                    Symbol::mk(name),
                    &parent,
                    perms,
                    Some(moor_var::v_int(1)),
                )
                .unwrap();
            if matches!(name, "written" | "cleared") {
                tx.set_property(&child, uuid, moor_var::v_int(2)).unwrap();
            }
            if name == "cleared" {
                tx.clear_property(&child, uuid).unwrap();
            }
            if name == "override" {
                tx.update_property_info(&child, uuid, Some(parent), None, None)
                    .unwrap();
            }
            let before = tx.resolve_property_by_uuid(&child, uuid).unwrap();
            let grandchild_perms = tx.retrieve_property_permissions(&grandchild, uuid).unwrap();
            properties.push((name, uuid, before, grandchild_perms));
        }
        success(tx.commit().unwrap());

        let mut tx = db.start_transaction();
        if warm {
            for (_, uuid, _, _) in &properties {
                tx.retrieve_property_permissions(&child, *uuid).unwrap();
            }
        }
        for owner in [new_owner, old_owner, new_owner] {
            tx.set_object_owner(&child, &owner).unwrap();
            for (name, uuid, before, grandchild_perms) in &properties {
                assert_eq!(
                    &tx.resolve_property_by_uuid(&child, *uuid).unwrap(),
                    before,
                    "{name}"
                );
                assert_eq!(
                    &tx.retrieve_property_permissions(&grandchild, *uuid)
                        .unwrap(),
                    grandchild_perms
                );
                if *name != "direct" {
                    assert_eq!(
                        tx.retrieve_property_permissions(&parent, *uuid)
                            .unwrap()
                            .owner(),
                        parent
                    );
                }
            }
        }
        success(tx.commit().unwrap());
        db.wait_for_persistence().unwrap();
        db.stop().unwrap();
        drop(db);

        let db = MoorDB::try_open(Some(dir.path()), DatabaseConfig::default())
            .unwrap()
            .0;
        let mut tx = db.start_transaction();
        assert_eq!(tx.get_object_owner(&child).unwrap(), new_owner);
        for (_, uuid, before, _) in &properties {
            assert_eq!(&tx.resolve_property_by_uuid(&child, *uuid).unwrap(), before);
        }
        // A core ownership helper can still explicitly transfer these properties.
        for (_, uuid, _, _) in &properties {
            tx.update_property_info(&child, *uuid, Some(new_owner), None, None)
                .unwrap();
            assert_eq!(
                tx.retrieve_property_permissions(&child, *uuid)
                    .unwrap()
                    .owner(),
                new_owner
            );
        }
        success(tx.commit().unwrap());
    }
}

#[test]
fn ownership_no_op_stays_lazy_and_new_properties_use_the_new_owner() {
    let db = MoorDB::try_open(None, DatabaseConfig::default()).unwrap().0;
    let mut tx = db.start_transaction();
    let parent = create(&mut tx, NOTHING, NOTHING);
    let child = create(&mut tx, NOTHING, parent);
    let uuid = tx
        .define_property(
            &parent,
            &parent,
            Symbol::mk("before"),
            &parent,
            BitEnum::new_with(PropFlag::Chown),
            Some(moor_var::v_int(1)),
        )
        .unwrap();
    success(tx.commit().unwrap());
    let mut tx = db.start_transaction();
    tx.set_object_owner(&child, &child).unwrap();
    assert!(!tx.has_mutations());
    assert!(
        !tx.object_propflags
            .has_domain(&ObjAndUUIDHolder::new(&child, uuid))
            .unwrap()
    );
    success(tx.commit().unwrap());

    let mut tx = db.start_transaction();
    tx.set_object_owner(&child, &parent).unwrap();
    let later = tx
        .define_property(
            &parent,
            &parent,
            Symbol::mk("after"),
            &parent,
            BitEnum::new_with(PropFlag::Chown),
            Some(moor_var::v_int(1)),
        )
        .unwrap();
    assert_eq!(
        tx.retrieve_property_permissions(&child, uuid)
            .unwrap()
            .owner(),
        child
    );
    assert_eq!(
        tx.retrieve_property_permissions(&child, later)
            .unwrap()
            .owner(),
        parent
    );
    success(tx.commit().unwrap());
}

#[test]
fn ownership_transfer_retries_if_its_property_scan_changes() {
    for change in [
        "add", "enable_c", "flags", "reparent", "delete", "override", "write", "recycle",
    ] {
        let db = MoorDB::try_open(None, DatabaseConfig::default()).unwrap().0;
        let mut tx = db.start_transaction();
        let parent = create(&mut tx, NOTHING, NOTHING);
        let child = create(&mut tx, NOTHING, parent);
        let other = create(&mut tx, NOTHING, NOTHING);
        let flags = if change == "enable_c" {
            BitEnum::new()
        } else {
            BitEnum::new_with(PropFlag::Chown)
        };
        let uuid = if change == "add" {
            None
        } else {
            Some(
                tx.define_property(
                    &parent,
                    &parent,
                    Symbol::mk("value"),
                    &parent,
                    flags,
                    Some(moor_var::v_int(1)),
                )
                .unwrap(),
            )
        };
        success(tx.commit().unwrap());
        let mut transfer = db.start_transaction();
        transfer.set_object_owner(&child, &other).unwrap();
        let mut edit = db.start_transaction();
        match change {
            "add" => {
                edit.define_property(
                    &parent,
                    &parent,
                    Symbol::mk("new"),
                    &parent,
                    BitEnum::new_with(PropFlag::Chown),
                    Some(moor_var::v_int(1)),
                )
                .unwrap();
            }
            "enable_c" | "flags" => edit
                .update_property_info(
                    &parent,
                    uuid.unwrap(),
                    None,
                    Some(BitEnum::new_with(PropFlag::Chown) | PropFlag::Read),
                    None,
                )
                .unwrap(),
            "reparent" => edit.set_object_parent(&child, &other).unwrap(),
            "delete" => edit.delete_property(&parent, uuid.unwrap()).unwrap(),
            "override" => edit
                .update_property_info(&child, uuid.unwrap(), Some(parent), None, None)
                .unwrap(),
            "write" => edit
                .set_property(&child, uuid.unwrap(), moor_var::v_int(2))
                .unwrap(),
            "recycle" => edit.recycle_object(&parent).unwrap(),
            _ => unreachable!(),
        }
        success(edit.commit().unwrap());
        assert!(
            matches!(
                transfer.commit().unwrap(),
                CommitResult::ConflictRetry { .. }
            ),
            "{change}"
        );
        let mut retry = db.start_transaction();
        retry.set_object_owner(&child, &other).unwrap();
        success(retry.commit().unwrap());
    }
}

#[test]
fn ownership_transfer_conflicts_with_a_first_local_write_in_either_order() {
    for transfer_first in [false, true] {
        let db = MoorDB::try_open(None, DatabaseConfig::default()).unwrap().0;
        let mut tx = db.start_transaction();
        let parent = create(&mut tx, NOTHING, NOTHING);
        let child = create(&mut tx, NOTHING, parent);
        let uuid = tx
            .define_property(
                &parent,
                &parent,
                Symbol::mk("value"),
                &parent,
                BitEnum::new_with(PropFlag::Chown) | PropFlag::Clobber,
                Some(moor_var::v_int(1)),
            )
            .unwrap();
        success(tx.commit().unwrap());
        let mut transfer = db.start_transaction();
        let mut write = db.start_transaction();
        transfer.set_object_owner(&child, &parent).unwrap();
        write
            .set_property(&child, uuid, moor_var::v_int(2))
            .unwrap();
        let (first, second) = if transfer_first {
            (transfer, write)
        } else {
            (write, transfer)
        };
        success(first.commit().unwrap());
        assert!(matches!(
            second.commit().unwrap(),
            CommitResult::ConflictRetry { .. }
        ));
        assert_eq!(
            db.start_transaction()
                .retrieve_property_permissions(&child, uuid)
                .unwrap()
                .owner(),
            child
        );
    }
}
