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

use micromeasure::{
    BenchContext, BenchmarkMainOptions, ConcurrentBenchContext, ConcurrentBenchControl, ConcurrentWorker,
    ConcurrentWorkerResult, black_box,
    benchmark_main,
};
use moor_common::model::PropDef;
use moor_db::PropResolutionCache;
use moor_var::{Obj, Symbol};
use std::{sync::RwLock, time::Duration};
use uuid::Uuid;

// Small cache context - simulates light usage
struct SmallPropCacheContext {
    prop_cache: Box<PropResolutionCache>,
    test_objs: Vec<Obj>,
    test_props: Vec<Symbol>,
    test_propdefs: Vec<PropDef>,
}

impl BenchContext for SmallPropCacheContext {
    fn prepare(_num_chunks: usize) -> Self {
        let prop_cache = Box::new(PropResolutionCache::new());

        // Create test data - small set
        let test_objs: Vec<Obj> = (1..=10).map(Obj::mk_id).collect();
        let test_props: Vec<Symbol> = ["name", "description", "location", "owner", "key"]
            .iter()
            .map(|&s| Symbol::mk(s))
            .collect();

        let test_propdefs: Vec<PropDef> = test_props
            .iter()
            .map(|prop| PropDef::new(Uuid::new_v4(), test_objs[0], test_objs[0], *prop))
            .collect();

        SmallPropCacheContext {
            prop_cache,
            test_objs,
            test_props,
            test_propdefs,
        }
    }
}

// Large cache context - simulates heavy usage with many objects/props
struct LargePropCacheContext {
    prop_cache: Box<PropResolutionCache>,
    test_objs: Vec<Obj>,
    test_props: Vec<Symbol>,
    test_propdefs: Vec<PropDef>,
}

impl BenchContext for LargePropCacheContext {
    fn prepare(_num_chunks: usize) -> Self {
        let prop_cache = Box::new(PropResolutionCache::new());

        // Create larger test data set
        let test_objs: Vec<Obj> = (1..=1000).map(Obj::mk_id).collect();
        let test_props: Vec<Symbol> = [
            "name",
            "description",
            "location",
            "owner",
            "key",
            "aliases",
            "exits",
            "contents",
            "player",
            "wizard",
            "programmer",
            "builder",
            "connected",
            "last_move",
            "home",
            "location_cache",
            "parent",
            "children",
            "verb_cache",
            "property_cache",
            "inheritance_cache",
            "security_cache",
            "feature_flags",
            "debug_info",
            "performance_stats",
            "memory_usage",
            "cpu_usage",
            "network_stats",
            "disk_usage",
            "cache_stats",
        ]
        .iter()
        .map(|&s| Symbol::mk(s))
        .collect();

        let test_propdefs: Vec<PropDef> = test_props
            .iter()
            .enumerate()
            .map(|(i, prop)| {
                PropDef::new(
                    Uuid::new_v4(),
                    test_objs[i % test_objs.len()],
                    test_objs[i % test_objs.len()],
                    *prop,
                )
            })
            .collect();

        LargePropCacheContext {
            prop_cache,
            test_objs,
            test_props,
            test_propdefs,
        }
    }
}

// Realistic cache context - matches real-world cache statistics
// Property cache: ~355 entries, 98.8% hit rate
struct RealisticPropCacheContext {
    prop_cache: Box<PropResolutionCache>,
    test_objs: Vec<Obj>,
    test_props: Vec<Symbol>,
}

impl BenchContext for RealisticPropCacheContext {
    fn prepare(_num_chunks: usize) -> Self {
        let mut prop_cache = Box::new(PropResolutionCache::new());

        // Realistic object count - approximate 50 objects based on 355 property entries and ~7 props per object
        let test_objs: Vec<Obj> = (1..=50).map(Obj::mk_id).collect();

        // Common MOO properties - about 12 core properties that get cached frequently
        let test_props: Vec<Symbol> = [
            "name",
            "description",
            "location",
            "owner",
            "key",
            "aliases",
            "exits",
            "contents",
            "parent",
            "wizard",
            "programmer",
            "player",
        ]
        .iter()
        .map(|&s| Symbol::mk(s))
        .collect();

        // Pre-populate to achieve realistic hit rates
        // 98.8% hit rate means 98.8% of lookups find cached entries
        let mut entry_count = 0;
        for obj in &test_objs {
            for prop in &test_props {
                if entry_count < 355 {
                    // Fill cache entry (98.8% will be hits)
                    let propdef = PropDef::new(Uuid::new_v4(), *obj, *obj, *prop);
                    prop_cache.fill_hit(obj, prop, &propdef);
                    entry_count += 1;
                }
            }
        }

        RealisticPropCacheContext {
            prop_cache,
            test_objs,
            test_props,
        }
    }
}

// Pre-populated cache context - tests performance with existing cache entries
struct PopulatedPropCacheContext {
    prop_cache: Box<PropResolutionCache>,
    test_objs: Vec<Obj>,
    test_props: Vec<Symbol>,
}

impl BenchContext for PopulatedPropCacheContext {
    fn prepare(_num_chunks: usize) -> Self {
        let mut prop_cache = Box::new(PropResolutionCache::new());

        let test_objs: Vec<Obj> = (1..=100).map(Obj::mk_id).collect();
        let test_props: Vec<Symbol> = [
            "name",
            "description",
            "location",
            "owner",
            "key",
            "aliases",
            "exits",
            "contents",
            "player",
            "wizard",
            "programmer",
            "builder",
            "connected",
            "last_move",
            "home",
        ]
        .iter()
        .map(|&s| Symbol::mk(s))
        .collect();

        // Pre-populate the caches
        for (i, obj) in test_objs.iter().enumerate() {
            for (j, prop) in test_props.iter().enumerate() {
                if (i + j) % 3 == 0 {
                    // Cache hit - create a propdef
                    let propdef = PropDef::new(Uuid::new_v4(), *obj, *obj, *prop);
                    prop_cache.fill_hit(obj, prop, &propdef);
                } else if (i + j) % 3 == 1 {
                    // Cache miss
                    prop_cache.fill_miss(obj, prop);
                }
                // 1/3 of entries are not cached (cold)
            }

            // Pre-populate first parent with props cache for some objects
            if i % 2 == 0 {
                let parent = if i > 0 { Some(Obj::mk_id(0)) } else { None };
                prop_cache.fill_first_parent_with_props(obj, parent);
            }
        }

        PopulatedPropCacheContext {
            prop_cache,
            test_objs,
            test_props,
        }
    }
}

struct SharedPropCacheContext {
    prop_cache: RwLock<PropResolutionCache>,
    test_objs: Vec<Obj>,
    test_props: Vec<Symbol>,
    test_propdefs: Vec<PropDef>,
}

impl ConcurrentBenchContext for SharedPropCacheContext {
    fn prepare(num_threads: usize) -> Self {
        let test_objs: Vec<Obj> = (1..=(num_threads.max(4) * 32) as i32)
            .map(Obj::mk_id)
            .collect();
        let test_props: Vec<Symbol> = [
            "name",
            "description",
            "location",
            "owner",
            "aliases",
            "contents",
            "parent",
            "wizard",
            "programmer",
            "player",
        ]
        .iter()
        .map(|&s| Symbol::mk(s))
        .collect();
        let test_propdefs: Vec<PropDef> = test_props
            .iter()
            .enumerate()
            .map(|(i, prop)| {
                let obj = test_objs[i % test_objs.len()];
                PropDef::new(Uuid::new_v4(), obj, obj, *prop)
            })
            .collect();

        let mut prop_cache = PropResolutionCache::new();
        for (i, obj) in test_objs.iter().enumerate() {
            for (j, prop) in test_props.iter().enumerate() {
                if (i + j) % 4 != 3 {
                    let propdef = &test_propdefs[j % test_propdefs.len()];
                    prop_cache.fill_hit(obj, prop, propdef);
                }
            }
            if i % 2 == 0 {
                prop_cache.fill_first_parent_with_props(obj, Some(Obj::mk_id(0)));
            }
        }

        Self {
            prop_cache: RwLock::new(prop_cache),
            test_objs,
            test_props,
            test_propdefs,
        }
    }
}

// === BENCHMARK FUNCTIONS ===

fn prop_cache_lookup_hits(
    ctx: &mut PopulatedPropCacheContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let prop_idx = i % ctx.test_props.len();

        // Only lookup entries that should be cache hits
        if (obj_idx + prop_idx).is_multiple_of(3) {
            let result = ctx
                .prop_cache
                .lookup(&ctx.test_objs[obj_idx], &ctx.test_props[prop_idx]);
            black_box(result);
        }
    }
}

fn prop_cache_lookup_misses(
    ctx: &mut PopulatedPropCacheContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let prop_idx = i % ctx.test_props.len();

        // Only lookup entries that should be cache misses
        if (obj_idx + prop_idx) % 3 == 1 {
            let result = ctx
                .prop_cache
                .lookup(&ctx.test_objs[obj_idx], &ctx.test_props[prop_idx]);
            black_box(result);
        }
    }
}

fn prop_cache_lookup_cold(
    ctx: &mut PopulatedPropCacheContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let prop_idx = i % ctx.test_props.len();

        // Only lookup entries that are not cached (cold lookups)
        if (obj_idx + prop_idx) % 3 == 2 {
            let result = ctx
                .prop_cache
                .lookup(&ctx.test_objs[obj_idx], &ctx.test_props[prop_idx]);
            black_box(result);
        }
    }
}

fn prop_cache_fill_hits(ctx: &mut SmallPropCacheContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let prop_idx = i % ctx.test_props.len();
        let propdef_idx = i % ctx.test_propdefs.len();

        ctx.prop_cache.fill_hit(
            &ctx.test_objs[obj_idx],
            &ctx.test_props[prop_idx],
            &ctx.test_propdefs[propdef_idx],
        );
    }
}

fn prop_cache_fill_misses(ctx: &mut SmallPropCacheContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let prop_idx = i % ctx.test_props.len();

        ctx.prop_cache
            .fill_miss(&ctx.test_objs[obj_idx], &ctx.test_props[prop_idx]);
    }
}

fn prop_cache_flush(ctx: &mut SmallPropCacheContext, chunk_size: usize, _chunk_num: usize) {
    // Fill cache first, then flush repeatedly
    for i in 0..chunk_size {
        if i % 100 == 0 {
            // Fill some entries
            for j in 0..10 {
                let obj_idx = (i + j) % ctx.test_objs.len();
                let prop_idx = (i + j) % ctx.test_props.len();
                let propdef_idx = (i + j) % ctx.test_propdefs.len();

                ctx.prop_cache.fill_hit(
                    &ctx.test_objs[obj_idx],
                    &ctx.test_props[prop_idx],
                    &ctx.test_propdefs[propdef_idx],
                );
            }
        }

        // Flush the cache
        ctx.prop_cache.flush();
        black_box(());
    }
}

fn prop_cache_fork(ctx: &mut SmallPropCacheContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let forked = ctx.prop_cache.fork();
        black_box(forked);
    }
}

fn prop_cache_parent_lookup(
    ctx: &mut PopulatedPropCacheContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let result = ctx
            .prop_cache
            .lookup_first_parent_with_props(&ctx.test_objs[obj_idx]);
        black_box(result);
    }
}

fn prop_cache_parent_fill(
    ctx: &mut PopulatedPropCacheContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let parent = if i % 2 == 0 {
            Some(Obj::mk_id(0))
        } else {
            None
        };
        ctx.prop_cache
            .fill_first_parent_with_props(&ctx.test_objs[obj_idx], parent);
    }
}

// Realistic workload - matches real-world usage patterns
fn prop_cache_realistic_workload(
    ctx: &mut RealisticPropCacheContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let prop_idx = i % ctx.test_props.len();

        match i % 1000 {
            0..=987 => {
                // 98.8% lookups with hits (matches real hit rate)
                let result = ctx
                    .prop_cache
                    .lookup(&ctx.test_objs[obj_idx], &ctx.test_props[prop_idx]);
                black_box(result);
            }
            _ => {
                // 1.2% cache misses
                let new_obj = Obj::mk_id(10000 + (i as i32)); // Uncached object
                let result = ctx.prop_cache.lookup(&new_obj, &ctx.test_props[prop_idx]);
                black_box(result);
            }
        }
    }
}

// Mixed workload - realistic simulation
fn prop_cache_mixed_workload(
    ctx: &mut LargePropCacheContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let prop_idx = i % ctx.test_props.len();
        let propdef_idx = i % ctx.test_propdefs.len();

        match i % 10 {
            0..=5 => {
                // 60% lookups (most common operation)
                let result = ctx
                    .prop_cache
                    .lookup(&ctx.test_objs[obj_idx], &ctx.test_props[prop_idx]);
                black_box(result);
            }
            6..=7 => {
                // 20% fill hits
                ctx.prop_cache.fill_hit(
                    &ctx.test_objs[obj_idx],
                    &ctx.test_props[prop_idx],
                    &ctx.test_propdefs[propdef_idx],
                );
            }
            8 => {
                // 10% fill misses
                ctx.prop_cache
                    .fill_miss(&ctx.test_objs[obj_idx], &ctx.test_props[prop_idx]);
            }
            _ => {
                // 10% other operations
                if i % 100 == 9 {
                    ctx.prop_cache.flush();
                } else {
                    let _forked = ctx.prop_cache.fork();
                }
            }
        }
    }
}

fn shared_prop_lookup_reader(
    ctx: &SharedPropCacheContext,
    control: &ConcurrentBenchControl,
) -> ConcurrentWorkerResult {
    let mut operations = 0_u64;
    let mut blocked_reads = 0_u64;
    while !control.should_stop() {
        if let Ok(cache) = ctx.prop_cache.try_read() {
            let obj_idx = (operations as usize + control.thread_index()) % ctx.test_objs.len();
            let prop_idx =
                (operations as usize + control.role_thread_index()) % ctx.test_props.len();
            let result = cache.lookup(&ctx.test_objs[obj_idx], &ctx.test_props[prop_idx]);
            black_box(result);
            operations = operations.wrapping_add(1);
        } else {
            blocked_reads = blocked_reads.wrapping_add(1);
        }
    }
    ConcurrentWorkerResult::operations(operations).with_counter("blocked_reads", blocked_reads)
}

fn shared_prop_mutator(
    ctx: &SharedPropCacheContext,
    control: &ConcurrentBenchControl,
) -> ConcurrentWorkerResult {
    let mut operations = 0_u64;
    let mut negative_fills = 0_u64;
    while !control.should_stop() {
        let obj_idx = (operations as usize + control.thread_index()) % ctx.test_objs.len();
        let prop_idx = (operations as usize + control.role_thread_index()) % ctx.test_props.len();
        let propdef_idx = (operations as usize + control.thread_index()) % ctx.test_propdefs.len();
        let mut cache = ctx.prop_cache.write().expect("prop cache rwlock poisoned");
        match operations % 8 {
            0 => cache.fill_first_parent_with_props(
                &ctx.test_objs[obj_idx],
                Some(ctx.test_objs[(obj_idx + 1) % ctx.test_objs.len()]),
            ),
            1 => {
                cache.fill_miss(&ctx.test_objs[obj_idx], &ctx.test_props[prop_idx]);
                negative_fills = negative_fills.wrapping_add(1);
            }
            _ => cache.fill_hit(
                &ctx.test_objs[obj_idx],
                &ctx.test_props[prop_idx],
                &ctx.test_propdefs[propdef_idx],
            ),
        }
        operations = operations.wrapping_add(1);
    }
    ConcurrentWorkerResult::operations(operations).with_counter("negative_fills", negative_fills)
}

fn shared_prop_flush_invalidator(
    ctx: &SharedPropCacheContext,
    control: &ConcurrentBenchControl,
) -> ConcurrentWorkerResult {
    let mut operations = 0_u64;
    let mut flushes = 0_u64;
    while !control.should_stop() {
        let mut cache = ctx.prop_cache.write().expect("prop cache rwlock poisoned");
        if operations % 32 == 0 {
            cache.flush();
            flushes = flushes.wrapping_add(1);
            for refill in 0..ctx.test_objs.len().min(16) {
                let obj = ctx.test_objs[(refill + control.thread_index()) % ctx.test_objs.len()];
                let prop = ctx.test_props[refill % ctx.test_props.len()];
                let propdef = &ctx.test_propdefs[refill % ctx.test_propdefs.len()];
                cache.fill_hit(&obj, &prop, propdef);
            }
        } else {
            let obj_idx = (operations as usize + control.thread_index()) % ctx.test_objs.len();
            let prop_idx =
                (operations as usize + control.role_thread_index()) % ctx.test_props.len();
            cache.fill_miss(&ctx.test_objs[obj_idx], &ctx.test_props[prop_idx]);
        }
        operations = operations.wrapping_add(1);
    }
    ConcurrentWorkerResult::operations(operations).with_counter("flushes", flushes)
}

benchmark_main!(
    BenchmarkMainOptions {
        filter_help: Some(
            "all, lookup, fill, flush, fork, parent, mixed, realistic, or any benchmark name substring".to_string()
        ),
        ..BenchmarkMainOptions::default()
    },
    |runner| {
    runner.group::<PopulatedPropCacheContext>("Prop Cache Lookup Benchmarks", |g| {
        g.bench("prop_cache_lookup_hits", prop_cache_lookup_hits);
        g.bench("prop_cache_lookup_misses", prop_cache_lookup_misses);
        g.bench("prop_cache_lookup_cold", prop_cache_lookup_cold);
    });

    runner.group::<SmallPropCacheContext>("Prop Cache Fill Benchmarks", |g| {
        g.bench("prop_cache_fill_hits", prop_cache_fill_hits);
        g.bench("prop_cache_fill_misses", prop_cache_fill_misses);
    });

    runner.group::<SmallPropCacheContext>("Prop Cache Flush Benchmarks", |g| {
        g.bench("prop_cache_flush", prop_cache_flush);
    });

    runner.group::<SmallPropCacheContext>("Prop Cache Fork Benchmarks", |g| {
        g.bench("prop_cache_fork", prop_cache_fork);
    });

    runner.group::<PopulatedPropCacheContext>("Prop Parent Cache Benchmarks", |g| {
        g.bench("prop_cache_parent_lookup", prop_cache_parent_lookup);
        g.bench("prop_cache_parent_fill", prop_cache_parent_fill);
    });

    let max_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(8);
    for &threads in &[2usize, 4, 8] {
        if threads > max_threads {
            continue;
        }
        let reader_threads = threads.saturating_sub(1).max(1);
        let lookup_vs_mutation = [
            ConcurrentWorker {
                name: "lookup_reader",
                threads: reader_threads,
                run: shared_prop_lookup_reader,
            },
            ConcurrentWorker {
                name: "mutator",
                threads: 1,
                run: shared_prop_mutator,
            },
        ];
        let lookup_vs_flush = [
            ConcurrentWorker {
                name: "lookup_reader",
                threads: reader_threads,
                run: shared_prop_lookup_reader,
            },
            ConcurrentWorker {
                name: "flush_invalidator",
                threads: 1,
                run: shared_prop_flush_invalidator,
            },
        ];

        runner.concurrent_group::<SharedPropCacheContext>("Prop Cache Concurrent Scenarios", |g| {
            g.bench(
                &format!("prop_cache_lookup_vs_mutation_{threads}t"),
                Duration::from_millis(100),
                &lookup_vs_mutation,
            );
            g.bench(
                &format!("prop_cache_lookup_vs_flush_{threads}t"),
                Duration::from_millis(100),
                &lookup_vs_flush,
            );
        });
    }

    runner.group::<LargePropCacheContext>("Mixed Workload Benchmarks", |g| {
        g.bench("prop_cache_mixed_workload", prop_cache_mixed_workload);
    });

    runner.group::<RealisticPropCacheContext>("Realistic Workload Benchmarks", |g| {
        g.bench("prop_cache_realistic_workload", prop_cache_realistic_workload);
    });
    }
);
