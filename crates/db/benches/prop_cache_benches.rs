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

use moor_bench_utils::{BenchContext, black_box};
use moor_common::model::PropDef;
use moor_db::prop_cache::PropResolutionCache;
use moor_var::{Obj, Symbol};
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
        let prop_cache = Box::new(PropResolutionCache::new());

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
        let prop_cache = Box::new(PropResolutionCache::new());

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

pub fn main() {
    use moor_bench_utils::{BenchmarkDef, generate_session_summary, run_benchmark_group};
    use std::env;

    #[cfg(target_os = "linux")]
    {
        use moor_bench_utils::perf_event::{Builder, events::Hardware};
        if Builder::new(Hardware::INSTRUCTIONS).build().is_err() {
            eprintln!(
                "⚠️  Perf events are not available on this system (insufficient permissions or kernel support)."
            );
            eprintln!("   Continuing with timing-only benchmarks (performance counters disabled).");
            eprintln!();
        }
    }

    let args: Vec<String> = env::args().collect();
    let filter = if let Some(separator_pos) = args.iter().position(|arg| arg == "--") {
        args.get(separator_pos + 1).map(|s| s.as_str())
    } else {
        args.iter()
            .skip(1)
            .find(|arg| !arg.starts_with("--") && !args[0].contains(arg.as_str()))
            .map(|s| s.as_str())
    };

    if let Some(f) = filter {
        eprintln!("Running prop cache benchmarks matching filter: '{f}'");
        eprintln!(
            "Available filters: all, lookup, fill, flush, fork, parent, mixed, realistic, or any benchmark name substring"
        );
        eprintln!();
    }

    // Define benchmark groups
    let lookup_benchmarks = [
        BenchmarkDef {
            name: "prop_cache_lookup_hits",
            group: "lookup",
            func: prop_cache_lookup_hits,
        },
        BenchmarkDef {
            name: "prop_cache_lookup_misses",
            group: "lookup",
            func: prop_cache_lookup_misses,
        },
        BenchmarkDef {
            name: "prop_cache_lookup_cold",
            group: "lookup",
            func: prop_cache_lookup_cold,
        },
    ];

    let fill_benchmarks = [
        BenchmarkDef {
            name: "prop_cache_fill_hits",
            group: "fill",
            func: prop_cache_fill_hits,
        },
        BenchmarkDef {
            name: "prop_cache_fill_misses",
            group: "fill",
            func: prop_cache_fill_misses,
        },
    ];

    let flush_benchmarks = [BenchmarkDef {
        name: "prop_cache_flush",
        group: "flush",
        func: prop_cache_flush,
    }];

    let fork_benchmarks = [BenchmarkDef {
        name: "prop_cache_fork",
        group: "fork",
        func: prop_cache_fork,
    }];

    let parent_benchmarks = [
        BenchmarkDef {
            name: "prop_cache_parent_lookup",
            group: "parent",
            func: prop_cache_parent_lookup,
        },
        BenchmarkDef {
            name: "prop_cache_parent_fill",
            group: "parent",
            func: prop_cache_parent_fill,
        },
    ];

    let mixed_benchmarks = [BenchmarkDef {
        name: "prop_cache_mixed_workload",
        group: "mixed",
        func: prop_cache_mixed_workload,
    }];

    let realistic_benchmarks = [BenchmarkDef {
        name: "prop_cache_realistic_workload",
        group: "realistic",
        func: prop_cache_realistic_workload,
    }];

    // Run benchmark groups
    run_benchmark_group::<PopulatedPropCacheContext>(
        &lookup_benchmarks,
        "Prop Cache Lookup Benchmarks",
        filter,
    );
    run_benchmark_group::<SmallPropCacheContext>(
        &fill_benchmarks,
        "Prop Cache Fill Benchmarks",
        filter,
    );
    run_benchmark_group::<SmallPropCacheContext>(
        &flush_benchmarks,
        "Prop Cache Flush Benchmarks",
        filter,
    );
    run_benchmark_group::<SmallPropCacheContext>(
        &fork_benchmarks,
        "Prop Cache Fork Benchmarks",
        filter,
    );
    run_benchmark_group::<PopulatedPropCacheContext>(
        &parent_benchmarks,
        "Prop Parent Cache Benchmarks",
        filter,
    );
    run_benchmark_group::<LargePropCacheContext>(
        &mixed_benchmarks,
        "Mixed Workload Benchmarks",
        filter,
    );
    run_benchmark_group::<RealisticPropCacheContext>(
        &realistic_benchmarks,
        "Realistic Workload Benchmarks",
        filter,
    );

    if filter.is_some() {
        eprintln!("\nProp cache benchmark filtering complete.");
    }

    generate_session_summary();
}
