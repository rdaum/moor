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

use super::*;

fn gc_error(context: &str, e: impl std::fmt::Debug) -> SchedulerError {
    SchedulerError::GarbageCollectionFailed(format!("{context}: {e:?}"))
}

impl Scheduler {
    /// Check if garbage collection should run
    pub(super) fn should_run_gc(&self) -> bool {
        // Force GC if requested via gc_collect() builtin
        if self.gc_force_collect {
            return true;
        }

        // Run automatic GC based on conditions
        self.should_run_automatic_gc()
    }

    /// Check if automatic GC should run based on heuristics
    fn should_run_automatic_gc(&self) -> bool {
        let gc_interval = if let Some(config_interval) = self.config.runtime.gc_interval {
            config_interval
        } else if let Some(db_secs) = self.server_options.gc_interval {
            Duration::from_secs(db_secs)
        } else {
            Duration::from_secs(DEFAULT_GC_INTERVAL_SECONDS)
        };

        let time_since_last_gc = self.gc_last_cycle_time.elapsed();

        if time_since_last_gc >= gc_interval {
            debug!(
                "Triggering automatic GC after {} seconds of inactivity (interval: {} seconds)",
                time_since_last_gc.as_secs(),
                gc_interval.as_secs()
            );
            return true;
        }

        // In the future, this could also check:
        // - Number of anonymous objects created since last GC
        // - Memory pressure
        // - Task activity levels
        false
    }

    /// Run a garbage collection cycle - mark & sweep collection
    pub(super) fn run_gc_cycle(&mut self) {
        self.gc_force_collect = false; // Clear force flag
        self.gc_cycle_count += 1;

        // Run concurrent mark & sweep GC with retry logic for conflicts
        let max_retries = 3;
        let mut mark_started = false;
        for attempt in 1..=max_retries {
            let result = self.run_concurrent_gc();

            match result {
                Ok(()) => {
                    mark_started = true;
                    break;
                } // Success, exit retry loop
                Err(e)
                    if e.to_string().contains("GC transaction conflict")
                        && attempt < max_retries =>
                {
                    warn!(
                        "GC cycle attempt {} failed with conflict, retrying in {}ms",
                        attempt,
                        attempt * 10
                    );
                    std::thread::sleep(Duration::from_millis((attempt * 10) as u64));
                    continue;
                }
                Err(e) => {
                    error!("GC cycle failed after {} attempts: {}", attempt, e);
                    break;
                }
            }
        }

        if !mark_started {
            self.task_q.suspended.enqueue_gc_waiting_tasks();
        }

        // Update the timestamp AFTER GC completes, not before
        self.gc_last_cycle_time = std::time::Instant::now();
    }

    /// Run concurrent mark & sweep GC
    fn run_concurrent_gc(&mut self) -> Result<(), SchedulerError> {
        // Collect VM references before spawning thread
        let vm_refs = self.task_q.collect_anonymous_object_references();
        let mutation_timestamp_before_mark = self.last_mutation_timestamp;

        // Create GC transaction for the background thread
        let gc_tx = self.database.gc_interface().map_err(|e| {
            SchedulerError::GarbageCollectionFailed(format!("Failed to create GC interface: {e}"))
        })?;
        let config_clone = self.config.clone();
        let scheduler_sender = self.scheduler_sender.clone();
        let gc_cycle_count = self.gc_cycle_count;

        // Set flag to prevent additional concurrent GC
        self.gc_mark_in_progress = true;

        // Spawn the mark thread
        let _handle = spawn_gc_mark_phase(
            gc_tx,
            config_clone,
            scheduler_sender,
            vm_refs,
            mutation_timestamp_before_mark,
            gc_cycle_count,
        );

        Ok(())
    }

    /// Wait for all active tasks to finish before starting sweep phase
    fn wait_for_active_tasks_to_finish(&mut self) -> Result<(), SchedulerError> {
        if self.task_q.active.is_empty() {
            info!("No active tasks to wait for");
            return Ok(());
        }

        debug!(
            "Waiting for {} active tasks to finish before GC sweep phase",
            self.task_q.active.len()
        );

        // Spin until all active tasks are done
        let tick_duration = self
            .config
            .runtime
            .scheduler_tick_duration
            .unwrap_or(Duration::from_millis(50));

        while !self.task_q.active.is_empty() {
            std::thread::sleep(tick_duration);

            // Process any incoming messages while waiting
            if let Ok((task_id, msg)) = self.task_control_receiver.try_recv() {
                self.handle_task_msg(task_id, msg);
            }
        }

        Ok(())
    }

    /// Blocking sweep phase for concurrent GC - waits for tasks and collects objects
    pub(super) fn run_blocking_sweep_phase(
        &mut self,
        unreachable_objects: std::collections::HashSet<Obj>,
    ) -> Result<(), SchedulerError> {
        debug!(
            "Starting blocking sweep phase for {} unreachable objects",
            unreachable_objects.len()
        );

        // Block new tasks during sweep
        self.gc_sweep_in_progress = true;

        // Check mutation timestamp before waiting for tasks
        let mutation_timestamp_before_wait = self.last_mutation_timestamp;

        // Wait for all active tasks to finish
        self.wait_for_active_tasks_to_finish()?;

        // Check mutation timestamp after waiting for tasks
        let mutation_timestamp_after_wait = self.last_mutation_timestamp;
        if mutation_timestamp_before_wait != mutation_timestamp_after_wait {
            info!(
                "Minor GC cycle #{}: mutations detected while waiting for tasks (before: {:?}, after: {:?}), sweep phase invalidated",
                self.gc_cycle_count, mutation_timestamp_before_wait, mutation_timestamp_after_wait
            );
            self.gc_sweep_in_progress = false;
            return Ok(());
        }

        // Run the actual sweep
        let result = self.run_gc_sweep_phase(std::collections::HashSet::new(), unreachable_objects);

        // Unblock new tasks
        self.gc_sweep_in_progress = false;

        result
    }

    /// Sweep phase of minor GC - collects unreachable objects (promotion already done in mark phase)
    fn run_gc_sweep_phase(
        &mut self,
        _reachable_objects: std::collections::HashSet<Obj>,
        unreachable_objects: std::collections::HashSet<Obj>,
    ) -> Result<(), SchedulerError> {
        let start_time = std::time::Instant::now();
        let perfc = sched_counters();
        let _t = PerfTimerGuard::new(&perfc.gc_sweep_phase);
        // Get a new GC interface for the sweep phase transaction
        let mut gc = self
            .database
            .gc_interface()
            .map_err(|e| gc_error("Failed to create GC interface for sweep phase", e))?;

        // Collect unreachable objects
        let collected = if !unreachable_objects.is_empty() {
            gc.collect_unreachable_anonymous_objects(&unreachable_objects)
                .map_err(|e| gc_error("Failed to collect unreachable objects", e))?
        } else {
            0
        };

        // Only log the collection if we actually collected some objects or if it took an unusual amount of time.
        let sweep_duration = start_time.elapsed();
        if collected != 0 || sweep_duration > Duration::from_secs(5) {
            if sweep_duration > Duration::from_secs(5) {
                warn!(
                    "GC sweep: {} objects collected in *{:.2}ms*",
                    collected,
                    sweep_duration.as_secs_f64() * 1000.0
                );
            } else {
                info!(
                    "GC sweep: {} objects collected in {:.2}ms",
                    collected,
                    sweep_duration.as_secs_f64() * 1000.0
                );
            }
        }

        // Commit the sweep phase transaction
        match gc.commit() {
            Ok(CommitResult::Success { .. }) => Ok(()),
            Ok(CommitResult::ConflictRetry { .. }) => {
                // Transaction conflict - our optimism wasn't justified
                warn!("GC sweep transaction conflict - retry needed");
                Err(SchedulerError::GarbageCollectionFailed(
                    "GC transaction conflict - retry needed".to_string(),
                ))
            }
            Err(e) => {
                error!("Failed to commit GC sweep transaction: {:?}", e);
                Err(gc_error("GC commit failed", e))
            }
        }
    }
}
