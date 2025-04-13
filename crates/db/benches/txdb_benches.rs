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

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use moor_common::model::{CommitResult, PropFlag, WorldStateSource};
use moor_common::util::BitEnum;
use moor_db::{DatabaseConfig, TxDB};
use moor_var::{NOTHING, SYSTEM_OBJECT, Symbol, v_int, v_list_iter};
use rand::prelude::SliceRandom;
use std::time::Duration;

fn create_db() -> TxDB {
    let (ws_source, _) = TxDB::open(None, DatabaseConfig::default());
    let mut tx = ws_source.new_world_state().unwrap();
    let _sysobj = tx
        .create_object(&SYSTEM_OBJECT, &NOTHING, &SYSTEM_OBJECT, BitEnum::all())
        .unwrap();
    assert_eq!(tx.commit().unwrap(), CommitResult::Success);
    ws_source
}

fn commit_latency(c: &mut Criterion) {
    let db = create_db();
    let mut tx = db.new_world_state().unwrap();
    let _sysobj = tx
        .create_object(&SYSTEM_OBJECT, &NOTHING, &SYSTEM_OBJECT, BitEnum::all())
        .unwrap();
    assert_eq!(tx.commit().unwrap(), CommitResult::Success);

    let append_values = v_list_iter((0..100).map(v_int));
    let mut group = c.benchmark_group("commit_latency");
    let mut all_props = vec![];
    group.sample_size(10);

    let num_tuples = 100;
    group.throughput(Throughput::Elements(num_tuples));

    // Benchmark for write-load "commit" time.
    group.bench_function("write_commit", |b| {
        b.iter_custom(|iters| {
            let mut cumulative_time = Duration::new(0, 0);
            for _ in 0..iters {
                // start a new tx
                let mut tx = db.new_world_state().unwrap();
                for _ in 0..num_tuples {
                    let new_prop_name = uuid::Uuid::new_v4();
                    let new_prop_name = Symbol::mk(&new_prop_name.to_string());
                    all_props.push(new_prop_name);
                    tx.define_property(
                        &SYSTEM_OBJECT,
                        &SYSTEM_OBJECT,
                        &SYSTEM_OBJECT,
                        new_prop_name,
                        &SYSTEM_OBJECT,
                        PropFlag::rw(),
                        Some(append_values.clone()),
                    )
                    .ok();
                }
                let start = std::time::Instant::now();
                tx.commit().unwrap();
                cumulative_time += start.elapsed();
            }
            cumulative_time
        })
    });

    // Benchmark for read-only "commit" time.
    group.bench_function("read_commit", |b| {
        b.iter_custom(|iters| {
            let mut cumulative_time = Duration::new(0, 0);
            for _ in 0..iters {
                // pick a prop name from random out of all_props
                let prop_name = *all_props.choose(&mut rand::thread_rng()).unwrap();
                let tx = db.new_world_state().unwrap();
                for _ in 0..num_tuples {
                    let _ = tx
                        .retrieve_property(&SYSTEM_OBJECT, &SYSTEM_OBJECT, prop_name)
                        .ok();
                }
                let start = std::time::Instant::now();
                tx.commit().unwrap();
                cumulative_time += start.elapsed();
            }
            cumulative_time
        })
    });
}
criterion_group!(benches, commit_latency);
criterion_main!(benches);
