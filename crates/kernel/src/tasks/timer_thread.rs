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

//! Dedicated timer thread for high-precision task wake scheduling.
//! Owns the timer wheel and sends expiration notifications to the scheduler.

use std::time::Duration;

use flume::{Receiver, RecvTimeoutError, Sender};
use hierarchical_hash_wheel_timer::wheels::{
    TimerEntryWithDelay,
    quad_wheel::{PruneDecision, QuadWheelWithOverflow},
};
use minstant::Instant;
use tracing::{debug, error, trace, warn};

use moor_common::tasks::TaskId;

use crate::tasks::scheduler_message::SchedulerMessage;

/// Request to schedule a timer for a task
#[derive(Debug)]
pub struct TimerRequest {
    pub task_id: TaskId,
    pub wake_time: Instant,
}

/// Timer entry for the hash wheel timer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TimerEntry {
    task_id: TaskId,
    delay: Duration,
}

impl TimerEntryWithDelay for TimerEntry {
    fn delay(&self) -> Duration {
        self.delay
    }
}

/// Handle for communicating with the timer thread
#[derive(Clone)]
pub struct TimerThreadHandle {
    request_sender: Sender<TimerRequest>,
    poke_sender: Sender<()>,
}

impl TimerThreadHandle {
    /// Schedule a task to wake at the given time.
    /// Also pokes the timer thread to process immediately.
    pub fn schedule(&self, task_id: TaskId, wake_time: Instant) {
        if let Err(e) = self.request_sender.send(TimerRequest { task_id, wake_time }) {
            error!(?e, ?task_id, "Failed to send timer request - timer thread dead?");
            return;
        }
        // Poke to wake the timer thread immediately
        let _ = self.poke_sender.send(());
    }

    /// Schedule without poking - for bulk operations during startup
    pub fn schedule_no_poke(&self, task_id: TaskId, wake_time: Instant) {
        if let Err(e) = self.request_sender.send(TimerRequest { task_id, wake_time }) {
            error!(?e, ?task_id, "Failed to send timer request - timer thread dead?");
        }
    }
}

/// Spawns the timer thread and returns a handle for scheduling timers.
pub fn spawn_timer_thread(
    scheduler_sender: Sender<SchedulerMessage>,
) -> TimerThreadHandle {
    let (request_sender, request_receiver) = flume::unbounded();
    let (poke_sender, poke_receiver) = flume::unbounded();

    std::thread::Builder::new()
        .name("moor-timer".to_string())
        .spawn(move || {
            timer_thread_loop(request_receiver, poke_receiver, scheduler_sender);
        })
        .expect("Failed to spawn timer thread");

    TimerThreadHandle {
        request_sender,
        poke_sender,
    }
}

fn timer_thread_loop(
    request_receiver: Receiver<TimerRequest>,
    poke_receiver: Receiver<()>,
    scheduler_sender: Sender<SchedulerMessage>,
) {
    let mut timer_wheel: QuadWheelWithOverflow<TimerEntry> =
        QuadWheelWithOverflow::new(|_| PruneDecision::Keep);
    let mut last_tick = Instant::now();

    debug!("Timer thread started");

    loop {
        // Wait for either 1ms timeout OR poke signal
        match poke_receiver.recv_timeout(Duration::from_millis(1)) {
            Ok(()) => {
                trace!("Timer thread poked");
            }
            Err(RecvTimeoutError::Timeout) => {
                // Normal tick
            }
            Err(RecvTimeoutError::Disconnected) => {
                debug!("Timer thread shutting down - poke channel disconnected");
                break;
            }
        }

        // Drain all pending timer requests
        while let Ok(request) = request_receiver.try_recv() {
            let now = Instant::now();
            if request.wake_time <= now {
                // Already past deadline - send immediately
                if scheduler_sender
                    .send(SchedulerMessage::TimerExpired(request.task_id))
                    .is_err()
                {
                    warn!("Scheduler channel closed, timer thread exiting");
                    return;
                }
            } else {
                let delay = request.wake_time.duration_since(now);
                let entry = TimerEntry {
                    task_id: request.task_id,
                    delay,
                };
                if let Err(e) = timer_wheel.insert_with_delay(entry, delay) {
                    error!(?e, task_id = request.task_id, "Failed to insert timer");
                }
            }
        }

        // Advance the timer wheel based on elapsed time
        let now = Instant::now();
        let elapsed = now.duration_since(last_tick);
        let millis_elapsed = elapsed.as_millis() as u64;

        for _ in 0..millis_elapsed {
            let expired = timer_wheel.tick();
            for entry in expired {
                trace!(task_id = entry.task_id, "Timer expired");
                if scheduler_sender
                    .send(SchedulerMessage::TimerExpired(entry.task_id))
                    .is_err()
                {
                    warn!("Scheduler channel closed, timer thread exiting");
                    return;
                }
            }
        }

        // Update last tick by the actual milliseconds we processed
        if millis_elapsed > 0 {
            last_tick = last_tick + Duration::from_millis(millis_elapsed);
        }
    }

    debug!("Timer thread exited");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_timer_thread_immediate_expiry() {
        let (scheduler_sender, scheduler_receiver) = flume::unbounded();
        let handle = spawn_timer_thread(scheduler_sender);

        // Schedule a timer for "now" - should expire immediately
        let now = Instant::now();
        handle.schedule(42, now);

        // Should receive the expiration quickly
        let msg = scheduler_receiver
            .recv_timeout(Duration::from_millis(100))
            .expect("Should receive timer expiration");

        match msg {
            SchedulerMessage::TimerExpired(task_id) => {
                assert_eq!(task_id, 42);
            }
            _ => panic!("Expected TimerExpired message"),
        }
    }

    #[test]
    fn test_timer_thread_delayed_expiry() {
        let (scheduler_sender, scheduler_receiver) = flume::unbounded();
        let handle = spawn_timer_thread(scheduler_sender);

        // Schedule a timer for 10ms from now
        let wake_time = Instant::now() + Duration::from_millis(10);
        handle.schedule(123, wake_time);

        // Should NOT receive immediately
        assert!(
            scheduler_receiver
                .recv_timeout(Duration::from_millis(5))
                .is_err(),
            "Should not receive before timer expires"
        );

        // Should receive after the delay
        let msg = scheduler_receiver
            .recv_timeout(Duration::from_millis(50))
            .expect("Should receive timer expiration");

        match msg {
            SchedulerMessage::TimerExpired(task_id) => {
                assert_eq!(task_id, 123);
            }
            _ => panic!("Expected TimerExpired message"),
        }
    }
}
