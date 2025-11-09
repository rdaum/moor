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

//! VM testing utilities for executing verbs, eval, and forks in test environments

use std::{sync::Arc, time::Duration};

use moor_common::model::WorldState;
use moor_compiler::Program;
use moor_var::{List, Obj, SYSTEM_OBJECT, Symbol, Var, v_obj};

use crate::{
    config::FeaturesConfig,
    task_context::TaskGuard,
    tasks::task_scheduler_client::TaskSchedulerClient,
    vm::{VMHostResponse, builtins::BuiltinRegistry, vm_host::VmHost},
};

use moor_common::tasks::{Exception, Session};

pub type ExecResult = Result<Var, Exception>;

/// Setup test task context with proper task scheduler client
pub fn setup_task_context(world_state: Box<dyn WorldState>) -> TaskGuard {
    let (scs_tx, _scs_rx) = flume::unbounded();
    let task_scheduler_client = TaskSchedulerClient::new(0, scs_tx);
    let session = std::sync::Arc::new(moor_common::tasks::NoopClientSession::new());
    TaskGuard::new(
        world_state,
        task_scheduler_client,
        0,
        moor_var::NOTHING,
        session,
    )
}

fn execute_fork(
    session: Arc<dyn Session>,
    builtins: &BuiltinRegistry,
    fork_request: crate::vm::Fork,
    task_id: usize,
) -> ExecResult {
    // For testing, forks execute in the same transaction context as the parent

    let mut vm_host = VmHost::new(task_id, 20, 90_000, Duration::from_secs(5));

    vm_host.start_fork(task_id, &fork_request, false);

    let config = Arc::new(FeaturesConfig::default());

    // Execute the forked task until completion
    loop {
        let exec_result =
            vm_host.exec_interpreter(task_id, session.as_ref(), builtins, config.as_ref());
        match exec_result {
            VMHostResponse::ContinueOk => {
                continue;
            }
            VMHostResponse::DispatchFork(nested_fork) => {
                // Execute nested fork - if it fails, propagate the error
                let nested_result =
                    execute_fork(session.clone(), builtins, *nested_fork, task_id + 1);
                nested_result?;
                continue;
            }
            VMHostResponse::AbortLimit(a) => {
                panic!("Fork task aborted: {a:?}");
            }
            VMHostResponse::CompleteException(e) => {
                return Err(e.as_ref().clone());
            }
            VMHostResponse::CompleteSuccess(v) => {
                return Ok(v);
            }
            VMHostResponse::CompleteAbort => {
                panic!("Fork task aborted");
            }
            VMHostResponse::Suspend(_) => {
                panic!("Fork task suspended");
            }
            VMHostResponse::SuspendNeedInput(_) => {
                panic!("Fork task needs input");
            }
            VMHostResponse::RollbackRetry => {
                panic!("Fork task rollback retry");
            }
            VMHostResponse::CompleteRollback(_) => {
                panic!("Fork task rollback");
            }
        }
    }
}

fn execute<F>(
    world_state: Box<dyn WorldState>,
    session: Arc<dyn Session>,
    builtins: BuiltinRegistry,
    fun: F,
) -> ExecResult
where
    F: FnOnce(&mut VmHost),
{
    let mut vm_host = VmHost::new(0, 20, 90_000, Duration::from_secs(5));

    let _tx_guard = setup_task_context(world_state);

    fun(&mut vm_host);

    let config = Arc::new(FeaturesConfig::default());

    // Call repeatedly into exec until we ge either an error or Complete.
    loop {
        let exec_result = vm_host.exec_interpreter(0, session.as_ref(), &builtins, config.as_ref());
        match exec_result {
            VMHostResponse::ContinueOk => {
                continue;
            }
            VMHostResponse::DispatchFork(f) => {
                // For testing, execute the fork separately (sequentially)
                // If the fork fails, propagate the error to terminate main execution
                let fork_result = execute_fork(session.clone(), &builtins, *f, 1);
                fork_result?;
                // Continue main execution after successful fork dispatch
                continue;
            }
            VMHostResponse::AbortLimit(a) => {
                panic!("Unexpected abort: {a:?}");
            }
            VMHostResponse::CompleteException(e) => {
                return Err(e.as_ref().clone());
            }
            VMHostResponse::CompleteSuccess(v) => {
                return Ok(v);
            }
            VMHostResponse::CompleteAbort => {
                panic!("Unexpected abort");
            }
            VMHostResponse::Suspend(_) => {
                panic!("Unexpected suspend");
            }
            VMHostResponse::SuspendNeedInput(_) => {
                panic!("Unexpected suspend need input");
            }
            VMHostResponse::RollbackRetry => {
                panic!("Unexpected rollback retry");
            }
            VMHostResponse::CompleteRollback(_) => {
                panic!("Unexpected rollback");
            }
        }
    }
}

pub fn call_verb(
    world_state: Box<dyn WorldState>,
    session: Arc<dyn Session>,
    builtins: BuiltinRegistry,
    verb_name: &str,
    args: List,
) -> ExecResult {
    // Set up the verb call before starting transaction context
    let verb_name = Symbol::mk(verb_name);
    let (program, verbdef) = world_state
        .find_method_verb_on(&SYSTEM_OBJECT, &SYSTEM_OBJECT, verb_name)
        .unwrap();

    execute(world_state, session, builtins, |vm_host| {
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
    })
}

pub fn call_eval_builtin(
    world_state: Box<dyn WorldState>,
    session: Arc<dyn Session>,
    builtins: BuiltinRegistry,
    player: Obj,
    program: Program,
) -> ExecResult {
    execute(world_state, session, builtins, |vm_host| {
        vm_host.start_eval(0, &player, program);
    })
}

pub fn call_fork(
    session: Arc<dyn Session>,
    builtins: BuiltinRegistry,
    fork_request: crate::vm::Fork,
) -> ExecResult {
    execute_fork(session, &builtins, fork_request, 0)
}
