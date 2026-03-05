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

use moor_var::Symbol;

use crate::util::ConcurrentCounter;
use minstant::Instant;
use std::cell::Cell;
use std::{sync::OnceLock, thread};

fn default_shard_count() -> usize {
    static SHARD_COUNT: OnceLock<usize> = OnceLock::new();
    *SHARD_COUNT.get_or_init(|| {
        thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    })
}

pub struct PerfCounter {
    pub operation: Symbol,
    pub invocations: ConcurrentCounter,
    pub cumulative_duration_nanos: ConcurrentCounter,
}

impl PerfCounter {
    pub fn new(name: impl Into<Symbol>) -> Self {
        Self {
            operation: name.into(),
            invocations: ConcurrentCounter::new(default_shard_count()),
            cumulative_duration_nanos: ConcurrentCounter::new(default_shard_count()),
        }
    }

    /// Get the invocations counter (when feature enabled) or a dummy counter (when disabled)
    pub fn invocations(&self) -> &ConcurrentCounter {
        &self.invocations
    }

    /// Get the cumulative duration counter (when feature enabled) or a dummy counter (when disabled)
    pub fn cumulative_duration_nanos(&self) -> &ConcurrentCounter {
        &self.cumulative_duration_nanos
    }

    /// Returns a sampled start instant for low-overhead timing, or `None` when this call should
    /// be unsampled.
    #[inline]
    pub fn sampled_start(&self) -> Option<Instant> {
        let _ = self;
        should_sample_timing().then(Instant::now)
    }

    /// Record elapsed time from an optionally sampled start instant, applying the sampling weight
    /// to preserve unbiased totals.
    #[inline]
    pub fn add_sampled_elapsed(&self, sampled_start: Option<Instant>) {
        if let Some(start) = sampled_start {
            let elapsed = start.elapsed().as_nanos();
            self.cumulative_duration_nanos()
                .add(scaled_elapsed_nanos(elapsed));
        }
    }
}

const PERF_TIMER_SAMPLE_SHIFT: u32 = 6;
const PERF_TIMER_SAMPLE_STRIDE: u128 = 1_u128 << PERF_TIMER_SAMPLE_SHIFT; // 1/64 sampling
const PERF_TIMER_SAMPLE_MASK: u64 = (1_u64 << PERF_TIMER_SAMPLE_SHIFT) - 1;

thread_local! {
    // Per-thread deterministic sampler tick for low-overhead timer sampling.
    static PERF_TIMER_SAMPLE_TICK: Cell<u64> = const { Cell::new(0) };
}

#[inline]
fn should_sample_timing() -> bool {
    PERF_TIMER_SAMPLE_TICK.with(|tick| {
        let next = tick.get().wrapping_add(1);
        tick.set(next);
        (next & PERF_TIMER_SAMPLE_MASK) == 0
    })
}

#[inline]
fn scaled_elapsed_nanos(elapsed_nanos: u128) -> isize {
    let scaled = elapsed_nanos.saturating_mul(PERF_TIMER_SAMPLE_STRIDE);
    isize::try_from(scaled).unwrap_or(isize::MAX)
}

pub struct PerfTimerGuard<'a> {
    perf: &'a PerfCounter,
    sampled_start: Option<Instant>,
}

impl<'a> PerfTimerGuard<'a> {
    pub fn new(perf: &'a PerfCounter) -> Self {
        let sampled_start = perf.sampled_start();
        Self {
            perf,
            sampled_start,
        }
    }

    pub fn from_start(perf: &'a PerfCounter, start: Instant) -> Self {
        let sampled_start = if should_sample_timing() {
            Some(start)
        } else {
            None
        };
        Self {
            perf,
            sampled_start,
        }
    }
}

impl Drop for PerfTimerGuard<'_> {
    fn drop(&mut self) {
        // Keep call counts exact even when timing is sampled.
        self.perf.invocations().add(1);
        self.perf.add_sampled_elapsed(self.sampled_start);
    }
}
