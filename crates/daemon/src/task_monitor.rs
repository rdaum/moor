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

use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use flume::Sender;
use moor_common::tasks::TaskId;
use moor_kernel::tasks::TaskHandle;
use rpc_common::ClientEvent;
use tracing::info;

use crate::rpc::SessionActions;

/// Monitors task completions and handles their lifecycle
pub struct TaskMonitor {
    task_handles: papaya::HashMap<TaskId, (Uuid, TaskHandle)>,
    mailbox_sender: Sender<SessionActions>,
}

impl TaskMonitor {
    pub fn new(mailbox_sender: Sender<SessionActions>) -> Arc<Self> {
        let monitor = Self {
            task_handles: papaya::HashMap::new(),
            mailbox_sender,
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
            Err(format!("Task ID {} already exists", task_id))
        } else {
            Ok(())
        }
    }

    /// Process incoming commands and task completions
    pub fn run_loop(&self, timeout: Duration) {
        // Check for completed tasks and process the first one found within the timeout
        if self.task_handles.is_empty() {
            return;
        }

        // Collect all the receives into one timeout-based select
        let mut receives = vec![];
        let mut task_client_ids = vec![];

        {
            let guard = self.task_handles.guard();
            for (task_id, (client_id, task_handle)) in self.task_handles.iter(&guard) {
                receives.push(task_handle.receiver().clone());
                task_client_ids.push((*task_id, *client_id));
            }
        }

        // Use flume's Selector to select across all receivers simultaneously
        let selector = flume::Selector::new();

        // Add all receivers to the selector with their index as the mapped value
        let selector = receives
            .iter()
            .enumerate()
            .fold(selector, |sel, (index, recv)| {
                sel.recv(recv, move |result| (index, result))
            });

        match selector.wait_timeout(timeout) {
            Ok((index, result)) => {
                let client_id = task_client_ids[index].1;
                let task_id = task_client_ids[index].0;
                let guard = self.task_handles.guard();
                match result {
                    Ok((task_id, r)) => {
                        let result = match r {
                            Ok(moor_kernel::tasks::TaskResult::Result(v)) => {
                                ClientEvent::TaskSuccess(task_id, v)
                            }
                            Ok(moor_kernel::tasks::TaskResult::Replaced(th)) => {
                                info!(?client_id, ?task_id, "Task restarted");
                                self.task_handles.insert(task_id, (client_id, th), &guard);
                                return;
                            }
                            Err(e) => ClientEvent::TaskError(task_id, e),
                        };

                        // Emit task completion event
                        // Send task completion directly to session actions
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
            Err(_) => {
                // Timeout, no tasks completed
            }
        }
    }
}
