// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
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

use std::sync::Arc;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;

use moor_compiler::compile;
use moor_db::odb::RelBoxWorldState;
use moor_kernel::tasks::scheduler::AbortLimitReason;
use moor_kernel::tasks::sessions::{NoopClientSession, Session};
use moor_kernel::tasks::vm_host::{VMHostResponse, VmHost};
use moor_kernel::tasks::VerbCall;
use moor_values::model::CommitResult;
use moor_values::model::VerbArgsSpec;
use moor_values::model::{BinaryType, VerbFlag};
use moor_values::model::{WorldState, WorldStateSource};
use moor_values::util::BitEnum;
use moor_values::var::Var;
use moor_values::{AsByteBuffer, NOTHING, SYSTEM_OBJECT};

async fn create_worldstate() -> RelBoxWorldState {
    let (ws_source, _) = RelBoxWorldState::open(None, 1 << 24).await;
    let mut tx = ws_source.new_world_state().await.unwrap();
    let _sysobj = tx
        .create_object(SYSTEM_OBJECT, NOTHING, SYSTEM_OBJECT, BitEnum::all())
        .await
        .unwrap();
    assert_eq!(tx.commit().await.unwrap(), CommitResult::Success);
    ws_source
}

pub async fn prepare_call_verb(
    world_state: &mut dyn WorldState,
    session: Arc<dyn Session>,
    verb_name: &str,
    args: Vec<Var>,
    max_ticks: usize,
) -> VmHost {
    let (scs_tx, _scs_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut vm_host = VmHost::new(
        20,
        max_ticks,
        Duration::from_secs(15),
        session.clone(),
        scs_tx,
    );

    let vi = world_state
        .find_method_verb_on(SYSTEM_OBJECT, SYSTEM_OBJECT, verb_name)
        .await
        .unwrap();
    vm_host
        .start_call_method_verb(
            0,
            SYSTEM_OBJECT,
            vi,
            VerbCall {
                verb_name: verb_name.to_string(),
                location: SYSTEM_OBJECT,
                this: SYSTEM_OBJECT,
                player: SYSTEM_OBJECT,
                args,
                argstr: "".to_string(),
                caller: SYSTEM_OBJECT,
            },
        )
        .await;
    vm_host
}

async fn prepare_vm_execution(
    ws_source: &mut RelBoxWorldState,
    program: &str,
    max_ticks: usize,
) -> VmHost {
    let binary = compile(program).unwrap();
    let mut tx = ws_source.new_world_state().await.unwrap();
    tx.add_verb(
        SYSTEM_OBJECT,
        SYSTEM_OBJECT,
        vec!["test".to_string()],
        SYSTEM_OBJECT,
        VerbFlag::rxd(),
        VerbArgsSpec::this_none_this(),
        binary.make_copy_as_vec(),
        BinaryType::LambdaMoo18X,
    )
    .await
    .unwrap();
    let session = Arc::new(NoopClientSession::new());
    let vm_host = prepare_call_verb(tx.as_mut(), session, "test", vec![], max_ticks).await;
    assert_eq!(tx.commit().await.unwrap(), CommitResult::Success);
    vm_host
}

/// Run the vm host until it runs out of ticks
async fn execute(world_state: &mut dyn WorldState, vm_host: &mut VmHost) -> bool {
    vm_host.reset_ticks();
    vm_host.reset_time();

    // Call repeatedly into exec until we ge either an error or Complete.
    loop {
        match vm_host.exec_interpreter(0, world_state).await {
            VMHostResponse::ContinueOk => {
                continue;
            }
            VMHostResponse::AbortLimit(AbortLimitReason::Ticks(_)) => {
                return true;
            }
            VMHostResponse::CompleteSuccess(_) => {
                return false;
            }
            VMHostResponse::AbortLimit(AbortLimitReason::Time(time)) => {
                panic!("Unexpected abort: {:?}", time);
            }
            VMHostResponse::DispatchFork(f) => {
                panic!("Unexpected fork: {:?}", f);
            }
            VMHostResponse::CompleteException(e) => {
                panic!("Unexpected exception: {:?}", e)
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
        }
    }
}

async fn do_program(program: &str, max_ticks: usize, iters: u64) -> Duration {
    let mut cumulative = Duration::new(0, 0);

    let mut state_source = create_worldstate().await;
    let mut vm_host = prepare_vm_execution(&mut state_source, program, max_ticks).await;
    let mut tx = state_source.new_world_state().await.unwrap();
    for _ in 0..iters {
        let start = std::time::Instant::now();
        let _ = execute(tx.as_mut(), &mut vm_host).await;
        let end = std::time::Instant::now();
        cumulative += end - start;
    }
    tx.rollback().await.unwrap();

    drop(state_source);
    cumulative
}

fn opcode_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("opcode_throughput");
    group.sample_size(300);
    group.measurement_time(Duration::from_secs(10));

    let num_ticks = 300000;
    group.throughput(criterion::Throughput::Elements(num_ticks as u64));
    group.bench_function("while_loop", |b| {
        b.to_async(&rt)
            .iter_custom(|iters| do_program("while (1) endwhile", num_ticks, iters));
    });
    group.bench_function("while_increment_var_loop", |b| {
        b.to_async(&rt)
            .iter_custom(|iters| do_program("i = 0; while(1) i=i+1; endwhile", num_ticks, iters));
    });
    group.bench_function("for_in_range_loop", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            do_program(
                "while(1) for i in [1..1000000] endfor endwhile",
                num_ticks,
                iters,
            )
        });
    });
    // Measure range iteration over a static list
    group.bench_function("for_in_static_list_loop", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            do_program(
                r#"while(1)
                            for i in ({1,2,3,4,5,6,7,8,9,10,1,2,3,4,5,6,7,8,9,10,1,2,3,4,5,6,7,8,9,10,1,2,3,4,5,6,7,8,9,10})
                            endfor
                          endwhile"#,
                num_ticks,
                iters,
            )
        });
    });
    // Measure how costly it is to append to a list
    group.bench_function("list_append_loop", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            do_program(
                r#"while(1)
                            base_list = {};
                            for i in [0..1000]
                                base_list = {@base_list, i};
                            endfor
                          endwhile"#,
                num_ticks,
                iters,
            )
        });
    });
    // Measure how costly it is to append to a list
    group.bench_function("list_set", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            do_program(
                r#"while(1) 
                            list = {1};
                            for i in [0..10000] 
                                list[1] = i;
                            endfor
                          endwhile"#,
                num_ticks,
                iters,
            )
        });
    });
    group.finish();
}

criterion_group!(benches, opcode_throughput);
criterion_main!(benches);
