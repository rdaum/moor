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

//! Microbenchmark for verb dispatch overhead.
//! Measures the cost of verb-calling-verb through the VM execution loop,
//! isolating it from scheduler overhead. Each operation runs a fixed number of
//! calls to completion so micromeasure can report per-call timing and PMU data.

use std::{sync::Arc, time::Duration};

use micromeasure::{
    BenchContext, BenchmarkMainOptions, BenchmarkRuntimeOptions, LinuxPerfBackend, Throughput,
    benchmark_main, black_box,
};

use moor_common::{
    model::{
        CommitResult, DispatchFlagsSource, ObjFlag, ObjectKind, TaskPermissions, VerbArgsSpec,
        VerbDispatch, VerbFlag, VerbLookup, WorldState, WorldStateSource,
    },
    tasks::{AbortLimitReason, NoopClientSession, Session},
    util::BitEnum,
};
use moor_compiler::{CompileOptions, compile};
use moor_db::{DatabaseConfig, TxDB};
use moor_kernel::{
    config::FeaturesConfig,
    task_context::{TaskGuard, rollback_current_transaction},
    tasks::{TaskProgramCache, task_scheduler_client::TaskSchedulerClient},
    testing::vm_test_utils::test_scheduler_for_db,
    vm::{VMHostResponse, builtins::BuiltinRegistry, vm_host::VmHost},
};
use moor_var::{List, NOTHING, SYSTEM_OBJECT, Symbol, program::ProgramType, v_empty_str, v_obj};

fn system_permissions() -> TaskPermissions {
    TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new())
}

fn create_db_with_verbs(inner_verb_code: &str, outer_verb_code: &str) -> TxDB {
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

    let inner_program = compile(inner_verb_code, CompileOptions::default()).unwrap();
    tx.add_verb(
        &system_permissions(),
        &SYSTEM_OBJECT,
        vec![Symbol::mk("inner")],
        &SYSTEM_OBJECT,
        VerbFlag::rxd(),
        VerbArgsSpec::this_none_this(),
        ProgramType::MooR(inner_program),
    )
    .unwrap();

    let outer_program = compile(outer_verb_code, CompileOptions::default()).unwrap();
    tx.add_verb(
        &system_permissions(),
        &SYSTEM_OBJECT,
        vec![Symbol::mk("outer")],
        &SYSTEM_OBJECT,
        VerbFlag::rxd(),
        VerbArgsSpec::this_none_this(),
        ProgramType::MooR(outer_program),
    )
    .unwrap();

    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    ws_source
}

fn prepare_call_verb(
    world_state: &mut dyn WorldState,
    verb_name: &str,
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
        panic!("Could not resolve benchmark verb");
    };
    let (program, _) = world_state
        .retrieve_verb(
            &system_permissions(),
            &verb_result.program_key.verb_definer,
            verb_result.program_key.verb_uuid,
        )
        .unwrap();
    // Use wizard + programmer flags for benchmarking
    let permissions_flags = BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer;
    vm_host.start_call_method_verb(
        0,
        verb_result.verbdef,
        verb_name,
        v_obj(SYSTEM_OBJECT),
        SYSTEM_OBJECT,
        List::mk_list(&[]),
        v_obj(SYSTEM_OBJECT),
        v_empty_str(),
        permissions_flags,
        program,
    );
    vm_host
}

fn build_outer_call_loop(num_calls: u64, callsites_per_iteration: u64, call_expr: &str) -> String {
    assert!(callsites_per_iteration > 0);
    assert_eq!(
        num_calls % callsites_per_iteration,
        0,
        "num_calls must be divisible by callsites_per_iteration"
    );
    let outer_iterations = num_calls / callsites_per_iteration;
    let mut body = String::new();
    for _ in 0..callsites_per_iteration {
        body.push_str(call_expr);
        body.push(';');
    }
    format!("for i in [1..{outer_iterations}] {body} endfor")
}

/// Run the VM until completion (for fixed iteration counts)
fn execute_to_completion(
    session: &dyn Session,
    vm_host: &mut VmHost,
    builtins: &BuiltinRegistry,
    config: &FeaturesConfig,
    program_cache: &mut TaskProgramCache,
) {
    vm_host.reset_ticks();
    vm_host.reset_time();

    loop {
        match vm_host.exec_interpreter(0, session, builtins, config, program_cache) {
            VMHostResponse::ContinueOk => continue,
            VMHostResponse::CompleteSuccess(_) => return,
            VMHostResponse::AbortLimit(AbortLimitReason::Ticks(t)) => {
                panic!("Ran out of ticks at {t}")
            }
            VMHostResponse::CompleteException(e) => panic!("Exception: {:?}", e),
            VMHostResponse::AbortLimit(AbortLimitReason::Time(_)) => {
                panic!("Unexpected time abort")
            }
            VMHostResponse::AbortLimit(AbortLimitReason::OutputEvents(_)) => {
                panic!("Unexpected captured output event abort")
            }
            VMHostResponse::AbortLimit(AbortLimitReason::OutputBytes(_)) => {
                panic!("Unexpected captured output abort")
            }
            VMHostResponse::DispatchFork(_) => panic!("Unexpected fork"),
            VMHostResponse::Suspend(_) => panic!("Unexpected suspend"),
            VMHostResponse::SuspendNeedInput(_) => panic!("Unexpected suspend need input"),
            VMHostResponse::CompleteAbort => panic!("Unexpected abort"),
            VMHostResponse::RollbackRetry => panic!("Unexpected rollback retry"),
            VMHostResponse::CompleteRollback(_) => panic!("Unexpected complete rollback"),
        }
    }
}

const NUM_CALLS: u64 = 10_000;
const VERB_DISPATCH_INVOCATIONS_PER_CHUNK: usize = 20;
const LOOP_BASELINE_INVOCATIONS_PER_CHUNK: usize = 640;
const MAX_TICKS: usize = (NUM_CALLS * 20) as usize;

struct VerbDispatchContext {
    db: TxDB,
    session: Arc<dyn Session>,
    task_scheduler_client: TaskSchedulerClient,
    builtins: BuiltinRegistry,
    features: FeaturesConfig,
    program_cache: TaskProgramCache,
}

impl VerbDispatchContext {
    fn new(inner_verb_code: &str, outer_verb_code: &str) -> Self {
        let db = create_db_with_verbs(inner_verb_code, outer_verb_code);
        let scheduler = test_scheduler_for_db(db.clone());
        Self {
            db,
            session: Arc::new(NoopClientSession::new()),
            task_scheduler_client: TaskSchedulerClient::new(0, scheduler),
            builtins: BuiltinRegistry::new(),
            features: FeaturesConfig::default(),
            program_cache: TaskProgramCache::default(),
        }
    }
}

impl BenchContext for VerbDispatchContext {
    fn prepare(_chunk_size: usize) -> Self {
        Self::new(
            "return 1;",
            &build_outer_call_loop(NUM_CALLS, 1, "this:inner()"),
        )
    }

    fn chunk_size() -> Option<usize> {
        Some(VERB_DISPATCH_INVOCATIONS_PER_CHUNK)
    }

    fn operations_per_chunk() -> Option<u64> {
        Some(NUM_CALLS * VERB_DISPATCH_INVOCATIONS_PER_CHUNK as u64)
    }
}

struct LoopBaselineContext(VerbDispatchContext);

impl BenchContext for LoopBaselineContext {
    fn prepare(_chunk_size: usize) -> Self {
        Self(VerbDispatchContext::new(
            "return 1;",
            &format!("for i in [1..{NUM_CALLS}] 1; endfor"),
        ))
    }

    fn chunk_size() -> Option<usize> {
        Some(LOOP_BASELINE_INVOCATIONS_PER_CHUNK)
    }

    fn operations_per_chunk() -> Option<u64> {
        Some(NUM_CALLS * LOOP_BASELINE_INVOCATIONS_PER_CHUNK as u64)
    }
}

fn run_verb_dispatch(ctx: &mut VerbDispatchContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let mut tx = ctx.db.new_world_state().unwrap();
        let mut vm_host = prepare_call_verb(tx.as_mut(), "outer", MAX_TICKS);
        let _task_guard = TaskGuard::new(
            tx,
            ctx.task_scheduler_client.clone(),
            0,
            NOTHING,
            ctx.session.clone(),
        );
        execute_to_completion(
            ctx.session.as_ref(),
            &mut vm_host,
            &ctx.builtins,
            &ctx.features,
            &mut ctx.program_cache,
        );
        rollback_current_transaction().unwrap();
        black_box(());
    }
}

fn run_for_loop_baseline(ctx: &mut LoopBaselineContext, chunk_size: usize, chunk_num: usize) {
    run_verb_dispatch(&mut ctx.0, chunk_size, chunk_num);
}

fn context_factory(inner_verb_code: &str, callsites: u64, call_expr: &str) -> VerbDispatchContext {
    VerbDispatchContext::new(
        inner_verb_code,
        &build_outer_call_loop(NUM_CALLS, callsites, call_expr),
    )
}

benchmark_main!(
    BenchmarkMainOptions {
        filter_help: Some("all, minimal_inner, multisite, locals, args, or baseline".to_string()),
        runtime: BenchmarkRuntimeOptions {
            warm_up_duration: Duration::from_millis(250),
            benchmark_duration: Duration::from_secs(1),
            min_samples: 8,
            max_samples: 24,
        },
        ..BenchmarkMainOptions::default()
    },
    |runner| {
        runner.set_case_cooldown(Duration::from_millis(500));
        runner.group::<VerbDispatchContext>("verb_dispatch", |g| {
            let g = g
                .throughput(Throughput::per_operation(1, "verb_calls"))
                .backend(|| {
                    Box::new(
                        LinuxPerfBackend::new()
                            .with_compact_counters()
                            .with_rapl_energy(),
                    )
                });
            g.factory(&|| context_factory("return 1;", 1, "this:inner()"))
                .bench("minimal_inner", run_verb_dispatch);
            g.factory(&|| context_factory("return 1;", 16, "this:inner()"))
                .bench("minimal_inner_multisite_16", run_verb_dispatch);
            g.factory(&|| context_factory("x = 1; y = 2; return x + y;", 1, "this:inner()"))
                .bench("inner_with_locals", run_verb_dispatch);
            g.factory(&|| context_factory("return args[1] + args[2];", 1, "this:inner(1, 2)"))
                .bench("inner_with_args", run_verb_dispatch);
        });

        runner.group::<LoopBaselineContext>("verb_dispatch_baseline", |g| {
            g.throughput(Throughput::per_operation(1, "loop_iterations"))
                .backend(|| {
                    Box::new(
                        LinuxPerfBackend::new()
                            .with_compact_counters()
                            .with_rapl_energy(),
                    )
                })
                .bench("for_loop_only", run_for_loop_baseline);
        });
    }
);
