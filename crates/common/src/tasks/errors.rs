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

use crate::{
    model::{CompileError, WorldStateError},
    tasks::TaskId,
};
use moor_var::{Error, Symbol, Var};
use std::{fmt::Display, time::Duration};
use strum::Display;
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq, Display)]
pub enum VerbProgramError {
    NoVerbToProgram,
    CompilationError(CompileError),
    DatabaseError,
}

/// Reasons a task might be aborted for a 'limit'
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbortLimitReason {
    /// This task hit its allotted tick limit.
    Ticks(usize),
    /// This task hit its allotted time limit.
    Time(Duration),
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum SchedulerError {
    #[error("Scheduler not responding")]
    SchedulerNotResponding,
    #[error("Task not found: {0:?}")]
    TaskNotFound(TaskId),
    #[error("Input request not found: {0:?}")]
    // Using u128 here because Uuid is not bincode-able, but this is just a v4 uuid.
    InputRequestNotFound(u128),
    #[error("Could not start task (internal error)")]
    CouldNotStartTask,
    #[error("Compilation error")]
    CompilationError(#[source] CompileError),
    #[error("Could not start command")]
    CommandExecutionError(#[source] CommandError),
    #[error("Task aborted due to limit: {0:?}")]
    TaskAbortedLimit(AbortLimitReason),
    #[error("Task aborted due to error.")]
    TaskAbortedError,
    #[error("Task aborted due to exception")]
    TaskAbortedException(#[source] Exception),
    #[error("Task aborted due to cancellation.")]
    TaskAbortedCancelled,
    #[error("Unable to program verb {0}")]
    VerbProgramFailed(VerbProgramError),
    #[error("Unable to retrieve property {0}")]
    PropertyRetrievalFailed(WorldStateError),
    #[error("Unable to retrieve verb {0}")]
    VerbRetrievalFailed(WorldStateError),
    #[error("Unable to resolve object reference {0}")]
    ObjectResolutionFailed(WorldStateError),
    #[error("Garbage collection failed: {0}")]
    GarbageCollectionFailed(String),
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Exception {
    pub error: Error,
    pub stack: Vec<Var>,
    pub backtrace: Vec<Var>,
}

impl Display for Exception {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Uncaught exception: {}", self.error)
    }
}

impl std::error::Error for Exception {}

/// Errors related to command matching.
#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum CommandError {
    #[error("Could not parse command")]
    CouldNotParseCommand,
    #[error("Could not find object match for command")]
    NoObjectMatch,
    #[error("Could not find verb match for command")]
    NoCommandMatch,
    #[error("Could not start transaction due to database error")]
    DatabaseError(#[source] WorldStateError),
    #[error("Permission denied")]
    PermissionDenied,
}

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum WorkerError {
    #[error("Permission denied")]
    PermissionDenied(String),
    #[error("Invalid request")]
    InvalidRequest(String),
    #[error("Invalid response")]
    InternalError(String),
    #[error("Request timed out")]
    RequestTimedOut(String),
    #[error("Request cancelled")]
    RequestError(String),
    #[error("Worker detached")]
    WorkerDetached(String),
    #[error("No worker available for {0}")]
    NoWorkerAvailable(Symbol),
}
