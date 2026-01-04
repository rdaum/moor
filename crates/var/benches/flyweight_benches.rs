// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use moor_bench_utils::{black_box, BenchContext};
use moor_var::{v_int, Flyweight, List, Obj, Symbol};

// Context for flyweight benchmarks
struct FlyweightContext {
    fw: Flyweight,
    lookup_sym: Symbol,
    add_sym: Symbol,
}

impl BenchContext for FlyweightContext {
    fn prepare(_num_chunks: usize) -> Self {
        // Create a flyweight with a reasonable number of slots
        let mut slots = Vec::new();
        for i in 0..10 {
            slots.push((Symbol::mk(&format!("slot_{}", i)), v_int(i as i64)));
        }
        let lookup_sym = Symbol::mk("slot_5");
        let add_sym = Symbol::mk("new_slot");

        let fw = Flyweight::mk_flyweight(
            Obj::mk_id(123),
            &slots,
            List::mk_list(&[v_int(1), v_int(2), v_int(3)]),
        );

        FlyweightContext {
            fw,
            lookup_sym,
            add_sym,
        }
    }
}

fn flyweight_clone(ctx: &mut FlyweightContext, chunk_size: usize, _chunk_num: usize) {
    let fw = &ctx.fw;
    for _ in 0..chunk_size {
        let _ = black_box(fw.clone());
    }
}

fn flyweight_get_slot(ctx: &mut FlyweightContext, chunk_size: usize, _chunk_num: usize) {
    let fw = &ctx.fw;
    let sym = &ctx.lookup_sym;
    for _ in 0..chunk_size {
        let _ = black_box(fw.get_slot(sym));
    }
}

fn flyweight_slots_vec(ctx: &mut FlyweightContext, chunk_size: usize, _chunk_num: usize) {
    let fw = &ctx.fw;
    for _ in 0..chunk_size {
        let _ = black_box(fw.slots());
    }
}

fn flyweight_get_contents(ctx: &mut FlyweightContext, chunk_size: usize, _chunk_num: usize) {
    let fw = &ctx.fw;
    for _ in 0..chunk_size {
        let _ = black_box(fw.contents());
    }
}

fn flyweight_get_delegate(ctx: &mut FlyweightContext, chunk_size: usize, _chunk_num: usize) {
    let fw = &ctx.fw;
    for _ in 0..chunk_size {
        let _ = black_box(fw.delegate());
    }
}

fn flyweight_add_slot(ctx: &mut FlyweightContext, chunk_size: usize, _chunk_num: usize) {
    let fw = &ctx.fw;
    let sym = ctx.add_sym;
    let val = v_int(999);
    for _ in 0..chunk_size {
        let _ = black_box(fw.add_slot(sym, val.clone()));
    }
}

fn flyweight_remove_slot(ctx: &mut FlyweightContext, chunk_size: usize, _chunk_num: usize) {
    let fw = &ctx.fw;
    let sym = ctx.lookup_sym; // Remove an existing slot
    for _ in 0..chunk_size {
        let _ = black_box(fw.remove_slot(sym));
    }
}

pub fn main() {
    use moor_bench_utils::{generate_session_summary, run_benchmark_group, BenchmarkDef};
    use std::env;

    #[cfg(target_os = "linux")]
    {
        use moor_bench_utils::perf_event::{events::Hardware, Builder};
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
        eprintln!("Running benchmarks matching filter: '{f}'");
    }

    let flyweight_benchmarks = [
        BenchmarkDef {
            name: "flyweight_clone",
            group: "flyweight",
            func: flyweight_clone,
        },
        BenchmarkDef {
            name: "flyweight_get_slot",
            group: "flyweight",
            func: flyweight_get_slot,
        },
        BenchmarkDef {
            name: "flyweight_slots_vec",
            group: "flyweight",
            func: flyweight_slots_vec,
        },
        BenchmarkDef {
            name: "flyweight_get_contents",
            group: "flyweight",
            func: flyweight_get_contents,
        },
        BenchmarkDef {
            name: "flyweight_get_delegate",
            group: "flyweight",
            func: flyweight_get_delegate,
        },
        BenchmarkDef {
            name: "flyweight_add_slot",
            group: "flyweight",
            func: flyweight_add_slot,
        },
        BenchmarkDef {
            name: "flyweight_remove_slot",
            group: "flyweight",
            func: flyweight_remove_slot,
        },
    ];

    run_benchmark_group::<FlyweightContext>(&flyweight_benchmarks, "Flyweight Operations", filter);

    if filter.is_some() {
        eprintln!("\nBenchmark filtering complete.");
    }

    generate_session_summary();
}
