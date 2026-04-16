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

#![recursion_limit = "256"]

use micromeasure::{
    BenchmarkMainOptions, ConcurrentBenchContext, ConcurrentBenchControl, ConcurrentWorker,
    ConcurrentWorkerResult, benchmark_main,
};
use moor_common::model::{CommitResult, ObjFlag, ObjectKind, PropFlag, WorldStateSource};
use moor_db::{DatabaseConfig, TxDB};
use moor_var::{NOTHING, Obj, SYSTEM_OBJECT, Symbol, v_int, v_list_iter};
use rand::Rng;
use std::time::Duration;

struct TxDbConcurrentContext {
    db: TxDB,
    prop_names: Vec<Symbol>,
    objects: Vec<Obj>,
    ops_per_tx: usize,
    write_percent: u32,
}

impl ConcurrentBenchContext for TxDbConcurrentContext {
    fn prepare(num_threads: usize) -> Self {
        let db = create_db();
        let prop_names = setup_single_object_properties(&db, 128);
        let objects = (0..num_threads.max(1)).map(|_| SYSTEM_OBJECT).collect();
        Self {
            db,
            prop_names,
            objects,
            ops_per_tx: 64,
            write_percent: 50,
        }
    }
}

fn create_db() -> TxDB {
    let (ws_source, _) = TxDB::open(None, DatabaseConfig::default());
    let mut tx = ws_source.new_world_state().unwrap();
    let _sysobj = tx
        .create_object(
            &SYSTEM_OBJECT,
            &NOTHING,
            &SYSTEM_OBJECT,
            ObjFlag::all_flags(),
            ObjectKind::NextObjid,
        )
        .unwrap();
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    ws_source
}

fn setup_multi_object(
    db: &TxDB,
    num_objects: usize,
    props_per_object: usize,
) -> (Vec<Obj>, Vec<Symbol>) {
    let initial_value = v_list_iter((0..10).map(v_int));
    let mut tx = db.new_world_state().unwrap();

    let mut prop_names = Vec::with_capacity(props_per_object);
    for i in 0..props_per_object {
        let name = Symbol::mk(&format!("bench_prop_{i}"));
        tx.define_property(
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            name,
            &SYSTEM_OBJECT,
            PropFlag::rw(),
            Some(initial_value.clone()),
        )
        .unwrap();
        prop_names.push(name);
    }
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));

    let mut objects = Vec::with_capacity(num_objects);
    let mut tx = db.new_world_state().unwrap();
    for _ in 0..num_objects {
        let obj = tx
            .create_object(
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                &SYSTEM_OBJECT,
                ObjFlag::all_flags(),
                ObjectKind::NextObjid,
            )
            .unwrap();
        objects.push(obj);
    }
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));

    (objects, prop_names)
}

fn setup_single_object_properties(db: &TxDB, count: usize) -> Vec<Symbol> {
    let append_values = v_list_iter((0..100).map(v_int));
    let mut tx = db.new_world_state().unwrap();
    let mut prop_names = Vec::with_capacity(count);
    for index in 0..count {
        let name = Symbol::mk(&format!("bench_prop_{index}"));
        tx.define_property(
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            name,
            &SYSTEM_OBJECT,
            PropFlag::rw(),
            Some(append_values.clone()),
        )
        .unwrap();
        prop_names.push(name);
    }
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    prop_names
}

fn run_single_object_worker(
    ctx: &TxDbConcurrentContext,
    control: &ConcurrentBenchControl,
) -> ConcurrentWorkerResult {
    let mut rng = rand::rng();
    let mut commits = 0_u64;
    let mut retries = 0_u64;
    let mut read_ops = 0_u64;
    let mut write_ops = 0_u64;

    while !control.should_stop() {
        let mut tx = ctx.db.new_world_state().unwrap();
        for _ in 0..ctx.ops_per_tx {
            let prop_name = ctx.prop_names[rng.random_range(0..ctx.prop_names.len())];
            if rng.random_range(0..100) < ctx.write_percent {
                let value = v_int(
                    (control.thread_index() as i64) * 1_000_000 + commits as i64 + write_ops as i64,
                );
                let _ = tx.update_property(&SYSTEM_OBJECT, &SYSTEM_OBJECT, prop_name, &value);
                write_ops = write_ops.wrapping_add(1);
            } else {
                let _ = tx.retrieve_property(&SYSTEM_OBJECT, &SYSTEM_OBJECT, prop_name);
                read_ops = read_ops.wrapping_add(1);
            }
        }

        match tx.commit() {
            Ok(CommitResult::Success { .. }) => {
                commits = commits.wrapping_add(1);
            }
            _ => {
                retries = retries.wrapping_add(1);
            }
        }
    }

    ConcurrentWorkerResult::operations(commits)
        .with_counter("retries", retries)
        .with_counter("read_ops", read_ops)
        .with_counter("write_ops", write_ops)
}

fn run_multi_object_worker(
    ctx: &TxDbConcurrentContext,
    control: &ConcurrentBenchControl,
) -> ConcurrentWorkerResult {
    let mut rng = rand::rng();
    let mut commits = 0_u64;
    let mut retries = 0_u64;
    let mut read_ops = 0_u64;
    let mut write_ops = 0_u64;
    let home_obj = ctx.objects[control.thread_index() % ctx.objects.len()];

    while !control.should_stop() {
        let mut tx = ctx.db.new_world_state().unwrap();
        for _ in 0..ctx.ops_per_tx {
            let prop_name = ctx.prop_names[rng.random_range(0..ctx.prop_names.len())];
            if rng.random_range(0..100) < ctx.write_percent {
                let value = v_int(
                    (control.thread_index() as i64) * 1_000_000 + commits as i64 + write_ops as i64,
                );
                let _ = tx.update_property(&home_obj, &SYSTEM_OBJECT, prop_name, &value);
                write_ops = write_ops.wrapping_add(1);
            } else {
                let read_obj = ctx.objects[rng.random_range(0..ctx.objects.len())];
                let _ = tx.retrieve_property(&read_obj, &SYSTEM_OBJECT, prop_name);
                read_ops = read_ops.wrapping_add(1);
            }
        }

        match tx.commit() {
            Ok(CommitResult::Success { .. }) => {
                commits = commits.wrapping_add(1);
            }
            _ => {
                retries = retries.wrapping_add(1);
            }
        }
    }

    ConcurrentWorkerResult::operations(commits)
        .with_counter("retries", retries)
        .with_counter("read_ops", read_ops)
        .with_counter("write_ops", write_ops)
}

fn make_single_object_context(num_threads: usize, ops_per_tx: usize, write_percent: u32) -> TxDbConcurrentContext {
    let db = create_db();
    let prop_names = setup_single_object_properties(&db, 128);
    TxDbConcurrentContext {
        db,
        prop_names,
        objects: vec![SYSTEM_OBJECT; num_threads.max(1)],
        ops_per_tx,
        write_percent,
    }
}

fn make_multi_object_context(num_threads: usize, ops_per_tx: usize, write_percent: u32) -> TxDbConcurrentContext {
    let db = create_db();
    let (objects, prop_names) = setup_multi_object(&db, num_threads.max(1), 32);
    TxDbConcurrentContext {
        db,
        prop_names,
        objects,
        ops_per_tx,
        write_percent,
    }
}

benchmark_main!(
    BenchmarkMainOptions {
        filter_help: Some("all, single, multi, commit, or benchmark name substring".to_string()),
        ..BenchmarkMainOptions::default()
    },
    |runner| {
    let max_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(16);

    for &threads in &[1usize, 2, 4, 8] {
        if threads > max_threads {
            continue;
        }

        let single_object_workers = [ConcurrentWorker {
            name: "tx_worker",
            threads,
            run: run_single_object_worker,
        }];
        let multi_object_workers = [ConcurrentWorker {
            name: "tx_worker",
            threads,
            run: run_multi_object_worker,
        }];

        runner.concurrent_group::<TxDbConcurrentContext>("commit_single_object", |g| {
            for &write_percent in &[10u32, 25u32, 50u32, 90u32] {
                let name = format!("threads={threads}/write={write_percent}%");
                let factory = |num_threads| make_single_object_context(num_threads, 64, write_percent);
                g.bench_with_factory(
                    &name,
                    Duration::from_millis(100),
                    &single_object_workers,
                    &factory,
                );
            }
        });

        runner.concurrent_group::<TxDbConcurrentContext>("commit_multi_object", |g| {
            for &write_percent in &[25u32, 50u32, 90u32] {
                let name = format!("threads={threads}/write={write_percent}%");
                let factory = |num_threads| make_multi_object_context(num_threads, 16, write_percent);
                g.bench_with_factory(
                    &name,
                    Duration::from_millis(100),
                    &multi_object_workers,
                    &factory,
                );
            }
        });
    }
    }
);
