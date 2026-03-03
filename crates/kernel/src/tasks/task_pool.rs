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

use flume::{Receiver, Sender};
use moor_common::threading::{
    pin_current_thread_to_core, set_current_task_worker_index, set_task_worker_count,
};
use std::{io, thread::JoinHandle};
use tracing::{error, warn};

trait WorkItem {
    fn run(self: Box<Self>);
}

impl<F> WorkItem for F
where
    F: FnOnce() + Send + 'static,
{
    fn run(self: Box<Self>) {
        (*self)()
    }
}

enum WorkerMsg {
    Run(Box<dyn WorkItem + Send + 'static>),
    Stop,
}

/// Fixed-size task worker pool with explicit worker lifecycle and affinity setup.
pub(crate) struct TaskThreadPool {
    sender: Sender<WorkerMsg>,
    threads: Vec<JoinHandle<()>>,
}

impl TaskThreadPool {
    pub(crate) fn new(num_threads: usize, pinned_core_ids: Option<Vec<usize>>) -> io::Result<Self> {
        let (sender, receiver) = flume::unbounded::<WorkerMsg>();
        let pinned_core_ids = pinned_core_ids.map(std::sync::Arc::new);
        set_task_worker_count(num_threads);

        let mut threads = Vec::with_capacity(num_threads);
        for index in 0..num_threads {
            let receiver = receiver.clone();
            let pinned_core_ids = pinned_core_ids.clone();

            let thread = std::thread::Builder::new()
                .name(format!("moor-task-pool-{index}"))
                .spawn(move || worker_loop(index, receiver, pinned_core_ids))?;
            threads.push(thread);
        }

        Ok(Self { sender, threads })
    }

    pub(crate) fn spawn<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let msg = WorkerMsg::Run(Box::new(task));
        if let Err(e) = self.sender.send(msg) {
            warn!(error = ?e, "Failed to enqueue task into task thread pool");
        }
    }
}

impl Drop for TaskThreadPool {
    fn drop(&mut self) {
        for _ in 0..self.threads.len() {
            self.sender.send(WorkerMsg::Stop).ok();
        }

        while let Some(thread) = self.threads.pop() {
            if let Err(e) = thread.join() {
                error!(error = ?e, "Task worker thread panicked during join");
            }
        }
    }
}

fn worker_loop(
    index: usize,
    receiver: Receiver<WorkerMsg>,
    pinned_core_ids: Option<std::sync::Arc<Vec<usize>>>,
) {
    set_current_task_worker_index(index);

    if let Some(core_ids) = pinned_core_ids {
        let core_id = core_ids[index % core_ids.len()];
        if let Err(e) = pin_current_thread_to_core(core_id) {
            warn!(
                thread_index = index,
                core_id,
                error = ?e,
                "Failed to pin task worker to core"
            );
        }
    }

    while let Ok(msg) = receiver.recv() {
        match msg {
            WorkerMsg::Run(task) => {
                let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    task.run();
                }));
                if let Err(panic_payload) = panic_result {
                    let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "Task worker panicked with unknown payload".to_string()
                    };

                    error!(
                        thread_index = index,
                        panic_msg, "Task worker recovered from task panic"
                    );
                }
            }
            WorkerMsg::Stop => return,
        }
    }
}
