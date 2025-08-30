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

//! Thread-local task context for eliminating parameter threading.
//! Provides RAII-based transaction management with automatic cleanup.
//! Contains WorldState, TaskSchedulerClient, task_id, and player objid.

use std::cell::RefCell;

use flume;
use moor_common::model::{CommitResult, WorldState, WorldStateError};
use moor_common::tasks::TaskId;
use moor_var::Obj;

use crate::tasks::task_scheduler_client::TaskSchedulerClient;

/// Complete current task execution context containing all necessary state.
/// There is one of these per-thread, and no more, and each running task *must* have one, and this
/// is considered an invariant (failure to have one is a panic).
pub struct TaskContext {
    pub world_state: Box<dyn WorldState>,
    pub task_scheduler_client: TaskSchedulerClient,
    pub task_id: TaskId,
    pub player: Obj,
}

thread_local! {
    static CURRENT_CONTEXT: RefCell<Option<TaskContext>> = const { RefCell::new(None) };
}

/// RAII guard that ensures transaction cleanup on drop.
/// Transaction must be explicitly committed or rolled back before drop.
pub struct TaskGuard(());

impl TaskGuard {
    /// Start a new task context on the current thread.
    /// Panics if a context is already active.
    pub fn new(
        world_state: Box<dyn WorldState>,
        task_scheduler_client: TaskSchedulerClient,
        task_id: TaskId,
        player: Obj,
    ) -> Self {
        CURRENT_CONTEXT.with(|ctx| {
            let mut current = ctx.borrow_mut();
            assert!(
                current.is_none(),
                "Task context already active on this thread"
            );
            *current = Some(TaskContext {
                world_state,
                task_scheduler_client,
                task_id,
                player,
            });
        });
        TaskGuard(())
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        // Emergency cleanup - rollback any remaining transaction
        CURRENT_CONTEXT.with(|ctx| {
            if let Some(task_ctx) = ctx.borrow_mut().take() {
                tracing::warn!(
                    "Task context dropped without explicit commit/rollback, rolling back"
                );
                let _ = task_ctx.world_state.rollback(); // Best effort cleanup
            }
        });
    }
}

/// Execute a closure with access to the current transaction.
/// Panics if no context is active.
pub fn with_current_transaction<R>(f: impl FnOnce(&dyn WorldState) -> R) -> R {
    CURRENT_CONTEXT.with(|ctx| {
        let ctx_ref = ctx.borrow();
        let task_ctx = ctx_ref
            .as_ref()
            .expect("No active task context on this thread");
        f(task_ctx.world_state.as_ref())
    })
}

/// Execute a closure with mutable access to the current transaction.
/// Panics if no context is active.
pub fn with_current_transaction_mut<R>(f: impl FnOnce(&mut dyn WorldState) -> R) -> R {
    CURRENT_CONTEXT.with(|ctx| {
        let mut ctx_ref = ctx.borrow_mut();
        let task_ctx = ctx_ref
            .as_mut()
            .expect("No active task context on this thread");
        f(task_ctx.world_state.as_mut())
    })
}

/// Get a clone of the current task scheduler client.
/// Panics if no context is active.
pub fn current_task_scheduler_client() -> TaskSchedulerClient {
    CURRENT_CONTEXT.with(|ctx| {
        let ctx_ref = ctx.borrow();
        let task_ctx = ctx_ref
            .as_ref()
            .expect("No active task context on this thread");
        task_ctx.task_scheduler_client.clone()
    })
}

/// Get the current task ID.
/// Panics if no context is active.
pub fn current_task_id() -> TaskId {
    CURRENT_CONTEXT.with(|ctx| {
        let ctx_ref = ctx.borrow();
        let task_ctx = ctx_ref
            .as_ref()
            .expect("No active task context on this thread");
        task_ctx.task_id
    })
}

/// Get the current player object.
/// Panics if no context is active.
pub fn current_player() -> Obj {
    CURRENT_CONTEXT.with(|ctx| {
        let ctx_ref = ctx.borrow();
        let task_ctx = ctx_ref
            .as_ref()
            .expect("No active task context on this thread");
        task_ctx.player
    })
}

/// Commit the current thread's active transaction.
/// Panics if no context is active.
pub fn commit_current_transaction() -> Result<CommitResult, WorldStateError> {
    CURRENT_CONTEXT.with(|ctx| {
        let task_ctx = ctx
            .borrow_mut()
            .take()
            .expect("No active task context to commit");
        task_ctx.world_state.commit()
    })
}

/// Rollback the current thread's active transaction.
/// Panics if no context is active.
pub fn rollback_current_transaction() -> Result<(), WorldStateError> {
    CURRENT_CONTEXT.with(|ctx| {
        let task_ctx = ctx
            .borrow_mut()
            .take()
            .expect("No active task context to rollback");
        task_ctx.world_state.rollback()
    })
}

/// Check if there's an active context on the current thread.
pub fn has_active_task() -> bool {
    CURRENT_CONTEXT.with(|ctx| ctx.borrow().is_some())
}

/// Extract the current transaction from thread-local storage.
/// This is a transitional helper for compatibility with existing parameter-passing code.
/// Panics if no context is active.
pub fn extract_current_transaction() -> Box<dyn WorldState> {
    CURRENT_CONTEXT.with(|ctx| {
        let task_ctx = ctx
            .borrow_mut()
            .take()
            .expect("No active task context to extract");
        task_ctx.world_state
    })
}

/// Replace the current transaction in thread-local storage.
/// This is a transitional helper for compatibility with existing parameter-passing code.
/// Panics if a context is already active.
pub fn replace_current_transaction(world_state: Box<dyn WorldState>) {
    use moor_var::NOTHING;
    // Create a dummy channel for the scheduler client
    let (tx, _rx) = flume::unbounded();
    let dummy_client = TaskSchedulerClient::new(0, tx);

    CURRENT_CONTEXT.with(|ctx| {
        let mut current = ctx.borrow_mut();
        assert!(
            current.is_none(),
            "Task context already active when trying to replace"
        );
        *current = Some(TaskContext {
            world_state,
            task_scheduler_client: dummy_client,
            task_id: 0,
            player: NOTHING,
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // For now, we just test the basic guard functionality without a full WorldState mock
    // since implementing the full WorldState trait would be quite large

    #[test]
    fn test_no_transaction_initially() {
        assert!(!has_active_task());
    }

    #[test]
    #[should_panic(expected = "No active task context")]
    fn test_panic_on_no_transaction() {
        with_current_transaction(|_| ());
    }

    #[test]
    #[should_panic(expected = "No active task context to commit")]
    fn test_panic_on_commit_no_transaction() {
        commit_current_transaction().unwrap();
    }

    #[test]
    #[should_panic(expected = "No active task context to rollback")]
    fn test_panic_on_rollback_no_transaction() {
        rollback_current_transaction().unwrap();
    }
}
