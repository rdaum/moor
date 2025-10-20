// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Benchmarks of various virtual machine executions
//! In general attempting to keep isolated from the object/world-state and simply execute
//! program code that doesn't interact with the DB, to measure opcode execution efficiency.
#![recursion_limit = "256"]

use std::{hint::black_box, sync::Arc, time::Duration};

use criterion::{Criterion, criterion_group, criterion_main};

use moor_common::{
    model::{CommitResult, ObjectKind, VerbArgsSpec, VerbFlag, WorldState, WorldStateSource},
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
use moor_var::{List, NOTHING, SYSTEM_OBJECT, Symbol, program::ProgramType, v_obj};

fn create_db() -> TxDB {
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
    assert!(matches!(tx.commit(), Ok(CommitResult::Success { .. })));
    ws_source
}

pub fn prepare_call_verb(
    world_state: &mut dyn WorldState,
    verb_name: &str,
    args: List,
    max_ticks: usize,
) -> VmHost {
    let mut vm_host = VmHost::new(0, 20, max_ticks, Duration::from_secs(1000));

    let verb_name = Symbol::mk(verb_name);
    let (program, verbdef) = world_state
        .find_method_verb_on(&SYSTEM_OBJECT, &SYSTEM_OBJECT, verb_name)
        .unwrap();
    vm_host.start_call_method_verb(
        0,
        SYSTEM_OBJECT,
        verbdef,
        verb_name,
        v_obj(SYSTEM_OBJECT),
        SYSTEM_OBJECT,
        args,
        v_obj(SYSTEM_OBJECT),
        "".to_string(),
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
        &SYSTEM_OBJECT,
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

/// Run the vm host until it runs out of ticks
fn execute(session: Arc<dyn Session>, vm_host: &mut VmHost) -> usize {
    vm_host.reset_ticks();
    vm_host.reset_time();

    let config = FeaturesConfig::default();

    // Call repeatedly into exec until we ge either an error or Complete.
    loop {
        match vm_host.exec_interpreter(0, session.as_ref(), &BuiltinRegistry::new(), &config) {
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
            VMHostResponse::DispatchFork(f) => {
                panic!("Unexpected fork: {f:?}");
            }
            VMHostResponse::CompleteException(e) => {
                panic!("Unexpected exception: {e:?}")
            }
            VMHostResponse::Suspend(_) => {
                panic!("Unexpected suspend");
            }
            VMHostResponse::SuspendNeedInput => {
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

fn do_program(
    state_source: TxDB,
    program: &str,
    max_ticks: usize,
    iters: u64,
) -> (Duration, usize) {
    let mut cumulative_time = Duration::new(0, 0);
    let mut cumulative_ticks = 0;
    let mut vm_host = prepare_vm_execution(&state_source, program, max_ticks);
    let tx = state_source.new_world_state().unwrap();
    let session = Arc::new(NoopClientSession::new());
    // Set up transaction context for benchmarking
    let _tx_guard = setup_task_context(tx);

    for _ in 0..iters {
        let start = std::time::Instant::now();
        let t = black_box(execute(session.clone(), &mut vm_host));
        cumulative_ticks += t;
        cumulative_time += start.elapsed();
    }

    // Transaction will be cleaned up automatically by tx_guard drop

    drop(state_source);
    (cumulative_time, cumulative_ticks)
}

fn opcode_throughput(c: &mut Criterion) {
    let db = create_db();

    let mut group = c.benchmark_group("opcode_throughput");
    group.sample_size(20);

    let num_ticks = 100000000;
    group.throughput(criterion::Throughput::Elements(num_ticks as u64));
    group.bench_function("while_loop", |b| {
        b.iter_custom(|iters| do_program(db.clone(), "while (1) endwhile", num_ticks, iters).0);
    });
    group.bench_function("while_increment_var_loop", |b| {
        b.iter_custom(|iters| {
            do_program(
                db.clone(),
                "i = 0; while(1) i=i+1; endwhile",
                num_ticks,
                iters,
            )
            .0
        });
    });
    group.bench_function("for_in_range_loop", |b| {
        b.iter_custom(|iters| {
            do_program(
                db.clone(),
                "while(1) for i in [1..1000000] endfor endwhile",
                num_ticks,
                iters,
            )
            .0
        });
    });
    // Measure range iteration over a static list

    group.bench_function("for_in_static_list_loop", |b| {
        b.iter_custom(|iters| {
            do_program(db.clone(),
                       r#"while(1)
                            for i in ({1,2,3,4,5,6,7,8,9,10,1,2,3,4,5,6,7,8,9,10,1,2,3,4,5,6,7,8,9,10,1,2,3,4,5,6,7,8,9,10})
                            endfor
                          endwhile"#,
                       num_ticks,
                       iters,
            ).0
        });
    });
    // Measure how costly it is to append to a list
    group.bench_function("list_append_loop", |b| {
        b.iter_custom(|iters| {
            do_program(
                db.clone(),
                r#"while(1)
                            base_list = {};
                            for i in [0..1000]
                                base_list = {@base_list, i};
                            endfor
                          endwhile"#,
                num_ticks,
                iters,
            )
            .0
        });
    });
    // Measure how costly it is to append to a list
    group.bench_function("list_set", |b| {
        b.iter_custom(|iters| {
            do_program(
                db.clone(),
                r#"while(1)
                            l = {1};
                            for i in [0..10000] 
                                l[1] = i;
                            endfor
                          endwhile"#,
                num_ticks,
                iters,
            )
            .0
        });
    });
    group.finish();
}

fn dispatch_micro_benchmarks(c: &mut Criterion) {
    let db = create_db();

    let mut group = c.benchmark_group("dispatch_micro");
    group.sample_size(20);

    let num_ticks = 100000000;
    group.throughput(criterion::Throughput::Elements(num_ticks as u64));

    // Tightest possible loop: just discard constants
    // This measures pure dispatch overhead with minimal instruction work
    group.bench_function("dispatch_constant_discard", |b| {
        b.iter_custom(|iters| do_program(db.clone(), "while(1) 1; endwhile", num_ticks, iters).0);
    });

    // Push/Pop only - measures stack operations dispatch
    group.bench_function("dispatch_push_pop", |b| {
        b.iter_custom(|iters| {
            do_program(db.clone(), "i=0; while(1) i; endwhile", num_ticks, iters).0
        });
    });

    // Simple arithmetic - measures dispatch + one operation
    group.bench_function("dispatch_simple_add", |b| {
        b.iter_custom(|iters| {
            do_program(db.clone(), "while(1) 1 + 1; endwhile", num_ticks, iters).0
        });
    });

    // Comparison only
    group.bench_function("dispatch_comparison", |b| {
        b.iter_custom(|iters| {
            do_program(db.clone(), "while(1) 1 == 1; endwhile", num_ticks, iters).0
        });
    });

    group.finish();
}

criterion_group!(benches, opcode_throughput, dispatch_micro_benchmarks);
criterion_main!(benches);
