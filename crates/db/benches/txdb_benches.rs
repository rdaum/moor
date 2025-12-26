// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

#![recursion_limit = "256"]

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use moor_common::{
    model::{CommitResult, ObjectKind, PropFlag, WorldStateSource},
    util::BitEnum,
};
use moor_db::{DatabaseConfig, TxDB};
use moor_var::{NOTHING, SYSTEM_OBJECT, Symbol, v_int, v_list_iter};
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
            BitEnum::all(),
            ObjectKind::NextObjid,
        )
        .unwrap();
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    ws_source
}

fn setup_properties(db: &TxDB, count: usize) -> Vec<Symbol> {
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

fn run_concurrent_workload(
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
                    let mut tx = db.new_world_state().unwrap();
                    for op in 0..ops_per_tx {
                        let prop_name =
                            prop_names[rng.random_range(0..prop_names.len())];
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
                            let _ = tx.retrieve_property(
                                &SYSTEM_OBJECT,
                                &SYSTEM_OBJECT,
                                prop_name,
                            );
                        }
                    }
                    let _ = tx.commit();
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

fn commit_concurrency(c: &mut Criterion) {
    let db = create_db();
    let prop_names = setup_properties(&db, 128);

    let mut group = c.benchmark_group("commit_concurrency");
    group.sample_size(10);

    let ops_per_tx = 64usize;
    let write_percents = [10u32, 50u32, 90u32];
    let max_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let concurrency_levels = [1usize, 2, 4, 8, 16];

    for &concurrency in concurrency_levels.iter() {
        if concurrency > max_threads {
            continue;
        }
        for &write_percent in write_percents.iter() {
            // Each criterion iteration = concurrency threads each doing 1 transaction
            // (criterion's `iters` parameter scales the transaction count per thread)
            group.throughput(Throughput::Elements(concurrency as u64));
            let label = format!("threads={concurrency}/write={write_percent}%");
            group.bench_function(label, |b| {
                b.iter_custom(|iters| {
                    run_concurrent_workload(
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
criterion_group!(benches, commit_concurrency);
criterion_main!(benches);
