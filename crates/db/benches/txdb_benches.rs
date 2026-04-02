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

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use moor_common::model::{CommitResult, ObjFlag, ObjectKind, PropFlag, WorldStateSource};
use moor_db::{DatabaseConfig, TxDB};
use moor_var::{NOTHING, Obj, SYSTEM_OBJECT, Symbol, v_int, v_list_iter};
use rand::Rng;
use std::sync::{Arc, Barrier};
use std::time::Duration;

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

/// Create `num_objects` child objects of SYSTEM_OBJECT, each with `props_per_object` properties.
/// Returns (objects, property_names).
fn setup_multi_object(
    db: &TxDB,
    num_objects: usize,
    props_per_object: usize,
) -> (Vec<Obj>, Vec<Symbol>) {
    let initial_value = v_list_iter((0..10).map(v_int));
    let mut tx = db.new_world_state().unwrap();

    // Define properties on SYSTEM_OBJECT (inherited by children)
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

    // Create child objects
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

/// Single-object workload: all threads read/write properties on SYSTEM_OBJECT.
/// This is the worst case for CAS — maximum key-level contention.
fn run_single_object_workload(
    db: &TxDB,
    prop_names: &[Symbol],
    concurrency: usize,
    ops_per_tx: usize,
    write_percent: u32,
    iters: u64,
) -> Duration {
    let ready = Arc::new(Barrier::new(concurrency + 1));
    let start = Arc::new(Barrier::new(concurrency + 1));
    let start_time = std::thread::scope(|scope| {
        for thread_id in 0..concurrency {
            let db = db.clone();
            let ready = Arc::clone(&ready);
            let start = Arc::clone(&start);
            scope.spawn(move || {
                let mut rng = rand::rng();
                ready.wait();
                start.wait();
                for iter in 0..iters {
                    loop {
                        let mut tx = db.new_world_state().unwrap();
                        for op in 0..ops_per_tx {
                            let prop_name = prop_names[rng.random_range(0..prop_names.len())];
                            if rng.random_range(0..100) < write_percent {
                                let value = v_int(
                                    (thread_id as i64) * 1_000_000
                                        + (iter as i64) * ops_per_tx as i64
                                        + op as i64,
                                );
                                let _ = tx.update_property(
                                    &SYSTEM_OBJECT,
                                    &SYSTEM_OBJECT,
                                    prop_name,
                                    &value,
                                );
                            } else {
                                let _ =
                                    tx.retrieve_property(&SYSTEM_OBJECT, &SYSTEM_OBJECT, prop_name);
                            }
                        }
                        match tx.commit() {
                            Ok(CommitResult::Success { .. }) => break,
                            _ => continue,
                        }
                    }
                }
            });
        }
        ready.wait();
        let start_time = std::time::Instant::now();
        start.wait();
        start_time
    });
    start_time.elapsed()
}

/// Multi-object workload: each thread operates primarily on its own object.
/// This represents typical MOO workloads where different tasks touch different
/// objects, with occasional cross-object reads.
fn run_multi_object_workload(
    db: &TxDB,
    objects: &[Obj],
    prop_names: &[Symbol],
    concurrency: usize,
    ops_per_tx: usize,
    write_percent: u32,
    iters: u64,
) -> Duration {
    let ready = Arc::new(Barrier::new(concurrency + 1));
    let start = Arc::new(Barrier::new(concurrency + 1));
    let start_time = std::thread::scope(|scope| {
        for thread_id in 0..concurrency {
            let db = db.clone();
            let ready = Arc::clone(&ready);
            let start = Arc::clone(&start);
            // Each thread has a "home" object it primarily writes to
            let home_obj = objects[thread_id % objects.len()];
            scope.spawn(move || {
                let mut rng = rand::rng();
                ready.wait();
                start.wait();
                for iter in 0..iters {
                    loop {
                        let mut tx = db.new_world_state().unwrap();
                        for op in 0..ops_per_tx {
                            let prop_name = prop_names[rng.random_range(0..prop_names.len())];
                            if rng.random_range(0..100) < write_percent {
                                // Writes go to this thread's home object
                                let value = v_int(
                                    (thread_id as i64) * 1_000_000
                                        + (iter as i64) * ops_per_tx as i64
                                        + op as i64,
                                );
                                let _ = tx.update_property(
                                    &home_obj,
                                    &SYSTEM_OBJECT,
                                    prop_name,
                                    &value,
                                );
                            } else {
                                // Reads can go to any object
                                let read_obj = objects[rng.random_range(0..objects.len())];
                                let _ = tx.retrieve_property(&read_obj, &SYSTEM_OBJECT, prop_name);
                            }
                        }
                        match tx.commit() {
                            Ok(CommitResult::Success { .. }) => break,
                            _ => continue,
                        }
                    }
                }
            });
        }
        ready.wait();
        let start_time = std::time::Instant::now();
        start.wait();
        start_time
    });
    start_time.elapsed()
}

/// Worst case: all threads contend on the same object's properties.
fn commit_single_object(c: &mut Criterion) {
    let db = create_db();
    let prop_names = setup_single_object_properties(&db, 128);

    let mut group = c.benchmark_group("commit_single_object");
    group.sample_size(10);

    let ops_per_tx = 64usize;
    let write_percents = [10u32, 25u32, 50u32, 90u32];
    let max_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let concurrency_levels = [1usize, 2, 4, 8];

    for &concurrency in &concurrency_levels {
        if concurrency > max_threads {
            continue;
        }
        for &write_percent in &write_percents {
            group.throughput(Throughput::Elements(concurrency as u64));
            let label = format!("threads={concurrency}/write={write_percent}%");
            group.bench_function(label, |b| {
                b.iter_custom(|iters| {
                    run_single_object_workload(
                        &db,
                        &prop_names,
                        concurrency,
                        ops_per_tx,
                        write_percent,
                        iters,
                    )
                })
            });
        }
    }
    group.finish();
}

/// Distributed workload: each thread writes to its own object.
/// This is where CAS shines — no key overlap between threads.
fn commit_multi_object(c: &mut Criterion) {
    let db = create_db();
    let max_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(16);

    // Create one object per potential thread
    let (objects, prop_names) = setup_multi_object(&db, max_threads, 32);

    let mut group = c.benchmark_group("commit_multi_object");
    group.sample_size(10);

    let ops_per_tx = 16usize;
    let write_percents = [25u32, 50u32, 90u32];
    let concurrency_levels = [1usize, 2, 4, 8];

    for &concurrency in &concurrency_levels {
        if concurrency > max_threads {
            continue;
        }
        for &write_percent in &write_percents {
            group.throughput(Throughput::Elements(concurrency as u64));
            let label = format!("threads={concurrency}/write={write_percent}%");
            group.bench_function(label, |b| {
                b.iter_custom(|iters| {
                    run_multi_object_workload(
                        &db,
                        &objects,
                        &prop_names,
                        concurrency,
                        ops_per_tx,
                        write_percent,
                        iters,
                    )
                })
            });
        }
    }
    group.finish();
}

criterion_group!(benches, commit_single_object, commit_multi_object);
criterion_main!(benches);
