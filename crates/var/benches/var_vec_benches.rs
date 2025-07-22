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

//! Benchmarks comparing Vec<Var> vs SlabVec<Var> performance
//! These benchmarks measure the specific performance characteristics that matter for MOO VM execution

use moor_bench_utils::{BenchContext, black_box};
use moor_common::util::{SlabVec, TaskVecPool};
use moor_var::{Var, v_int, v_str, v_list, v_none, v_float};

// === VAR VECTOR CREATION BENCHMARKS ===
// These measure the primary bottleneck: creating vectors of Vars in MooStackFrame

struct VarVecCreationContext {
    var_pool: TaskVecPool<Var>,
    var_option_pool: TaskVecPool<Option<Var>>,
    sample_vars: Vec<Var>,
    sample_option_vars: Vec<Option<Var>>,
}

impl BenchContext for VarVecCreationContext {
    fn prepare(_num_chunks: usize) -> Self {
        let sample_vars = vec![
            v_int(42), v_str("test"), v_float(3.14), v_none(),
            v_list(&[v_int(1), v_int(2)]), v_int(0), v_str("hello"),
        ];
        let sample_option_vars = vec![
            Some(v_int(42)), Some(v_str("test")), None, Some(v_float(3.14)), 
            None, Some(v_none()), Some(v_list(&[v_int(1), v_int(2)])),
        ];
        
        Self {
            var_pool: TaskVecPool::new(),
            var_option_pool: TaskVecPool::new(),
            sample_vars,
            sample_option_vars,
        }
    }
    
    fn chunk_size() -> Option<usize> {
        Some(10000) // Smaller chunks for more precise measurements
    }
}

// Standard Vec<Var> creation (original implementation)
fn stdvec_var_creation(ctx: &mut VarVecCreationContext, chunk_size: usize, _chunk_num: usize) {
    for _i in 0..chunk_size {
        let mut vec: Vec<Var> = Vec::with_capacity(64); // Typical global var count
        // Simulate MooStackFrame initialization
        for j in 0..64 {
            vec.push(ctx.sample_vars[j % ctx.sample_vars.len()].clone());
        }
        black_box(vec);
    }
}

// SlabVec<Var> creation (new implementation)
fn slabvec_var_creation(ctx: &mut VarVecCreationContext, chunk_size: usize, _chunk_num: usize) {
    for _i in 0..chunk_size {
        let mut vec = SlabVec::with_capacity(ctx.var_pool.inner().clone(), 64);
        // Simulate MooStackFrame initialization  
        for j in 0..64 {
            vec.push(ctx.sample_vars[j % ctx.sample_vars.len()].clone());
        }
        black_box(vec);
    }
}

// Standard Vec<Option<Var>> creation (environment variables)
fn stdvec_var_option_creation(ctx: &mut VarVecCreationContext, chunk_size: usize, _chunk_num: usize) {
    for _i in 0..chunk_size {
        let mut vec: Vec<Option<Var>> = Vec::with_capacity(64);
        for j in 0..64 {
            vec.push(ctx.sample_option_vars[j % ctx.sample_option_vars.len()].clone());
        }
        black_box(vec);
    }
}

// SlabVec<Option<Var>> creation (environment variables)
fn slabvec_var_option_creation(ctx: &mut VarVecCreationContext, chunk_size: usize, _chunk_num: usize) {
    for _i in 0..chunk_size {
        let mut vec = SlabVec::with_capacity(ctx.var_option_pool.inner().clone(), 64);
        for j in 0..64 {
            vec.push(ctx.sample_option_vars[j % ctx.sample_option_vars.len()].clone());
        }
        black_box(vec);
    }
}

// === VAR VECTOR DESTRUCTION BENCHMARKS ===
// These measure the original bottleneck: dropping large numbers of Var vectors

struct VarVecDestructionContext {
    var_pool: TaskVecPool<Var>,
    var_option_pool: TaskVecPool<Option<Var>>,
    // Pre-created vectors for destruction testing
    std_var_vecs: Vec<Vec<Var>>,
    slab_var_vecs: Vec<SlabVec<Var>>,
    std_option_vecs: Vec<Vec<Option<Var>>>,
    slab_option_vecs: Vec<SlabVec<Option<Var>>>,
}

impl BenchContext for VarVecDestructionContext {
    fn prepare(_num_chunks: usize) -> Self {
        let sample_vars = vec![v_int(42), v_str("test"), v_float(3.14), v_none()];
        let sample_options = vec![Some(v_int(42)), None, Some(v_str("test")), None];
        
        let var_pool = TaskVecPool::new();
        let var_option_pool = TaskVecPool::new();
        
        // Pre-create vectors for destruction
        let pool_size = 50000;
        let mut std_var_vecs = Vec::with_capacity(pool_size);
        let mut slab_var_vecs = Vec::with_capacity(pool_size);
        let mut std_option_vecs = Vec::with_capacity(pool_size);
        let mut slab_option_vecs = Vec::with_capacity(pool_size);
        
        for _i in 0..pool_size {
            // Standard Vec<Var>
            let mut std_vec = Vec::with_capacity(64);
            for j in 0..64 {
                std_vec.push(sample_vars[j % sample_vars.len()].clone());
            }
            std_var_vecs.push(std_vec);
            
            // SlabVec<Var>
            let mut slab_vec = SlabVec::with_capacity(var_pool.inner().clone(), 64);
            for j in 0..64 {
                slab_vec.push(sample_vars[j % sample_vars.len()].clone());
            }
            slab_var_vecs.push(slab_vec);
            
            // Standard Vec<Option<Var>>
            let mut std_opt_vec = Vec::with_capacity(64);
            for j in 0..64 {
                std_opt_vec.push(sample_options[j % sample_options.len()].clone());
            }
            std_option_vecs.push(std_opt_vec);
            
            // SlabVec<Option<Var>>
            let mut slab_opt_vec = SlabVec::with_capacity(var_option_pool.inner().clone(), 64);
            for j in 0..64 {
                slab_opt_vec.push(sample_options[j % sample_options.len()].clone());
            }
            slab_option_vecs.push(slab_opt_vec);
        }
        
        Self {
            var_pool,
            var_option_pool,
            std_var_vecs,
            slab_var_vecs,
            std_option_vecs,
            slab_option_vecs,
        }
    }
    
    fn chunk_size() -> Option<usize> {
        Some(25000) // Use half the pool size for multiple samples
    }
}

// Destruction benchmarks - these measure the original performance bottleneck
fn stdvec_var_destruction(ctx: &mut VarVecDestructionContext, chunk_size: usize, _chunk_num: usize) {
    // Drop exactly chunk_size Vec<Var> instances
    ctx.std_var_vecs.truncate(ctx.std_var_vecs.len().saturating_sub(chunk_size));
}

fn slabvec_var_destruction(ctx: &mut VarVecDestructionContext, chunk_size: usize, _chunk_num: usize) {
    // Drop exactly chunk_size SlabVec<Var> instances
    ctx.slab_var_vecs.truncate(ctx.slab_var_vecs.len().saturating_sub(chunk_size));
}

fn stdvec_var_option_destruction(ctx: &mut VarVecDestructionContext, chunk_size: usize, _chunk_num: usize) {
    // Drop exactly chunk_size Vec<Option<Var>> instances
    ctx.std_option_vecs.truncate(ctx.std_option_vecs.len().saturating_sub(chunk_size));
}

fn slabvec_var_option_destruction(ctx: &mut VarVecDestructionContext, chunk_size: usize, _chunk_num: usize) {
    // Drop exactly chunk_size SlabVec<Option<Var>> instances
    ctx.slab_option_vecs.truncate(ctx.slab_option_vecs.len().saturating_sub(chunk_size));
}

// === VAR VECTOR CLONING BENCHMARKS ===
// Task activation cloning was part of the memory leak

struct VarVecCloningContext {
    var_pool: TaskVecPool<Var>,
    var_option_pool: TaskVecPool<Option<Var>>,
    std_var_vec: Vec<Var>,
    slab_var_vec: SlabVec<Var>,
    std_option_vec: Vec<Option<Var>>,
    slab_option_vec: SlabVec<Option<Var>>,
}

impl BenchContext for VarVecCloningContext {
    fn prepare(_num_chunks: usize) -> Self {
        let sample_vars = vec![v_int(42), v_str("test"), v_float(3.14), v_none()];
        let sample_options = vec![Some(v_int(42)), None, Some(v_str("test")), None];
        
        let var_pool = TaskVecPool::new();
        let var_option_pool = TaskVecPool::new();
        
        // Create vectors to clone
        let mut std_var_vec = Vec::with_capacity(64);
        let mut slab_var_vec = SlabVec::with_capacity(var_pool.inner().clone(), 64);
        let mut std_option_vec = Vec::with_capacity(64);
        let mut slab_option_vec = SlabVec::with_capacity(var_option_pool.inner().clone(), 64);
        
        for i in 0..64 {
            std_var_vec.push(sample_vars[i % sample_vars.len()].clone());
            slab_var_vec.push(sample_vars[i % sample_vars.len()].clone());
            std_option_vec.push(sample_options[i % sample_options.len()].clone());
            slab_option_vec.push(sample_options[i % sample_options.len()].clone());
        }
        
        Self {
            var_pool,
            var_option_pool,
            std_var_vec,
            slab_var_vec,
            std_option_vec,
            slab_option_vec,
        }
    }
}

fn stdvec_var_cloning(ctx: &mut VarVecCloningContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let cloned = ctx.std_var_vec.clone();
        black_box(cloned);
    }
}

fn slabvec_var_cloning(ctx: &mut VarVecCloningContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let cloned = ctx.slab_var_vec.clone();
        black_box(cloned);
    }
}

fn stdvec_var_option_cloning(ctx: &mut VarVecCloningContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let cloned = ctx.std_option_vec.clone();
        black_box(cloned);
    }
}

fn slabvec_var_option_cloning(ctx: &mut VarVecCloningContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let cloned = ctx.slab_option_vec.clone();
        black_box(cloned);
    }
}

// === PUSH/POP OPERATION BENCHMARKS ===
// VM execution involves lots of stack operations

struct VarVecOperationsContext {
    var_pool: TaskVecPool<Var>,
    sample_vars: Vec<Var>,
}

impl BenchContext for VarVecOperationsContext {
    fn prepare(_num_chunks: usize) -> Self {
        Self {
            var_pool: TaskVecPool::new(),
            sample_vars: vec![v_int(1), v_int(2), v_int(3), v_int(4), v_int(5)],
        }
    }
}

fn stdvec_var_push_pop(ctx: &mut VarVecOperationsContext, chunk_size: usize, _chunk_num: usize) {
    let mut vec: Vec<Var> = Vec::with_capacity(64);
    
    // Interleaved push/pop operations
    for i in 0..chunk_size {
        vec.push(ctx.sample_vars[i % ctx.sample_vars.len()].clone());
        if i % 10 == 9 && !vec.is_empty() {
            vec.pop();
        }
        black_box(&vec);
    }
}

fn slabvec_var_push_pop(ctx: &mut VarVecOperationsContext, chunk_size: usize, _chunk_num: usize) {
    let mut vec = SlabVec::with_capacity(ctx.var_pool.inner().clone(), 64);
    
    // Interleaved push/pop operations  
    for i in 0..chunk_size.min(64) { // Respect capacity limit
        vec.push(ctx.sample_vars[i % ctx.sample_vars.len()].clone());
        if i % 10 == 9 && !vec.is_empty() {
            vec.pop();
        }
        black_box(&vec);
    }
}

pub fn main() {
    use moor_bench_utils::{
        BenchmarkDef, generate_session_summary, run_benchmark_group,
    };
    use std::env;

    #[cfg(target_os = "linux")]
    {
        use moor_bench_utils::perf_event::{Builder, events::Hardware};
        if Builder::new(Hardware::INSTRUCTIONS).build().is_err() {
            eprintln!("⚠️  Perf events are not available on this system (insufficient permissions or kernel support).");
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
        eprintln!("Running Vec<Var> vs SlabVec<Var> benchmarks matching filter: '{f}'");
        eprintln!("Available filters: all, creation, destruction, cloning, operations, or any benchmark name substring");
        eprintln!();
    }

    // Vector creation benchmarks - each operation creates a vector with 64 elements
    let creation_benchmarks = [
        BenchmarkDef { name: "stdvec_var_creation", group: "creation", func: stdvec_var_creation, throughput_elements: Some(64) },
        BenchmarkDef { name: "slabvec_var_creation", group: "creation", func: slabvec_var_creation, throughput_elements: Some(64) },
        BenchmarkDef { name: "stdvec_var_option_creation", group: "creation", func: stdvec_var_option_creation, throughput_elements: Some(64) },
        BenchmarkDef { name: "slabvec_var_option_creation", group: "creation", func: slabvec_var_option_creation, throughput_elements: Some(64) },
    ];

    // Vector destruction benchmarks (original bottleneck) - each operation destroys a vector with 64 elements
    let destruction_benchmarks = [
        BenchmarkDef { name: "stdvec_var_destruction", group: "destruction", func: stdvec_var_destruction, throughput_elements: Some(64) },
        BenchmarkDef { name: "slabvec_var_destruction", group: "destruction", func: slabvec_var_destruction, throughput_elements: Some(64) },
        BenchmarkDef { name: "stdvec_var_option_destruction", group: "destruction", func: stdvec_var_option_destruction, throughput_elements: Some(64) },
        BenchmarkDef { name: "slabvec_var_option_destruction", group: "destruction", func: slabvec_var_option_destruction, throughput_elements: Some(64) },
    ];

    // Vector cloning benchmarks - each operation clones a vector with 64 elements  
    let cloning_benchmarks = [
        BenchmarkDef { name: "stdvec_var_cloning", group: "cloning", func: stdvec_var_cloning, throughput_elements: Some(64) },
        BenchmarkDef { name: "slabvec_var_cloning", group: "cloning", func: slabvec_var_cloning, throughput_elements: Some(64) },
        BenchmarkDef { name: "stdvec_var_option_cloning", group: "cloning", func: stdvec_var_option_cloning, throughput_elements: Some(64) },
        BenchmarkDef { name: "slabvec_var_option_cloning", group: "cloning", func: slabvec_var_option_cloning, throughput_elements: Some(64) },
    ];

    // Vector operations benchmarks - these are per-operation, not per-element
    let operations_benchmarks = [
        BenchmarkDef { name: "stdvec_var_push_pop", group: "operations", func: stdvec_var_push_pop, throughput_elements: None },
        BenchmarkDef { name: "slabvec_var_push_pop", group: "operations", func: slabvec_var_push_pop, throughput_elements: None },
    ];

    // Run benchmark groups
    run_benchmark_group::<VarVecCreationContext>(&creation_benchmarks, "Vec<Var> vs SlabVec<Var> Creation", filter);
    run_benchmark_group::<VarVecDestructionContext>(&destruction_benchmarks, "Vec<Var> vs SlabVec<Var> Destruction", filter);
    run_benchmark_group::<VarVecCloningContext>(&cloning_benchmarks, "Vec<Var> vs SlabVec<Var> Cloning", filter);
    run_benchmark_group::<VarVecOperationsContext>(&operations_benchmarks, "Vec<Var> vs SlabVec<Var> Operations", filter);

    if filter.is_some() {
        eprintln!("\nVec<Var> vs SlabVec<Var> benchmark filtering complete.");
    }

    eprintln!("\n🎯 Key Metrics to Compare:");
    eprintln!("  • Creation: SlabVec should show reduced allocation overhead");
    eprintln!("  • Destruction: SlabVec should show significant improvement (original bottleneck)");
    eprintln!("  • Cloning: SlabVec should maintain competitive performance");
    eprintln!("  • Operations: SlabVec should be comparable for VM stack operations");

    generate_session_summary();
}