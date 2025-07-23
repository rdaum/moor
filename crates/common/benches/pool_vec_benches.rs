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

//! Benchmarks for SlabVec slab allocator performance

use moor_bench_utils::{BenchContext, black_box};
use moor_common::util::{PoolVec, TaskVecPool, VecPool};
use std::cell::RefCell;
use std::rc::Rc;

// === BASIC SLABVEC OPERATIONS ===

// Context for small SlabVec benchmarks (16 elements - smallest size class)
struct SmallSlabVecContext {
    pool: Rc<RefCell<VecPool<i64>>>,
    vec: PoolVec<i64>,
}

impl BenchContext for SmallSlabVecContext {
    fn prepare(_num_chunks: usize) -> Self {
        let pool = Rc::new(RefCell::new(VecPool::new()));
        let vec = PoolVec::new(pool.clone());
        Self { pool, vec }
    }
}

// Context for medium SlabVec benchmarks (32 elements)
struct MediumSlabVecContext {
    pool: Rc<RefCell<VecPool<i64>>>,
    vec: PoolVec<i64>,
}

impl BenchContext for MediumSlabVecContext {
    fn prepare(_num_chunks: usize) -> Self {
        let pool = Rc::new(RefCell::new(VecPool::new()));
        let vec = PoolVec::with_capacity(pool.clone(), 32);
        Self { pool, vec }
    }
}

// Context for large SlabVec benchmarks (128 elements - largest size class)
struct LargeSlabVecContext {
    pool: Rc<RefCell<VecPool<i64>>>,
    vec: PoolVec<i64>,
}

impl BenchContext for LargeSlabVecContext {
    fn prepare(_num_chunks: usize) -> Self {
        let pool = Rc::new(RefCell::new(VecPool::new()));
        let vec = PoolVec::with_capacity(pool.clone(), 128);
        Self { pool, vec }
    }
}

// Context for pool reuse benchmarks
struct PoolReuseContext {
    pool: Rc<RefCell<VecPool<i64>>>,
    vecs: Vec<PoolVec<i64>>,
}

impl BenchContext for PoolReuseContext {
    fn prepare(_num_chunks: usize) -> Self {
        let pool = Rc::new(RefCell::new(VecPool::new()));
        // Pre-create and drop some vectors to populate the pool's free list
        let mut vecs = Vec::new();
        for _ in 0..10 {
            let mut vec = PoolVec::with_capacity(pool.clone(), 64);
            for i in 0..32 {
                vec.push(i);
            }
            vecs.push(vec);
        }
        // Drop half of them to create free slots
        vecs.truncate(5);

        Self { pool, vecs }
    }
}

fn slabvec_push_small(ctx: &mut SmallSlabVecContext, chunk_size: usize, _chunk_num: usize) {
    let mut vec = PoolVec::new(ctx.pool.clone());
    for i in 0..chunk_size.min(16) {
        // Respect size class limit
        vec.push(i as i64);
        black_box(&vec);
    }
}

fn slabvec_push_medium(ctx: &mut MediumSlabVecContext, chunk_size: usize, _chunk_num: usize) {
    let mut vec = PoolVec::with_capacity(ctx.pool.clone(), 32);
    for i in 0..chunk_size.min(32) {
        vec.push(i as i64);
        black_box(&vec);
    }
}

fn slabvec_push_large(ctx: &mut LargeSlabVecContext, chunk_size: usize, _chunk_num: usize) {
    let mut vec = PoolVec::with_capacity(ctx.pool.clone(), 128);
    for i in 0..chunk_size.min(128) {
        vec.push(i as i64);
        black_box(&vec);
    }
}

fn slabvec_pop_small(ctx: &mut SmallSlabVecContext, chunk_size: usize, _chunk_num: usize) {
    // Pre-fill vector
    let mut vec = PoolVec::with_capacity(ctx.pool.clone(), 16);
    for i in 0..16 {
        vec.push(i);
    }

    for _ in 0..chunk_size.min(16) {
        if let Some(val) = vec.pop() {
            black_box(val);
        }
    }
}

fn slabvec_clone_small(ctx: &mut SmallSlabVecContext, chunk_size: usize, _chunk_num: usize) {
    // Pre-fill vector
    let mut vec = PoolVec::with_capacity(ctx.pool.clone(), 16);
    for i in 0..8 {
        vec.push(i);
    }

    for _ in 0..chunk_size {
        let cloned = vec.clone();
        black_box(cloned);
    }
}

fn slabvec_clone_large(ctx: &mut LargeSlabVecContext, chunk_size: usize, _chunk_num: usize) {
    // Pre-fill vector
    let mut vec = PoolVec::with_capacity(ctx.pool.clone(), 128);
    for i in 0..64 {
        vec.push(i);
    }

    for _ in 0..chunk_size {
        let cloned = vec.clone();
        black_box(cloned);
    }
}

fn slabvec_resize_grow(ctx: &mut MediumSlabVecContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let mut vec = PoolVec::with_capacity(ctx.pool.clone(), 32);
        let new_size = (i % 16) + 1; // Keep within reasonable bounds for medium size class
        vec.resize(new_size, i as i64);
        black_box(vec);
    }
}

fn pool_reuse_allocation(ctx: &mut PoolReuseContext, chunk_size: usize, _chunk_num: usize) {
    // Test rapid allocation/deallocation to measure pool reuse efficiency
    for i in 0..chunk_size {
        let mut vec = PoolVec::with_capacity(ctx.pool.clone(), 64);
        for j in 0..(i % 32 + 1) {
            vec.push(j as i64);
        }
        black_box(vec); // vec gets dropped here, returning buffer to pool
    }
}

// === COMPARISON WITH STANDARD VEC ===

// Context for Vec<i64> comparison benchmarks
struct StdVecContext {
    vecs: Vec<Vec<i64>>,
}

impl BenchContext for StdVecContext {
    fn prepare(_num_chunks: usize) -> Self {
        Self { vecs: Vec::new() }
    }
}

fn stdvec_push_small(_ctx: &mut StdVecContext, chunk_size: usize, _chunk_num: usize) {
    let mut vec = Vec::with_capacity(16);
    for i in 0..chunk_size.min(16) {
        vec.push(i as i64);
        black_box(&vec);
    }
}

fn stdvec_push_medium(_ctx: &mut StdVecContext, chunk_size: usize, _chunk_num: usize) {
    let mut vec = Vec::with_capacity(32);
    for i in 0..chunk_size.min(32) {
        vec.push(i as i64);
        black_box(&vec);
    }
}

fn stdvec_push_large(_ctx: &mut StdVecContext, chunk_size: usize, _chunk_num: usize) {
    let mut vec = Vec::with_capacity(128);
    for i in 0..chunk_size.min(128) {
        vec.push(i as i64);
        black_box(&vec);
    }
}

fn stdvec_clone_small(_ctx: &mut StdVecContext, chunk_size: usize, _chunk_num: usize) {
    let vec: Vec<i64> = (0..8).collect();
    for _ in 0..chunk_size {
        let cloned = vec.clone();
        black_box(cloned);
    }
}

fn stdvec_clone_large(_ctx: &mut StdVecContext, chunk_size: usize, _chunk_num: usize) {
    let vec: Vec<i64> = (0..64).collect();
    for _ in 0..chunk_size {
        let cloned = vec.clone();
        black_box(cloned);
    }
}

fn stdvec_rapid_alloc(_ctx: &mut StdVecContext, chunk_size: usize, _chunk_num: usize) {
    // Compare with SlabVec pool reuse - standard Vec has to allocate each time
    for i in 0..chunk_size {
        let mut vec = Vec::with_capacity(64);
        for j in 0..(i % 32 + 1) {
            vec.push(j as i64);
        }
        black_box(vec);
    }
}

// === TASKVECPOOL BENCHMARKS ===

struct TaskVecPoolContext {
    pool: TaskVecPool<i64>,
}

impl BenchContext for TaskVecPoolContext {
    fn prepare(_num_chunks: usize) -> Self {
        Self {
            pool: TaskVecPool::new(),
        }
    }
}

fn taskvecpool_create_destroy(ctx: &mut TaskVecPoolContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let mut vec = PoolVec::with_capacity(ctx.pool.inner().clone(), 64);
        for j in 0..(i % 32 + 1) {
            vec.push(j as i64);
        }
        black_box(vec);
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
        eprintln!("Running SlabVec benchmarks matching filter: '{f}'");
        eprintln!(
            "Available filters: all, small, medium, large, pool, comparison, or any benchmark name substring"
        );
        eprintln!();
    }

    // SlabVec operation benchmarks
    let small_slabvec_benchmarks = [
        BenchmarkDef {
            name: "slabvec_push_small",
            group: "small",
            func: slabvec_push_small,
            throughput_elements: None,
        },
        BenchmarkDef {
            name: "slabvec_pop_small",
            group: "small",
            func: slabvec_pop_small,
            throughput_elements: None,
        },
        BenchmarkDef {
            name: "slabvec_clone_small",
            group: "small",
            func: slabvec_clone_small,
            throughput_elements: None,
        },
    ];

    let medium_slabvec_benchmarks = [
        BenchmarkDef {
            name: "slabvec_push_medium",
            group: "medium",
            func: slabvec_push_medium,
            throughput_elements: None,
        },
        BenchmarkDef {
            name: "slabvec_resize_grow",
            group: "medium",
            func: slabvec_resize_grow,
            throughput_elements: None,
        },
    ];

    let large_slabvec_benchmarks = [
        BenchmarkDef {
            name: "slabvec_push_large",
            group: "large",
            func: slabvec_push_large,
            throughput_elements: None,
        },
        BenchmarkDef {
            name: "slabvec_clone_large",
            group: "large",
            func: slabvec_clone_large,
            throughput_elements: None,
        },
    ];

    let pool_benchmarks = [BenchmarkDef {
        name: "pool_reuse_allocation",
        group: "pool",
        func: pool_reuse_allocation,
        throughput_elements: None,
    }];

    // Standard Vec comparison benchmarks
    let comparison_benchmarks = [
        BenchmarkDef {
            name: "stdvec_push_small",
            group: "comparison",
            func: stdvec_push_small,
            throughput_elements: None,
        },
        BenchmarkDef {
            name: "stdvec_push_medium",
            group: "comparison",
            func: stdvec_push_medium,
            throughput_elements: None,
        },
        BenchmarkDef {
            name: "stdvec_push_large",
            group: "comparison",
            func: stdvec_push_large,
            throughput_elements: None,
        },
        BenchmarkDef {
            name: "stdvec_clone_small",
            group: "comparison",
            func: stdvec_clone_small,
            throughput_elements: None,
        },
        BenchmarkDef {
            name: "stdvec_clone_large",
            group: "comparison",
            func: stdvec_clone_large,
            throughput_elements: None,
        },
        BenchmarkDef {
            name: "stdvec_rapid_alloc",
            group: "comparison",
            func: stdvec_rapid_alloc,
            throughput_elements: None,
        },
    ];

    // Run benchmark groups
    run_benchmark_group::<SmallSlabVecContext>(
        &small_slabvec_benchmarks,
        "SlabVec Small (16 elements)",
        filter,
    );
    run_benchmark_group::<MediumSlabVecContext>(
        &medium_slabvec_benchmarks,
        "SlabVec Medium (32 elements)",
        filter,
    );
    run_benchmark_group::<LargeSlabVecContext>(
        &large_slabvec_benchmarks,
        "SlabVec Large (128 elements)",
        filter,
    );
    run_benchmark_group::<PoolReuseContext>(&pool_benchmarks, "SlabVec Pool Reuse", filter);
    run_benchmark_group::<StdVecContext>(&comparison_benchmarks, "Standard Vec Comparison", filter);

    let taskvec_pool_benchmarks = [BenchmarkDef {
        name: "taskvecpool_create_destroy",
        group: "pool",
        func: taskvecpool_create_destroy,
        throughput_elements: None,
    }];
    run_benchmark_group::<TaskVecPoolContext>(
        &taskvec_pool_benchmarks,
        "TaskVecPool Operations",
        filter,
    );

    if filter.is_some() {
        eprintln!("\nSlabVec benchmark filtering complete.");
    }

    generate_session_summary();
}
