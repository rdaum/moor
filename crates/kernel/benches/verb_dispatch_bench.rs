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
//! isolating from scheduler overhead. Uses tick exhaustion like vm_benches.

use std::{hint::black_box, sync::Arc, time::Duration};

use criterion::{Criterion, criterion_group, criterion_main};

use moor_common::{
    model::{
        CommitResult, DispatchFlagsSource, ObjFlag, ObjectKind, VerbArgsSpec, VerbDispatch,
        VerbFlag, VerbLookup, WorldState, WorldStateSource,
    },
    tasks::{AbortLimitReason, NoopClientSession, Session},
    util::BitEnum,
};
use moor_compiler::{CompileOptions, compile};
use moor_db::{DatabaseConfig, TxDB};
use moor_kernel::{
    config::FeaturesConfig,
    tasks::TaskProgramCache,
    testing::vm_test_utils::setup_task_context,
    vm::{VMHostResponse, builtins::BuiltinRegistry, vm_host::VmHost},
};
use moor_var::{List, NOTHING, SYSTEM_OBJECT, Symbol, program::ProgramType, v_empty_str, v_obj};

fn create_db_with_verbs(inner_verb_code: &str, outer_verb_code: &str) -> TxDB {
    let (ws_source, _) = TxDB::open(None, DatabaseConfig::default());
    let mut tx = ws_source.new_world_state().unwrap();

    let _sysobj = tx
        .create_object(
            &SYSTEM_OBJECT,
            &NOTHING,
            &SYSTEM_OBJECT,
            ObjFlag::all_flags(),
            ObjectKind::NextObjid,
        )
        .unwrap();

    let inner_program = compile(inner_verb_code, CompileOptions::default()).unwrap();
    tx.add_verb(
        &SYSTEM_OBJECT,
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
        &SYSTEM_OBJECT,
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
            &SYSTEM_OBJECT,
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
            &SYSTEM_OBJECT,
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
fn execute_to_completion(session: Arc<dyn Session>, vm_host: &mut VmHost) {
    vm_host.reset_ticks();
    vm_host.reset_time();

    let config = FeaturesConfig::default();
    let builtins = BuiltinRegistry::new();
    let mut program_cache = TaskProgramCache::default();

    loop {
        match vm_host.exec_interpreter(0, session.as_ref(), &builtins, &config, &mut program_cache)
        {
            VMHostResponse::ContinueOk => continue,
            VMHostResponse::CompleteSuccess(_) => return,
            VMHostResponse::AbortLimit(AbortLimitReason::Ticks(t)) => {
                panic!("Ran out of ticks at {t}")
            }
            VMHostResponse::CompleteException(e) => panic!("Exception: {:?}", e),
            VMHostResponse::AbortLimit(AbortLimitReason::Time(_)) => {
                panic!("Unexpected time abort")
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

fn verb_dispatch_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("verb_dispatch");
    group.sample_size(20);

    // Number of verb calls per benchmark iteration
    let num_calls: u64 = 10_000;
    let max_ticks = (num_calls * 20) as usize; // plenty of headroom

    // Throughput is verb calls - so we get per-call timing
    group.throughput(criterion::Throughput::Elements(num_calls));

    // Benchmark: minimal inner verb - pure dispatch overhead
    group.bench_function("minimal_inner", |b| {
        let db = create_db_with_verbs(
            "return 1;",
            &build_outer_call_loop(num_calls, 1, "this:inner()"),
        );

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            // Fresh vm_host each iteration (verb runs to completion)
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "outer", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // Same total number of calls, but spread across multiple static callsites.
    // This isolates callsite-cache overhead from verb body work.
    group.bench_function("minimal_inner_multisite_16", |b| {
        let db = create_db_with_verbs(
            "return 1;",
            &build_outer_call_loop(num_calls, 16, "this:inner()"),
        );

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            // Fresh vm_host each iteration (verb runs to completion)
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "outer", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // Inner verb with some local variable work
    group.bench_function("inner_with_locals", |b| {
        let db = create_db_with_verbs(
            "x = 1; y = 2; return x + y;",
            &build_outer_call_loop(num_calls, 1, "this:inner()"),
        );

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "outer", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    // Passing arguments to inner verb
    group.bench_function("inner_with_args", |b| {
        let db = create_db_with_verbs(
            "return args[1] + args[2];",
            &build_outer_call_loop(num_calls, 1, "this:inner(1, 2)"),
        );

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "outer", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    group.finish();
}

/// Baseline: measure for-loop overhead without verb calls for comparison
fn baseline_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("verb_dispatch_baseline");
    group.sample_size(20);

    let num_iterations: u64 = 10_000;
    let max_ticks = (num_iterations * 10) as usize;

    // Same iteration count as verb dispatch benches for fair comparison
    group.throughput(criterion::Throughput::Elements(num_iterations));

    // Pure for-loop - no verb calls
    group.bench_function("for_loop_only", |b| {
        let db = create_db_with_verbs(
            "return 1;",
            &format!("for i in [1..{num_iterations}] 1; endfor"),
        );

        let session = Arc::new(NoopClientSession::new());

        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "outer", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    group.finish();
}

criterion_group!(benches, verb_dispatch_benchmarks, baseline_benchmarks);
criterion_main!(benches);
