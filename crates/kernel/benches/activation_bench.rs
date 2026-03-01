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

//! Activation and frame construction micro-benchmarks using bench-utils.
//! Focuses on small, filterable pieces of the activation setup path.

use std::{cmp::max, sync::Arc};

use moor_bench_utils::{BenchContext, black_box};
use uuid::Uuid;

use moor_common::{
    model::{VerbArgsSpec, VerbDef, VerbFlag},
    util::BitEnum,
};
use moor_compiler::{CompileOptions, compile};
use moor_kernel::testing::{
    ActivationAssemblyBenchState, ActivationBenchResult, MooFrameBenchResult,
    create_activation_assembly_state_for_bench, create_activation_for_bench,
    create_nested_activation_for_bench, create_nested_environment_for_bench,
    create_nested_moo_frame_for_bench, create_top_level_environment_for_bench,
    create_top_level_moo_frame_for_bench, run_activation_assembly_cycle_for_bench,
    run_activation_assembly_cycle_overhead_for_bench,
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
    this: Var,
    caller: Var,
    empty_args: List,
    small_args: List,
    empty_argstr: Var,
    short_argstr: Var,
    simple_program: ProgramType,
    complex_program: ProgramType,
}

impl ActivationFixture {
    fn new() -> Self {
        let verb_name = Symbol::mk("test");
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

        Self {
            verb_name,
            base_verbdef: make_verbdef(verb_name),
            this: v_obj(SYSTEM_OBJECT),
            caller: v_obj(SYSTEM_OBJECT),
            empty_args: List::mk_list(&[]),
            small_args: List::mk_list(&[v_int(1), v_int(2), v_str("hello")]),
            empty_argstr: v_empty_str(),
            short_argstr: v_str("some argument string"),
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
            fixture.base_verbdef.clone(),
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
            fixture.base_verbdef.clone(),
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
            fixture.base_verbdef.clone(),
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
        let resolved_verb = fixture.base_verbdef.clone();
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
        let resolved_verb = fixture.base_verbdef.clone();
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
            fixture.base_verbdef.clone(),
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
            fixture.base_verbdef.clone(),
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
            fixture.base_verbdef.clone(),
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
            fixture.base_verbdef.clone(),
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
            fixture.base_verbdef.clone(),
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

pub fn main() {
    use moor_bench_utils::{generate_session_summary, op_bench_with_factory_filtered};

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

    let args: Vec<String> = std::env::args().collect();
    let filter = if let Some(separator_pos) = args.iter().position(|arg| arg == "--") {
        args.get(separator_pos + 1).map(|s| s.as_str())
    } else {
        args.iter()
            .skip(1)
            .find(|arg| !arg.starts_with("--") && !args[0].contains(arg.as_str()))
            .map(|s| s.as_str())
    };

    if let Some(f) = filter {
        eprintln!("Running activation benchmarks matching filter: '{f}'");
        eprintln!(
            "Available filters: all, activation, primitives, environment, frame, assembly, inputs, for_call, or any benchmark name substring"
        );
        eprintln!();
    }

    let fixture = Arc::new(ActivationFixture::new());

    op_bench_with_factory_filtered(
        "activation_verbdef_new",
        "activation_inputs",
        bench_verbdef_new,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_primitive_clone_verbdef",
        "activation_primitives",
        bench_primitive_clone_verbdef,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_primitive_clone_args_empty",
        "activation_primitives",
        bench_primitive_clone_args_empty,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_primitive_args_to_var",
        "activation_primitives",
        bench_primitive_args_to_var,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_primitive_clone_this",
        "activation_primitives",
        bench_primitive_clone_this,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_primitive_v_symbol_str",
        "activation_primitives",
        bench_primitive_v_symbol_str,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_primitive_clone_program",
        "activation_primitives",
        bench_primitive_clone_program,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_input_clone_materialization",
        "activation_inputs",
        bench_input_clone_materialization,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_input_clone_frame_top_simple",
        "activation_inputs",
        bench_input_clone_frame_top_simple,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_input_clone_frame_nested_simple",
        "activation_inputs",
        bench_input_clone_frame_nested_simple,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_input_clone_activation_top_simple",
        "activation_inputs",
        bench_input_clone_activation_top_simple,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_input_clone_activation_nested_simple",
        "activation_inputs",
        bench_input_clone_activation_nested_simple,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_environment_top_level_simple",
        "activation_environment",
        bench_environment_top_level_simple,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_environment_nested_simple",
        "activation_environment",
        bench_environment_nested_simple,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_frame_top_level_simple",
        "activation_frame",
        bench_frame_top_level_simple,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_frame_nested_simple",
        "activation_frame",
        bench_frame_nested_simple,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_assembly_top_level_simple_direct",
        "activation_assembly",
        bench_activation_assembly_top_level_simple_direct,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_assembly_top_level_simple_overhead",
        "activation_assembly",
        bench_activation_assembly_top_level_simple_overhead,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_assembly_nested_simple_direct",
        "activation_assembly",
        bench_activation_assembly_nested_simple_direct,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_assembly_nested_simple_overhead",
        "activation_assembly",
        bench_activation_assembly_nested_simple_overhead,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_for_call_top_level_simple",
        "activation_for_call",
        bench_activation_top_level_simple,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_for_call_top_level_complex",
        "activation_for_call",
        bench_activation_top_level_complex,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_for_call_with_args",
        "activation_for_call",
        bench_activation_with_args,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_for_call_with_argstr",
        "activation_for_call",
        bench_activation_with_argstr,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );
    op_bench_with_factory_filtered(
        "activation_for_call_nested_simple",
        "activation_for_call",
        bench_activation_nested_simple,
        &|| ActivationBenchContext::from_fixture(Arc::clone(&fixture)),
        filter,
    );

    if filter.is_some() {
        eprintln!("\nActivation benchmark filtering complete.");
    }

    generate_session_summary();
}
