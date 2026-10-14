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

//! Foreground publication and applied-drain baseline. Run with THREADS COMMITS_PER_THREAD.
//! Each worker writes its own property; shared roots still exercise CAS publication/rebase.

use moor_common::{
    model::{
        CommitResult, ObjFlag, ObjectKind, PropFlag, TaskPermissions, WorldStateCountOp,
        WorldStateSource,
    },
    threading::{spawn_worker_perf, worker_performance_core_ids},
    util::BitEnum,
};
use moor_db::{Database, DatabaseConfig, TxDB, db_counters};
use moor_var::{NOTHING, SYSTEM_OBJECT, Symbol, v_int};
use std::{
    sync::{Arc, Barrier},
    time::Instant,
};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let threads: usize = args.get(1).map_or(4, |s| s.parse().unwrap());
    let commits: usize = args.get(2).map_or(10_000, |s| s.parse().unwrap());
    assert!(threads > 0 && commits > 0);
    println!(
        "worker_performance_cores={:?}",
        worker_performance_core_ids()
    );
    let (db, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
    let db = Arc::new(db);
    let perms = TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new());
    let mut seed = db.new_world_state().unwrap();
    seed.create_object(
        &perms,
        &NOTHING,
        &SYSTEM_OBJECT,
        ObjFlag::all_flags(),
        ObjectKind::NextObjid,
    )
    .unwrap();
    for worker in 0..threads {
        seed.define_property(
            &perms,
            &SYSTEM_OBJECT,
            &SYSTEM_OBJECT,
            Symbol::mk(&format!("worker_{worker}")),
            &SYSTEM_OBJECT,
            PropFlag::rw(),
            Some(v_int(0)),
        )
        .unwrap();
    }
    assert!(matches!(
        seed.commit().unwrap(),
        CommitResult::Success { .. }
    ));
    db.wait_for_persistence().unwrap();
    let barrier = Arc::new(Barrier::new(threads + 1));
    let mut workers = Vec::new();
    for worker in 0..threads {
        let db = db.clone();
        let barrier = barrier.clone();
        workers.push(
            spawn_worker_perf(format!("persistence-worker-{worker}"), move || {
                let perms = TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new());
                let name = Symbol::mk(&format!("worker_{worker}"));
                let mut samples = Vec::with_capacity(commits);
                let mut retries = 0;
                for index in 0..(commits + 1000) {
                    if index == 1000 {
                        barrier.wait();
                        barrier.wait();
                    }
                    let started = Instant::now();
                    loop {
                        let mut tx = db.new_world_state().unwrap();
                        let _ = tx.retrieve_property(&perms, &SYSTEM_OBJECT, name).unwrap();
                        tx.update_property(&perms, &SYSTEM_OBJECT, name, &v_int(index as i64))
                            .unwrap();
                        match tx.commit().unwrap() {
                            CommitResult::Success { .. } => break,
                            CommitResult::ConflictRetry { .. } => retries += 1,
                        }
                    }
                    if index >= 1000 {
                        samples.push(started.elapsed().as_nanos() as u64);
                    }
                }
                (samples, retries)
            })
            .unwrap(),
        );
    }
    // Workers finish warmup before the measurement starts.
    barrier.wait();
    db.wait_for_persistence().unwrap();
    let admission_waits_before = db_counters()
        .counters
        .get(WorldStateCountOp::BatchWriterBackpressure);
    let started = Instant::now();
    barrier.wait();
    let mut samples = Vec::with_capacity(threads * commits);
    let mut retries = 0;
    for worker in workers {
        let (mut values, conflicts) = worker.join().unwrap();
        samples.append(&mut values);
        retries += conflicts;
    }
    let producer = started.elapsed();
    let drain = Instant::now();
    db.wait_for_persistence().unwrap();
    let applied = drain.elapsed();
    let admission_waits = db_counters()
        .counters
        .get(WorldStateCountOp::BatchWriterBackpressure)
        - admission_waits_before;
    samples.sort_unstable();
    let percentile = |p: usize| samples[(samples.len() - 1) * p / 100];
    println!(
        "threads={threads} commits={} producer_seconds={} applied_drain_seconds={} commits_per_second={} p50_ns={} p95_ns={} p99_ns={} retries={retries} admission_waits={admission_waits}",
        samples.len(),
        producer.as_secs_f64(),
        applied.as_secs_f64(),
        samples.len() as f64 / producer.as_secs_f64(),
        percentile(50),
        percentile(95),
        percentile(99)
    );
    // Verify the applied workload through an independent snapshot and the resident transaction.
    let snapshot = db.create_snapshot().unwrap();
    let properties = snapshot.get_property_snapshots(&SYSTEM_OBJECT).unwrap();
    assert_eq!(properties.len(), threads);
    for property in properties {
        assert_eq!(property.value, Some(v_int((commits + 999) as i64)));
    }
    let tx = db.new_world_state().unwrap();
    for worker in 0..threads {
        assert_eq!(
            tx.retrieve_property(
                &perms,
                &SYSTEM_OBJECT,
                Symbol::mk(&format!("worker_{worker}"))
            )
            .unwrap(),
            v_int((commits + 999) as i64)
        );
    }
    drop(tx);
}
