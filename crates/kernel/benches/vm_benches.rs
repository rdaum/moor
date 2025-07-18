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

use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};

use moor_common::model::CommitResult;
use moor_common::model::VerbArgsSpec;
use moor_common::model::VerbFlag;
use moor_common::model::{WorldState, WorldStateSource};
use moor_common::tasks::AbortLimitReason;
use moor_common::tasks::{NoopClientSession, Session};
use moor_common::util::BitEnum;
use moor_compiler::{CompileOptions, compile};
use moor_db::{DatabaseConfig, TxDB};
use moor_kernel::config::FeaturesConfig;
use moor_kernel::tasks::task_scheduler_client::TaskSchedulerClient;
use moor_kernel::vm::VMHostResponse;
use moor_kernel::vm::VerbCall;
use moor_kernel::vm::builtins::BuiltinRegistry;
use moor_kernel::vm::vm_host::VmHost;
use moor_var::program::ProgramType;
use moor_var::{List, Symbol, v_obj};
use moor_var::{NOTHING, SYSTEM_OBJECT};

fn create_db() -> TxDB {
    let (ws_source, _) = TxDB::open(None, DatabaseConfig::default());
    let mut tx = ws_source.new_world_state().unwrap();
    let _sysobj = tx
        .create_object(&SYSTEM_OBJECT, &NOTHING, &SYSTEM_OBJECT, BitEnum::all())
        .unwrap();
    assert_eq!(tx.commit().unwrap(), CommitResult::Success);
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
        &SYSTEM_OBJECT,
        (program, verbdef),
        VerbCall {
            verb_name,
            location: v_obj(SYSTEM_OBJECT),
            this: v_obj(SYSTEM_OBJECT),
            player: SYSTEM_OBJECT,
            args,
            argstr: "".to_string(),
            caller: v_obj(SYSTEM_OBJECT),
        },
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
    assert_eq!(tx.commit().unwrap(), CommitResult::Success);
    vm_host
}

/// Run the vm host until it runs out of ticks
fn execute(
    world_state: &mut dyn WorldState,
    task_scheduler_client: TaskSchedulerClient,
    session: Arc<dyn Session>,
    vm_host: &mut VmHost,
) -> usize {
    vm_host.reset_ticks();
    vm_host.reset_time();

    let config = FeaturesConfig::default();

    // Call repeatedly into exec until we ge either an error or Complete.
    loop {
        match vm_host.exec_interpreter(
            0,
            world_state,
            &task_scheduler_client,
            session.as_ref(),
            &BuiltinRegistry::new(),
            &config,
        ) {
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
    let mut tx = state_source.new_world_state().unwrap();
    let session = Arc::new(NoopClientSession::new());
    let (scs_tx, _scs_rx) = flume::unbounded();
    let task_scheduler_client = TaskSchedulerClient::new(0, scs_tx);

    for _ in 0..iters {
        let start = std::time::Instant::now();
        let t = black_box(execute(
            tx.as_mut(),
            task_scheduler_client.clone(),
            session.clone(),
            &mut vm_host,
        ));
        cumulative_ticks += t;
        cumulative_time += start.elapsed();
    }
    tx.rollback().unwrap();

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

criterion_group!(benches, opcode_throughput);
criterion_main!(benches);
