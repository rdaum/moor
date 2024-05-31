// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! Single-threaded benchmark of pushing through a (jepsen-produced) append-only workload.
//! Does not measure single-item reads, deletes, or updates, or concurrent access.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use moor_values::util::SliceRef;
use relbox::index::{AttrType, IndexType};
use relbox::{RelBox, RelationInfo};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

// This is a struct that tells Criterion.rs to use the "futures" crate's current-thread executor
use crate::support::{History, Type, Value};
use moor_values::util::{BitArray, Bitset64};
use relbox::RelationId;

#[path = "../tests/test-support.rs"]
mod support;

/// Build a test database with a bunch of relations
fn test_db() -> Arc<RelBox> {
    // Generate the test relations that we'll use for testing.
    let relations = (0..63)
        .map(|i| RelationInfo {
            name: format!("relation_{}", i),
            domain_type: AttrType::Integer,
            codomain_type: AttrType::Integer,
            secondary_indexed: false,
            unique_domain: true,
            index_type: IndexType::AdaptiveRadixTree,
            codomain_index_type: None,
        })
        .collect::<Vec<_>>();

    RelBox::new(1 << 24, None, &relations, 0)
}

fn from_val(value: i64) -> SliceRef {
    SliceRef::from_bytes(&value.to_le_bytes()[..])
}

fn load_history() -> Vec<History> {
    let lines = include_str!("list-append-dataset.json")
        .lines()
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>();
    let events = lines
        .iter()
        .map(|l| serde_json::from_str::<History>(l).unwrap());
    events.collect::<Vec<_>>()
}

fn list_append_scan_workload(iters: u64, events: &Vec<History>) -> Duration {
    let mut cumulative = Duration::new(0, 0);
    for _ in 0..iters {
        // We create a brand new db for each iteration, so we have a clean slate.
        let db = test_db();

        // Where to track the transactions running.
        let mut processes: BitArray<_, 256, Bitset64<8>> = BitArray::new();

        let start = Instant::now();

        for e in events {
            match e.r#type {
                Type::invoke => {
                    // Start a transaction.
                    let tx = Rc::new(db.clone().start_tx());
                    assert!(
                        !processes.check(e.process as usize),
                        "T{} already exists committed",
                        e.process
                    );
                    processes.set(e.process as usize, tx.clone());
                    // Execute the actions
                    for ev in &e.value {
                        match ev {
                            Value::append(_, register, value) => {
                                // Insert the value into the relation.
                                let relation = RelationId(*register as usize);
                                tx.clone()
                                    .relation(relation)
                                    .insert_tuple(from_val(*value), from_val(*value))
                                    .unwrap();
                            }
                            Value::r(_, register, _) => {
                                let relation = RelationId(*register as usize);

                                // Full-scan.
                                tx.relation(relation).predicate_scan(&|_| true).unwrap();
                            }
                        }
                    }
                }
                Type::ok => {
                    let tx = processes.erase(e.process as usize).unwrap();
                    tx.commit().unwrap();
                }
                Type::fail => {
                    let tx = processes.erase(e.process as usize).unwrap();
                    tx.rollback().unwrap();
                }
            }
        }
        black_box(());
        cumulative += start.elapsed();
    }
    cumulative
}

/// Same as above, but instead of predicate scan, does an individual tuple lookup, to measure that.
fn list_append_seek_workload(iters: u64, events: &Vec<History>) -> Duration {
    let mut cumulative = Duration::new(0, 0);
    for _ in 0..iters {
        // We create a brand new db for each iteration, so we have a clean slate.
        let db = test_db();

        // Where to track the transactions running.
        let mut processes: BitArray<_, 256, Bitset64<8>> = BitArray::new();
        let start = Instant::now();

        for e in events {
            match e.r#type {
                Type::invoke => {
                    // Start a transaction.
                    let tx = Rc::new(db.clone().start_tx());
                    assert!(
                        !processes.check(e.process as usize),
                        "T{} already exists committed",
                        e.process
                    );
                    processes.set(e.process as usize, tx.clone());
                    // Execute the actions
                    for ev in &e.value {
                        match ev {
                            Value::append(_, register, value) => {
                                // Insert the value into the relation.
                                let relation = RelationId(*register as usize);
                                tx.clone()
                                    .relation(relation)
                                    .insert_tuple(from_val(*value), from_val(*value))
                                    .unwrap();
                            }
                            Value::r(_, register, Some(tuples)) => {
                                let relation = RelationId(*register as usize);

                                for t in tuples {
                                    tx.relation(relation)
                                        .seek_unique_by_domain(from_val(*t))
                                        .unwrap();
                                }
                            }
                            Value::r(_, _, None) => {
                                continue;
                            }
                        }
                    }
                }
                Type::ok => {
                    let tx = processes.erase(e.process as usize).unwrap();
                    tx.commit().unwrap();
                }
                Type::fail => {
                    let tx = processes.erase(e.process as usize).unwrap();
                    tx.rollback().unwrap();
                }
            }
        }
        black_box(());
        cumulative += start.elapsed();
    }
    cumulative
}

// Measure the # of commit/rollbacks per second using the list-append Jepsen workload.
pub fn throughput_bench(c: &mut Criterion) {
    let events = load_history();

    // Count the # of commit/rollback (unique transactions) in the workload.
    let tx_count = events.iter().filter(|e| e.r#type == Type::invoke).count();

    let mut group = c.benchmark_group("throughput");
    group.sample_size(1000);
    group.measurement_time(Duration::from_secs(10));
    group.throughput(criterion::Throughput::Elements(tx_count as u64));
    group.bench_function("list_append_scan", |b| {
        b.iter_custom(|iters| list_append_scan_workload(iters, &events));
    });
    group.bench_function("list_append_seek", |b| {
        b.iter_custom(|iters| list_append_seek_workload(iters, &events));
    });
    group.finish();
}

criterion_group!(benches, throughput_bench);
criterion_main!(benches);
