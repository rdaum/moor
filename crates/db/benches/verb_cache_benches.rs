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
use moor_common::model::{VerbArgsSpec, VerbDef, VerbFlag};
use moor_db::{AncestryCache, VerbResolutionCache};
use moor_var::{Obj, Symbol};
use std::{sync::RwLock, time::Duration};
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
            .map(|verb| {
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
            .map(|(i, verb)| {
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

// Realistic cache context - matches real-world cache statistics
// Verb cache: ~633 entries, 99.5% hit rate
// Ancestry cache: ~87 entries, 92.6% hit rate
struct RealisticCacheContext {
    verb_cache: Box<VerbResolutionCache>,
    ancestry_cache: Box<AncestryCache>,
    test_objs: Vec<Obj>,
    test_verbs: Vec<Symbol>,
}

impl BenchContext for RealisticCacheContext {
    fn prepare(_num_chunks: usize) -> Self {
        let mut verb_cache = Box::new(VerbResolutionCache::new());
        let mut ancestry_cache = Box::new(AncestryCache::default());

        // Realistic object count - approximate 120 objects based on 633 verb entries and ~5 verbs per object
        let test_objs: Vec<Obj> = (1..=120).map(Obj::mk_id).collect();

        // Common MOO verbs - about 15 core verbs that get cached frequently
        let test_verbs: Vec<Symbol> = [
            "look",
            "get",
            "drop",
            "examine",
            "inventory",
            "say",
            "tell",
            "emote",
            "go",
            "enter",
            "read",
            "open",
            "close",
            "give",
            "take",
        ]
        .iter()
        .map(|&s| Symbol::mk(s))
        .collect();

        // Pre-populate to achieve realistic hit rates
        // 99.5% hit rate means 99.5% of lookups find cached entries
        let mut entry_count = 0;
        for (i, obj) in test_objs.iter().enumerate() {
            for verb in test_verbs.iter() {
                if entry_count < 633 {
                    // Fill cache entry (99.5% will be hits)
                    let verbdef = VerbDef::new(
                        Uuid::new_v4(),
                        *obj,
                        *obj,
                        &[*verb],
                        VerbFlag::rwx(),
                        VerbArgsSpec::this_none_this(),
                    );
                    verb_cache.fill_hit(obj, verb, verbdef.as_resolved());
                    entry_count += 1;
                }
            }

            // Pre-populate ancestry cache for ~87 objects (92.6% hit rate)
            if i < 87 {
                let ancestors: Vec<Obj> = (0..=(i.min(3))).map(|k| Obj::mk_id(k as i32)).collect();
                ancestry_cache.fill(obj, &ancestors);
            }
        }

        RealisticCacheContext {
            verb_cache,
            ancestry_cache,
            test_objs,
            test_verbs,
        }
    }
}

// Pre-populated cache context - tests performance with existing cache entries
struct PopulatedCacheContext {
    verb_cache: Box<VerbResolutionCache>,
    ancestry_cache: Box<AncestryCache>,
    test_objs: Vec<Obj>,
    test_verbs: Vec<Symbol>,
    // Pre-computed index pairs for each lookup type to avoid branching in benchmarks
    hit_pairs: Vec<(usize, usize)>,
    miss_pairs: Vec<(usize, usize)>,
    cold_pairs: Vec<(usize, usize)>,
}

impl BenchContext for PopulatedCacheContext {
    fn prepare(_num_chunks: usize) -> Self {
        let mut verb_cache = Box::new(VerbResolutionCache::new());
        let mut ancestry_cache = Box::new(AncestryCache::default());

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
                    verb_cache.fill_hit(obj, verb, verbdef.as_resolved());
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

        // Pre-compute index pairs for each lookup type
        let mut hit_pairs = Vec::new();
        let mut miss_pairs = Vec::new();
        let mut cold_pairs = Vec::new();
        for i in 0..test_objs.len() {
            for j in 0..test_verbs.len() {
                match (i + j) % 3 {
                    0 => hit_pairs.push((i, j)),
                    1 => miss_pairs.push((i, j)),
                    _ => cold_pairs.push((i, j)),
                }
            }
        }

        PopulatedCacheContext {
            verb_cache,
            ancestry_cache,
            test_objs,
            test_verbs,
            hit_pairs,
            miss_pairs,
            cold_pairs,
        }
    }
}

// Concurrent access simulation context
struct ConcurrentCacheContext {
    caches: Vec<VerbResolutionCache>,
    test_objs: Vec<Obj>,
    test_verbs: Vec<Symbol>,
}

impl BenchContext for ConcurrentCacheContext {
    fn prepare(_num_chunks: usize) -> Self {
        // Create multiple cache instances to simulate concurrent access
        let main_cache = VerbResolutionCache::new();
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

struct SharedVerbCacheContext {
    verb_cache: RwLock<VerbResolutionCache>,
    ancestry_cache: RwLock<AncestryCache>,
    test_objs: Vec<Obj>,
    test_verbs: Vec<Symbol>,
    test_verbdefs: Vec<VerbDef>,
}

impl ConcurrentBenchContext for SharedVerbCacheContext {
    fn prepare(num_threads: usize) -> Self {
        let test_objs: Vec<Obj> = (1..=(num_threads.max(4) * 32) as i32)
            .map(Obj::mk_id)
            .collect();
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
        ]
        .iter()
        .map(|&s| Symbol::mk(s))
        .collect();
        let test_verbdefs: Vec<VerbDef> = test_verbs
            .iter()
            .enumerate()
            .map(|(i, verb)| {
                let obj = test_objs[i % test_objs.len()];
                VerbDef::new(
                    Uuid::new_v4(),
                    obj,
                    obj,
                    &[*verb],
                    VerbFlag::rwx(),
                    VerbArgsSpec::this_none_this(),
                )
            })
            .collect();

        let mut verb_cache = VerbResolutionCache::new();
        let mut ancestry_cache = AncestryCache::default();
        for (i, obj) in test_objs.iter().enumerate() {
            for (j, verb) in test_verbs.iter().enumerate() {
                if (i + j) % 5 != 4 {
                    let verbdef = test_verbdefs[j % test_verbdefs.len()].clone();
                    verb_cache.fill_hit(obj, verb, verbdef.as_resolved());
                }
            }
            if i % 2 == 0 {
                let ancestors: Vec<Obj> = (0..=(i.min(4))).map(|k| Obj::mk_id(k as i32)).collect();
                ancestry_cache.fill(obj, &ancestors);
            }
        }

        Self {
            verb_cache: RwLock::new(verb_cache),
            ancestry_cache: RwLock::new(ancestry_cache),
            test_objs,
            test_verbs,
            test_verbdefs,
        }
    }
}

// === BENCHMARK FUNCTIONS ===

fn verb_cache_lookup_hits(ctx: &mut PopulatedCacheContext, chunk_size: usize, _chunk_num: usize) {
    let pairs = &ctx.hit_pairs;
    let len = pairs.len();
    for i in 0..chunk_size {
        let (obj_idx, verb_idx) = pairs[i % len];
        let result = ctx
            .verb_cache
            .lookup(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
        black_box(result);
    }
}

fn verb_cache_lookup_misses(ctx: &mut PopulatedCacheContext, chunk_size: usize, _chunk_num: usize) {
    let pairs = &ctx.miss_pairs;
    let len = pairs.len();
    for i in 0..chunk_size {
        let (obj_idx, verb_idx) = pairs[i % len];
        let result = ctx
            .verb_cache
            .lookup(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
        black_box(result);
    }
}

fn verb_cache_lookup_cold(ctx: &mut PopulatedCacheContext, chunk_size: usize, _chunk_num: usize) {
    let pairs = &ctx.cold_pairs;
    let len = pairs.len();
    for i in 0..chunk_size {
        let (obj_idx, verb_idx) = pairs[i % len];
        let result = ctx
            .verb_cache
            .lookup(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
        black_box(result);
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
            ctx.test_verbdefs[verbdef_idx].as_resolved(),
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
                    ctx.test_verbdefs[verbdef_idx].as_resolved(),
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

fn shared_verb_lookup_reader(
    ctx: &SharedVerbCacheContext,
    control: &ConcurrentBenchControl,
) -> ConcurrentWorkerResult {
    let mut operations = 0_u64;
    let mut blocked_reads = 0_u64;
    while !control.should_stop() {
        if let Ok(cache) = ctx.verb_cache.try_read() {
            let slot = (operations as usize + control.thread_index()) % ctx.test_objs.len();
            let verb_slot =
                (operations as usize + control.role_thread_index()) % ctx.test_verbs.len();
            let result = cache.lookup(&ctx.test_objs[slot], &ctx.test_verbs[verb_slot]);
            black_box(result);
            operations = operations.wrapping_add(1);
        } else {
            blocked_reads = blocked_reads.wrapping_add(1);
        }
    }
    ConcurrentWorkerResult::operations(operations).with_counter("blocked_reads", blocked_reads)
}

fn shared_verb_mutator(
    ctx: &SharedVerbCacheContext,
    control: &ConcurrentBenchControl,
) -> ConcurrentWorkerResult {
    let mut operations = 0_u64;
    let mut negative_fills = 0_u64;
    while !control.should_stop() {
        let obj_idx = (operations as usize + control.thread_index()) % ctx.test_objs.len();
        let verb_idx = (operations as usize + control.role_thread_index()) % ctx.test_verbs.len();
        let verbdef_idx = (operations as usize + control.thread_index()) % ctx.test_verbdefs.len();
        let mut cache = ctx.verb_cache.write().expect("verb cache rwlock poisoned");
        if operations % 5 == 4 {
            cache.fill_miss(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
            negative_fills = negative_fills.wrapping_add(1);
        } else {
            cache.fill_hit(
                &ctx.test_objs[obj_idx],
                &ctx.test_verbs[verb_idx],
                ctx.test_verbdefs[verbdef_idx].as_resolved(),
            );
        }
        operations = operations.wrapping_add(1);
    }
    ConcurrentWorkerResult::operations(operations).with_counter("negative_fills", negative_fills)
}

fn ancestry_lookup_reader(
    ctx: &SharedVerbCacheContext,
    control: &ConcurrentBenchControl,
) -> ConcurrentWorkerResult {
    let mut operations = 0_u64;
    let mut blocked_reads = 0_u64;
    while !control.should_stop() {
        if let Ok(cache) = ctx.ancestry_cache.try_read() {
            let obj_idx = (operations as usize + control.thread_index()) % ctx.test_objs.len();
            let result = cache.lookup(&ctx.test_objs[obj_idx]);
            black_box(result);
            operations = operations.wrapping_add(1);
        } else {
            blocked_reads = blocked_reads.wrapping_add(1);
        }
    }
    ConcurrentWorkerResult::operations(operations).with_counter("blocked_reads", blocked_reads)
}

fn ancestry_invalidator(
    ctx: &SharedVerbCacheContext,
    control: &ConcurrentBenchControl,
) -> ConcurrentWorkerResult {
    let mut operations = 0_u64;
    let mut flushes = 0_u64;
    while !control.should_stop() {
        let mut cache = ctx
            .ancestry_cache
            .write()
            .expect("ancestry cache rwlock poisoned");
        if operations % 32 == 0 {
            cache.flush();
            flushes = flushes.wrapping_add(1);
            for refill in 0..ctx.test_objs.len().min(16) {
                let obj = ctx.test_objs[(refill + control.thread_index()) % ctx.test_objs.len()];
                let ancestors: Vec<Obj> =
                    (0..=((refill % 4) + 1)).map(|k| Obj::mk_id(k as i32)).collect();
                cache.fill(&obj, &ancestors);
            }
        } else {
            let obj_idx = (operations as usize + control.thread_index()) % ctx.test_objs.len();
            let ancestors: Vec<Obj> = (0..=((obj_idx % 4) + 1))
                .map(|k| Obj::mk_id(k as i32))
                .collect();
            cache.fill(&ctx.test_objs[obj_idx], &ancestors);
        }
        operations = operations.wrapping_add(1);
    }
    ConcurrentWorkerResult::operations(operations).with_counter("flushes", flushes)
}

// Realistic workload - matches real-world usage patterns
fn verb_cache_realistic_workload(
    ctx: &mut RealisticCacheContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        let obj_idx = i % ctx.test_objs.len();
        let verb_idx = i % ctx.test_verbs.len();

        match i % 200 {
            0..=199 => {
                // 99.5% lookups with hits (matches real hit rate)
                let result = ctx
                    .verb_cache
                    .lookup(&ctx.test_objs[obj_idx], &ctx.test_verbs[verb_idx]);
                black_box(result);
            }
            _ => {
                // 0.5% cache misses
                let new_obj = Obj::mk_id(10000 + (i as i32)); // Uncached object
                let result = ctx.verb_cache.lookup(&new_obj, &ctx.test_verbs[verb_idx]);
                black_box(result);
            }
        }
    }
}

fn ancestry_cache_realistic_workload(
    ctx: &mut RealisticCacheContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for i in 0..chunk_size {
        match i % 100 {
            0..=92 => {
                // 92.6% hit rate for ancestry cache
                let obj_idx = i % 87; // Only first 87 objects are cached
                let result = ctx.ancestry_cache.lookup(&ctx.test_objs[obj_idx]);
                black_box(result);
            }
            _ => {
                // 7.4% misses
                let uncached_obj_idx = 87 + (i % 33); // Objects beyond the cached range
                let result = ctx.ancestry_cache.lookup(&ctx.test_objs[uncached_obj_idx]);
                black_box(result);
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
                    ctx.test_verbdefs[verbdef_idx].as_resolved(),
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

benchmark_main!(
    BenchmarkMainOptions {
        filter_help: Some(
            "all, lookup, fill, flush, fork, ancestry, concurrent, mixed, realistic, or any benchmark name substring".to_string()
        ),
        ..BenchmarkMainOptions::default()
    },
    |runner| {
    runner.group::<PopulatedCacheContext>("Verb Cache Lookup Benchmarks", |g| {
        g.bench("verb_cache_lookup_hits", verb_cache_lookup_hits);
        g.bench("verb_cache_lookup_misses", verb_cache_lookup_misses);
        g.bench("verb_cache_lookup_cold", verb_cache_lookup_cold);
    });

    runner.group::<SmallCacheContext>("Verb Cache Fill Benchmarks", |g| {
        g.bench("verb_cache_fill_hits", verb_cache_fill_hits);
        g.bench("verb_cache_fill_misses", verb_cache_fill_misses);
    });

    runner.group::<SmallCacheContext>("Verb Cache Flush Benchmarks", |g| {
        g.bench("verb_cache_flush", verb_cache_flush);
    });

    runner.group::<SmallCacheContext>("Verb Cache Fork Benchmarks", |g| {
        g.bench("verb_cache_fork", verb_cache_fork);
    });

    runner.group::<PopulatedCacheContext>("Ancestry Cache Benchmarks", |g| {
        g.bench("ancestry_cache_lookup", ancestry_cache_lookup);
        g.bench("ancestry_cache_fill", ancestry_cache_fill);
    });

    runner.group::<ConcurrentCacheContext>("Concurrent Cache Benchmarks", |g| {
        g.bench("concurrent_cache_access", concurrent_cache_access);
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
                run: shared_verb_lookup_reader,
            },
            ConcurrentWorker {
                name: "mutator",
                threads: 1,
                run: shared_verb_mutator,
            },
        ];
        let ancestry_lookup_vs_invalidation = [
            ConcurrentWorker {
                name: "ancestry_reader",
                threads: reader_threads,
                run: ancestry_lookup_reader,
            },
            ConcurrentWorker {
                name: "ancestry_invalidator",
                threads: 1,
                run: ancestry_invalidator,
            },
        ];

        runner.concurrent_group::<SharedVerbCacheContext>("Verb Cache Concurrent Scenarios", |g| {
            g.bench(
                &format!("verb_cache_lookup_vs_mutation_{threads}t"),
                Duration::from_millis(100),
                &lookup_vs_mutation,
            );
            g.bench(
                &format!("ancestry_lookup_vs_invalidation_{threads}t"),
                Duration::from_millis(100),
                &ancestry_lookup_vs_invalidation,
            );
        });
    }

    runner.group::<LargeCacheContext>("Mixed Workload Benchmarks", |g| {
        g.bench("verb_cache_mixed_workload", verb_cache_mixed_workload);
    });

    runner.group::<RealisticCacheContext>("Realistic Workload Benchmarks", |g| {
        g.bench("verb_cache_realistic_workload", verb_cache_realistic_workload);
        g.bench("ancestry_cache_realistic_workload", ancestry_cache_realistic_workload);
    });
    }
);
