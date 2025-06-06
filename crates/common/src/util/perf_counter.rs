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

use moor_var::Symbol;

/// No-op counter that does nothing when perf_counters feature is disabled
#[cfg(not(feature = "perf_counters"))]
pub struct DummyCounter;

#[cfg(not(feature = "perf_counters"))]
impl DummyCounter {
    #[inline]
    pub fn add(&self, _value: isize) {
        // No-op
    }
    
    #[inline]
    pub fn get(&self) -> isize {
        0
    }
    
    #[inline]
    pub fn sum(&self) -> isize {
        0
    }
}

#[cfg(feature = "perf_counters")]
use fast_counter::ConcurrentCounter;
#[cfg(feature = "perf_counters")]
use minstant::Instant;

#[cfg(feature = "perf_counters")]
pub struct PerfCounter {
    pub operation: Symbol,
    pub invocations: ConcurrentCounter,
    pub cumulative_duration_nanos: ConcurrentCounter,
}

#[cfg(not(feature = "perf_counters"))]
pub struct PerfCounter {
    pub operation: Symbol,
}

impl PerfCounter {
    #[cfg(feature = "perf_counters")]
    pub fn new(name: impl Into<Symbol>) -> Self {
        Self {
            operation: name.into(),
            invocations: ConcurrentCounter::new(0),
            cumulative_duration_nanos: ConcurrentCounter::new(0),
        }
    }

    #[cfg(not(feature = "perf_counters"))]
    pub fn new(name: impl Into<Symbol>) -> Self {
        Self {
            operation: name.into(),
        }
    }

    /// Get the invocations counter (when feature enabled) or a dummy counter (when disabled)
    #[cfg(feature = "perf_counters")]
    pub fn invocations(&self) -> &ConcurrentCounter {
        &self.invocations
    }

    /// Get the cumulative duration counter (when feature enabled) or a dummy counter (when disabled)
    #[cfg(feature = "perf_counters")]
    pub fn cumulative_duration_nanos(&self) -> &ConcurrentCounter {
        &self.cumulative_duration_nanos
    }

    /// No-op implementation when perf_counters feature is disabled
    #[cfg(not(feature = "perf_counters"))]
    pub fn invocations(&self) -> DummyCounter {
        DummyCounter
    }

    /// No-op implementation when perf_counters feature is disabled
    #[cfg(not(feature = "perf_counters"))]
    pub fn cumulative_duration_nanos(&self) -> DummyCounter {
        DummyCounter
    }
}

#[cfg(feature = "perf_counters")]
pub struct PerfTimerGuard<'a>(&'a PerfCounter, Instant);

#[cfg(not(feature = "perf_counters"))]
pub struct PerfTimerGuard<'a>(#[allow(dead_code)] &'a PerfCounter);

impl<'a> PerfTimerGuard<'a> {
    #[cfg(feature = "perf_counters")]
    pub fn new(perf: &'a PerfCounter) -> Self {
        Self(perf, Instant::now())
    }

    #[cfg(not(feature = "perf_counters"))]
    pub fn new(perf: &'a PerfCounter) -> Self {
        Self(perf)
    }
}

#[cfg(feature = "perf_counters")]
impl Drop for PerfTimerGuard<'_> {
    fn drop(&mut self) {
        let elapsed = self.1.elapsed().as_nanos();
        self.0.invocations().add(1);
        self.0.cumulative_duration_nanos().add(elapsed as isize);
    }
}

#[cfg(not(feature = "perf_counters"))]
impl Drop for PerfTimerGuard<'_> {
    fn drop(&mut self) {
        // No-op when perf_counters feature is disabled
    }
}
