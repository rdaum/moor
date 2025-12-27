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

use crate::tasks::task_q::SuspendedTask;
use moor_common::tasks::TaskId;

#[derive(Debug, thiserror::Error)]
pub enum TasksDbError {
    #[error("Could not load tasks")]
    CouldNotLoadTasks,
    #[error("Could not save task")]
    CouldNotSaveTask,
    #[error("Could not delete task")]
    CouldNotDeleteTask,
    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),
}

pub trait TasksDb: Send {
    fn load_tasks(&self) -> Result<Vec<SuspendedTask>, TasksDbError>;
    fn save_task(&self, task: &SuspendedTask) -> Result<(), TasksDbError>;
    fn delete_task(&self, task_id: TaskId) -> Result<(), TasksDbError>;
    fn delete_all_tasks(&self) -> Result<(), TasksDbError>;

    /// Trigger database compaction to reclaim space and reduce journal size.
    /// Should be called periodically (e.g., every few minutes).
    fn compact(&self);
}

pub struct NoopTasksDb {}

impl TasksDb for NoopTasksDb {
    fn load_tasks(&self) -> Result<Vec<SuspendedTask>, TasksDbError> {
        Ok(vec![])
    }

    fn save_task(&self, _task: &SuspendedTask) -> Result<(), TasksDbError> {
        Ok(())
    }

    fn delete_task(&self, _task_id: TaskId) -> Result<(), TasksDbError> {
        Ok(())
    }

    fn delete_all_tasks(&self) -> Result<(), TasksDbError> {
        Ok(())
    }

    fn compact(&self) {
        // No-op for in-memory implementation
    }
}
