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

use moor_bench_utils::{BenchContext, black_box};
use moor_var::{IndexMode, Symbol, Var, v_int, v_sym};

const BASE_MAP_SIZE: usize = 4096;
const WORKING_KEY_SET_SIZE: usize = 4096;

struct MapContext {
    base_map: Var,
    existing_keys: Vec<Var>,
    insert_keys: Vec<Var>,
    missing_keys: Vec<Var>,
    update_value: Var,
}

impl BenchContext for MapContext {
    fn prepare(_num_chunks: usize) -> Self {
        let mut pairs = Vec::with_capacity(BASE_MAP_SIZE);
        let mut existing_keys = Vec::with_capacity(WORKING_KEY_SET_SIZE);
        let mut insert_keys = Vec::with_capacity(WORKING_KEY_SET_SIZE);
        let mut missing_keys = Vec::with_capacity(WORKING_KEY_SET_SIZE);

        for i in 0..BASE_MAP_SIZE {
            let sym = Symbol::mk(&format!("k_existing_{i}"));
            let key = v_sym(sym);
            pairs.push((key.clone(), v_int(i as i64)));
            if i < WORKING_KEY_SET_SIZE {
                existing_keys.push(key);
            }
        }

        for i in 0..WORKING_KEY_SET_SIZE {
            insert_keys.push(v_sym(Symbol::mk(&format!("k_insert_{i}"))));
            missing_keys.push(v_sym(Symbol::mk(&format!("k_missing_{i}"))));
        }

        Self {
            base_map: Var::mk_map(&pairs),
            existing_keys,
            insert_keys,
            missing_keys,
            update_value: v_int(42),
        }
    }
}

fn map_get_hit(ctx: &mut MapContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let key = &ctx.existing_keys[i & (WORKING_KEY_SET_SIZE - 1)];
        let value = ctx.base_map.get(key, IndexMode::ZeroBased).unwrap();
        black_box(value);
    }
}

fn map_get_miss(ctx: &mut MapContext, chunk_size: usize, _chunk_num: usize) {
    for i in 0..chunk_size {
        let key = &ctx.missing_keys[i & (WORKING_KEY_SET_SIZE - 1)];
        let value = ctx.base_map.get(key, IndexMode::ZeroBased);
        let _ = black_box(value);
    }
}

fn map_set_existing(ctx: &mut MapContext, chunk_size: usize, _chunk_num: usize) {
    let mut map = ctx.base_map.clone();
    for i in 0..chunk_size {
        let key = &ctx.existing_keys[i & (WORKING_KEY_SET_SIZE - 1)];
        map = map
            .set(key, &ctx.update_value, IndexMode::ZeroBased)
            .unwrap();
    }
    black_box(map);
}

fn map_set_new_insert_destructive(ctx: &mut MapContext, chunk_size: usize, _chunk_num: usize) {
    let mut map = ctx.base_map.clone();
    for i in 0..chunk_size {
        let key = &ctx.insert_keys[i & (WORKING_KEY_SET_SIZE - 1)];
        map = map
            .set(key, &ctx.update_value, IndexMode::ZeroBased)
            .unwrap();
    }
    black_box(map);
}

fn map_set_new_insert_steady(ctx: &mut MapContext, chunk_size: usize, _chunk_num: usize) {
    let mut map = ctx.base_map.clone();
    for i in 0..chunk_size {
        let key = &ctx.insert_keys[i & (WORKING_KEY_SET_SIZE - 1)];
        map = map
            .set(key, &ctx.update_value, IndexMode::ZeroBased)
            .unwrap();
        let (new_map, _) = map.remove(key, false).unwrap();
        map = new_map;
    }
    black_box(map);
}

fn map_set_owned_existing(ctx: &mut MapContext, chunk_size: usize, _chunk_num: usize) {
    let mut map = ctx.base_map.clone();
    for i in 0..chunk_size {
        let key = &ctx.existing_keys[i & (WORKING_KEY_SET_SIZE - 1)];
        map = map
            .set_owned(key, &ctx.update_value, IndexMode::ZeroBased)
            .unwrap();
    }
    black_box(map);
}

fn map_remove_hit_destructive(ctx: &mut MapContext, chunk_size: usize, _chunk_num: usize) {
    let mut map = ctx.base_map.clone();
    for i in 0..chunk_size {
        let key = &ctx.existing_keys[i & (WORKING_KEY_SET_SIZE - 1)];
        let (new_map, _) = map.remove(key, false).unwrap();
        map = new_map;
    }
    black_box(map);
}

fn map_remove_hit_steady(ctx: &mut MapContext, chunk_size: usize, _chunk_num: usize) {
    let mut map = ctx.base_map.clone();
    for i in 0..chunk_size {
        let key = &ctx.existing_keys[i & (WORKING_KEY_SET_SIZE - 1)];
        let (new_map, _) = map.remove(key, false).unwrap();
        map = new_map
            .set(key, &ctx.update_value, IndexMode::ZeroBased)
            .unwrap();
    }
    black_box(map);
}

fn map_remove_miss(ctx: &mut MapContext, chunk_size: usize, _chunk_num: usize) {
    let mut map = ctx.base_map.clone();
    for i in 0..chunk_size {
        let key = &ctx.missing_keys[i & (WORKING_KEY_SET_SIZE - 1)];
        let (new_map, _) = map.remove(key, false).unwrap();
        map = new_map;
    }
    black_box(map);
}

fn map_remove_case_sensitive_hit_destructive(
    ctx: &mut MapContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let mut map = ctx.base_map.clone();
    for i in 0..chunk_size {
        let key = &ctx.existing_keys[i & (WORKING_KEY_SET_SIZE - 1)];
        let (new_map, _) = map.remove(key, true).unwrap();
        map = new_map;
    }
    black_box(map);
}

fn map_remove_case_sensitive_hit_steady(
    ctx: &mut MapContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let mut map = ctx.base_map.clone();
    for i in 0..chunk_size {
        let key = &ctx.existing_keys[i & (WORKING_KEY_SET_SIZE - 1)];
        let (new_map, _) = map.remove(key, true).unwrap();
        map = new_map
            .set(key, &ctx.update_value, IndexMode::ZeroBased)
            .unwrap();
    }
    black_box(map);
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
        eprintln!("Running benchmarks matching filter: '{f}'");
    }

    let map_benchmarks = [
        BenchmarkDef {
            name: "map_get_hit",
            group: "map",
            func: map_get_hit,
        },
        BenchmarkDef {
            name: "map_get_miss",
            group: "map",
            func: map_get_miss,
        },
        BenchmarkDef {
            name: "map_set_existing",
            group: "map",
            func: map_set_existing,
        },
        BenchmarkDef {
            name: "map_set_new_insert_destructive",
            group: "map",
            func: map_set_new_insert_destructive,
        },
        BenchmarkDef {
            name: "map_set_new_insert_steady",
            group: "map",
            func: map_set_new_insert_steady,
        },
        BenchmarkDef {
            name: "map_set_owned_existing",
            group: "map",
            func: map_set_owned_existing,
        },
        BenchmarkDef {
            name: "map_remove_hit_destructive",
            group: "map",
            func: map_remove_hit_destructive,
        },
        BenchmarkDef {
            name: "map_remove_hit_steady",
            group: "map",
            func: map_remove_hit_steady,
        },
        BenchmarkDef {
            name: "map_remove_miss",
            group: "map",
            func: map_remove_miss,
        },
        BenchmarkDef {
            name: "map_remove_case_sensitive_hit_destructive",
            group: "map",
            func: map_remove_case_sensitive_hit_destructive,
        },
        BenchmarkDef {
            name: "map_remove_case_sensitive_hit_steady",
            group: "map",
            func: map_remove_case_sensitive_hit_steady,
        },
    ];

    run_benchmark_group::<MapContext>(&map_benchmarks, "Map Operations", filter);
    generate_session_summary();
}
