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

use fast_counter::ConcurrentCounter;
use moor_var::Symbol;
use std::time::Instant;

pub struct PerfCounter {
    pub operation: Symbol,
    pub invocations: ConcurrentCounter,
    pub cumulative_duration_us: ConcurrentCounter,
}

impl PerfCounter {
    pub fn new(name: &str) -> Self {
        Self {
            operation: Symbol::mk(name),
            invocations: ConcurrentCounter::new(0),
            cumulative_duration_us: ConcurrentCounter::new(0),
        }
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
        let elapsed = self.1.elapsed().as_micros();
        self.0.invocations.add(1);
        self.0.cumulative_duration_us.add(elapsed as isize);
    }
}
