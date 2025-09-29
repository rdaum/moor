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

pub use crate::{
    tasks::{
        ServerOptions,
        scheduler_client::SchedulerClient,
        task::Task,
        task_q::{SuspendedTask, WakeCondition},
    },
    tracing_events::{TraceEventType, emit_trace_event, init_tracing, shutdown_tracing},
};
pub use moor_common::tasks::TaskId;

use std::{cell::Cell, marker::PhantomData};

pub mod config;
pub mod task_context;
pub mod tasks;
pub mod tracing_events;
pub mod vm;

pub mod testing;

/// A phantom type for explicitly marking types as !Sync
type PhantomUnsync = PhantomData<Cell<()>>;
