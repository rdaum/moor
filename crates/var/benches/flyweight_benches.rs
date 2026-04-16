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
use moor_var::{Flyweight, List, Obj, Symbol, v_int};

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

benchmark_main!(|runner| {
    runner.group::<FlyweightContext>("Flyweight Operations", |g| {
        g.bench("flyweight_clone", flyweight_clone);
        g.bench("flyweight_get_slot", flyweight_get_slot);
        g.bench("flyweight_slots_vec", flyweight_slots_vec);
        g.bench("flyweight_get_contents", flyweight_get_contents);
        g.bench("flyweight_get_delegate", flyweight_get_delegate);
        g.bench("flyweight_add_slot", flyweight_add_slot);
        g.bench("flyweight_remove_slot", flyweight_remove_slot);
    });
});
