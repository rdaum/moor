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

//! VM opcode dispatch microbenchmarks using micromeasure.
//! Measures cycles, instructions, and branch behavior per dispatched opcode.

use micromeasure::{
    BenchContext, BenchmarkMainOptions, LinuxPerfBackend, Throughput, benchmark_main, black_box,
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
use moor_var::{List, NOTHING, SYSTEM_OBJECT, Symbol, v_empty_str, v_obj};
use std::{sync::Arc, time::Duration};

const MAX_TICKS: usize = 100_000_000;
const CHUNK_SIZE: usize = 1; // Each "chunk" is one full execution run (100M opcodes)

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
        moor_var::program::ProgramType::MooR(program),
    )
    .unwrap();
    let vm_host = prepare_call_verb(tx.as_mut(), "test", List::mk_list(&[]), max_ticks);
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    vm_host
}

/// Fresh VM and DB infrastructure prepared outside each sample's measurement window.
struct DispatchContext {
    db: TxDB,
    vm_host: VmHost,
    session: Arc<dyn Session>,
    features: FeaturesConfig,
    task_scheduler_client: TaskSchedulerClient,
    builtins: BuiltinRegistry,
    program_cache: TaskProgramCache,
}

impl BenchContext for DispatchContext {
    fn prepare(_num_chunks: usize) -> Self {
        // We use a default program here; actual contexts override this
        let db = create_db();
        let vm_host = prepare_vm_execution(&db, "while (1) 1; endwhile", MAX_TICKS);
        let scheduler = test_scheduler_for_db(db.clone());
        let session = Arc::new(NoopClientSession::new());

        DispatchContext {
            db,
            vm_host,
            session,
            features: FeaturesConfig::default(),
            task_scheduler_client: TaskSchedulerClient::new(0, scheduler),
            builtins: BuiltinRegistry::new(),
            program_cache: TaskProgramCache::default(),
        }
    }

    fn chunk_size() -> Option<usize> {
        Some(CHUNK_SIZE)
    }

    fn operations_per_chunk() -> Option<u64> {
        Some(MAX_TICKS as u64)
    }
}

impl DispatchContext {
    /// Create a context with a specific program
    fn with_program(program: &str) -> Self {
        Self::with_program_and_features(program, FeaturesConfig::default())
    }

    fn with_program_and_features(program: &str, features: FeaturesConfig) -> Self {
        let db = create_db();
        let vm_host = prepare_vm_execution(&db, program, MAX_TICKS);
        let scheduler = test_scheduler_for_db(db.clone());
        let session = Arc::new(NoopClientSession::new());

        DispatchContext {
            db,
            vm_host,
            session,
            features,
            task_scheduler_client: TaskSchedulerClient::new(0, scheduler),
            builtins: BuiltinRegistry::new(),
            program_cache: TaskProgramCache::default(),
        }
    }
}

fn run_dispatch(ctx: &mut DispatchContext) -> usize {
    let tx = ctx.db.new_world_state().unwrap();
    let _task_guard = TaskGuard::new(
        tx,
        ctx.task_scheduler_client.clone(),
        0,
        NOTHING,
        ctx.session.clone(),
    );
    let ticks = execute_until_ticks_with_features(
        ctx.session.as_ref(),
        &mut ctx.vm_host,
        &ctx.features,
        &ctx.builtins,
        &mut ctx.program_cache,
    );
    rollback_current_transaction().unwrap();
    ticks
}

/// Dispatch constant discard: pure dispatch overhead with minimal work
fn dispatch_constant_discard(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Dispatch push/pop: measures stack operation dispatch
fn dispatch_push_pop(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Dispatch simple add: dispatch + one binary operation
fn dispatch_simple_add(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Dispatch comparison: dispatch + comparison operation
fn dispatch_comparison(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Dispatch binary-op chain: dispatch + a denser sequence of arithmetic operations
fn dispatch_binary_chain(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Dispatch dependent local integer update chain.
fn dispatch_dependent_int_chain(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Dispatch several independent local integer update chains.
fn dispatch_independent_int_chains(
    ctx: &mut DispatchContext,
    _chunk_size: usize,
    _chunk_num: usize,
) {
    black_box(run_dispatch(ctx));
}

/// Dispatch local integer arithmetic without storing the result.
fn dispatch_local_add(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Dispatch local complex-value load/drop churn.
fn dispatch_complex_value_clone(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Dispatch scope churn: repeated for-range scope push/pop with a stable source list.
fn dispatch_scope_churn(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Dispatch integer for-range loop control with an empty body.
fn dispatch_for_range_empty(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Dispatch integer for-range loop control plus loop-variable arithmetic.
fn dispatch_for_range_accumulate(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Dispatch jump-only control flow with minimal value work.
fn dispatch_jump_only(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Dispatch expression temporary churn through nested arithmetic and comparison.
fn dispatch_valstack_churn(ctx: &mut DispatchContext, _chunk_size: usize, _chunk_num: usize) {
    black_box(run_dispatch(ctx));
}

/// Execute VM until tick limit reached with explicit feature configuration.
fn execute_until_ticks_with_features(
    session: &dyn Session,
    vm_host: &mut VmHost,
    features: &FeaturesConfig,
    builtins: &BuiltinRegistry,
    program_cache: &mut TaskProgramCache,
) -> usize {
    vm_host.reset_ticks();
    vm_host.reset_time();

    loop {
        match vm_host.exec_interpreter(0, session, builtins, features, program_cache) {
            VMHostResponse::ContinueOk => continue,
            VMHostResponse::AbortLimit(AbortLimitReason::Ticks(t)) => return t,
            _ => panic!("Unexpected VM response"),
        }
    }
}

benchmark_main!(
    BenchmarkMainOptions {
        filter_help: Some("all, dispatch, or any benchmark name substring".to_string()),
        ..BenchmarkMainOptions::default()
    },
    |runner| {
        runner.group::<DispatchContext>("dispatch", |g| {
            let g = g.backend(|| Box::new(LinuxPerfBackend::new().with_compact_counters()));
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| DispatchContext::with_program("while(1) 1; endwhile"))
                .bench(
                    "dispatch_constant_discard",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_constant_discard(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| DispatchContext::with_program("i=0; while(1) i; endwhile"))
                .bench(
                    "dispatch_push_pop",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_push_pop(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| DispatchContext::with_program("while(1) 1 + 1; endwhile"))
                .bench(
                    "dispatch_simple_add",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_simple_add(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| DispatchContext::with_program("while(1) 1 == 1; endwhile"))
                .bench(
                    "dispatch_comparison",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_comparison(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| {
                    let cfg = FeaturesConfig {
                        use_boolean_returns: false,
                        ..FeaturesConfig::default()
                    };
                    DispatchContext::with_program_and_features("while(1) 1 == 1; endwhile", cfg)
                })
                .bench(
                    "dispatch_comparison_int_returns",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_comparison(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| {
                    let cfg = FeaturesConfig {
                        use_boolean_returns: true,
                        ..FeaturesConfig::default()
                    };
                    DispatchContext::with_program_and_features("while(1) 1 == 1; endwhile", cfg)
                })
                .bench(
                    "dispatch_comparison_bool_returns",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_comparison(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| {
                    DispatchContext::with_program("i = 1; while(1) i + 1 + 2 + 3 + 4 + 5; endwhile")
                })
                .bench(
                    "dispatch_binary_chain",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_binary_chain(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| DispatchContext::with_program("i = 0; while(1) i = i + 1; endwhile"))
                .bench(
                    "dispatch_dependent_int_chain",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_dependent_int_chain(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| {
                    DispatchContext::with_program(
                        "a = 0; b = 0; c = 0; d = 0; while(1) a = a + 1; b = b + 1; c = c + 1; d = d + 1; endwhile",
                    )
                })
                .bench(
                    "dispatch_independent_int_chains",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_independent_int_chains(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| DispatchContext::with_program("i = 1; while(1) i + 1; endwhile"))
                .bench(
                    "dispatch_local_add",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_local_add(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| {
                    DispatchContext::with_program("values = {1, 2, 3, 4}; while(1) values; endwhile")
                })
                .bench(
                    "dispatch_complex_value_clone",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_complex_value_clone(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| {
                    DispatchContext::with_program(
                        "values = {1, 2, 3, 4}; while(1) for value in (values) value; endfor endwhile",
                    )
                })
                .bench(
                    "dispatch_scope_churn",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_scope_churn(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| DispatchContext::with_program("while(1) for i in [1..5] endfor endwhile"))
                .bench(
                    "dispatch_for_range_empty",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_for_range_empty(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| {
                    DispatchContext::with_program(
                        "z = 0; while(1) for i in [1..5] z = z + i; endfor endwhile",
                    )
                })
                .bench(
                    "dispatch_for_range_accumulate",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_for_range_accumulate(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| DispatchContext::with_program("while(1) if (1) 1; endif endwhile"))
                .bench(
                    "dispatch_jump_only",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_jump_only(ctx, 1, 0)
                    },
                );
            g.throughput(Throughput::per_operation(1, "opcodes"))
                .factory(&|| {
                    DispatchContext::with_program(
                        "while(1) ((1 + 2) * (3 + 4)) == ((5 + 6) * (7 + 8)); endwhile",
                    )
                })
                .bench(
                    "dispatch_valstack_churn",
                    |ctx: &mut DispatchContext, _chunk_size, _chunk_num| {
                        dispatch_valstack_churn(ctx, 1, 0)
                    },
                );
        });
    }
);
