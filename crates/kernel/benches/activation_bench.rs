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

//! Microbenchmark for Activation::for_call frame creation.
//! Measures the cost of setting up a new verb activation frame, isolating it from
//! VM execution and database access costs.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use uuid::Uuid;

use moor_common::{
    model::{VerbArgsSpec, VerbDef, VerbFlag},
    util::BitEnum,
};
use moor_compiler::{CompileOptions, compile};
use moor_kernel::testing::{create_activation_for_bench, create_nested_activation_for_bench};
use moor_var::{List, SYSTEM_OBJECT, Symbol, program::ProgramType, v_empty_str, v_obj, v_str};

/// Create a VerbDef for benchmarking - this is a cheap operation
fn make_verbdef(verb_name: Symbol) -> VerbDef {
    VerbDef::new(
        Uuid::new_v4(),
        SYSTEM_OBJECT,
        SYSTEM_OBJECT,
        &[verb_name],
        BitEnum::new_with(VerbFlag::Exec) | VerbFlag::Debug,
        VerbArgsSpec::this_none_this(),
    )
}

fn activation_creation(c: &mut Criterion) {
    // Pre-compile programs of varying complexity
    let simple_program = compile("return 1;", CompileOptions::default()).unwrap();
    let medium_program = compile(
        r#"
        x = 1;
        y = 2;
        z = x + y;
        return z;
        "#,
        CompileOptions::default(),
    )
    .unwrap();
    let complex_program = compile(
        r#"
        result = {};
        for i in [1..10]
            result = {@result, i * 2};
        endfor
        return result;
        "#,
        CompileOptions::default(),
    )
    .unwrap();

    // Pre-create reusable inputs
    let verb_name = Symbol::mk("test");
    let this = v_obj(SYSTEM_OBJECT);
    let caller = v_obj(SYSTEM_OBJECT);
    let empty_args = List::mk_list(&[]);

    let mut group = c.benchmark_group("activation_for_call");

    // Benchmark with simple program (minimal environment setup)
    group.bench_function("simple_program", |b| {
        let verbdef = make_verbdef(verb_name);
        let program = ProgramType::MooR(simple_program.clone());
        b.iter(|| {
            black_box(create_activation_for_bench(
                SYSTEM_OBJECT,
                verbdef.clone(),
                verb_name,
                this.clone(),
                SYSTEM_OBJECT,
                empty_args.clone(),
                caller.clone(),
                v_empty_str(),
                program.clone(),
            ))
        });
    });

    // Benchmark with medium complexity program
    group.bench_function("medium_program", |b| {
        let verbdef = make_verbdef(verb_name);
        let program = ProgramType::MooR(medium_program.clone());
        b.iter(|| {
            black_box(create_activation_for_bench(
                SYSTEM_OBJECT,
                verbdef.clone(),
                verb_name,
                this.clone(),
                SYSTEM_OBJECT,
                empty_args.clone(),
                caller.clone(),
                v_empty_str(),
                program.clone(),
            ))
        });
    });

    // Benchmark with complex program (more variables, loops)
    group.bench_function("complex_program", |b| {
        let verbdef = make_verbdef(verb_name);
        let program = ProgramType::MooR(complex_program.clone());
        b.iter(|| {
            black_box(create_activation_for_bench(
                SYSTEM_OBJECT,
                verbdef.clone(),
                verb_name,
                this.clone(),
                SYSTEM_OBJECT,
                empty_args.clone(),
                caller.clone(),
                v_empty_str(),
                program.clone(),
            ))
        });
    });

    // Benchmark with non-empty args (tests List cloning cost)
    group.bench_function("with_args", |b| {
        let verbdef = make_verbdef(verb_name);
        let program = ProgramType::MooR(simple_program.clone());
        let args = List::mk_list(&[
            moor_var::v_int(1),
            moor_var::v_int(2),
            moor_var::v_str("hello"),
        ]);
        b.iter(|| {
            black_box(create_activation_for_bench(
                SYSTEM_OBJECT,
                verbdef.clone(),
                verb_name,
                this.clone(),
                SYSTEM_OBJECT,
                args.clone(),
                caller.clone(),
                v_empty_str(),
                program.clone(),
            ))
        });
    });

    let sas = v_str("some argument string");
    // Benchmark with argstr (tests String allocation)
    group.bench_function("with_argstr", |b| {
        let verbdef = make_verbdef(verb_name);
        let program = ProgramType::MooR(simple_program.clone());
        b.iter(|| {
            black_box(create_activation_for_bench(
                SYSTEM_OBJECT,
                verbdef.clone(),
                verb_name,
                this.clone(),
                SYSTEM_OBJECT,
                empty_args.clone(),
                caller.clone(),
                sas.clone(),
                program.clone(),
            ))
        });
    });

    // Benchmark nested call (with source frame) - this is the hot path for verb dispatch
    // Tests the with_globals_from_source path which copies parsing globals from parent
    group.bench_function("nested_call", |b| {
        let verbdef = make_verbdef(verb_name);
        let program = ProgramType::MooR(simple_program.clone());
        // Create a parent activation to serve as source frame
        let parent = create_activation_for_bench(
            SYSTEM_OBJECT,
            verbdef.clone(),
            verb_name,
            this.clone(),
            SYSTEM_OBJECT,
            empty_args.clone(),
            caller.clone(),
            v_empty_str(),
            program.clone(),
        );
        b.iter(|| {
            black_box(create_nested_activation_for_bench(
                SYSTEM_OBJECT,
                verbdef.clone(),
                verb_name,
                this.clone(),
                SYSTEM_OBJECT,
                empty_args.clone(),
                caller.clone(),
                v_empty_str(),
                &parent,
                program.clone(),
            ))
        });
    });

    group.finish();
}

/// Benchmark comparing VerbDef creation costs (to understand its contribution)
fn verbdef_creation(c: &mut Criterion) {
    let verb_name = Symbol::mk("test");

    c.bench_function("verbdef_creation", |b| {
        b.iter(|| black_box(make_verbdef(verb_name)));
    });
}

criterion_group!(benches, activation_creation, verbdef_creation);
criterion_main!(benches);
