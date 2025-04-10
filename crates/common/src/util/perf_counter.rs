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
