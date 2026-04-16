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

//! Activation and frame construction micro-benchmarks using bench-utils.
//! Focuses on small, filterable pieces of the activation setup path.

use std::{cmp::max, sync::Arc};

use micromeasure::{BenchContext, BenchmarkMainOptions, benchmark_main, black_box};
use uuid::Uuid;

use moor_common::{
    matching::ParsedCommand,
    model::{PrepSpec, ResolvedVerb, VerbArgsSpec, VerbDef, VerbFlag},
    util::BitEnum,
};
use moor_compiler::{CompileOptions, compile};
use moor_kernel::testing::{
    ActivationAssemblyBenchState, ActivationBenchResult, MooFrameBenchResult,
    create_activation_assembly_state_for_bench, create_activation_for_bench,
    create_command_activation_for_bench, create_nested_activation_for_bench,
    create_nested_environment_for_bench, create_nested_moo_frame_for_bench,
    create_top_level_environment_for_bench, create_top_level_moo_frame_for_bench,
    run_activation_assembly_cycle_for_bench, run_activation_assembly_cycle_overhead_for_bench,
};
use moor_var::{
    List, SYSTEM_OBJECT, Symbol, Var,
    program::{ProgramType, names::GlobalName},
    v_empty_list, v_empty_str, v_int, v_obj, v_str, v_symbol_str,
};
use strum::EnumCount;

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

struct ActivationFixture {
    verb_name: Symbol,
    base_verbdef: VerbDef,
    base_resolved_verb: ResolvedVerb,
    this: Var,
    caller: Var,
    empty_args: List,
    small_args: List,
    empty_argstr: Var,
    short_argstr: Var,
    empty_command: ParsedCommand,
    args_command: ParsedCommand,
    simple_program: ProgramType,
    complex_program: ProgramType,
}

impl ActivationFixture {
    fn new() -> Self {
        let verb_name = Symbol::mk("test");
        let base_verbdef = make_verbdef(verb_name);
        let base_resolved_verb = base_verbdef.as_resolved();
        let simple_program = ProgramType::MooR(
            compile("return 1;", CompileOptions::default()).expect("simple program should compile"),
        );
        let complex_program = ProgramType::MooR(
            compile(
                r#"
                result = {};
                for i in [1..10]
                    result = {@result, i * 2};
                endfor
                return result;
                "#,
                CompileOptions::default(),
            )
            .expect("complex program should compile"),
        );
        let empty_command = ParsedCommand {
            verb: verb_name,
            argstr: String::new(),
            args: Vec::new(),
            dobjstr: None,
            dobj: None,
            ambiguous_dobj: None,
            prepstr: None,
            prep: PrepSpec::None,
            iobjstr: None,
            iobj: None,
            ambiguous_iobj: None,
        };
        let args_command = ParsedCommand {
            verb: verb_name,
            argstr: "1 2 hello".to_string(),
            args: vec![v_int(1), v_int(2), v_str("hello")],
            dobjstr: None,
            dobj: None,
            ambiguous_dobj: None,
            prepstr: None,
            prep: PrepSpec::None,
            iobjstr: None,
            iobj: None,
            ambiguous_iobj: None,
        };

        Self {
            verb_name,
            base_verbdef,
            base_resolved_verb,
            this: v_obj(SYSTEM_OBJECT),
            caller: v_obj(SYSTEM_OBJECT),
            empty_args: List::mk_list(&[]),
            small_args: List::mk_list(&[v_int(1), v_int(2), v_str("hello")]),
            empty_argstr: v_empty_str(),
            short_argstr: v_str("some argument string"),
            empty_command,
            args_command,
            simple_program,
            complex_program,
        }
    }
}

struct ActivationBenchContext {
    fixture: Arc<ActivationFixture>,
    parent_activation: ActivationBenchResult,
    parent_frame: MooFrameBenchResult,
    top_assembly_state: ActivationAssemblyBenchState,
    nested_assembly_state: ActivationAssemblyBenchState,
}

impl ActivationBenchContext {
    fn from_fixture(fixture: Arc<ActivationFixture>) -> Self {
        let parent_activation = create_activation_for_bench(
            SYSTEM_OBJECT,
            fixture.base_resolved_verb,
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            fixture.empty_argstr.clone(),
            fixture.simple_program.clone(),
        );

        let parent_frame = create_top_level_moo_frame_for_bench(
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            fixture.empty_argstr.clone(),
            fixture.simple_program.clone(),
        );
        let top_assembly_frame = create_top_level_moo_frame_for_bench(
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            fixture.empty_argstr.clone(),
            fixture.simple_program.clone(),
        );
        let top_assembly_state = create_activation_assembly_state_for_bench(
            fixture.base_resolved_verb,
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            top_assembly_frame,
        );
        let nested_assembly_frame = create_nested_moo_frame_for_bench(
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            &parent_frame,
            fixture.simple_program.clone(),
        );
        let nested_assembly_state = create_activation_assembly_state_for_bench(
            fixture.base_resolved_verb,
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            nested_assembly_frame,
        );
        Self {
            fixture,
            parent_activation,
            parent_frame,
            top_assembly_state,
            nested_assembly_state,
        }
    }
}

impl BenchContext for ActivationBenchContext {
    fn prepare(_num_chunks: usize) -> Self {
        Self::from_fixture(Arc::new(ActivationFixture::new()))
    }
}

fn bench_verbdef_new(ctx: &mut ActivationBenchContext, chunk_size: usize, _chunk_num: usize) {
    let verb_name = ctx.fixture.verb_name;
    for _ in 0..chunk_size {
        black_box(make_verbdef(verb_name));
    }
}

fn bench_primitive_clone_verbdef(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(fixture.base_verbdef.clone());
    }
}

fn bench_primitive_clone_args_empty(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(fixture.empty_args.clone());
    }
}

fn bench_primitive_args_to_var(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(if fixture.empty_args.is_empty() {
            v_empty_list()
        } else {
            Var::from(fixture.empty_args.clone())
        });
    }
}

fn bench_primitive_clone_this(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(fixture.this.clone());
    }
}

fn bench_primitive_v_symbol_str(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(v_symbol_str(fixture.verb_name));
    }
}

fn bench_primitive_clone_program(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(fixture.simple_program.clone());
    }
}

fn bench_input_clone_materialization(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        let cloned = (
            fixture.base_verbdef.clone(),
            fixture.this.clone(),
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            fixture.empty_argstr.clone(),
            fixture.simple_program.clone(),
            v_symbol_str(fixture.verb_name),
        );
        black_box(cloned);
    }
}

#[allow(irrefutable_let_patterns)]
fn bench_input_clone_frame_top_simple(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        let ProgramType::MooR(program) = fixture.simple_program.clone() else {
            continue;
        };
        let width = max(program.var_names().global_width(), GlobalName::COUNT);
        let cloned = (
            v_obj(SYSTEM_OBJECT),
            fixture.this.clone(),
            fixture.caller.clone(),
            v_symbol_str(fixture.verb_name),
            if fixture.empty_args.is_empty() {
                v_empty_list()
            } else {
                Var::from(fixture.empty_args.clone())
            },
            fixture.empty_argstr.clone(),
            width,
            program,
        );
        black_box(cloned);
    }
}

#[allow(irrefutable_let_patterns)]
fn bench_input_clone_frame_nested_simple(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        let ProgramType::MooR(program) = fixture.simple_program.clone() else {
            continue;
        };
        let width = max(program.var_names().global_width(), GlobalName::COUNT);
        let cloned = (
            v_obj(SYSTEM_OBJECT),
            fixture.this.clone(),
            fixture.caller.clone(),
            v_symbol_str(fixture.verb_name),
            if fixture.empty_args.is_empty() {
                v_empty_list()
            } else {
                Var::from(fixture.empty_args.clone())
            },
            width,
            true,
            program,
        );
        black_box(cloned);
    }
}

#[allow(irrefutable_let_patterns)]
fn bench_input_clone_activation_top_simple(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        let ProgramType::MooR(program) = fixture.simple_program.clone() else {
            continue;
        };
        let resolved_verb = fixture.base_resolved_verb;
        let cloned = (
            resolved_verb.owner(),
            resolved_verb,
            v_obj(SYSTEM_OBJECT),
            fixture.this.clone(),
            fixture.empty_args.clone(),
            if fixture.empty_args.is_empty() {
                v_empty_list()
            } else {
                Var::from(fixture.empty_args.clone())
            },
            fixture.caller.clone(),
            fixture.empty_argstr.clone(),
            v_symbol_str(fixture.verb_name),
            false,
            program,
        );
        black_box(cloned);
    }
}

#[allow(irrefutable_let_patterns)]
fn bench_input_clone_activation_nested_simple(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        let ProgramType::MooR(program) = fixture.simple_program.clone() else {
            continue;
        };
        let resolved_verb = fixture.base_resolved_verb;
        let cloned = (
            resolved_verb.owner(),
            resolved_verb,
            v_obj(SYSTEM_OBJECT),
            fixture.this.clone(),
            fixture.empty_args.clone(),
            if fixture.empty_args.is_empty() {
                v_empty_list()
            } else {
                Var::from(fixture.empty_args.clone())
            },
            fixture.caller.clone(),
            v_symbol_str(fixture.verb_name),
            true,
            program,
        );
        black_box(cloned);
    }
}

fn bench_environment_top_level_simple(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(create_top_level_environment_for_bench(
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            fixture.empty_argstr.clone(),
            fixture.simple_program.clone(),
        ));
    }
}

fn bench_environment_nested_simple(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(create_nested_environment_for_bench(
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            &ctx.parent_frame,
            fixture.simple_program.clone(),
        ));
    }
}

fn bench_frame_top_level_simple(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(create_top_level_moo_frame_for_bench(
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            fixture.empty_argstr.clone(),
            fixture.simple_program.clone(),
        ));
    }
}

fn bench_frame_nested_simple(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(create_nested_moo_frame_for_bench(
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            &ctx.parent_frame,
            fixture.simple_program.clone(),
        ));
    }
}

fn bench_activation_assembly_top_level_simple_direct(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for _ in 0..chunk_size {
        run_activation_assembly_cycle_for_bench(&mut ctx.top_assembly_state);
    }
}

fn bench_activation_assembly_nested_simple_direct(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for _ in 0..chunk_size {
        run_activation_assembly_cycle_for_bench(&mut ctx.nested_assembly_state);
    }
}

fn bench_activation_assembly_top_level_simple_overhead(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for _ in 0..chunk_size {
        run_activation_assembly_cycle_overhead_for_bench(&mut ctx.top_assembly_state);
    }
}

fn bench_activation_assembly_nested_simple_overhead(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    for _ in 0..chunk_size {
        run_activation_assembly_cycle_overhead_for_bench(&mut ctx.nested_assembly_state);
    }
}

fn bench_activation_top_level_simple(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(create_activation_for_bench(
            SYSTEM_OBJECT,
            fixture.base_resolved_verb,
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            fixture.empty_argstr.clone(),
            fixture.simple_program.clone(),
        ));
    }
}

fn bench_activation_top_level_complex(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(create_activation_for_bench(
            SYSTEM_OBJECT,
            fixture.base_resolved_verb,
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            fixture.empty_argstr.clone(),
            fixture.complex_program.clone(),
        ));
    }
}

fn bench_activation_with_args(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(create_activation_for_bench(
            SYSTEM_OBJECT,
            fixture.base_resolved_verb,
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.small_args.clone(),
            fixture.caller.clone(),
            fixture.empty_argstr.clone(),
            fixture.simple_program.clone(),
        ));
    }
}

fn bench_activation_with_argstr(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(create_activation_for_bench(
            SYSTEM_OBJECT,
            fixture.base_resolved_verb,
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            fixture.short_argstr.clone(),
            fixture.simple_program.clone(),
        ));
    }
}

fn bench_activation_nested_simple(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(create_nested_activation_for_bench(
            SYSTEM_OBJECT,
            fixture.base_resolved_verb,
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.empty_args.clone(),
            fixture.caller.clone(),
            fixture.empty_argstr.clone(),
            &ctx.parent_activation,
            fixture.simple_program.clone(),
        ));
    }
}

fn bench_command_activation_empty(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(create_command_activation_for_bench(
            SYSTEM_OBJECT,
            fixture.base_resolved_verb,
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.caller.clone(),
            fixture.empty_command.clone(),
            fixture.simple_program.clone(),
        ));
    }
}

fn bench_command_activation_with_args(
    ctx: &mut ActivationBenchContext,
    chunk_size: usize,
    _chunk_num: usize,
) {
    let fixture = &ctx.fixture;
    for _ in 0..chunk_size {
        black_box(create_command_activation_for_bench(
            SYSTEM_OBJECT,
            fixture.base_resolved_verb,
            fixture.verb_name,
            fixture.this.clone(),
            SYSTEM_OBJECT,
            fixture.caller.clone(),
            fixture.args_command.clone(),
            fixture.simple_program.clone(),
        ));
    }
}

benchmark_main!(
    BenchmarkMainOptions {
        filter_help: Some(
            "all, activation, primitives, environment, frame, assembly, inputs, for_call, command, or any benchmark name substring".to_string()
        ),
        ..BenchmarkMainOptions::default()
    },
    |runner| {
    let fixture = Arc::new(ActivationFixture::new());
    let context_factory = || ActivationBenchContext::from_fixture(Arc::clone(&fixture));

    runner.group::<ActivationBenchContext>("activation_inputs", |g| {
        g.bench_with_factory("activation_verbdef_new", &context_factory, bench_verbdef_new);
        g.bench_with_factory(
            "activation_input_clone_materialization",
            &context_factory,
            bench_input_clone_materialization,
        );
        g.bench_with_factory(
            "activation_input_clone_frame_top_simple",
            &context_factory,
            bench_input_clone_frame_top_simple,
        );
        g.bench_with_factory(
            "activation_input_clone_frame_nested_simple",
            &context_factory,
            bench_input_clone_frame_nested_simple,
        );
        g.bench_with_factory(
            "activation_input_clone_activation_top_simple",
            &context_factory,
            bench_input_clone_activation_top_simple,
        );
        g.bench_with_factory(
            "activation_input_clone_activation_nested_simple",
            &context_factory,
            bench_input_clone_activation_nested_simple,
        );
    });

    runner.group::<ActivationBenchContext>("activation_primitives", |g| {
        g.bench_with_factory(
            "activation_primitive_clone_verbdef",
            &context_factory,
            bench_primitive_clone_verbdef,
        );
        g.bench_with_factory(
            "activation_primitive_clone_args_empty",
            &context_factory,
            bench_primitive_clone_args_empty,
        );
        g.bench_with_factory(
            "activation_primitive_args_to_var",
            &context_factory,
            bench_primitive_args_to_var,
        );
        g.bench_with_factory(
            "activation_primitive_clone_this",
            &context_factory,
            bench_primitive_clone_this,
        );
        g.bench_with_factory(
            "activation_primitive_v_symbol_str",
            &context_factory,
            bench_primitive_v_symbol_str,
        );
        g.bench_with_factory(
            "activation_primitive_clone_program",
            &context_factory,
            bench_primitive_clone_program,
        );
    });

    runner.group::<ActivationBenchContext>("activation_environment", |g| {
        g.bench_with_factory(
            "activation_environment_top_level_simple",
            &context_factory,
            bench_environment_top_level_simple,
        );
        g.bench_with_factory(
            "activation_environment_nested_simple",
            &context_factory,
            bench_environment_nested_simple,
        );
    });

    runner.group::<ActivationBenchContext>("activation_frame", |g| {
        g.bench_with_factory(
            "activation_frame_top_level_simple",
            &context_factory,
            bench_frame_top_level_simple,
        );
        g.bench_with_factory(
            "activation_frame_nested_simple",
            &context_factory,
            bench_frame_nested_simple,
        );
    });

    runner.group::<ActivationBenchContext>("activation_assembly", |g| {
        g.bench_with_factory(
            "activation_assembly_top_level_simple_direct",
            &context_factory,
            bench_activation_assembly_top_level_simple_direct,
        );
        g.bench_with_factory(
            "activation_assembly_top_level_simple_overhead",
            &context_factory,
            bench_activation_assembly_top_level_simple_overhead,
        );
        g.bench_with_factory(
            "activation_assembly_nested_simple_direct",
            &context_factory,
            bench_activation_assembly_nested_simple_direct,
        );
        g.bench_with_factory(
            "activation_assembly_nested_simple_overhead",
            &context_factory,
            bench_activation_assembly_nested_simple_overhead,
        );
    });

    runner.group::<ActivationBenchContext>("activation_for_call", |g| {
        g.bench_with_factory(
            "activation_for_call_top_level_simple",
            &context_factory,
            bench_activation_top_level_simple,
        );
        g.bench_with_factory(
            "activation_for_call_top_level_complex",
            &context_factory,
            bench_activation_top_level_complex,
        );
        g.bench_with_factory(
            "activation_for_call_with_args",
            &context_factory,
            bench_activation_with_args,
        );
        g.bench_with_factory(
            "activation_for_call_with_argstr",
            &context_factory,
            bench_activation_with_argstr,
        );
        g.bench_with_factory(
            "activation_for_call_nested_simple",
            &context_factory,
            bench_activation_nested_simple,
        );
    });

    runner.group::<ActivationBenchContext>("activation_command", |g| {
        g.bench_with_factory(
            "activation_command_request_top_level_simple",
            &context_factory,
            bench_command_activation_empty,
        );
        g.bench_with_factory(
            "activation_command_request_with_args",
            &context_factory,
            bench_command_activation_with_args,
        );
    });
    }
);
