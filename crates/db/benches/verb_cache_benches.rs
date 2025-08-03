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
use moor_common::model::{VerbArgsSpec, VerbDef, VerbFlag};
use moor_db::verb_cache::{AncestryCache, VerbResolutionCache};
use moor_var::{Obj, Symbol};
use uuid::Uuid;

// Small cache context - simulates light usage
struct SmallCacheContext {
    verb_cache: Box<VerbResolutionCache>,
    #[allow(dead_code)]
    ancestry_cache: Box<AncestryCache>,
    test_objs: Vec<Obj>,
    test_verbs: Vec<Symbol>,
    test_verbdefs: Vec<VerbDef>,
}

impl BenchContext for SmallCacheContext {
    fn prepare(_num_chunks: usize) -> Self {
        let verb_cache = Box::new(VerbResolutionCache::new());
        let ancestry_cache = Box::new(AncestryCache::default());

        // Create test data - small set
        let test_objs: Vec<Obj> = (1..=10).map(Obj::mk_id).collect();
        let test_verbs: Vec<Symbol> = ["look", "get", "drop", "give", "examine"]
            .iter()
            .map(|&s| Symbol::mk(s))
            .collect();

        let test_verbdefs: Vec<VerbDef> = test_verbs
            .iter()
            .enumerate()
            .map(|(_i, verb)| {
                VerbDef::new(
                    Uuid::new_v4(),
                    test_objs[0],
                    test_objs[0],
                    &[*verb],
                    VerbFlag::rwx(),
                    VerbArgsSpec::this_none_this(),
                )
            })
            .collect();

        SmallCacheContext {
            verb_cache,
            ancestry_cache,
            test_objs,
            test_verbs,
            test_verbdefs,
        }
    }
}

// Large cache context - simulates heavy usage with many objects/verbs
struct LargeCacheContext {
    verb_cache: Box<VerbResolutionCache>,
    #[allow(dead_code)]
    ancestry_cache: Box<AncestryCache>,
    test_objs: Vec<Obj>,
    test_verbs: Vec<Symbol>,
    test_verbdefs: Vec<VerbDef>,
}

impl BenchContext for LargeCacheContext {
    fn prepare(_num_chunks: usize) -> Self {
        let verb_cache = Box::new(VerbResolutionCache::new());
        let ancestry_cache = Box::new(AncestryCache::default());

        // Create larger test data set
        let test_objs: Vec<Obj> = (1..=1000).map(Obj::mk_id).collect();
        let test_verbs: Vec<Symbol> = [
            "look",
            "get",
            "drop",
            "give",
            "examine",
            "inventory",
            "who",
            "score",
            "tell",
            "say",
            "emote",
            "pose",
            "think",
            "whisper",
            "page",
            "goto",
            "move",
            "take",
            "put",
            "open",
            "close",
            "lock",
            "unlock",
            "read",
            "write",
            "edit",
            "create",
            "destroy",
            "clone",
            "teleport",
        ]
        .iter()
        .map(|&s| Symbol::mk(s))
        .collect();

        let test_verbdefs: Vec<VerbDef> = test_verbs
            .iter()
            .enumerate()
            .map(|(_i, verb)| {
                VerbDef::new(
                    Uuid::new_v4(),
                    test_objs[i % test_objs.len()],
                    test_objs[i % test_objs.len()],
                    &[*verb],
                    VerbFlag::rwx(),
                    VerbArgsSpec::this_none_this(),
                )
            })
            .collect();

        LargeCacheContext {
            verb_cache,
            ancestry_cache,
            test_objs,
            test_verbs,
            test_verbdefs,
        }
    }
}

// Pre-populated cache context - tests performance with existing cache entries
struct PopulatedCacheContext {
    verb_cache: Box<VerbResolutionCache>,
    ancestry_cache: Box<AncestryCache>,
    test_objs: Vec<Obj>,
    test_verbs: Vec<Symbol>,
}

impl BenchContext for PopulatedCacheContext {
    fn prepare(_num_chunks: usize) -> Self {
        let verb_cache = Box::new(VerbResolutionCache::new());
        let ancestry_cache = Box::new(AncestryCache::default());

        let test_objs: Vec<Obj> = (1..=100).map(Obj::mk_id).collect();
        let test_verbs: Vec<Symbol> = [
            "look",
            "get",
            "drop",
            "give",
            "examine",
            "inventory",
            "who",
            "score",
            "tell",
            "say",
            "emote",
            "pose",
            "think",
            "whisper",
            "page",
        ]
        .iter()
        .map(|&s| Symbol::mk(s))
        .collect();

        // Pre-populate the caches
        for (i, obj) in test_objs.iter().enumerate() {
            for (j, verb) in test_verbs.iter().enumerate() {
                if (i + j) % 3 == 0 {
                    // Cache hit - create a verbdef
                    let verbdef = VerbDef::new(
                        Uuid::new_v4(),
                        *obj,
                        *obj,
                        &[*verb],
                        VerbFlag::rwx(),
                        VerbArgsSpec::this_none_this(),
                    );
                    verb_cache.fill_hit(obj, verb, &verbdef);
                } else if (i + j) % 3 == 1 {
                    // Cache miss
                    verb_cache.fill_miss(obj, verb);
                }
                // 1/3 of entries are not cached (cold)
            }

            // Pre-populate ancestry cache for some objects
            if i % 2 == 0 {
                let ancestors: Vec<Obj> = (0..=i.min(5)).map(|j| Obj::mk_id(j as i32)).collect();
                ancestry_cache.fill(obj, &ancestors);
            }
        }

        PopulatedCacheContext {
            verb_cache,
            ancestry_cache,
            test_objs,
            test_verbs,
        }
    }
}

// Concurrent access simulation context
struct ConcurrentCacheContext {
    caches: Vec<Box<VerbResolutionCache>>,
    test_objs: Vec<Obj>,
    test_verbs: Vec<Symbol>,
}

impl BenchContext for ConcurrentCacheContext {
    fn prepare(_num_chunks: usize) -> Self {
        // Create multiple cache instances to simulate concurrent access
        let main_cache = Box::new(VerbResolutionCache::new());
        let mut caches = vec![main_cache];

        // Create several forked caches to simulate transactions
        for _ in 0..10 {
            caches.push(caches[0].fork());
        }

        let test_objs: Vec<Obj> = (1..=50).map(Obj::mk_id).collect();
        let test_verbs: Vec<Symbol> = ["look", "get", "drop", "give", "examine", "inventory"]
            .iter()
            .map(|&s| Symbol::mk(s))
            .collect();

        ConcurrentCacheContext {
            caches,
            test_objs,
            test_verbs,
        }
    }
}

// === BENCHMARK FUNCTIONS ===

fn verb_cache_lookup_hits(ctx: &mut PopulatedCacheContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let verb_idx = i % ctx.test_verbs.len();

        // Only lookup entries that should be cache hits
        if (obj_idx + verb_idx) % 3 == 0 {
            let result = ctx
                .verb_cache
                .lookup(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
            black_box(result);
        }
    }
}

fn verb_cache_lookup_misses(ctx: &mut PopulatedCacheContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let verb_idx = i % ctx.test_verbs.len();

        // Only lookup entries that should be cache misses
        if (obj_idx + verb_idx) % 3 == 1 {
            let result = ctx
                .verb_cache
                .lookup(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
            black_box(result);
        }
    }
}

fn verb_cache_lookup_cold(ctx: &mut PopulatedCacheContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let verb_idx = i % ctx.test_verbs.len();

        // Only lookup entries that are not cached (cold lookups)
        if (obj_idx + verb_idx) % 3 == 2 {
            let result = ctx
                .verb_cache
                .lookup(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
            black_box(result);
        }
    }
}

fn verb_cache_fill_hits(ctx: &mut SmallCacheContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let verb_idx = i % ctx.test_verbs.len();
        let verbdef_idx = i % ctx.test_verbdefs.len();

        ctx.verb_cache.fill_hit(
            &ctx.test_objs[obj_idx],
            &ctx.test_verbs[verb_idx],
            &ctx.test_verbdefs[verbdef_idx],
        );
    }
}

fn verb_cache_fill_misses(ctx: &mut SmallCacheContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let verb_idx = i % ctx.test_verbs.len();

        ctx.verb_cache
            .fill_miss(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
    }
}

fn verb_cache_flush(ctx: &mut SmallCacheContext, chunk_size: usize, _chunk_num: usize) {
    // Fill cache first, then flush repeatedly
    for i in 0..chunk_size {
        if i % 100 == 0 {
            // Fill some entries
            for j in 0..10 {
                let obj_idx = (i + j) % ctx.test_objs.len();
                let verb_idx = (i + j) % ctx.test_verbs.len();
                let verbdef_idx = (i + j) % ctx.test_verbdefs.len();

                ctx.verb_cache.fill_hit(
                    &ctx.test_objs[obj_idx],
                    &ctx.test_verbs[verb_idx],
                    &ctx.test_verbdefs[verbdef_idx],
                );
            }
        }

        // Flush the cache
        ctx.verb_cache.flush();
        black_box(());
    }
}

fn verb_cache_fork(ctx: &mut SmallCacheContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let forked = ctx.verb_cache.fork();
        black_box(forked);
    }
}

fn ancestry_cache_lookup(ctx: &mut PopulatedCacheContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let result = ctx.ancestry_cache.lookup(&ctx.test_objs[obj_idx]);
        black_box(result);
    }
}

fn ancestry_cache_fill(ctx: &mut PopulatedCacheContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let ancestors: Vec<Obj> = (0..=(i % 5)).map(|j| Obj::mk_id(j as i32)).collect();
        ctx.ancestry_cache.fill(&ctx.test_objs[obj_idx], &ancestors);
    }
}

fn concurrent_cache_access(ctx: &mut ConcurrentCacheContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let cache_idx = i % ctx.caches.len();
        let obj_idx = i % ctx.test_objs.len();
        let verb_idx = i % ctx.test_verbs.len();

        // Mix of operations
        match i % 4 {
            0 => {
                // Lookup
                let result = ctx.caches[cache_idx]
                    .lookup(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
                black_box(result);
            }
            1 => {
                // Fill miss
                ctx.caches[cache_idx].fill_miss(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
            }
            2 => {
                // Check if changed
                let changed = ctx.caches[cache_idx].has_changed();
                black_box(changed);
            }
            _ => {
                // Fork cache
                let forked = ctx.caches[cache_idx].fork();
                black_box(forked);
            }
        }
    }
}

// Mixed workload - realistic simulation
fn verb_cache_mixed_workload(ctx: &mut LargeCacheContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let verb_idx = i % ctx.test_verbs.len();
        let verbdef_idx = i % ctx.test_verbdefs.len();

        match i % 10 {
            0..=5 => {
                // 60% lookups (most common operation)
                let result = ctx
                    .verb_cache
                    .lookup(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
                black_box(result);
            }
            6..=7 => {
                // 20% fill hits
                ctx.verb_cache.fill_hit(
                    &ctx.test_objs[obj_idx],
                    &ctx.test_verbs[verb_idx],
                    &ctx.test_verbdefs[verbdef_idx],
                );
            }
            8 => {
                // 10% fill misses
                ctx.verb_cache
                    .fill_miss(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
            }
            _ => {
                // 10% other operations
                if i % 100 == 9 {
                    ctx.verb_cache.flush();
                } else {
                    let _forked = ctx.verb_cache.fork();
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
        eprintln!("Running verb cache benchmarks matching filter: '{f}'");
        eprintln!(
            "Available filters: all, lookup, fill, flush, fork, ancestry, concurrent, mixed, or any benchmark name substring"
        );
        eprintln!();
    }

    // Define benchmark groups
    let lookup_benchmarks = [
        BenchmarkDef {
            name: "verb_cache_lookup_hits",
            group: "lookup",
            func: verb_cache_lookup_hits,
        },
        BenchmarkDef {
            name: "verb_cache_lookup_misses",
            group: "lookup",
            func: verb_cache_lookup_misses,
        },
        BenchmarkDef {
            name: "verb_cache_lookup_cold",
            group: "lookup",
            func: verb_cache_lookup_cold,
        },
    ];

    let fill_benchmarks = [
        BenchmarkDef {
            name: "verb_cache_fill_hits",
            group: "fill",
            func: verb_cache_fill_hits,
        },
        BenchmarkDef {
            name: "verb_cache_fill_misses",
            group: "fill",
            func: verb_cache_fill_misses,
        },
    ];

    let flush_benchmarks = [BenchmarkDef {
        name: "verb_cache_flush",
        group: "flush",
        func: verb_cache_flush,
    }];

    let fork_benchmarks = [BenchmarkDef {
        name: "verb_cache_fork",
        group: "fork",
        func: verb_cache_fork,
    }];

    let ancestry_benchmarks = [
        BenchmarkDef {
            name: "ancestry_cache_lookup",
            group: "ancestry",
            func: ancestry_cache_lookup,
        },
        BenchmarkDef {
            name: "ancestry_cache_fill",
            group: "ancestry",
            func: ancestry_cache_fill,
        },
    ];

    let concurrent_benchmarks = [BenchmarkDef {
        name: "concurrent_cache_access",
        group: "concurrent",
        func: concurrent_cache_access,
    }];

    let mixed_benchmarks = [BenchmarkDef {
        name: "verb_cache_mixed_workload",
        group: "mixed",
        func: verb_cache_mixed_workload,
    }];

    // Run benchmark groups
    run_benchmark_group::<PopulatedCacheContext>(
        &lookup_benchmarks,
        "Verb Cache Lookup Benchmarks",
        filter,
    );
    run_benchmark_group::<SmallCacheContext>(
        &fill_benchmarks,
        "Verb Cache Fill Benchmarks",
        filter,
    );
    run_benchmark_group::<SmallCacheContext>(
        &flush_benchmarks,
        "Verb Cache Flush Benchmarks",
        filter,
    );
    run_benchmark_group::<SmallCacheContext>(
        &fork_benchmarks,
        "Verb Cache Fork Benchmarks",
        filter,
    );
    run_benchmark_group::<PopulatedCacheContext>(
        &ancestry_benchmarks,
        "Ancestry Cache Benchmarks",
        filter,
    );
    run_benchmark_group::<ConcurrentCacheContext>(
        &concurrent_benchmarks,
        "Concurrent Cache Benchmarks",
        filter,
    );
    run_benchmark_group::<LargeCacheContext>(
        &mixed_benchmarks,
        "Mixed Workload Benchmarks",
        filter,
    );

    if filter.is_some() {
        eprintln!("\nVerb cache benchmark filtering complete.");
    }

    generate_session_summary();
}
