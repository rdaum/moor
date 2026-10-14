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

//! Scheduler service startup, loops, and shutdown.
//! Joins and external shutdown operations run without the lifecycle mutex.

use super::{Scheduler, SchedulerClientRequest, SchedulerState};
use crate::tasks::{DEFAULT_COMPACT_INTERVAL_SECONDS, workers::WorkerResponse};
use flume::{Receiver, RecvTimeoutError};
use moor_common::{
    tasks::{SchedulerError, SchedulerError::TaskAbortedCancelled, SessionFactory},
    threading::{set_current_thread_background_priority, spawn_perf},
};
use parking_lot::{Condvar, Mutex};
use std::{io, sync::Arc, thread::JoinHandle, time::Duration};
use tracing::{debug, error, info, warn};

/// Threads owned by a running scheduler. Explicit join waits for normal shutdown.
/// Dropping this owner first stops the scheduler, then joins its service threads.
#[must_use = "scheduler service threads must be joined during shutdown"]
pub struct SchedulerThreads {
    scheduler: Option<Scheduler>,
    handles: Vec<JoinHandle<()>>,
}

impl SchedulerThreads {
    /// Join every service thread and return the first panic, if any.
    /// The caller must arrange scheduler shutdown before waiting here.
    pub fn join(mut self) -> std::thread::Result<()> {
        if self
            .handles
            .iter()
            .any(|handle| handle.thread().id() == std::thread::current().id())
        {
            return Err(Box::new(
                "A scheduler service cannot join itself".to_string(),
            ));
        }
        // Explicit join preserves the wait-for-shutdown contract. Drop is the
        // fallback for abandoned handles, not a second shutdown after this join.
        self.scheduler.take();
        self.join_handles()
    }

    fn join_handles(&mut self) -> std::thread::Result<()> {
        let mut first_panic = None;
        // Reverse startup order: timer, client requests, then worker responses.
        while let Some(handle) = self.handles.pop() {
            if handle.thread().id() == std::thread::current().id() {
                // Shutdown has signalled this loop. It exits after its callback returns.
                continue;
            }
            if let Err(panic) = handle.join()
                && first_panic.is_none()
            {
                first_panic = Some(panic);
            }
        }
        first_panic.map_or(Ok(()), Err)
    }
}

impl Drop for SchedulerThreads {
    fn drop(&mut self) {
        let Some(scheduler) = self.scheduler.take() else {
            return;
        };
        if scheduler.state() == SchedulerState::Running {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scheduler.stop(None))) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => error!(
                    ?error,
                    "Scheduler shutdown failed while releasing service threads"
                ),
                Err(error) => error!(
                    ?error,
                    "Scheduler shutdown panicked while releasing service threads"
                ),
            }
        }
        if let Err(error) = self.join_handles() {
            error!(?error, "Scheduler service panicked during join");
        }
    }
}

#[derive(Clone, Copy)]
enum StartupOutcome {
    Pending,
    Started,
    Aborted,
}

struct StartupGate {
    outcome: Mutex<StartupOutcome>,
    ready: Condvar,
}

impl StartupGate {
    fn wait(&self) -> bool {
        let mut outcome = self.outcome.lock();
        while matches!(*outcome, StartupOutcome::Pending) {
            self.ready.wait(&mut outcome);
        }
        matches!(*outcome, StartupOutcome::Started)
    }

    fn resolve(&self, outcome: StartupOutcome) {
        *self.outcome.lock() = outcome;
        self.ready.notify_all();
    }
}

/// Owns restoration and threads until the scheduler can accept work.
/// Its destructor releases the startup gate before collecting thread handles.
struct SchedulerStartup {
    services: Option<SchedulerThreads>,
    gate: Arc<StartupGate>,
}

impl SchedulerStartup {
    fn new(scheduler: Scheduler) -> Self {
        Self {
            services: Some(SchedulerThreads {
                scheduler: Some(scheduler),
                handles: Vec::with_capacity(3),
            }),
            gate: Arc::new(StartupGate {
                outcome: Mutex::new(StartupOutcome::Pending),
                ready: Condvar::new(),
            }),
        }
    }

    fn spawn(
        &mut self,
        name: &'static str,
        run: impl FnOnce() + Send + 'static,
        spawner: &mut impl FnMut(&'static str, Box<dyn FnOnce() + Send>) -> io::Result<JoinHandle<()>>,
    ) -> Result<(), SchedulerError> {
        let gate = self.gate.clone();
        let handle = spawner(
            name,
            Box::new(move || {
                if gate.wait() {
                    run();
                }
            }),
        )
        .map_err(|error| {
            error!(name, ?error, "Could not start scheduler service");
            SchedulerError::CouldNotStartTask
        })?;
        self.services.as_mut().unwrap().handles.push(handle);
        Ok(())
    }

    fn finish(mut self) -> SchedulerThreads {
        let services = self.services.take().unwrap();
        services.scheduler.as_ref().unwrap().lifecycle.lock().state = SchedulerState::Running;
        self.gate.resolve(StartupOutcome::Started);
        services
    }
}

impl Drop for SchedulerStartup {
    fn drop(&mut self) {
        let Some(services) = self.services.as_ref() else {
            return;
        };
        // No service loop has run, so there are no GC or maintenance jobs to drain.
        // Restoration removes persisted continuations; save them again on failure.
        self.gate.resolve(StartupOutcome::Aborted);
        let scheduler = services.scheduler.as_ref().unwrap();
        {
            let mut lc = scheduler.lifecycle.lock();
            lc.state = SchedulerState::Stopped;
            lc.task_q.suspended.save_tasks();
            lc.save_schedules();
        }
        // The services field drops after the gate opens and outside the lifecycle lock.
    }
}

/// Always release service loops after shutdown, including an error or panic.
struct ShutdownCompletion<'a>(&'a Scheduler);

impl Drop for ShutdownCompletion<'_> {
    fn drop(&mut self) {
        self.0.lifecycle.lock().state = SchedulerState::Stopped;
        self.0.wake_timer_thread();
    }
}

impl Scheduler {
    /// Restore tasks, start services, then admit work and return their owner.
    pub fn start(
        &self,
        bg_session_factory: Arc<dyn SessionFactory>,
    ) -> Result<SchedulerThreads, SchedulerError> {
        self.start_with_spawner(bg_session_factory, spawn_perf)
    }

    fn start_with_spawner(
        &self,
        bg_session_factory: Arc<dyn SessionFactory>,
        mut spawner: impl FnMut(&'static str, Box<dyn FnOnce() + Send>) -> io::Result<JoinHandle<()>>,
    ) -> Result<SchedulerThreads, SchedulerError> {
        // Taking this receiver reserves the one allowed startup while state stays
        // Created. Other callers cannot enter restoration or admit work yet.
        let client_request_recv = {
            let lc = self.lifecycle.lock();
            if lc.state != SchedulerState::Created {
                return Err(SchedulerError::SchedulerNotResponding);
            }
            self.client_request_recv
                .lock()
                .take()
                .ok_or(SchedulerError::CouldNotStartTask)?
        };
        let mut startup = SchedulerStartup::new(self.clone());
        {
            let mut lc = self.lifecycle.lock();
            if let Some(max_restored_task_id) =
                lc.task_q.suspended.load_tasks(bg_session_factory.clone())
            {
                let next_restored_task_id = max_restored_task_id
                    .checked_add(1)
                    .expect("Restored task ID exhausted the task ID space");
                lc.next_task_id = lc.next_task_id.max(next_restored_task_id);
            }
            lc.load_schedules();
            lc.bg_session_factory = Some(bg_session_factory);
        }

        if let Some(recv) = self.worker_response_recv.lock().take() {
            let scheduler = self.clone();
            startup.spawn(
                "moor-worker-recv",
                move || scheduler.worker_response_loop(recv),
                &mut spawner,
            )?;
        }
        let scheduler = self.clone();
        startup.spawn(
            "moor-scheduler-requests",
            move || scheduler.client_request_loop(client_request_recv),
            &mut spawner,
        )?;
        let scheduler = self.clone();
        startup.spawn(
            "moor-timer",
            move || {
                set_current_thread_background_priority().ok();
                scheduler.timer_loop();
            },
            &mut spawner,
        )?;

        let services = startup.finish();
        info!("Scheduler started");
        Ok(services)
    }

    fn client_request_loop(&self, recv: Receiver<SchedulerClientRequest>) {
        loop {
            match recv.recv_timeout(Duration::from_millis(50)) {
                Ok(request) => request(self),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }

            if self.state() == SchedulerState::Stopped {
                break;
            }
        }
        debug!("Scheduler client request loop exited");
    }

    pub(crate) fn enqueue_client_request(
        &self,
        request: SchedulerClientRequest,
    ) -> Result<(), SchedulerError> {
        let lc = self.lifecycle.lock();
        if lc.state != SchedulerState::Running {
            return Err(SchedulerError::SchedulerNotResponding);
        }
        self.client_request_send
            .send(request)
            .map_err(|_| SchedulerError::SchedulerNotResponding)
    }

    /// The timer loop replaces the old run() main loop.
    /// Handles: timer expirations, GC checks, compaction, immediate wakes.
    fn timer_loop(&self) {
        loop {
            {
                let lc = self.lifecycle.lock();
                if lc.state == SchedulerState::Stopped {
                    break;
                }
            }

            let run_gc = {
                let lc = self.lifecycle.lock();
                lc.state == SchedulerState::Running
                    && self.config.features.anonymous_objects
                    && !lc.gc_phase.is_active()
                    && self.should_run_gc(&lc)
            };
            if run_gc {
                self.run_gc_cycle();
            }

            {
                let mut lc = self.lifecycle.lock();
                // Periodic tasks DB compaction
                if lc.last_compact_time.elapsed()
                    >= Duration::from_secs(DEFAULT_COMPACT_INTERVAL_SECONDS)
                {
                    debug!("Triggering periodic tasks database compaction");
                    lc.task_q.compact();
                    lc.last_compact_time = std::time::Instant::now();
                }
            }

            // Drain immediate wakes
            self.drain_immediate_wakes();

            // Collect timer-based wakes
            self.collect_and_wake_expired_tasks();

            // Settle finished firings and fire due native schedules
            {
                let mut lc = self.lifecycle.lock();
                lc.settle_schedule_firings();
            }
            self.collect_and_fire_schedules();

            // Sleep until next timer expiry or notification
            let tick_duration = self
                .config
                .runtime
                .scheduler_tick_duration
                .unwrap_or(Duration::from_millis(10));

            let (lock, cvar) = &*self.timer_notify;
            let mut notified = lock.lock();
            *notified = false;
            cvar.wait_for(&mut notified, tick_duration);
        }

        // Write out all the suspended tasks to the database.
        info!("Timer loop done; saving suspended tasks");
        let lc = self.lifecycle.lock();
        lc.task_q.suspended.save_tasks();
        lc.save_schedules();
        info!("Saved.");
    }

    /// Wake the timer thread to recompute its sleep duration.
    pub(crate) fn wake_timer_thread(&self) {
        let (lock, cvar) = &*self.timer_notify;
        let mut notified = lock.lock();
        *notified = true;
        cvar.notify_one();
    }

    /// Dedicated thread for receiving worker responses.
    fn worker_response_loop(&self, recv: Receiver<WorkerResponse>) {
        loop {
            match recv.recv_timeout(Duration::from_millis(50)) {
                Ok(response) => self.handle_worker_response(response),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }

            if self.state() == SchedulerState::Stopped {
                break;
            }
        }
        debug!("Worker response loop exited");
    }

    /// Stop the scheduler run loop.
    pub(crate) fn stop(&self, msg: Option<String>) -> Result<(), SchedulerError> {
        let _completion;
        // Stop accepting work, notify sessions, and ask active tasks to cancel. Keep their
        // scheduler records until their callbacks finish so shutdown can wait on real progress.
        {
            let mut lc = self.lifecycle.lock();
            if lc.state != SchedulerState::Running {
                return Err(SchedulerError::SchedulerNotResponding);
            }
            lc.state = SchedulerState::Stopping;
            _completion = ShutdownCompletion(self);
            // Accepted responses are runnable work, not durable waits. Settle them before save.
            lc.cancel_pending_resumes(&msg);

            // Notify all live tasks of shutdown.
            for task in lc.task_q.active.values() {
                let _ = task.session.notify_shutdown(msg.clone());
                task.control.request_cancel();
            }
            info!(
                active_tasks = lc.task_q.active.len(),
                "Stopping scheduler tasks"
            );
        }

        let active_maintenance = self.maintenance_coordinator.close();

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let active_tasks = self.lifecycle.lock().task_q.active.len();
            if active_tasks == 0 {
                break;
            }
            if std::time::Instant::now() >= deadline {
                let mut lc = self.lifecycle.lock();
                let remaining = lc.task_q.active.len();
                let task_ids = lc.task_q.active.keys().copied().collect::<Vec<_>>();
                for task_id in task_ids {
                    lc.discard_task_effects(task_id);
                    lc.task_q.remove_message_queue(task_id);
                    let Some(mut task) = lc.task_q.active.remove(&task_id) else {
                        continue;
                    };
                    lc.task_q.suspended.enqueue_dependents_for(task_id);
                    lc.task_q.send_task_result_direct(
                        task.registration,
                        task.result_sender.take(),
                        Err(TaskAbortedCancelled),
                    );
                }
                warn!(
                    remaining,
                    "Timed out waiting for scheduler tasks; detaching them"
                );
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        let gc_result = self.join_gc_thread();
        let maintenance_result = active_maintenance.map_or(Ok(()), |ticket| {
            info!(
                generation = ticket.generation(),
                kind = ?ticket.kind(),
                "Waiting for active database maintenance before shutdown"
            );
            ticket.wait()
        });

        // Now ask the rpc server and hosts to shutdown (no lock held).
        let system_result = self.system_control.shutdown(msg).map_err(|error| {
            error!(?error, "Could not cleanly shut down system services");
            SchedulerError::SchedulerNotResponding
        });

        info!("Scheduler tasks stopped");
        gc_result.and(maintenance_result).and(system_result)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{NoopSessionFactory, scheduler, suspended_task};
    use super::*;
    use crate::{
        config::Config,
        tasks::{TasksDb, TasksDbError, registry::SuspendedTask},
    };
    use moor_common::tasks::{NoopSystemControl, TaskId};
    use moor_db::{DatabaseConfig, TxDB};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn every_partial_startup_joins_services_and_rejects_work() {
        for fail_at in 1..=3 {
            let scheduler = scheduler();
            let (_worker_send, worker_receive) = flume::unbounded();
            *scheduler.worker_response_recv.lock() = Some(worker_receive);
            let finished = Arc::new(AtomicUsize::new(0));
            let mut attempted = 0;
            let result = scheduler.start_with_spawner(Arc::new(NoopSessionFactory), |name, run| {
                assert_eq!(scheduler.state(), SchedulerState::Created);
                assert!(
                    scheduler
                        .enqueue_client_request(Box::new(|_| panic!("startup admitted work")))
                        .is_err()
                );
                attempted += 1;
                if attempted == fail_at {
                    return Err(io::Error::other("injected service spawn failure"));
                }
                let finished = finished.clone();
                std::thread::Builder::new()
                    .name(name.into())
                    .spawn(move || {
                        run();
                        finished.fetch_add(1, Ordering::Release);
                    })
            });
            assert!(matches!(result, Err(SchedulerError::CouldNotStartTask)));
            assert_eq!(finished.load(Ordering::Acquire), fail_at - 1);
            assert_eq!(scheduler.state(), SchedulerState::Stopped);
            assert!(scheduler.start(Arc::new(NoopSessionFactory)).is_err());
        }
    }

    #[test]
    fn abandoned_service_owner_stops_and_joins_all_threads() {
        let scheduler = scheduler();
        let (_worker_send, worker_receive) = flume::unbounded();
        *scheduler.worker_response_recv.lock() = Some(worker_receive);
        let finished = Arc::new(AtomicUsize::new(0));
        let services = scheduler
            .start_with_spawner(Arc::new(NoopSessionFactory), |name, run| {
                let finished = finished.clone();
                std::thread::Builder::new()
                    .name(name.into())
                    .spawn(move || {
                        run();
                        finished.fetch_add(1, Ordering::Release);
                    })
            })
            .unwrap();
        assert_eq!(scheduler.state(), SchedulerState::Running);
        drop(services);
        assert_eq!(scheduler.state(), SchedulerState::Stopped);
        assert_eq!(finished.load(Ordering::Acquire), 3);
    }

    #[test]
    fn service_callback_can_release_its_own_thread_owner() {
        let scheduler = scheduler();
        let services = scheduler.start(Arc::new(NoopSessionFactory)).unwrap();
        let (finished, completion) = flume::bounded(1);
        scheduler
            .enqueue_client_request(Box::new(move |_| {
                drop(services);
                finished.send(()).unwrap();
            }))
            .unwrap();
        completion.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(scheduler.state(), SchedulerState::Stopped);
    }

    struct RestoredTasksDb {
        tasks: Mutex<Option<Vec<SuspendedTask>>>,
        saved: Arc<Mutex<Vec<TaskId>>>,
    }

    impl TasksDb for RestoredTasksDb {
        fn load_tasks(&self) -> Result<Vec<SuspendedTask>, TasksDbError> {
            Ok(self.tasks.lock().take().unwrap_or_default())
        }
        fn save_task(&self, task: &SuspendedTask) -> Result<(), TasksDbError> {
            self.saved.lock().push(task.task.task_id);
            Ok(())
        }
        fn delete_task(&self, _: TaskId) -> Result<(), TasksDbError> {
            Ok(())
        }
        fn delete_all_tasks(&self) -> Result<(), TasksDbError> {
            Ok(())
        }
        fn compact(&self) {}
    }

    #[test]
    fn failed_startup_saves_restored_continuations() {
        let (database, _) = TxDB::try_open_temporary(DatabaseConfig::default()).unwrap();
        let saved = Arc::new(Mutex::new(Vec::new()));
        let scheduler = Scheduler::new(
            semver::Version::new(0, 0, 0),
            Box::new(database),
            Box::new(RestoredTasksDb {
                tasks: Mutex::new(Some(vec![suspended_task(71)])),
                saved: saved.clone(),
            }),
            Arc::new(Config::default()),
            Arc::new(NoopSystemControl::default()),
            None,
            None,
        );
        let result = scheduler.start_with_spawner(Arc::new(NoopSessionFactory), |_, _| {
            Err(io::Error::other("injected service spawn failure"))
        });
        assert!(result.is_err());
        assert_eq!(*saved.lock(), [71]);
    }

    use moor_common::tasks::{EventLogPurgeResult, EventLogStats, SystemControl, WorkerInfo};
    use moor_var::{Error, Obj, Symbol, Var};
    use std::time::SystemTime;

    enum ShutdownFailure {
        Error,
        Panic,
    }
    impl SystemControl for ShutdownFailure {
        fn shutdown(&self, _: Option<String>) -> Result<(), Error> {
            match self {
                Self::Error => {
                    Err(moor_var::E_INVARG.with_msg(|| "injected shutdown failure".to_string()))
                }
                Self::Panic => panic!("injected shutdown panic"),
            }
        }

        fn listen(
            &self,
            _handler_object: Obj,
            _host_type: &str,
            _port: u16,
            _options: Vec<(Symbol, Var)>,
        ) -> Result<(), Error> {
            Ok(())
        }

        fn unlisten(&self, _port: u16, _host_type: &str) -> Result<(), Error> {
            Ok(())
        }

        fn listeners(&self) -> Result<Vec<(Obj, String, u16, Vec<(Symbol, Var)>)>, Error> {
            Ok(vec![])
        }

        fn switch_player(
            &self,
            _connection_obj: Obj,
            _new_player: Obj,
            _silent: bool,
            _preserve_history: bool,
        ) -> Result<(), Error> {
            Ok(())
        }

        fn rotate_enrollment_token(&self) -> Result<String, Error> {
            Ok(String::new())
        }

        fn player_event_log_stats(
            &self,
            _player: Obj,
            _since: Option<SystemTime>,
            _until: Option<SystemTime>,
        ) -> Result<EventLogStats, Error> {
            Ok(EventLogStats::default())
        }

        fn purge_player_event_log(
            &self,
            _player: Obj,
            _before: Option<SystemTime>,
            _drop_pubkey: bool,
        ) -> Result<EventLogPurgeResult, Error> {
            Ok(EventLogPurgeResult::default())
        }

        fn workers_info(&self) -> Result<Vec<WorkerInfo>, Error> {
            Ok(vec![])
        }
    }

    #[test]
    fn shutdown_error_still_releases_service_loops() {
        let mut scheduler = scheduler();
        scheduler.system_control = Arc::new(ShutdownFailure::Error);
        let services = scheduler.start(Arc::new(NoopSessionFactory)).unwrap();
        assert!(scheduler.stop(None).is_err());
        assert_eq!(scheduler.state(), SchedulerState::Stopped);
        services.join().unwrap();
    }

    #[test]
    fn shutdown_panic_during_drop_still_joins_services() {
        let mut scheduler = scheduler();
        scheduler.system_control = Arc::new(ShutdownFailure::Panic);
        let finished = Arc::new(AtomicUsize::new(0));
        let services = scheduler
            .start_with_spawner(Arc::new(NoopSessionFactory), |name, run| {
                let finished = finished.clone();
                std::thread::Builder::new()
                    .name(name.into())
                    .spawn(move || {
                        run();
                        finished.fetch_add(1, Ordering::Release);
                    })
            })
            .unwrap();
        drop(services);
        assert_eq!(scheduler.state(), SchedulerState::Stopped);
        assert_eq!(finished.load(Ordering::Acquire), 2);
    }
}

#[cfg(test)]
mod shutdown_tests;
