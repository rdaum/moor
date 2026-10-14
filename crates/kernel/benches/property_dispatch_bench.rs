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

//! Microbenchmark for property access overhead.
//! Measures getprop/putprop-heavy loops through the VM execution loop,
//! isolating from scheduler overhead.

use std::{sync::Arc, time::Duration};

use micromeasure::{
    BenchContext, BenchmarkMainOptions, BenchmarkRuntimeOptions, LinuxPerfBackend, Throughput,
    benchmark_main, black_box,
};

use moor_common::{
    model::{
        CommitResult, DispatchFlagsSource, ObjFlag, ObjectKind, PropFlag, TaskPermissions,
        VerbArgsSpec, VerbDispatch, VerbFlag, VerbLookup, WorldState, WorldStateSource,
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
use moor_var::{
    List, NOTHING, SYSTEM_OBJECT, Symbol, program::ProgramType, v_empty_str, v_int, v_obj,
};

fn system_permissions() -> TaskPermissions {
    TaskPermissions::new(SYSTEM_OBJECT, BitEnum::new())
}

fn create_db_with_property_outer(outer_verb_code: &str) -> TxDB {
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

    tx.define_property(
        &system_permissions(),
        &SYSTEM_OBJECT,
        &SYSTEM_OBJECT,
        Symbol::mk("p"),
        &SYSTEM_OBJECT,
        BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
        Some(v_int(0)),
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
    vm_host.start_call_method_verb(
        0,
        verb_result.verbdef,
        verb_name,
        v_obj(SYSTEM_OBJECT),
        SYSTEM_OBJECT,
        List::mk_list(&[]),
        v_obj(SYSTEM_OBJECT),
        v_empty_str(),
        verb_result.permissions_flags,
        program,
    );
    vm_host
}

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

fn build_outer_loop(num_ops: u64, callsites_per_iteration: u64, op_expr: &str) -> String {
    assert!(callsites_per_iteration > 0);
    assert_eq!(
        num_ops % callsites_per_iteration,
        0,
        "num_ops must be divisible by callsites_per_iteration"
    );
    let outer_iterations = num_ops / callsites_per_iteration;
    let mut body = String::new();
    for _ in 0..callsites_per_iteration {
        body.push_str(op_expr);
        body.push(';');
    }
    format!("x = 0; for i in [1..{outer_iterations}] {body} endfor return x;")
}

const NUM_OPS: u64 = 10_000;
const PROPERTY_INVOCATIONS_PER_CHUNK: usize = 20;
const LOOP_BASELINE_INVOCATIONS_PER_CHUNK: usize = 640;
const PROPERTY_MAX_TICKS: usize = (NUM_OPS * 25) as usize;
const BASELINE_MAX_TICKS: usize = (NUM_OPS * 10) as usize;

struct PropertyDispatchContext {
    db: TxDB,
    session: Arc<dyn Session>,
    task_scheduler_client: TaskSchedulerClient,
    builtins: BuiltinRegistry,
    features: FeaturesConfig,
    program_cache: TaskProgramCache,
    max_ticks: usize,
}

impl PropertyDispatchContext {
    fn new(outer_verb_code: &str, max_ticks: usize) -> Self {
        let db = create_db_with_property_outer(outer_verb_code);
        let scheduler = test_scheduler_for_db(db.clone());
        Self {
            db,
            session: Arc::new(NoopClientSession::new()),
            task_scheduler_client: TaskSchedulerClient::new(0, scheduler),
            builtins: BuiltinRegistry::new(),
            features: FeaturesConfig::default(),
            program_cache: TaskProgramCache::default(),
            max_ticks,
        }
    }
}

impl BenchContext for PropertyDispatchContext {
    fn prepare(_chunk_size: usize) -> Self {
        property_context(1, "x = this.p")
    }

    fn chunk_size() -> Option<usize> {
        Some(PROPERTY_INVOCATIONS_PER_CHUNK)
    }

    fn operations_per_chunk() -> Option<u64> {
        Some(NUM_OPS * PROPERTY_INVOCATIONS_PER_CHUNK as u64)
    }
}

struct LoopBaselineContext(PropertyDispatchContext);

impl BenchContext for LoopBaselineContext {
    fn prepare(_chunk_size: usize) -> Self {
        Self(PropertyDispatchContext::new(
            &build_outer_loop(NUM_OPS, 1, "x = 1"),
            BASELINE_MAX_TICKS,
        ))
    }

    fn chunk_size() -> Option<usize> {
        Some(LOOP_BASELINE_INVOCATIONS_PER_CHUNK)
    }

    fn operations_per_chunk() -> Option<u64> {
        Some(NUM_OPS * LOOP_BASELINE_INVOCATIONS_PER_CHUNK as u64)
    }
}

fn property_context(callsites: u64, expression: &str) -> PropertyDispatchContext {
    PropertyDispatchContext::new(
        &build_outer_loop(NUM_OPS, callsites, expression),
        PROPERTY_MAX_TICKS,
    )
}

fn run_property_dispatch(ctx: &mut PropertyDispatchContext, chunk_size: usize, _chunk_num: usize) {
    for _ in 0..chunk_size {
        let mut tx = ctx.db.new_world_state().unwrap();
        let mut vm_host = prepare_call_verb(tx.as_mut(), "outer", ctx.max_ticks);
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

fn run_loop_baseline(ctx: &mut LoopBaselineContext, chunk_size: usize, chunk_num: usize) {
    run_property_dispatch(&mut ctx.0, chunk_size, chunk_num);
}

benchmark_main!(
    BenchmarkMainOptions {
        filter_help: Some("all, getprop, putprop, or baseline".to_string()),
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
        runner.group::<PropertyDispatchContext>("property_dispatch", |g| {
            let g = g
                .throughput(Throughput::per_operation(1, "property_ops"))
                .backend(|| {
                    Box::new(
                        LinuxPerfBackend::new()
                            .with_compact_counters()
                            .with_rapl_energy(),
                    )
                });
            g.factory(&|| property_context(1, "x = this.p"))
                .bench("getprop_single_site", run_property_dispatch);
            g.factory(&|| property_context(16, "x = this.p"))
                .bench("getprop_multisite_16", run_property_dispatch);
            g.factory(&|| property_context(1, "this.p = i"))
                .bench("putprop_single_site", run_property_dispatch);
        });

        runner.group::<LoopBaselineContext>("property_dispatch_baseline", |g| {
            g.throughput(Throughput::per_operation(1, "loop_iterations"))
                .backend(|| {
                    Box::new(
                        LinuxPerfBackend::new()
                            .with_compact_counters()
                            .with_rapl_energy(),
                    )
                })
                .bench("for_loop_only", run_loop_baseline);
        });
    }
);
