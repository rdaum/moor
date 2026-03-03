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

//! Microbenchmark for property access overhead.
//! Measures getprop/putprop-heavy loops through the VM execution loop,
//! isolating from scheduler overhead.

use std::{hint::black_box, sync::Arc, time::Duration};

use criterion::{Criterion, criterion_group, criterion_main};

use moor_common::{
    model::{
        CommitResult, ObjFlag, ObjectKind, PropFlag, VerbArgsSpec, VerbFlag, WorldState,
        WorldStateSource,
    },
    tasks::{AbortLimitReason, NoopClientSession, Session},
    util::BitEnum,
};
use moor_compiler::{CompileOptions, compile};
use moor_db::{DatabaseConfig, TxDB};
use moor_kernel::{
    config::FeaturesConfig,
    testing::vm_test_utils::setup_task_context,
    vm::{VMHostResponse, builtins::BuiltinRegistry, vm_host::VmHost},
};
use moor_var::{
    List, NOTHING, SYSTEM_OBJECT, Symbol, program::ProgramType, v_empty_str, v_int, v_obj,
};

fn create_db_with_property_outer(outer_verb_code: &str) -> TxDB {
    let (ws_source, _) = TxDB::open(None, DatabaseConfig::default());
    let mut tx = ws_source.new_world_state().unwrap();

    let _sysobj = tx
        .create_object(
            &SYSTEM_OBJECT,
            &NOTHING,
            &SYSTEM_OBJECT,
            BitEnum::all(),
            ObjectKind::NextObjid,
        )
        .unwrap();

    tx.define_property(
        &SYSTEM_OBJECT,
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
    let (program, verbdef) = world_state
        .find_method_verb_on(&SYSTEM_OBJECT, &SYSTEM_OBJECT, verb_name)
        .unwrap();
    let permissions_flags = BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer;
    vm_host.start_call_method_verb(
        0,
        verbdef,
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

fn execute_to_completion(session: Arc<dyn Session>, vm_host: &mut VmHost) {
    vm_host.reset_ticks();
    vm_host.reset_time();

    let config = FeaturesConfig::default();
    let builtins = BuiltinRegistry::new();

    loop {
        match vm_host.exec_interpreter(0, session.as_ref(), &builtins, &config) {
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

fn property_dispatch_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("property_dispatch");
    group.sample_size(20);

    let num_ops: u64 = 10_000;
    let max_ticks = (num_ops * 25) as usize;
    group.throughput(criterion::Throughput::Elements(num_ops));

    group.bench_function("getprop_single_site", |b| {
        let db = create_db_with_property_outer(&build_outer_loop(num_ops, 1, "x = this.p"));
        let session = Arc::new(NoopClientSession::new());
        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "outer", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    group.bench_function("getprop_multisite_16", |b| {
        let db = create_db_with_property_outer(&build_outer_loop(num_ops, 16, "x = this.p"));
        let session = Arc::new(NoopClientSession::new());
        b.iter(|| {
            let mut tx = db.new_world_state().unwrap();
            let mut vm_host = prepare_call_verb(tx.as_mut(), "outer", max_ticks);
            let _tx_guard = setup_task_context(tx);
            execute_to_completion(session.clone(), &mut vm_host);
            black_box(());
        });
    });

    group.bench_function("putprop_single_site", |b| {
        let db = create_db_with_property_outer(&build_outer_loop(num_ops, 1, "this.p = i"));
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

fn baseline_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("property_dispatch_baseline");
    group.sample_size(20);

    let num_ops: u64 = 10_000;
    let max_ticks = (num_ops * 10) as usize;
    group.throughput(criterion::Throughput::Elements(num_ops));

    group.bench_function("for_loop_only", |b| {
        let db = create_db_with_property_outer(&build_outer_loop(num_ops, 1, "x = 1"));
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

criterion_group!(benches, property_dispatch_benchmarks, baseline_benchmarks);
criterion_main!(benches);
