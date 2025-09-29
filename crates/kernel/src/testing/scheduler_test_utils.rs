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

//! Scheduler testing utilities for integration tests with scheduler clients

use std::{sync::Arc, time::Duration};

use moor_common::tasks::{CommandError, SchedulerError};
use moor_var::{E_VERBNF, Obj, SYSTEM_OBJECT, Var};

use crate::{
    config::FeaturesConfig,
    tasks::{TaskHandle, TaskResult, scheduler_client::SchedulerClient},
};
use moor_common::tasks::{
    Exception,
    SchedulerError::{CommandExecutionError, TaskAbortedException},
    Session,
};

pub type ExecResult = Result<Var, Exception>;

fn execute<F>(fun: F) -> Result<Var, SchedulerError>
where
    F: FnOnce() -> Result<TaskHandle, SchedulerError>,
{
    let task_handle = fun()?;
    match task_handle
        .receiver()
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
        (_, Err(TaskAbortedException(Exception { error, .. }))) => Ok(error.into()),
        (_, Err(CommandExecutionError(CommandError::NoCommandMatch))) => {
            Ok(E_VERBNF.msg("No command match").into())
        }
        (_, Err(err)) => Err(err),
        (_, Ok(TaskResult::Result(var))) => Ok(var),
        (_, Ok(TaskResult::Replaced(_))) => panic!("Unexpected task restart"),
    }
}

pub fn call_command(
    scheduler: SchedulerClient,
    session: Arc<dyn Session>,
    player: &Obj,
    command: &str,
) -> Result<Var, SchedulerError> {
    execute(|| scheduler.submit_command_task(&SYSTEM_OBJECT, player, command, session))
}

pub fn call_eval(
    scheduler: SchedulerClient,
    session: Arc<dyn Session>,
    player: &Obj,
    code: String,
) -> Result<Var, SchedulerError> {
    execute(|| {
        scheduler.submit_eval_task(
            player,
            player,
            code,
            session,
            Arc::new(FeaturesConfig::default()),
        )
    })
}
