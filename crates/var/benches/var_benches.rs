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

use moor_bench_utils::{BenchContext, NoContext, black_box};
use moor_var::{IndexMode, Var, v_bool, v_float, v_int, v_list, v_none, v_str};

// Context for integer benchmarks
struct IntContext(Var);
impl BenchContext for IntContext {
    fn prepare(_num_chunks: usize) -> Self {
        IntContext(v_int(0))
    }
}

// Context for small list benchmarks
struct SmallListContext(Var);
impl BenchContext for SmallListContext {
    fn prepare(_num_chunks: usize) -> Self {
        SmallListContext(v_list(&(0..8).map(v_int).collect::<Vec<_>>()))
    }
}

// Context for large list benchmarks
struct LargeListContext(Var);
impl BenchContext for LargeListContext {
    fn prepare(_num_chunks: usize) -> Self {
        LargeListContext(v_list(&(0..100_000).map(v_int).collect::<Vec<_>>()))
    }
}

fn int_add(ctx: &mut IntContext, chunk_size: usize, _chunk_num: usize) {
    let mut v = ctx.0.clone();
    for _ in 0..chunk_size {
        v = v.add(&v_int(1)).unwrap();
    }
}

fn int_eq(ctx: &mut IntContext, chunk_size: usize, _chunk_num: usize) {
    let v = ctx.0.clone();
    for _ in 0..chunk_size {
        let _ = v.eq(&v);
    }
}

fn int_cmp(ctx: &mut IntContext, chunk_size: usize, _chunk_num: usize) {
    let v = ctx.0.clone();
    for _ in 0..chunk_size {
        let _ = v.cmp(&v);
    }
}

fn list_push(ctx: &mut SmallListContext, chunk_size: usize, _chunk_num: usize) {
    let mut v = ctx.0.clone();
    for _ in 0..chunk_size {
        v = v.push(&v_int(1)).unwrap();
    }
}

fn list_index_pos(ctx: &mut LargeListContext, chunk_size: usize, _chunk_num: usize) {
    let v = ctx.0.clone();
    let list_len = 100_000; // LargeListContext has 100k items
    for c in 0..chunk_size {
        let index = c % list_len; // Cycle through available indices
        let _ = v.index(&v_int(index as i64), IndexMode::ZeroBased).unwrap();
    }
}

fn list_index_assign(ctx: &mut LargeListContext, chunk_size: usize, _chunk_num: usize) {
    let mut v = ctx.0.clone();
    let list_len = 100_000; // LargeListContext has 100k items
    for c in 0..chunk_size {
        let index = c % list_len; // Cycle through available indices
        v = v
            .index_set(
                &v_int(index as i64),
                &v_int(index as i64),
                IndexMode::ZeroBased,
            )
            .unwrap();
    }
}

// === VAR CONSTRUCTION BENCHMARKS ===
// These measure the cost of creating different types of Vars

fn var_construct_ints(_ctx: &mut NoContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let var = v_int(i as i64);
        black_box(var);
    }
}

fn var_construct_strings(_ctx: &mut NoContext, chunk_size: usize, chunk_num: usize) {
    for i in 0..chunk_size {
        let s = format!("string_{chunk_num}_{i})");
        let var = v_str(&s);
        black_box(var);
    }
}

fn var_construct_small_lists(_ctx: &mut NoContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let var = v_list(&[v_int(i as i64), v_int((i + 1) as i64)]);
        black_box(var);
    }
}

fn var_construct_nested_lists(_ctx: &mut NoContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let inner = v_list(&[v_int(i as i64), v_str("nested")]);
        let var = v_list(&[inner, v_int((i + 1) as i64)]);
        black_box(var);
    }
}

// CLONE BENCHMARKS ===
// These measure cloning costs which are relevant for scope operations

// Context for int clone benchmarks (simple Copy-like types)
struct IntCloneContext(Var, Var);
impl BenchContext for IntCloneContext {
    fn prepare(_num_chunks: usize) -> Self {
        IntCloneContext(v_int(42), v_none())
    }
}

// Context for string clone benchmarks
struct StringCloneContext(Var, Var);
impl BenchContext for StringCloneContext {
    fn prepare(_num_chunks: usize) -> Self {
        StringCloneContext(v_str("test_string_for_cloning"), v_none())
    }
}

// Context for list clone benchmarks
struct ListCloneContext(Var, Var);
impl BenchContext for ListCloneContext {
    fn prepare(_num_chunks: usize) -> Self {
        ListCloneContext(
            v_list(&[v_int(1), v_str("test"), v_int(2), v_str("clone")]),
            v_none(),
        )
    }
}

fn var_clone_ints(ctx: &mut IntCloneContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        ctx.1 = ctx.0.clone();
    }
}

fn var_clone_strings(ctx: &mut StringCloneContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        ctx.1 = ctx.0.clone();
    }
}

fn var_clone_lists(ctx: &mut ListCloneContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        ctx.1 = ctx.0.clone();
    }
}

// === AS_INTEGER BENCHMARKS ===

fn var_as_integer(ctx: &mut IntCloneContext, chunk_size: usize, _chunk_num: usize) {
    let v = &ctx.0;
    for _ in 0..chunk_size {
        let _ = black_box(v.as_integer());
    }
}

// Context that pre-creates pools of Vars to drop
struct DropContext {
    int_vars: Vec<Var>,
    string_vars: Vec<Var>,
    list_vars: Vec<Var>,
    mixed_vars: Vec<Var>,
}

impl BenchContext for DropContext {
    fn prepare(_num_chunks: usize) -> Self {
        let pool_size = 100_000; // Pool size matches our preferred chunk size
        DropContext {
            int_vars: (0..pool_size).map(|i| v_int(i as i64)).collect(),
            string_vars: (0..pool_size).map(|i| v_str(&format!("str_{i}"))).collect(),
            list_vars: (0..pool_size)
                .map(|i| v_list(&[v_int(i as i64), v_str("item")]))
                .collect(),
            mixed_vars: (0..pool_size)
                .map(|i| match i % 5 {
                    0 => v_int(i as i64),
                    1 => v_str(&format!("str_{i}")),
                    2 => v_list(&[v_int(i as i64)]),
                    3 => v_float(i as f64),
                    _ => v_bool(i % 2 == 0),
                })
                .collect(),
        }
    }

    fn chunk_size() -> Option<usize> {
        Some(50_000) // Use half the pool size so we can do multiple samples
    }
}

fn var_drop_ints(ctx: &mut DropContext, chunk_size: usize, _chunk_num: usize) {
    // Drop exactly chunk_size items from the pool
    ctx.int_vars.truncate(ctx.int_vars.len() - chunk_size);
}

fn var_drop_strings(ctx: &mut DropContext, chunk_size: usize, _chunk_num: usize) {
    // Drop exactly chunk_size items from the pool
    ctx.string_vars.truncate(ctx.string_vars.len() - chunk_size);
}

fn var_drop_lists(ctx: &mut DropContext, chunk_size: usize, _chunk_num: usize) {
    // Drop exactly chunk_size items from the pool
    ctx.list_vars.truncate(ctx.list_vars.len() - chunk_size);
}

fn var_drop_mixed(ctx: &mut DropContext, chunk_size: usize, _chunk_num: usize) {
    // Drop exactly chunk_size items from the pool
    ctx.mixed_vars.truncate(ctx.mixed_vars.len() - chunk_size);
}

pub fn main() {
    use moor_bench_utils::{
        BenchmarkDef, NoContext, generate_session_summary, run_benchmark_group,
    };
    use std::env;

    #[cfg(target_os = "linux")]
    {
        use moor_bench_utils::perf_event::{Builder, events::Hardware};
        // Check if we can do perf events, and if not warn but continue with timing-only benchmarks
        if Builder::new(Hardware::INSTRUCTIONS).build().is_err() {
            eprintln!(
                "⚠️  Perf events are not available on this system (insufficient permissions or kernel support)."
            );
            eprintln!("   Continuing with timing-only benchmarks (performance counters disabled).");
            eprintln!();
        }
    }

    let args: Vec<String> = env::args().collect();
    // Look for filter arguments after "--"
    let filter = if let Some(separator_pos) = args.iter().position(|arg| arg == "--") {
        // Filter is the first argument after "--"
        args.get(separator_pos + 1).map(|s| s.as_str())
    } else {
        // Fallback: look for any non-flag argument that's not our binary name
        args.iter()
            .skip(1)
            .find(|arg| !arg.starts_with("--") && !args[0].contains(arg.as_str()))
            .map(|s| s.as_str())
    };

    if let Some(f) = filter {
        eprintln!("Running benchmarks matching filter: '{f}'");
        eprintln!(
            "Available filters: all, int, list, scope, construct, drop, clone, or any benchmark name substring"
        );
        eprintln!();
    }

    // Define all benchmark groups declaratively
    let int_benchmarks = [
        BenchmarkDef {
            name: "int_add",
            group: "int",
            func: int_add,
        },
        BenchmarkDef {
            name: "int_eq",
            group: "int",
            func: int_eq,
        },
        BenchmarkDef {
            name: "int_cmp",
            group: "int",
            func: int_cmp,
        },
    ];

    let small_list_benchmarks = [BenchmarkDef {
        name: "list_push",
        group: "list",
        func: list_push,
    }];

    let large_list_benchmarks = [
        BenchmarkDef {
            name: "list_index_pos",
            group: "list",
            func: list_index_pos,
        },
        BenchmarkDef {
            name: "list_index_assign",
            group: "list",
            func: list_index_assign,
        },
    ];

    let construct_benchmarks = [
        BenchmarkDef {
            name: "var_construct_ints",
            group: "construct",
            func: var_construct_ints,
        },
        BenchmarkDef {
            name: "var_construct_strings",
            group: "construct",
            func: var_construct_strings,
        },
        BenchmarkDef {
            name: "var_construct_small_lists",
            group: "construct",
            func: var_construct_small_lists,
        },
        BenchmarkDef {
            name: "var_construct_nested_lists",
            group: "construct",
            func: var_construct_nested_lists,
        },
    ];

    let drop_benchmarks = [
        BenchmarkDef {
            name: "var_drop_ints",
            group: "drop",
            func: var_drop_ints,
        },
        BenchmarkDef {
            name: "var_drop_strings",
            group: "drop",
            func: var_drop_strings,
        },
        BenchmarkDef {
            name: "var_drop_lists",
            group: "drop",
            func: var_drop_lists,
        },
        BenchmarkDef {
            name: "var_drop_mixed",
            group: "drop",
            func: var_drop_mixed,
        },
    ];

    let clone_int_benchmarks = [BenchmarkDef {
        name: "var_clone_ints",
        group: "clone",
        func: var_clone_ints,
    }];

    let clone_string_benchmarks = [BenchmarkDef {
        name: "var_clone_strings",
        group: "clone",
        func: var_clone_strings,
    }];

    let clone_list_benchmarks = [BenchmarkDef {
        name: "var_clone_lists",
        group: "clone",
        func: var_clone_lists,
    }];

    // Run benchmark groups
    run_benchmark_group::<IntContext>(&int_benchmarks, "Integer Operations", filter);
    run_benchmark_group::<SmallListContext>(
        &small_list_benchmarks,
        "Small List Operations",
        filter,
    );
    run_benchmark_group::<LargeListContext>(
        &large_list_benchmarks,
        "Large List Operations",
        filter,
    );
    run_benchmark_group::<NoContext>(&construct_benchmarks, "Var Construction Benchmarks", filter);
    run_benchmark_group::<DropContext>(&drop_benchmarks, "Var Drop Benchmarks", filter);

    run_benchmark_group::<IntCloneContext>(
        &clone_int_benchmarks,
        "Var Clone (Int) Benchmarks",
        filter,
    );
    run_benchmark_group::<StringCloneContext>(
        &clone_string_benchmarks,
        "Var Clone (String) Benchmarks",
        filter,
    );
    run_benchmark_group::<ListCloneContext>(
        &clone_list_benchmarks,
        "Var Clone (List) Benchmarks",
        filter,
    );

    // as_integer() benchmarks
    let var_as_int_benchmarks = [BenchmarkDef {
        name: "var_as_integer",
        group: "accessor",
        func: var_as_integer,
    }];
    run_benchmark_group::<IntCloneContext>(
        &var_as_int_benchmarks,
        "Var.as_integer() Benchmarks",
        filter,
    );

    if filter.is_some() {
        eprintln!("\nBenchmark filtering complete.");
    }

    // Generate session summary with regression analysis
    generate_session_summary();
}
