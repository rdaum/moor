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

use micromeasure::{BenchContext, benchmark_main, black_box};
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

benchmark_main!(|runner| {
    runner.group::<MapContext>("Map Operations", |g| {
        g.bench("map_get_hit", map_get_hit);
        g.bench("map_get_miss", map_get_miss);
        g.bench("map_set_existing", map_set_existing);
        g.bench("map_set_new_insert_destructive", map_set_new_insert_destructive);
        g.bench("map_set_new_insert_steady", map_set_new_insert_steady);
        g.bench("map_set_owned_existing", map_set_owned_existing);
        g.bench("map_remove_hit_destructive", map_remove_hit_destructive);
        g.bench("map_remove_hit_steady", map_remove_hit_steady);
        g.bench("map_remove_miss", map_remove_miss);
        g.bench(
            "map_remove_case_sensitive_hit_destructive",
            map_remove_case_sensitive_hit_destructive,
        );
        g.bench(
            "map_remove_case_sensitive_hit_steady",
            map_remove_case_sensitive_hit_steady,
        );
    });
});
