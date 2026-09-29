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

use micromeasure::{BenchContext, BenchmarkMainOptions, Throughput, benchmark_main, black_box};
use moor_common::{
    model::{CommitResult, ObjFlag, ObjectKind, PropFlag, TaskPermissions, WorldStateSource},
    util::BitEnum,
};
use moor_db::{Database, DatabaseConfig, TxDB};
use moor_var::{NOTHING, SYSTEM_OBJECT, Symbol, v_int, v_list_iter, v_str};

const PROPERTIES: usize = 64;
const WIDE_DESCENDANTS: usize = 1_000;
const DEEP_DESCENDANTS: usize = 1_024;
const PROPERTY_CHAIN_APPENDS: usize = 63;
const PROPERTY_CHAIN_INITIAL_ENTRIES: usize = 1_024;
const PROPERTY_CHAIN_ENTRY_BYTES: usize = 9_472;

struct WideExportContext {
    db: TxDB,
}

struct DeepExportContext {
    db: TxDB,
}

struct PropertyChainExportContext {
    db: TxDB,
}

fn create_db() -> TxDB {
    let (db, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
    let permissions = TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new());
    let mut tx = db.new_world_state().unwrap();
    let system = tx
        .create_object(
            &permissions,
            &NOTHING,
            &SYSTEM_OBJECT,
            ObjFlag::all_flags(),
            ObjectKind::NextObjid,
        )
        .unwrap();
    assert_eq!(system, SYSTEM_OBJECT);
    for index in 0..PROPERTIES {
        tx.define_property(
            &permissions,
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            Symbol::mk(&format!("export_property_{index}")),
            &SYSTEM_OBJECT,
            PropFlag::rw(),
            Some(v_int(index as i64)),
        )
        .unwrap();
    }
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    db
}

fn create_wide_db() -> TxDB {
    let db = create_db();
    let permissions = TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new());
    let mut tx = db.new_world_state().unwrap();
    for _ in 0..WIDE_DESCENDANTS {
        tx.create_object(
            &permissions,
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            ObjFlag::all_flags(),
            ObjectKind::NextObjid,
        )
        .unwrap();
    }
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    db
}

fn create_deep_db() -> TxDB {
    let db = create_db();
    let permissions = TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new());
    let mut tx = db.new_world_state().unwrap();
    let mut parent = SYSTEM_OBJECT;
    for _ in 0..DEEP_DESCENDANTS {
        parent = tx
            .create_object(
                &permissions,
                &parent,
                &SYSTEM_OBJECT,
                ObjFlag::all_flags(),
                ObjectKind::NextObjid,
            )
            .unwrap();
    }
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    db
}

fn create_property_chain_db() -> TxDB {
    let (db, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
    let permissions = TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new());
    let property = Symbol::mk("history");
    let entry_text = "x".repeat(PROPERTY_CHAIN_ENTRY_BYTES);
    let entry = v_str(&entry_text);
    let mut tx = db.new_world_state().unwrap();
    tx.create_object(
        &permissions,
        &NOTHING,
        &SYSTEM_OBJECT,
        ObjFlag::all_flags(),
        ObjectKind::NextObjid,
    )
    .unwrap();
    tx.define_property(
        &permissions,
        &SYSTEM_OBJECT,
        &SYSTEM_OBJECT,
        property,
        &SYSTEM_OBJECT,
        PropFlag::rw(),
        Some(v_list_iter(
            (0..PROPERTY_CHAIN_INITIAL_ENTRIES).map(|_| entry.clone()),
        )),
    )
    .unwrap();
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));

    for _ in 0..PROPERTY_CHAIN_APPENDS {
        let mut tx = db.new_world_state().unwrap();
        let current = tx
            .retrieve_property(&permissions, &SYSTEM_OBJECT, property)
            .unwrap();
        let appended = current.push(&entry).unwrap();
        tx.update_property(&permissions, &SYSTEM_OBJECT, property, &appended)
            .unwrap();
        assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    }
    db.wait_for_persistence().unwrap();
    db
}

impl BenchContext for WideExportContext {
    fn prepare(_num_chunks: usize) -> Self {
        Self {
            db: create_wide_db(),
        }
    }
}

impl BenchContext for DeepExportContext {
    fn prepare(_num_chunks: usize) -> Self {
        Self {
            db: create_deep_db(),
        }
    }
}

impl BenchContext for PropertyChainExportContext {
    fn prepare(_num_chunks: usize) -> Self {
        Self {
            db: create_property_chain_db(),
        }
    }
}

fn scan_export(db: &TxDB, expected_objects: usize) {
    let snapshot = db.create_snapshot().unwrap();
    let mut export = snapshot.begin_export(&[]).unwrap();
    let mut objects = 0;
    while let Some(object) = export.next_object().unwrap() {
        black_box(object);
        objects += 1;
    }
    assert_eq!(objects, expected_objects);
}

fn scan_wide(ctx: &mut WideExportContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        scan_export(&ctx.db, WIDE_DESCENDANTS + 1);
    }
}

fn scan_deep(ctx: &mut DeepExportContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        scan_export(&ctx.db, DEEP_DESCENDANTS + 1);
    }
}

fn scan_property_chain(ctx: &mut PropertyChainExportContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        scan_export(&ctx.db, 1);
    }
}

benchmark_main!(BenchmarkMainOptions::default(), |runner| {
    runner.group::<WideExportContext>("Objdef snapshot export", |group| {
        group
            .throughput(Throughput::per_operation(
                (WIDE_DESCENDANTS + 1) as u64,
                "objects",
            ))
            .bench("wide_shared_ancestor", scan_wide);
    });
    runner.group::<DeepExportContext>("Objdef snapshot export", |group| {
        group
            .throughput(Throughput::per_operation(
                (DEEP_DESCENDANTS + 1) as u64,
                "objects",
            ))
            .bench("deep_inheritance", scan_deep);
    });
    runner.group::<PropertyChainExportContext>("Objdef snapshot export", |group| {
        group
            .throughput(Throughput::per_operation(
                (PROPERTY_CHAIN_INITIAL_ENTRIES * PROPERTY_CHAIN_ENTRY_BYTES) as u64,
                "logical bytes",
            ))
            .bench("bounded_property_append_chain", scan_property_chain);
    });
});
