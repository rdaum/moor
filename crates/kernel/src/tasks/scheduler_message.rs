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

//! Unified message type for the scheduler's main loop.
//! Consolidates all inbound channels into a single enum for simpler select logic.

use flume::Sender;
use moor_common::tasks::TaskId;

use crate::tasks::{
    scheduler_client::SchedulerClientMsg,
    task_scheduler_client::TaskControlMsg,
    workers::WorkerResponse,
};

/// All messages the scheduler can receive, unified into a single enum.
/// This eliminates the need for flume::Selector and simplifies the main loop.
pub enum SchedulerMessage {
    /// Control message from a running task
    Task(TaskId, TaskControlMsg),
    /// Client message (new tasks, shutdown, checkpoint, etc.)
    Client(SchedulerClientMsg),
    /// Response from an async worker
    Worker(WorkerResponse),
    /// Timer expired for a task - sent by the timer thread
    TimerExpired(TaskId),
    /// Immediate wake for a task - sent when a task is submitted with no delay
    ImmediateWake(TaskId),
}

/// Wrapper sender for task control messages that converts to SchedulerMessage
#[derive(Clone)]
pub struct TaskControlSender {
    inner: Sender<SchedulerMessage>,
}

impl TaskControlSender {
    pub fn new(inner: Sender<SchedulerMessage>) -> Self {
        Self { inner }
    }

    pub fn send(&self, task_id: TaskId, msg: TaskControlMsg) -> Result<(), flume::SendError<(TaskId, TaskControlMsg)>> {
        self.inner
            .send(SchedulerMessage::Task(task_id, msg))
            .map_err(|e| {
                // Extract the original message from the error
                match e.into_inner() {
                    SchedulerMessage::Task(id, m) => flume::SendError((id, m)),
                    _ => unreachable!(),
                }
            })
    }
}

/// Wrapper sender for scheduler client messages that converts to SchedulerMessage
#[derive(Clone)]
pub struct SchedulerClientSender {
    inner: Sender<SchedulerMessage>,
}

impl SchedulerClientSender {
    pub fn new(inner: Sender<SchedulerMessage>) -> Self {
        Self { inner }
    }

    pub fn send(&self, msg: SchedulerClientMsg) -> Result<(), flume::SendError<SchedulerClientMsg>> {
        self.inner
            .send(SchedulerMessage::Client(msg))
            .map_err(|e| {
                match e.into_inner() {
                    SchedulerMessage::Client(m) => flume::SendError(m),
                    _ => unreachable!(),
                }
            })
    }
}

/// Wrapper sender for worker response messages that converts to SchedulerMessage
#[derive(Clone)]
pub struct WorkerResponseSender {
    inner: Sender<SchedulerMessage>,
}

impl WorkerResponseSender {
    pub fn new(inner: Sender<SchedulerMessage>) -> Self {
        Self { inner }
    }

    pub fn send(&self, msg: WorkerResponse) -> Result<(), flume::SendError<WorkerResponse>> {
        self.inner
            .send(SchedulerMessage::Worker(msg))
            .map_err(|e| {
                match e.into_inner() {
                    SchedulerMessage::Worker(m) => flume::SendError(m),
                    _ => unreachable!(),
                }
            })
    }
}

/// Wrapper sender for immediate wake messages
#[derive(Clone)]
pub struct ImmediateWakeSender {
    inner: Sender<SchedulerMessage>,
}

impl ImmediateWakeSender {
    pub fn new(inner: Sender<SchedulerMessage>) -> Self {
        Self { inner }
    }

    pub fn send(&self, task_id: TaskId) -> Result<(), flume::SendError<TaskId>> {
        self.inner
            .send(SchedulerMessage::ImmediateWake(task_id))
            .map_err(|e| {
                match e.into_inner() {
                    SchedulerMessage::ImmediateWake(id) => flume::SendError(id),
                    _ => unreachable!(),
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn test_scheduler_message_size() {
        // Should be reasonably sized - dominated by the largest variant
        let size = size_of::<SchedulerMessage>();
        assert!(
            size <= 128,
            "SchedulerMessage is unexpectedly large: {} bytes",
            size
        );
    }
}
