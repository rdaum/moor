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
}

pub struct PerfTimerGuard<'a>(&'a PerfCounter, Instant);

impl<'a> PerfTimerGuard<'a> {
    pub fn new(perf: &'a PerfCounter) -> Self {
        Self(perf, Instant::now())
    }
}

impl Drop for PerfTimerGuard<'_> {
    fn drop(&mut self) {
        let elapsed = self.1.elapsed().as_nanos();
        self.0.invocations().add(1);
        self.0.cumulative_duration_nanos().add(elapsed as isize);
    }
}
