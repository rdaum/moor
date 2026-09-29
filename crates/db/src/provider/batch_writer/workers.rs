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

//! Own workers from startup through ordered shutdown, including partial startup.

use super::{EncoderMsg, RollupMsg, WriterMsg, remaining};
use flume::Sender;
use parking_lot::{Condvar, Mutex};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tracing::error;

type WorkerHandle = JoinHandle<Result<(), String>>;

pub(super) struct WorkerGroup {
    // Keep the writer channel connected until its explicit drain signal.
    _writer_sender: Sender<WriterMsg>,
    encoder_sender: Sender<EncoderMsg>,
    kill_switch: Arc<AtomicBool>,
    rollup_sender: Sender<RollupMsg>,
    encoders: Vec<WorkerHandle>,
    writer: Option<WorkerHandle>,
    rollup: Option<WorkerHandle>,
}

impl WorkerGroup {
    pub(super) fn new(
        writer_sender: Sender<WriterMsg>,
        encoder_sender: Sender<EncoderMsg>,
        kill_switch: Arc<AtomicBool>,
        rollup_sender: Sender<RollupMsg>,
    ) -> Self {
        Self {
            _writer_sender: writer_sender,
            encoder_sender,
            kill_switch,
            rollup_sender,
            encoders: Vec::new(),
            writer: None,
            rollup: None,
        }
    }

    pub(super) fn add_encoder(&mut self, handle: WorkerHandle) {
        self.encoders.push(handle);
    }

    pub(super) fn set_writer(&mut self, handle: WorkerHandle) {
        assert!(self.writer.is_none(), "writer already started");
        self.writer = Some(handle);
    }

    pub(super) fn set_rollup(&mut self, handle: WorkerHandle) {
        assert!(self.rollup.is_none(), "rollup encoder already started");
        self.rollup = Some(handle);
    }

    #[cfg(test)]
    fn shutdown(self) -> Result<(), String> {
        self.shutdown_until(Instant::now() + Duration::from_secs(30))
    }

    fn shutdown_until(mut self, deadline: Instant) -> Result<(), String> {
        self.stop_and_join(Some(deadline))
    }

    fn stop_and_join(&mut self, deadline: Option<Instant>) -> Result<(), String> {
        let mut first_error = None;
        for _ in 0..self.encoders.len() {
            if self
                .encoder_sender
                .send_timeout(EncoderMsg::Stop, remaining(deadline))
                .is_err()
            {
                first_error
                    .get_or_insert("failed to stop encoders before shutdown deadline".to_string());
                break;
            }
        }
        for handle in self.encoders.drain(..) {
            join(handle, "batch encoder", deadline, &mut first_error);
        }
        // The writer can finish its drain only after every encoder has sent its output.
        self.kill_switch.store(true, Ordering::SeqCst);
        if let Some(handle) = self.writer.take() {
            join(handle, "batch writer", deadline, &mut first_error);
        }
        // Foreground rollups remain available until the writer has finished.
        if let Some(handle) = self.rollup.take() {
            if self
                .rollup_sender
                .send_timeout(RollupMsg::Stop, remaining(deadline))
                .is_err()
            {
                first_error.get_or_insert(
                    "failed to stop rollup encoder before shutdown deadline".to_string(),
                );
            }
            join(
                handle,
                "property-value rollup encoder",
                deadline,
                &mut first_error,
            );
        }
        first_error.map_or(Ok(()), Err)
    }
}

fn join(
    handle: WorkerHandle,
    name: &str,
    deadline: Option<Instant>,
    first_error: &mut Option<String>,
) {
    while !handle.is_finished() {
        if remaining(deadline).is_zero() {
            first_error.get_or_insert_with(|| {
                format!("{name} thread did not stop before the persistence shutdown deadline")
            });
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let result = handle
        .join()
        .unwrap_or_else(|_| Err(format!("{name} thread panicked")));
    if let Err(error) = result {
        first_error.get_or_insert(error);
    }
}

impl Drop for WorkerGroup {
    fn drop(&mut self) {
        if let Err(error) = self.stop_and_join(Instant::now().checked_add(Duration::from_secs(30)))
        {
            error!("Failed to stop batch writer workers: {error}");
        }
    }
}

enum WorkerState {
    Running(WorkerGroup),
    Stopping,
    Stopped(Result<(), String>),
}

pub(super) struct Workers {
    state: Mutex<WorkerState>,
    stopped: Condvar,
}

impl Workers {
    pub(super) fn new(group: WorkerGroup) -> Self {
        Self {
            state: Mutex::new(WorkerState::Running(group)),
            stopped: Condvar::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn stop(&self) -> Result<(), String> {
        self.stop_until(Instant::now() + Duration::from_secs(30))
    }

    pub(super) fn stop_until(&self, deadline: Instant) -> Result<(), String> {
        let group = {
            let Some(mut state) = self.state.try_lock_for(remaining(Some(deadline))) else {
                return Err("timed out waiting for persistence shutdown".to_string());
            };
            loop {
                match &*state {
                    WorkerState::Stopped(result) => return result.clone(),
                    WorkerState::Stopping => {
                        if self
                            .stopped
                            .wait_for(&mut state, remaining(Some(deadline)))
                            .timed_out()
                        {
                            return Err("timed out waiting for persistence shutdown".to_string());
                        }
                    }
                    WorkerState::Running(_) => {
                        let WorkerState::Running(group) =
                            std::mem::replace(&mut *state, WorkerState::Stopping)
                        else {
                            unreachable!("running worker state held under lock");
                        };
                        break group;
                    }
                }
            }
        };
        // One caller owns all joins; other callers wait without holding this mutex.
        let result = group.shutdown_until(deadline);
        *self.state.lock() = WorkerState::Stopped(result.clone());
        self.stopped.notify_all();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::batch_writer::{BatchWriter, RollupEncoder};
    use crate::tx::Timestamp;
    use std::time::{Duration, Instant};

    #[test]
    fn partial_startup_unwinds_in_dependency_order() {
        for started in 1..=3 {
            let (events, receiver) = flume::unbounded();
            let result = std::panic::catch_unwind(|| {
                let (enc_sender, enc_receiver) = flume::bounded(1);
                let (rollup_sender, rollup_receiver) = flume::bounded(1);
                let kill = Arc::new(AtomicBool::new(false));
                let mut group = WorkerGroup::new(
                    flume::unbounded().0,
                    enc_sender,
                    kill.clone(),
                    rollup_sender.clone(),
                );
                let event = events.clone();
                group.set_rollup(std::thread::spawn(move || {
                    let result = BatchWriter::rollup_encoder_loop(rollup_receiver);
                    event.send("rollup").unwrap();
                    result
                }));
                if started >= 2 {
                    let event = events.clone();
                    group.set_writer(std::thread::spawn(move || {
                        let deadline = Instant::now() + Duration::from_secs(2);
                        while !kill.load(Ordering::SeqCst) {
                            assert!(Instant::now() < deadline, "writer never stopped");
                            std::thread::yield_now();
                        }
                        // Draining still needs a live rollup encoder.
                        RollupEncoder {
                            sender: rollup_sender,
                        }
                        .encode(moor_var::v_int(1), Timestamp(1))?;
                        event.send("writer").unwrap();
                        Ok(())
                    }));
                }
                if started >= 3 {
                    let event = events.clone();
                    group.add_encoder(std::thread::spawn(move || {
                        assert!(matches!(enc_receiver.recv().unwrap(), EncoderMsg::Stop));
                        event.send("encoder").unwrap();
                        Ok(())
                    }));
                }
                panic!("simulate failure starting the next worker");
            });
            assert!(result.is_err());
            let expected = match started {
                1 => vec!["rollup"],
                2 => vec!["writer", "rollup"],
                _ => vec!["encoder", "writer", "rollup"],
            };
            assert_eq!(receiver.try_iter().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn concurrent_and_repeated_stops_share_worker_error() {
        let (enc_sender, enc_receiver) = flume::bounded(1);
        let (rollup_sender, _rollup_receiver) = flume::bounded(1);
        let mut group = WorkerGroup::new(
            flume::unbounded().0,
            enc_sender,
            Arc::new(AtomicBool::new(false)),
            rollup_sender,
        );
        let (release, released) = flume::bounded(1);
        let (stopping, stopped) = flume::bounded(1);
        group.add_encoder(std::thread::spawn(move || {
            assert!(matches!(enc_receiver.recv().unwrap(), EncoderMsg::Stop));
            stopping.send(()).unwrap();
            released.recv().unwrap();
            Err("injected encoder error".to_string())
        }));
        let workers = Arc::new(Workers::new(group));
        let first = {
            let workers = workers.clone();
            std::thread::spawn(move || workers.stop())
        };
        stopped.recv_timeout(Duration::from_secs(2)).unwrap();
        // Accessing the mutex during the join proves the owner released it.
        assert!(matches!(*workers.state.lock(), WorkerState::Stopping));
        let second = {
            let workers = workers.clone();
            std::thread::spawn(move || workers.stop())
        };
        release.send(()).unwrap();
        let expected = Err("injected encoder error".to_string());
        assert_eq!(first.join().unwrap(), expected);
        assert_eq!(second.join().unwrap(), expected);
        assert_eq!(workers.stop(), expected);
    }

    #[test]
    fn worker_panic_does_not_skip_later_joins() {
        let (enc_sender, _enc_receiver) = flume::bounded(1);
        let (rollup_sender, rollup_receiver) = flume::bounded(1);
        let mut group = WorkerGroup::new(
            flume::unbounded().0,
            enc_sender,
            Arc::new(AtomicBool::new(false)),
            rollup_sender,
        );
        group.add_encoder(std::thread::spawn(|| panic!("injected worker panic")));
        let finished = Arc::new(AtomicBool::new(false));
        let joined = finished.clone();
        group.set_rollup(std::thread::spawn(move || {
            assert!(matches!(rollup_receiver.recv().unwrap(), RollupMsg::Stop));
            joined.store(true, Ordering::Release);
            Ok(())
        }));
        assert_eq!(
            group.shutdown(),
            Err("batch encoder thread panicked".to_string())
        );
        assert!(finished.load(Ordering::Acquire));
    }
}
