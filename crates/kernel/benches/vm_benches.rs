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

//! Benchmarks of virtual machine execution throughput.
//!
//! These keep the measured programs isolated from object/world-state mutation and run the
//! interpreter to a tick limit so the result is mostly opcode execution cost.

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

const MAX_TICKS: usize = 100_000_000;
const CHUNK_SIZE: usize = 1;

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
        ProgramType::MooR(program),
    )
    .unwrap();
    let vm_host = prepare_call_verb(tx.as_mut(), "test", List::mk_list(&[]), max_ticks);
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    vm_host
}

fn execute(
    session: &dyn Session,
    vm_host: &mut VmHost,
    builtins: &BuiltinRegistry,
    config: &FeaturesConfig,
    program_cache: &mut TaskProgramCache,
) -> usize {
    vm_host.reset_ticks();
    vm_host.reset_time();

    loop {
        match vm_host.exec_interpreter(0, session, builtins, config, program_cache) {
            VMHostResponse::ContinueOk => {
                continue;
            }
            VMHostResponse::AbortLimit(AbortLimitReason::Ticks(t)) => {
                return t;
            }
            VMHostResponse::CompleteSuccess(_) => {
                panic!("Unexpected success");
            }
            VMHostResponse::AbortLimit(AbortLimitReason::Time(time)) => {
                panic!("Unexpected abort: {time:?}");
            }
            VMHostResponse::AbortLimit(AbortLimitReason::OutputEvents(events)) => {
                panic!("Unexpected captured output event abort: {events} events");
            }
            VMHostResponse::AbortLimit(AbortLimitReason::OutputBytes(bytes)) => {
                panic!("Unexpected captured output abort: {bytes} bytes");
            }
            VMHostResponse::DispatchFork(f) => {
                panic!("Unexpected fork: {f:?}");
            }
            VMHostResponse::CompleteException(e) => {
                panic!("Unexpected exception: {e:?}")
            }
            VMHostResponse::Suspend(_) => {
                panic!("Unexpected suspend");
            }
            VMHostResponse::SuspendNeedInput(_) => {
                panic!("Unexpected suspend need input");
            }
            VMHostResponse::CompleteAbort => {
                panic!("Unexpected abort");
            }
            VMHostResponse::RollbackRetry => {
                panic!("Unexpected rollback retry");
            }
            VMHostResponse::CompleteRollback(_) => {
                panic!("Unexpected rollback abort");
            }
        }
    }
}

struct VmBenchContext {
    db: TxDB,
    vm_host: VmHost,
    session: Arc<dyn Session>,
    task_scheduler_client: TaskSchedulerClient,
    builtins: BuiltinRegistry,
    features: FeaturesConfig,
    program_cache: TaskProgramCache,
}

impl BenchContext for VmBenchContext {
    fn prepare(_num_chunks: usize) -> Self {
        Self::with_program("while (1) endwhile")
    }

    fn chunk_size() -> Option<usize> {
        Some(CHUNK_SIZE)
    }

    fn operations_per_chunk() -> Option<u64> {
        Some(MAX_TICKS as u64)
    }
}

impl VmBenchContext {
    fn with_program(program: &str) -> Self {
        let db = create_db();
        let vm_host = prepare_vm_execution(&db, program, MAX_TICKS);
        let scheduler = test_scheduler_for_db(db.clone());
        let session = Arc::new(NoopClientSession::new());

        Self {
            db,
            vm_host,
            session,
            task_scheduler_client: TaskSchedulerClient::new(0, scheduler),
            builtins: BuiltinRegistry::new(),
            features: FeaturesConfig::default(),
            program_cache: TaskProgramCache::default(),
        }
    }
}

fn run_program(ctx: &mut VmBenchContext, _chunk_size: usize, _chunk_num: usize) {
    let tx = ctx.db.new_world_state().unwrap();
    let _task_guard = TaskGuard::new(
        tx,
        ctx.task_scheduler_client.clone(),
        0,
        NOTHING,
        ctx.session.clone(),
    );
    let ticks = execute(
        ctx.session.as_ref(),
        &mut ctx.vm_host,
        &ctx.builtins,
        &ctx.features,
        &mut ctx.program_cache,
    );
    rollback_current_transaction().unwrap();
    black_box(ticks);
}

benchmark_main!(
    BenchmarkMainOptions {
        filter_help: Some("all, opcode, dispatch, or any benchmark name substring".to_string()),
        runtime: BenchmarkRuntimeOptions {
            warm_up_duration: Duration::from_millis(500),
            benchmark_duration: Duration::from_secs(2),
            min_samples: 10,
            max_samples: 20,
        },
        ..BenchmarkMainOptions::default()
    },
    |runner| {
        runner.group::<VmBenchContext>("opcode_throughput", |g| {
            let g = g
                .backend(|| Box::new(LinuxPerfBackend::new().with_compact_counters()))
                .throughput(Throughput::per_operation(1, "opcodes"));
            g.factory(&|| VmBenchContext::with_program("while (1) endwhile"))
                .bench("while_loop", run_program);
            g.factory(&|| VmBenchContext::with_program("i = 0; while(1) i=i+1; endwhile"))
                .bench("while_increment_var_loop", run_program);
            g.factory(&|| {
                VmBenchContext::with_program("while(1) for i in [1..1000000] endfor endwhile")
            })
            .bench("for_in_range_loop", run_program);
            g.factory(&|| {
                VmBenchContext::with_program(
                    r#"while(1)
                            for i in ({1,2,3,4,5,6,7,8,9,10,1,2,3,4,5,6,7,8,9,10,1,2,3,4,5,6,7,8,9,10,1,2,3,4,5,6,7,8,9,10})
                            endfor
                          endwhile"#,
                )
            })
            .bench("for_in_static_list_loop", run_program);
            g.factory(&|| {
                VmBenchContext::with_program(
                    r#"while(1)
                            base_list = {};
                            for i in [0..1000]
                                base_list = {@base_list, i};
                            endfor
                          endwhile"#,
                )
            })
            .bench("list_append_loop", run_program);
            g.factory(&|| {
                VmBenchContext::with_program(
                    r#"while(1)
                            l = {1};
                            for i in [0..10000]
                                l[1] = i;
                            endfor
                          endwhile"#,
                )
            })
            .bench("list_set", run_program);
        });

        runner.group::<VmBenchContext>("dispatch_micro", |g| {
            let g = g
                .backend(|| Box::new(LinuxPerfBackend::new().with_compact_counters()))
                .throughput(Throughput::per_operation(1, "opcodes"));
            g.factory(&|| VmBenchContext::with_program("while(1) 1; endwhile"))
                .bench("dispatch_constant_discard", run_program);
            g.factory(&|| VmBenchContext::with_program("i=0; while(1) i; endwhile"))
                .bench("dispatch_push_pop", run_program);
            g.factory(&|| VmBenchContext::with_program("while(1) 1 + 1; endwhile"))
                .bench("dispatch_simple_add", run_program);
            g.factory(&|| VmBenchContext::with_program("while(1) 1 == 1; endwhile"))
                .bench("dispatch_comparison", run_program);
        });
    }
);
