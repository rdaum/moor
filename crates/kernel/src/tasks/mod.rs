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

use crate::tasks::scheduler::TaskResult;
use moor_values::var::{List, Objid};
use std::time::SystemTime;

pub mod command_parse;
pub mod scheduler;
pub mod sessions;

mod task;
pub mod task_messages;
pub mod vm_host;

pub type TaskId = usize;

/// Just a handle to a task, with a receiver for the result.
pub struct TaskHandle(TaskId, oneshot::Receiver<TaskResult>);
impl TaskHandle {
    pub fn task_id(&self) -> TaskId {
        self.0
    }

    /// Dissolve the handle into a receiver for the result.
    pub fn into_receiver(self) -> oneshot::Receiver<TaskResult> {
        self.1
    }
}

/// The minimum set of information needed to make a *resolution* call for a verb.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerbCall {
    pub verb_name: String,
    pub location: Objid,
    pub this: Objid,
    pub player: Objid,
    pub args: List,
    pub argstr: String,
    pub caller: Objid,
}

/// External interface description of a task, for purpose of e.g. the queued_tasks() builtin.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TaskDescription {
    pub task_id: TaskId,
    pub start_time: Option<SystemTime>,
    pub permissions: Objid,
    pub verb_name: String,
    pub verb_definer: Objid,
    pub line_number: usize,
    pub this: Objid,
}

pub mod vm_test_utils {
    use crate::tasks::sessions::Session;
    use crate::tasks::vm_host::{VMHostResponse, VmHost};
    use crate::tasks::VerbCall;
    use crate::vm::UncaughtException;
    use crate::vm::VmExecParams;
    use moor_compiler::Program;
    use moor_values::model::WorldState;
    use moor_values::var::{List, Objid, Var};
    use moor_values::SYSTEM_OBJECT;
    use std::sync::Arc;
    use std::time::Duration;

    pub type ExecResult = Result<Var, UncaughtException>;

    fn execute<F>(world_state: &mut dyn WorldState, session: Arc<dyn Session>, fun: F) -> ExecResult
    where
        F: FnOnce(&mut dyn WorldState, &mut VmHost),
    {
        let (scs_tx, _scs_rx) = crossbeam_channel::unbounded();
        let mut vm_host = VmHost::new(
            0,
            20,
            90_000,
            Duration::from_secs(5),
            session.clone(),
            scs_tx,
        );

        let (sched_send, _) = crossbeam_channel::unbounded();
        let _vm_exec_params = VmExecParams {
            scheduler_sender: sched_send.clone(),
            max_stack_depth: 50,
        };

        fun(world_state, &mut vm_host);

        // Call repeatedly into exec until we ge either an error or Complete.
        loop {
            match vm_host.exec_interpreter(0, world_state) {
                VMHostResponse::ContinueOk => {
                    continue;
                }
                VMHostResponse::DispatchFork(f) => {
                    panic!("Unexpected fork: {:?}", f);
                }
                VMHostResponse::AbortLimit(a) => {
                    panic!("Unexpected abort: {:?}", a);
                }
                VMHostResponse::CompleteException(e) => {
                    return Err(e);
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
                VMHostResponse::SuspendNeedInput => {
                    panic!("Unexpected suspend need input");
                }
                VMHostResponse::RollbackRetry => {
                    panic!("Unexpected rollback retry");
                }
            }
        }
    }

    pub fn call_verb(
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
        verb_name: &str,
        args: Vec<Var>,
    ) -> ExecResult {
        execute(world_state, session, |world_state, vm_host| {
            let vi = world_state
                .find_method_verb_on(SYSTEM_OBJECT, SYSTEM_OBJECT, verb_name)
                .unwrap();
            vm_host.start_call_method_verb(
                0,
                SYSTEM_OBJECT,
                vi,
                VerbCall {
                    verb_name: verb_name.to_string(),
                    location: SYSTEM_OBJECT,
                    this: SYSTEM_OBJECT,
                    player: SYSTEM_OBJECT,
                    args: List::from_slice(&args),
                    argstr: "".to_string(),
                    caller: SYSTEM_OBJECT,
                },
            );
        })
    }

    pub fn call_eval_builtin(
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
        player: Objid,
        program: Program,
    ) -> ExecResult {
        execute(world_state, session, |world_state, vm_host| {
            vm_host.start_eval(0, player, program, world_state);
        })
    }
}

pub mod scheduler_test_utils {
    use crate::tasks::sessions::Session;
    use crate::vm::UncaughtException;
    use moor_values::model::CommandError;
    use moor_values::var::{Error::E_VERBNF, Objid, Var};
    use std::sync::Arc;
    use std::time::Duration;

    use super::scheduler::{Scheduler, SchedulerError, TaskResult};
    use super::TaskHandle;
    use crate::tasks::scheduler_test_utils::SchedulerError::{
        CommandExecutionError, TaskAbortedException,
    };

    pub type ExecResult = Result<Var, UncaughtException>;

    fn execute<F>(fun: F) -> Result<Var, SchedulerError>
    where
        F: FnOnce() -> Result<TaskHandle, SchedulerError>,
    {
        let task_handle = fun()?;
        match task_handle
            .1
            .recv_timeout(Duration::from_secs(1))
            .inspect_err(|e| {
                eprintln!(
                    "subscriber.recv_timeout() failed for task {}: {e}",
                    task_handle.task_id(),
                )
            })
            .unwrap()
        {
            // Some errors can be represented as a MOO `Var`; translate those to a `Var`, so that
            // `moot` tests can match against them.
            TaskResult::Error(TaskAbortedException(UncaughtException { code, .. })) => {
                Ok(code.into())
            }
            TaskResult::Error(CommandExecutionError(CommandError::NoCommandMatch)) => {
                Ok(E_VERBNF.into())
            }
            TaskResult::Error(err) => Err(err),
            TaskResult::Success(var) => Ok(var),
        }
    }

    pub fn call_command(
        scheduler: Arc<Scheduler>,
        session: Arc<dyn Session>,
        player: Objid,
        command: &str,
    ) -> Result<Var, SchedulerError> {
        execute(|| scheduler.submit_command_task(player, command, session))
    }

    pub fn call_eval(
        scheduler: Arc<Scheduler>,
        session: Arc<dyn Session>,
        player: Objid,
        code: String,
    ) -> Result<Var, SchedulerError> {
        execute(|| scheduler.submit_eval_task(player, player, code, session))
    }
}
