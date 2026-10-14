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

//! Builtin dispatch floor benchmarks.

use micromeasure::{
    BenchContext, BenchSampleResult, BenchmarkMainOptions, BenchmarkRuntimeOptions,
    LinuxPerfBackend, Throughput, benchmark_main, black_box,
};
use moor_common::{
    model::{
        CommitResult, DispatchFlagsSource, ObjFlag, ObjectKind, TaskPermissions, VerbArgsSpec,
        VerbDispatch, VerbFlag, VerbLookup, WorldState, WorldStateSource,
    },
    tasks::{AbortLimitReason, NoopClientSession, Session},
    util::BitEnum,
};
use moor_compiler::{BuiltinId, CompileOptions, compile, offset_for_builtin};
use moor_db::{DatabaseConfig, TxDB};
use moor_kernel::{
    config::FeaturesConfig,
    task_context::{TaskGuard, rollback_current_transaction},
    tasks::{TaskProgramCache, task_scheduler_client::TaskSchedulerClient},
    testing::vm_test_utils::{
        benchmark_builtin_call_function, benchmark_builtin_direct_function, test_scheduler_for_db,
    },
    vm::{VMHostResponse, builtins::BuiltinRegistry, vm_host::VmHost},
};
use moor_var::{
    List, NOTHING, SYSTEM_OBJECT, Symbol, v_empty_str, v_int, v_list, v_map, v_obj, v_str,
};
use std::{sync::Arc, time::Duration};

const FULL_MOO_TICKS: usize = 20_000_000;
const DIRECT_BUILTIN_CALLS: usize = 2_000_000;

fn system_permissions() -> TaskPermissions {
    TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new())
}

fn create_db() -> TxDB {
    let (ws_source, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
    let mut tx = ws_source.new_world_state().unwrap();
    let _sysobj = tx
        .create_object(
            &system_permissions(),
            &NOTHING,
            &SYSTEM_OBJECT,
            ObjFlag::all_flags(),
            ObjectKind::NextObjid,
        )
        .unwrap();
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    ws_source
}

fn prepare_call_verb(
    world_state: &mut dyn WorldState,
    verb_name: &str,
    args: List,
    max_ticks: usize,
) -> VmHost {
    let mut vm_host = VmHost::new(0, 20, max_ticks, Duration::from_secs(1000));

    let verb_name = Symbol::mk(verb_name);
    let verb_result = world_state
        .dispatch_verb(
            &system_permissions(),
            VerbDispatch::new(
                VerbLookup::method(&SYSTEM_OBJECT, verb_name),
                DispatchFlagsSource::Permissions,
            ),
        )
        .unwrap();
    let Some(verb_result) = verb_result else {
        panic!("could not resolve benchmark verb");
    };
    let (program, _) = world_state
        .retrieve_verb(
            &system_permissions(),
            &verb_result.program_key.verb_definer,
            verb_result.program_key.verb_uuid,
        )
        .unwrap();
    let permissions_flags = BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer;
    vm_host.start_call_method_verb(
        0,
        verb_result.verbdef,
        verb_name,
        v_obj(SYSTEM_OBJECT),
        SYSTEM_OBJECT,
        args,
        v_obj(SYSTEM_OBJECT),
        v_empty_str(),
        permissions_flags,
        program,
    );
    vm_host
}

fn prepare_vm_execution(
    ws_source: &dyn WorldStateSource,
    program: &str,
    max_ticks: usize,
) -> VmHost {
    let program = compile(program, CompileOptions::default()).unwrap();
    let mut tx = ws_source.new_world_state().unwrap();
    tx.add_verb(
        &system_permissions(),
        &SYSTEM_OBJECT,
        vec![Symbol::mk("test")],
        &SYSTEM_OBJECT,
        VerbFlag::rxd(),
        VerbArgsSpec::this_none_this(),
        moor_var::program::ProgramType::MooR(program),
    )
    .unwrap();
    let vm_host = prepare_call_verb(tx.as_mut(), "test", List::mk_list(&[]), max_ticks);
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    vm_host
}

fn execute_until_ticks(
    session: &dyn Session,
    vm_host: &mut VmHost,
    builtins: &BuiltinRegistry,
    features: &FeaturesConfig,
    program_cache: &mut TaskProgramCache,
) -> usize {
    vm_host.reset_ticks();
    vm_host.reset_time();

    loop {
        match vm_host.exec_interpreter(0, session, builtins, features, program_cache) {
            VMHostResponse::ContinueOk => continue,
            VMHostResponse::AbortLimit(AbortLimitReason::Ticks(t)) => return t,
            _ => panic!("unexpected VM response"),
        }
    }
}

struct BuiltinDispatchContext {
    db: TxDB,
    vm_host: VmHost,
    builtins: BuiltinRegistry,
    session: Arc<dyn Session>,
    features: FeaturesConfig,
    builtin: BuiltinId,
    args: List,
    iterations: usize,
    task_scheduler_client: TaskSchedulerClient,
    program_cache: TaskProgramCache,
}

impl BuiltinDispatchContext {
    fn with_program(program: &str, max_ticks: usize) -> Self {
        let db = create_db();
        let vm_host = prepare_vm_execution(&db, program, max_ticks);
        let scheduler = test_scheduler_for_db(db.clone());
        let builtins = BuiltinRegistry::new();
        let session = Arc::new(NoopClientSession::new());
        let features = FeaturesConfig::default();
        let builtin = BuiltinId(offset_for_builtin("typeof") as u16);
        let args = List::mk_list(&[v_int(1)]);

        Self {
            db,
            vm_host,
            builtins,
            session,
            features,
            builtin,
            args,
            iterations: DIRECT_BUILTIN_CALLS,
            task_scheduler_client: TaskSchedulerClient::new(0, scheduler),
            program_cache: TaskProgramCache::default(),
        }
    }

    fn with_builtin(name: &str, args: List, iterations: usize) -> Self {
        let mut context = Self::with_program("while (1) 1; endwhile", FULL_MOO_TICKS);
        context.builtin = BuiltinId(offset_for_builtin(name) as u16);
        context.args = args;
        context.iterations = iterations;
        context
    }
}

impl BenchContext for BuiltinDispatchContext {
    fn prepare(_num_chunks: usize) -> Self {
        Self::with_program("while (1) 1; endwhile", FULL_MOO_TICKS)
    }

    fn chunk_size() -> Option<usize> {
        Some(1)
    }
}

fn builtin_full_moo(
    ctx: &mut BuiltinDispatchContext,
    _chunk_size: usize,
    _chunk_num: usize,
) -> BenchSampleResult {
    let tx = ctx.db.new_world_state().unwrap();
    let _task_guard = TaskGuard::new(
        tx,
        ctx.task_scheduler_client.clone(),
        0,
        NOTHING,
        ctx.session.clone(),
    );
    let ticks = execute_until_ticks(
        ctx.session.as_ref(),
        &mut ctx.vm_host,
        &ctx.builtins,
        &ctx.features,
        &mut ctx.program_cache,
    );
    rollback_current_transaction().unwrap();
    BenchSampleResult::operations(black_box(ticks) as u64)
}

fn builtin_current_call_function(
    ctx: &mut BuiltinDispatchContext,
    _chunk_size: usize,
    _chunk_num: usize,
) -> BenchSampleResult {
    let tx = ctx.db.new_world_state().unwrap();
    let _task_guard = TaskGuard::new(
        tx,
        ctx.task_scheduler_client.clone(),
        0,
        NOTHING,
        ctx.session.clone(),
    );
    let calls = benchmark_builtin_call_function(
        &mut ctx.vm_host,
        &ctx.builtins,
        &ctx.features,
        ctx.session.as_ref(),
        ctx.builtin,
        &ctx.args,
        ctx.iterations,
    );
    rollback_current_transaction().unwrap();
    BenchSampleResult::operations(black_box(calls) as u64)
}

fn builtin_direct_function(
    ctx: &mut BuiltinDispatchContext,
    _chunk_size: usize,
    _chunk_num: usize,
) -> BenchSampleResult {
    let tx = ctx.db.new_world_state().unwrap();
    let _task_guard = TaskGuard::new(
        tx,
        ctx.task_scheduler_client.clone(),
        0,
        NOTHING,
        ctx.session.clone(),
    );
    let calls = benchmark_builtin_direct_function(
        &mut ctx.vm_host,
        &ctx.builtins,
        &ctx.features,
        ctx.builtin,
        &ctx.args,
        ctx.iterations,
    );
    rollback_current_transaction().unwrap();
    BenchSampleResult::operations(black_box(calls) as u64)
}

benchmark_main!(
    BenchmarkMainOptions {
        filter_help: Some(
            "all, full_moo, current_call, direct_function, typeof, valid_task, tostr, or toliteral"
                .to_string(),
        ),
        runtime: BenchmarkRuntimeOptions {
            warm_up_duration: Duration::from_millis(250),
            benchmark_duration: Duration::from_secs(1),
            min_samples: 8,
            max_samples: 24,
        },
        ..BenchmarkMainOptions::default()
    },
    |runner| {
        runner.group::<BuiltinDispatchContext>("builtin_dispatch", |g| {
            g.backend(|| Box::new(LinuxPerfBackend::new().with_compact_counters()))
                .throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| {
                    BuiltinDispatchContext::with_program(
                        "while (1) typeof(1); endwhile",
                        FULL_MOO_TICKS,
                    )
                })
                .bench_sample("builtin_full_moo_typeof", builtin_full_moo);

            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| {
                    BuiltinDispatchContext::with_program(
                        "while (1) valid_task(0); endwhile",
                        FULL_MOO_TICKS,
                    )
                })
                .bench_sample("builtin_full_moo_valid_task_current", builtin_full_moo);

            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| {
                    BuiltinDispatchContext::with_program(
                        "while (1) valid_task(1); endwhile",
                        FULL_MOO_TICKS,
                    )
                })
                .bench_sample("builtin_full_moo_valid_task_missing", builtin_full_moo);

            g.throughput(Throughput::per_operation(1, "builtin_calls"))
                .factory(&|| {
                    BuiltinDispatchContext::with_builtin(
                        "typeof",
                        List::mk_list(&[v_int(1)]),
                        DIRECT_BUILTIN_CALLS,
                    )
                })
                .bench_sample(
                    "builtin_current_call_function_typeof",
                    builtin_current_call_function,
                );

            g.throughput(Throughput::per_operation(1, "builtin_calls"))
                .factory(&|| {
                    BuiltinDispatchContext::with_builtin(
                        "typeof",
                        List::mk_list(&[v_int(1)]),
                        DIRECT_BUILTIN_CALLS,
                    )
                })
                .bench_sample("builtin_direct_function_typeof", builtin_direct_function);

            g.throughput(Throughput::per_operation(1, "builtin_calls"))
                .factory(&|| {
                    BuiltinDispatchContext::with_builtin(
                        "tostr",
                        List::mk_list(&[v_str("planner_symbol_covered")]),
                        1_000_000,
                    )
                })
                .bench_sample("builtin_direct_tostr_string", builtin_direct_function);

            g.throughput(Throughput::per_operation(1, "builtin_calls"))
                .factory(&|| {
                    BuiltinDispatchContext::with_builtin(
                        "tostr",
                        List::mk_list(&[
                            v_str("task "),
                            v_int(123_456),
                            v_str(" on "),
                            v_obj(SYSTEM_OBJECT),
                            v_str(": "),
                            v_str("planner_symbol_covered"),
                        ]),
                        500_000,
                    )
                })
                .bench_sample("builtin_direct_tostr_mixed", builtin_direct_function);

            g.throughput(Throughput::per_operation(1, "builtin_calls"))
                .factory(&|| {
                    let mut args = Vec::with_capacity(64);
                    for value in 0..32 {
                        args.push(v_str("planner_symbol_covered="));
                        args.push(v_int(value));
                    }
                    BuiltinDispatchContext::with_builtin("tostr", List::mk_list(&args), 100_000)
                })
                .bench_sample(
                    "builtin_direct_tostr_many_arguments",
                    builtin_direct_function,
                );

            g.throughput(Throughput::per_operation(1, "builtin_calls"))
                .factory(&|| {
                    BuiltinDispatchContext::with_builtin(
                        "toliteral",
                        List::mk_list(&[v_int(123_456)]),
                        1_000_000,
                    )
                })
                .bench_sample("builtin_direct_toliteral_integer", builtin_direct_function);

            g.throughput(Throughput::per_operation(1, "builtin_calls"))
                .factory(&|| {
                    BuiltinDispatchContext::with_builtin(
                        "toliteral",
                        List::mk_list(&[v_str(
                            "A quoted value: \"planner\".\nA second line with a tab:\tend.",
                        )]),
                        500_000,
                    )
                })
                .bench_sample(
                    "builtin_direct_toliteral_escaped_string",
                    builtin_direct_function,
                );

            g.throughput(Throughput::per_operation(1, "builtin_calls"))
                .factory(&|| {
                    let value = v_list(&[
                        v_int(42),
                        v_str("quoted \"value\""),
                        v_map(&[
                            (v_str("name"), v_str("planner")),
                            (v_str("values"), v_list(&[v_int(1), v_int(2), v_int(3)])),
                        ]),
                    ]);
                    BuiltinDispatchContext::with_builtin(
                        "toliteral",
                        List::mk_list(&[value]),
                        100_000,
                    )
                })
                .bench_sample("builtin_direct_toliteral_nested", builtin_direct_function);

            g.throughput(Throughput::per_operation(1, "builtin_calls"))
                .factory(&|| {
                    let mut pairs = Vec::with_capacity(32);
                    for value in 0..32 {
                        let nested = v_map(&[
                            (v_str("active"), v_int(value & 1)),
                            (v_str("sequence"), v_int(value)),
                        ]);
                        pairs.push((
                            v_str(format!("planner-key-{value}").as_str()),
                            v_list(&[v_int(value), v_str("planner_symbol_covered"), nested]),
                        ));
                    }
                    let value = v_map(&pairs);
                    BuiltinDispatchContext::with_builtin(
                        "toliteral",
                        List::mk_list(&[value]),
                        25_000,
                    )
                })
                .bench_sample(
                    "builtin_direct_toliteral_large_nested",
                    builtin_direct_function,
                );
        });
    }
);
