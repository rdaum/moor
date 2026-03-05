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

use crate::util::Instant;
use crate::util::{ConcurrentCounter, preferred_shared_shard_count};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub struct PerfCounter {
    pub operation: Symbol,
    pub invocations: ConcurrentCounter,
    pub cumulative_duration_nanos: ConcurrentCounter,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PerfIntensity {
    HotPath,
    MediumPath,
    RarePath,
    Exact,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PerfTimingPolicy {
    pub enabled: bool,
    pub hot_path_shift: u32,
    pub medium_path_shift: u32,
}

impl Default for PerfTimingPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            hot_path_shift: 6,
            medium_path_shift: 3,
        }
    }
}

const MAX_SAMPLE_SHIFT: u32 = 30;
static PERF_TIMING_ENABLED: AtomicBool = AtomicBool::new(true);
static PERF_TIMING_HOT_SHIFT: AtomicU32 = AtomicU32::new(6);
static PERF_TIMING_MEDIUM_SHIFT: AtomicU32 = AtomicU32::new(3);

#[inline]
const fn clamp_sample_shift(shift: u32) -> u32 {
    if shift > MAX_SAMPLE_SHIFT {
        MAX_SAMPLE_SHIFT
    } else {
        shift
    }
}

#[inline]
pub fn perf_timing_policy() -> PerfTimingPolicy {
    PerfTimingPolicy {
        enabled: PERF_TIMING_ENABLED.load(Ordering::Relaxed),
        hot_path_shift: PERF_TIMING_HOT_SHIFT.load(Ordering::Relaxed),
        medium_path_shift: PERF_TIMING_MEDIUM_SHIFT.load(Ordering::Relaxed),
    }
}

#[inline]
pub fn set_perf_timing_policy(policy: PerfTimingPolicy) {
    PERF_TIMING_ENABLED.store(policy.enabled, Ordering::Relaxed);
    PERF_TIMING_HOT_SHIFT.store(clamp_sample_shift(policy.hot_path_shift), Ordering::Relaxed);
    PERF_TIMING_MEDIUM_SHIFT.store(
        clamp_sample_shift(policy.medium_path_shift),
        Ordering::Relaxed,
    );
}

#[derive(Debug, Clone, Copy)]
pub struct PerfSample {
    started_at: Instant,
    shift: u32,
}

#[inline]
fn shift_for_intensity(intensity: PerfIntensity) -> Option<u32> {
    if intensity == PerfIntensity::Exact {
        return Some(0);
    }
    if !PERF_TIMING_ENABLED.load(Ordering::Relaxed) {
        return None;
    }

    match intensity {
        PerfIntensity::HotPath => Some(PERF_TIMING_HOT_SHIFT.load(Ordering::Relaxed)),
        PerfIntensity::MediumPath => Some(PERF_TIMING_MEDIUM_SHIFT.load(Ordering::Relaxed)),
        PerfIntensity::RarePath => Some(0),
        PerfIntensity::Exact => Some(0),
    }
}

impl PerfCounter {
    pub fn new(name: impl Into<Symbol>) -> Self {
        Self {
            operation: name.into(),
            invocations: ConcurrentCounter::new(preferred_shared_shard_count()),
            cumulative_duration_nanos: ConcurrentCounter::new(preferred_shared_shard_count()),
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

    /// Returns a sampled start marker for the requested intensity.
    #[inline]
    pub fn sampled_start_with_intensity(&self, intensity: PerfIntensity) -> Option<PerfSample> {
        let _ = self;
        let shift = shift_for_intensity(intensity)?;
        should_sample_timing(shift).then(|| PerfSample {
            started_at: Instant::now(),
            shift,
        })
    }

    /// Record elapsed time from an optionally sampled marker.
    #[inline]
    pub fn add_elapsed_sample(&self, sample: Option<PerfSample>) {
        if let Some(sample) = sample {
            let elapsed = sample.started_at.elapsed().as_nanos();
            self.cumulative_duration_nanos()
                .add(scaled_elapsed_nanos(elapsed, sample.shift));
        }
    }

    /// Measure execution of a closure, with sampled timing and exact invocation counts.
    #[inline]
    pub fn time<T>(&self, f: impl FnOnce() -> T) -> T {
        let _timer = PerfTimerGuard::new(self);
        f()
    }

    /// Measure execution of a closure using the requested timing intensity.
    #[inline]
    pub fn time_with_intensity<T>(&self, intensity: PerfIntensity, f: impl FnOnce() -> T) -> T {
        let _timer = PerfTimerGuard::with_intensity(self, intensity);
        f()
    }

    /// Measure execution of a fallible closure, with sampled timing and exact invocation counts.
    #[inline]
    pub fn time_result<T, E>(&self, f: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
        let _timer = PerfTimerGuard::new(self);
        f()
    }

    /// Measure execution of a fallible closure using the requested timing intensity.
    #[inline]
    pub fn time_result_with_intensity<T, E>(
        &self,
        intensity: PerfIntensity,
        f: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let _timer = PerfTimerGuard::with_intensity(self, intensity);
        f()
    }

    /// Record elapsed time from an external start point using an intensity-based policy.
    #[inline]
    pub fn record_elapsed_from_with(&self, intensity: PerfIntensity, started_at: Instant) {
        self.invocations().add(1);
        let Some(shift) = shift_for_intensity(intensity) else {
            return;
        };
        if !should_sample_timing(shift) {
            return;
        }
        let elapsed_nanos = started_at.elapsed().as_nanos().min(isize::MAX as u128) as isize;
        self.cumulative_duration_nanos()
            .add(scale_isize(elapsed_nanos, shift));
    }
}

thread_local! {
    // Per-thread deterministic sampler tick for low-overhead timer sampling.
    static PERF_TIMER_SAMPLE_TICK: Cell<u64> = const { Cell::new(0) };
}

#[inline]
fn should_sample_timing(shift: u32) -> bool {
    if shift == 0 {
        return true;
    }
    let sample_mask = (1_u64 << shift) - 1;
    PERF_TIMER_SAMPLE_TICK.with(|tick| {
        let next = tick.get().wrapping_add(1);
        tick.set(next);
        (next & sample_mask) == 0
    })
}

#[inline]
fn scaled_elapsed_nanos(elapsed_nanos: u128, shift: u32) -> isize {
    let stride = 1_u128 << shift;
    let scaled = elapsed_nanos.saturating_mul(stride);
    isize::try_from(scaled).unwrap_or(isize::MAX)
}

#[inline]
fn scale_isize(value: isize, shift: u32) -> isize {
    if shift == 0 {
        return value;
    }
    value.checked_shl(shift).unwrap_or(isize::MAX)
}

pub struct PerfTimerGuard<'a> {
    perf: &'a PerfCounter,
    sampled: Option<PerfSample>,
}

impl<'a> PerfTimerGuard<'a> {
    pub fn new(perf: &'a PerfCounter) -> Self {
        let sampled = perf.sampled_start_with_intensity(PerfIntensity::HotPath);
        Self { perf, sampled }
    }

    pub fn with_intensity(perf: &'a PerfCounter, intensity: PerfIntensity) -> Self {
        let sampled = perf.sampled_start_with_intensity(intensity);
        Self { perf, sampled }
    }

    pub fn from_start_with_intensity(
        perf: &'a PerfCounter,
        start: Instant,
        intensity: PerfIntensity,
    ) -> Self {
        let sampled = shift_for_intensity(intensity).and_then(|shift| {
            should_sample_timing(shift).then_some(PerfSample {
                started_at: start,
                shift,
            })
        });
        Self { perf, sampled }
    }
}

impl Drop for PerfTimerGuard<'_> {
    fn drop(&mut self) {
        // Keep call counts exact even when timing is sampled.
        self.perf.invocations().add(1);
        self.perf.add_elapsed_sample(self.sampled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    fn timing_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn with_policy(policy: PerfTimingPolicy, f: impl FnOnce()) {
        let old = perf_timing_policy();
        set_perf_timing_policy(policy);
        f();
        set_perf_timing_policy(old);
    }

    #[test]
    fn policy_roundtrip_and_clamp() {
        let _g = timing_test_lock();
        let old = perf_timing_policy();
        set_perf_timing_policy(PerfTimingPolicy {
            enabled: false,
            hot_path_shift: 999,
            medium_path_shift: 42,
        });
        let got = perf_timing_policy();
        assert!(!got.enabled);
        assert_eq!(got.hot_path_shift, MAX_SAMPLE_SHIFT);
        assert_eq!(got.medium_path_shift, MAX_SAMPLE_SHIFT);
        set_perf_timing_policy(old);
    }

    #[test]
    fn hot_path_disabled_skips_duration() {
        let _g = timing_test_lock();
        with_policy(
            PerfTimingPolicy {
                enabled: false,
                hot_path_shift: 6,
                medium_path_shift: 3,
            },
            || {
                let c = PerfCounter::new("disabled_hot");
                let start = Instant::now();
                std::thread::sleep(Duration::from_millis(1));
                c.record_elapsed_from_with(PerfIntensity::HotPath, start);
                assert_eq!(c.invocations().sum(), 1);
                assert_eq!(c.cumulative_duration_nanos().sum(), 0);
            },
        );
    }

    #[test]
    fn exact_intensity_records_even_when_disabled() {
        let _g = timing_test_lock();
        with_policy(
            PerfTimingPolicy {
                enabled: false,
                hot_path_shift: 6,
                medium_path_shift: 3,
            },
            || {
                let c = PerfCounter::new("exact_disabled");
                let start = Instant::now();
                std::thread::sleep(Duration::from_millis(1));
                c.record_elapsed_from_with(PerfIntensity::Exact, start);
                assert_eq!(c.invocations().sum(), 1);
                assert!(c.cumulative_duration_nanos().sum() > 0);
            },
        );
    }
}
