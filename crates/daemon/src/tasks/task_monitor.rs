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

//! Task completion monitoring and lifecycle management

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

use crate::rpc::SessionActions;
use flume::Sender;
use moor_common::{
    schema::{convert::var_to_flatbuffer, rpc as moor_rpc},
    tasks::{SchedulerError, TaskId},
};
use moor_kernel::tasks::TaskHandle;
use tracing::info;

/// Monitors task completions and handles their lifecycle
pub struct TaskMonitor {
    task_handles: papaya::HashMap<TaskId, (Uuid, TaskHandle)>,
    mailbox_sender: Sender<SessionActions>,
    wake_signal: (Sender<()>, flume::Receiver<()>),
}

impl TaskMonitor {
    pub fn new(mailbox_sender: Sender<SessionActions>) -> Arc<Self> {
        let wake_signal = flume::unbounded();
        let monitor = Self {
            task_handles: papaya::HashMap::new(),
            mailbox_sender,
            wake_signal,
        };

        Arc::new(monitor)
    }

    pub fn add_task(
        &self,
        task_id: TaskId,
        client_id: Uuid,
        task_handle: TaskHandle,
    ) -> Result<(), String> {
        // Insert the task handle into the map
        let guard = self.task_handles.guard();
        if self
            .task_handles
            .insert(task_id, (client_id, task_handle), &guard)
            .is_some()
        {
            Err(format!("Task ID {task_id} already exists"))
        } else {
            // Signal the waiting thread that a new task was added
            let _ = self.wake_signal.0.try_send(());
            Ok(())
        }
    }

    /// Block indefinitely waiting for task completions until kill switch is activated
    pub fn wait_for_completions(&self, kill_switch: Arc<AtomicBool>) {
        loop {
            if kill_switch.load(Ordering::Relaxed) {
                return;
            }

            // Collect all the receives into one select
            let task_count = self.task_handles.len();
            let mut receives = Vec::with_capacity(task_count);
            let mut task_client_ids = Vec::with_capacity(task_count);

            {
                let guard = self.task_handles.guard();
                for (task_id, (client_id, task_handle)) in self.task_handles.iter(&guard) {
                    receives.push(task_handle.receiver().clone());
                    task_client_ids.push((*task_id, *client_id));
                }
            }

            // If no tasks, wait for wake signal or timeout
            if receives.is_empty() {
                match self.wake_signal.1.recv_timeout(Duration::from_millis(1000)) {
                    Ok(_) => continue,  // Woken up by new task, restart loop
                    Err(_) => continue, // Timeout, check kill switch
                }
            }

            // Use flume's Selector to select across all task receivers simultaneously
            let selector = flume::Selector::new();

            // Add all task receivers to the selector with their index as the mapped value
            let selector = receives
                .iter()
                .enumerate()
                .fold(selector, |sel, (index, recv)| {
                    sel.recv(recv, move |result| (index, result))
                });

            match selector.wait_timeout(Duration::from_millis(1000)) {
                Ok((index, result)) => {
                    self.process_task_completion(index, result, &task_client_ids);
                }
                Err(_) => {
                    // Timeout, check kill switch and continue
                }
            }
        }
    }

    fn process_task_completion(
        &self,
        index: usize,
        result: Result<
            (
                TaskId,
                Result<moor_kernel::tasks::TaskResult, SchedulerError>,
            ),
            flume::RecvError,
        >,
        task_client_ids: &[(TaskId, uuid::Uuid)],
    ) {
        let client_id = task_client_ids[index].1;
        let task_id = task_client_ids[index].0;
        let guard = self.task_handles.guard();
        match result {
            Ok((task_id, r)) => {
                let result = match r {
                    Ok(moor_kernel::tasks::TaskResult::Result(v)) => match var_to_flatbuffer(&v) {
                        Ok(value_fb) => moor_rpc::ClientEvent {
                            event: moor_rpc::ClientEventUnion::TaskSuccessEvent(Box::new(
                                moor_rpc::TaskSuccessEvent {
                                    task_id: task_id as u64,
                                    result: Box::new(value_fb),
                                },
                            )),
                        },
                        Err(e) => {
                            tracing::error!(?task_id, ?client_id, error = ?e, "Failed to encode task result - likely contains lambda or anonymous object");
                            let error_event = moor_rpc::SchedulerError {
                                error: moor_rpc::SchedulerErrorUnion::SchedulerNotResponding(
                                    Box::new(moor_rpc::SchedulerNotResponding {}),
                                ),
                            };
                            moor_rpc::ClientEvent {
                                event: moor_rpc::ClientEventUnion::TaskErrorEvent(Box::new(
                                    moor_rpc::TaskErrorEvent {
                                        task_id: task_id as u64,
                                        error: Box::new(error_event),
                                    },
                                )),
                            }
                        }
                    },
                    Ok(moor_kernel::tasks::TaskResult::Replaced(th)) => {
                        info!(?client_id, ?task_id, "Task restarted");
                        self.task_handles.insert(task_id, (client_id, th), &guard);
                        return;
                    }
                    Err(e) => {
                        let scheduler_error = rpc_common::scheduler_error_to_flatbuffer_struct(&e)
                            .unwrap_or_else(|_| {
                                // Fallback to SchedulerNotResponding if conversion fails
                                moor_rpc::SchedulerError {
                                    error: moor_rpc::SchedulerErrorUnion::SchedulerNotResponding(
                                        Box::new(moor_rpc::SchedulerNotResponding {}),
                                    ),
                                }
                            });
                        moor_rpc::ClientEvent {
                            event: moor_rpc::ClientEventUnion::TaskErrorEvent(Box::new(
                                moor_rpc::TaskErrorEvent {
                                    task_id: task_id as u64,
                                    error: Box::new(scheduler_error),
                                },
                            )),
                        }
                    }
                };

                // Emit task completion event
                if let Err(e) = self
                    .mailbox_sender
                    .send(SessionActions::PublishTaskCompletion(client_id, result))
                {
                    tracing::error!(error = ?e, client_id = ?client_id, "Failed to send task completion for publishing");
                }

                // Remove the completed task
                self.task_handles.remove(&task_id, &guard);
            }
            Err(_e) => {
                // Task completion receive failed, remove the task
                self.task_handles.remove(&task_id, &guard);
            }
        }
    }
}
