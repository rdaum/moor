// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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
//! Contains WorldState, TaskSchedulerClient, task_id, player objid, and Session.

use std::{cell::RefCell, sync::Arc};

#[cfg(feature = "trace_events")]
use std::collections::hash_map::DefaultHasher;
#[cfg(feature = "trace_events")]
use std::hash::{Hash, Hasher};

use moor_common::{
    model::{CommitResult, WorldState, WorldStateError, loader::LoaderInterface},
    tasks::{Session, TaskId},
};
use moor_var::Obj;

use crate::tasks::nursery::Nursery;
use crate::tasks::task_scheduler_client::TaskSchedulerClient;

/// Complete current task execution context containing all necessary state.
/// There is one of these per-thread, and no more, and each running task *must* have one, and this
/// is considered an invariant (failure to have one is a panic).
pub struct TaskContext {
    pub world_state: Box<dyn WorldState>,
    pub task_scheduler_client: TaskSchedulerClient,
    pub task_id: TaskId,
    pub player: Obj,
    pub session: Arc<dyn Session>,
    pub nursery: Nursery,
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
        session: Arc<dyn Session>,
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
                session,
                nursery: Nursery::new(),
            });
        });

        #[cfg(feature = "trace_events")]
        {
            let thread_id = {
                let mut hasher = DefaultHasher::new();
                std::thread::current().id().hash(&mut hasher);
                hasher.finish()
            };
            crate::trace_transaction_begin!(format!("task_{task_id}"), thread_id);
        }

        TaskGuard(())
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        // Emergency cleanup - rollback any remaining transaction
        CURRENT_CONTEXT.with(|ctx| {
            if let Some(task_ctx) = ctx.borrow_mut().take() {
                // Only warn if we're not already panicking (which would trigger this drop)
                if !std::thread::panicking() {
                    tracing::warn!(
                        "Task context dropped without explicit commit/rollback, rolling back"
                    );
                }

                #[cfg(feature = "trace_events")]
                {
                    let task_id = task_ctx.task_id;
                    let thread_id = {
                        let mut hasher = DefaultHasher::new();
                        std::thread::current().id().hash(&mut hasher);
                        hasher.finish()
                    };

                    // Emit emergency rollback event
                    crate::trace_transaction_rollback!(
                        format!("task_{task_id}"),
                        thread_id,
                        "emergency_cleanup"
                    );

                    // End the transaction span
                    use crate::tracing_events::{TraceEventType, emit_trace_event};
                    emit_trace_event(TraceEventType::TransactionEnd {
                        tx_id: format!("task_{task_id}"),
                        thread_id,
                    });
                }

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

/// Get a clone of the current session.
/// Panics if no context is active.
pub fn current_session() -> Arc<dyn Session> {
    CURRENT_CONTEXT.with(|ctx| {
        let ctx_ref = ctx.borrow();
        let task_ctx = ctx_ref
            .as_ref()
            .expect("No active task context on this thread");
        task_ctx.session.clone()
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

        #[cfg(feature = "trace_events")]
        let task_id = task_ctx.task_id;
        #[cfg(feature = "trace_events")]
        let thread_id = {
            let mut hasher = DefaultHasher::new();
            std::thread::current().id().hash(&mut hasher);
            hasher.finish()
        };

        let result = task_ctx.world_state.commit();

        #[cfg(feature = "trace_events")]
        {
            // Emit commit event and end the transaction span
            let success = matches!(result, Ok(moor_common::model::CommitResult::Success { .. }));
            let timestamp = match &result {
                Ok(moor_common::model::CommitResult::Success { timestamp, .. }) => *timestamp,
                _ => 0,
            };

            crate::trace_transaction_commit!(
                format!("task_{task_id}"),
                thread_id,
                success,
                timestamp
            );

            // End the transaction span
            use crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TransactionEnd {
                tx_id: format!("task_{task_id}"),
                thread_id,
            });
        }

        result
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

        #[cfg(feature = "trace_events")]
        {
            let task_id = task_ctx.task_id;
            let thread_id = {
                let mut hasher = DefaultHasher::new();
                std::thread::current().id().hash(&mut hasher);
                hasher.finish()
            };

            // Emit rollback event and end the transaction span
            crate::trace_transaction_rollback!(
                format!("task_{task_id}"),
                thread_id,
                "explicit_rollback"
            );

            // End the transaction span
            use crate::tracing_events::{TraceEventType, emit_trace_event};
            emit_trace_event(TraceEventType::TransactionEnd {
                tx_id: format!("task_{task_id}"),
                thread_id,
            });
        }

        task_ctx.world_state.rollback()
    })
}

/// Check if there's an active context on the current thread.
pub fn has_active_task() -> bool {
    CURRENT_CONTEXT.with(|ctx| ctx.borrow().is_some())
}

/// Execute a closure with immutable access to the current task's nursery.
/// Panics if no context is active.
pub fn with_current_nursery<R>(f: impl FnOnce(&Nursery) -> R) -> R {
    CURRENT_CONTEXT.with(|ctx| {
        let ctx_ref = ctx.borrow();
        let task_ctx = ctx_ref
            .as_ref()
            .expect("No active task context on this thread");
        f(&task_ctx.nursery)
    })
}

/// Execute a closure with mutable access to the current task's nursery.
/// Panics if no context is active.
pub fn with_current_nursery_mut<R>(f: impl FnOnce(&mut Nursery) -> R) -> R {
    CURRENT_CONTEXT.with(|ctx| {
        let mut ctx_ref = ctx.borrow_mut();
        let task_ctx = ctx_ref
            .as_mut()
            .expect("No active task context on this thread");
        f(&mut task_ctx.nursery)
    })
}

/// Execute a closure with mutable access to both nursery and world state.
/// This is needed for swizzling nursery refs when storing to DB properties.
/// Panics if no context is active.
pub fn with_nursery_and_transaction_mut<R>(
    f: impl FnOnce(&mut Nursery, &mut dyn WorldState) -> R,
) -> R {
    CURRENT_CONTEXT.with(|ctx| {
        let mut ctx_ref = ctx.borrow_mut();
        let task_ctx = ctx_ref
            .as_mut()
            .expect("No active task context on this thread");
        f(&mut task_ctx.nursery, task_ctx.world_state.as_mut())
    })
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

/// Execute a closure that creates a new transaction while preserving the current task context.
/// This atomically commits the current transaction and starts a new one with preserved context.
pub fn with_new_transaction<F, R>(
    create_transaction: F,
) -> Result<(CommitResult, Option<R>), WorldStateError>
where
    F: FnOnce() -> Result<(Box<dyn WorldState>, R), WorldStateError>,
{
    // Extract context before commit to preserve it (including nursery which persists across transactions)
    let preserved_context = CURRENT_CONTEXT.with(|ctx| {
        let mut task_ctx = ctx.borrow_mut();
        let task_ctx = task_ctx.as_mut().expect("No active task context");
        (
            task_ctx.task_scheduler_client.clone(),
            task_ctx.task_id,
            task_ctx.player,
            task_ctx.session.clone(),
            std::mem::take(&mut task_ctx.nursery),
        )
    });

    // Commit current transaction (this removes the context)
    let commit_result = commit_current_transaction()?;

    match commit_result {
        CommitResult::Success { .. } => {
            // Create the new transaction
            let (new_world_state, result) = create_transaction()?;

            // Restore context with new world state and preserved values
            CURRENT_CONTEXT.with(|ctx| {
                let mut current = ctx.borrow_mut();
                assert!(
                    current.is_none(),
                    "Task context unexpectedly active after commit"
                );
                *current = Some(TaskContext {
                    world_state: new_world_state,
                    task_scheduler_client: preserved_context.0,
                    task_id: preserved_context.1,
                    player: preserved_context.2,
                    session: preserved_context.3,
                    nursery: preserved_context.4,
                });
            });

            Ok((commit_result, Some(result)))
        }
        CommitResult::ConflictRetry { conflict_info } => {
            // On conflict, we don't create a new transaction
            Ok((CommitResult::ConflictRetry { conflict_info }, None))
        }
    }
}

/// Execute a closure with loader interface access to the current transaction.
/// This temporarily extracts the WorldState, converts it to LoaderInterface using
/// the same underlying transaction, executes the closure, then restores it as WorldState.
/// Returns an error if no context is active or if the WorldState doesn't support conversion.
pub fn with_loader_interface<F, R, E>(f: F) -> Result<R, E>
where
    F: FnOnce(&mut dyn LoaderInterface) -> Result<R, E>,
{
    // Extract the current WorldState and context info
    let (world_state, task_scheduler_client, task_id, player, session, nursery) =
        CURRENT_CONTEXT.with(|ctx| {
            let task_ctx = ctx.borrow_mut().take().expect("No active task context");
            (
                task_ctx.world_state,
                task_ctx.task_scheduler_client,
                task_ctx.task_id,
                task_ctx.player,
                task_ctx.session,
                task_ctx.nursery,
            )
        });

    // Convert WorldState to LoaderInterface
    let mut loader = world_state
        .as_loader_interface()
        .expect("Could not extract loader from world state");

    // Execute the closure with loader interface
    let result = f(loader.as_mut());

    // Convert back to WorldState
    let world_state = loader
        .as_world_state()
        .expect("Could not extract world state from loader");

    // Restore the context
    CURRENT_CONTEXT.with(|ctx| {
        let mut current = ctx.borrow_mut();
        *current = Some(TaskContext {
            world_state,
            task_scheduler_client,
            task_id,
            player,
            session,
            nursery,
        });
    });

    result
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

    #[test]
    #[should_panic(expected = "No active task context")]
    fn test_panic_on_no_nursery_context() {
        with_current_nursery(|_| ());
    }

    #[test]
    #[should_panic(expected = "No active task context")]
    fn test_panic_on_no_nursery_mut_context() {
        with_current_nursery_mut(|_| ());
    }
}
