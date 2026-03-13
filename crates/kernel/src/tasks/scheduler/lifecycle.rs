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

//! TaskLifecycle: all mutable state that must be consistent during task state transitions.
//! Protected by a single Mutex in the Scheduler handle.

use std::collections::HashMap;

use moor_common::tasks::TaskId;
use moor_var::Var;

use crate::tasks::task_q::TaskQ;

/// All mutable state that must be consistent during task state transitions.
/// Protected by a single Mutex in the Scheduler handle.
pub(crate) struct TaskLifecycle {
    /// The internal task queue holding active and suspended tasks.
    pub(crate) task_q: TaskQ,

    /// Buffered inter-task messages awaiting commit. Keyed by sending task_id.
    /// Delivered to target queues when the sending task commits; discarded on abort/conflict.
    pub(crate) pending_task_sends: HashMap<TaskId, Vec<(TaskId, Var)>>,

    /// Task ID counter.
    pub(crate) next_task_id: usize,

    /// Anonymous object garbage collection flag.
    pub(crate) gc_collection_in_progress: bool,
    /// Flag indicating concurrent GC mark phase is in progress.
    pub(crate) gc_mark_in_progress: bool,
    /// Flag indicating GC sweep phase is in progress (blocks new tasks).
    pub(crate) gc_sweep_in_progress: bool,
    /// Flag to force GC on next opportunity (set by gc_collect() builtin).
    pub(crate) gc_force_collect: bool,
    /// Counter tracking the number of GC cycles completed.
    pub(crate) gc_cycle_count: u64,
    /// Time of last GC cycle (for interval-based collection).
    pub(crate) gc_last_cycle_time: std::time::Instant,

    /// Transaction timestamp (monotonically incrementing) of the last mutating task/transaction.
    pub(crate) last_mutation_timestamp: Option<u64>,

    /// Whether the scheduler is running.
    pub(crate) running: bool,

    /// Time of last tasks DB compaction (independent of GC).
    pub(crate) last_compact_time: std::time::Instant,
}

impl TaskLifecycle {
    /// Deliver all buffered messages from the given task to their target queues.
    /// Called when a task commits (success, suspend, input request, exception, new transaction).
    pub(crate) fn flush_pending_sends(&mut self, task_id: TaskId) {
        if let Some(sends) = self.pending_task_sends.remove(&task_id) {
            for (target_task_id, value) in sends {
                self.task_q.deliver_message(target_task_id, value);
            }
        }
    }

    /// Discard all buffered messages from the given task without delivering.
    /// Called when a task aborts (conflict retry, cancelled, panicked, limits reached).
    pub(crate) fn discard_pending_sends(&mut self, task_id: TaskId) {
        self.pending_task_sends.remove(&task_id);
    }
}
