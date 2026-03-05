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

use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::sync::OnceLock;
use std::time::{Duration, Instant as StdInstant};

#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
const FIXED_SHIFT: u32 = 32;
#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
const NANOS_PER_SEC: u128 = 1_000_000_000;

/// Monotonic instant type tuned for low overhead.
///
/// On Linux x86/x86_64 this uses invariant TSC when available and calibrated.
/// Elsewhere it falls back to monotonic elapsed nanoseconds from `std::time::Instant`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Instant(u64);

impl Instant {
    pub const ZERO: Instant = Instant(0);

    #[inline]
    pub fn now() -> Instant {
        Instant(clock().now_ticks())
    }

    #[inline]
    pub fn elapsed(&self) -> Duration {
        Instant::now() - *self
    }

    #[inline]
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.checked_duration_since(earlier).unwrap_or_default()
    }

    #[inline]
    pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
        let delta = self.0.checked_sub(earlier.0)?;
        Some(clock().ticks_to_duration(delta))
    }

    #[inline]
    pub fn saturating_duration_since(&self, earlier: Instant) -> Duration {
        self.checked_duration_since(earlier).unwrap_or_default()
    }

    #[inline]
    pub fn checked_add(&self, duration: Duration) -> Option<Instant> {
        let ticks = clock().duration_to_ticks(duration);
        self.0.checked_add(ticks).map(Instant)
    }

    #[inline]
    pub fn checked_sub(&self, duration: Duration) -> Option<Instant> {
        let ticks = clock().duration_to_ticks(duration);
        self.0.checked_sub(ticks).map(Instant)
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    #[inline]
    fn add(self, other: Duration) -> Instant {
        self.checked_add(other)
            .expect("overflow when adding duration to instant")
    }
}

impl AddAssign<Duration> for Instant {
    #[inline]
    fn add_assign(&mut self, other: Duration) {
        *self = *self + other;
    }
}

impl Sub<Duration> for Instant {
    type Output = Instant;

    #[inline]
    fn sub(self, other: Duration) -> Instant {
        self.checked_sub(other)
            .expect("overflow when subtracting duration from instant")
    }
}

impl SubAssign<Duration> for Instant {
    #[inline]
    fn sub_assign(&mut self, other: Duration) {
        *self = *self - other;
    }
}

impl Sub<Instant> for Instant {
    type Output = Duration;

    #[inline]
    fn sub(self, other: Instant) -> Duration {
        self.duration_since(other)
    }
}

impl std::fmt::Debug for Instant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[inline]
pub fn is_tsc_available() -> bool {
    #[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
    {
        return matches!(clock(), Clock::Tsc(_));
    }

    #[cfg(not(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64"))))]
    {
        false
    }
}

static CLOCK: OnceLock<Clock> = OnceLock::new();

#[inline]
fn clock() -> &'static Clock {
    CLOCK.get_or_init(Clock::initialize)
}

enum Clock {
    #[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
    Tsc(TscClock),
    Monotonic(MonotonicClock),
}

impl Clock {
    fn initialize() -> Self {
        #[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
        {
            if let Some(tsc) = TscClock::try_new() {
                return Clock::Tsc(tsc);
            }
        }

        Clock::Monotonic(MonotonicClock::new())
    }

    #[inline]
    fn now_ticks(&self) -> u64 {
        match self {
            #[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
            Clock::Tsc(tsc) => tsc.now_ticks(),
            Clock::Monotonic(mono) => mono.now_ticks(),
        }
    }

    #[inline]
    fn ticks_to_duration(&self, ticks: u64) -> Duration {
        Duration::from_nanos(self.ticks_to_nanos(ticks))
    }

    #[inline]
    fn ticks_to_nanos(&self, ticks: u64) -> u64 {
        match self {
            #[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
            Clock::Tsc(tsc) => tsc.cycles_to_nanos(ticks),
            Clock::Monotonic(_) => ticks,
        }
    }

    #[inline]
    fn duration_to_ticks(&self, duration: Duration) -> u64 {
        let nanos = duration.as_nanos().min(u64::MAX as u128) as u64;
        match self {
            #[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
            Clock::Tsc(tsc) => tsc.nanos_to_cycles(nanos),
            Clock::Monotonic(_) => nanos,
        }
    }
}

struct MonotonicClock {
    base: StdInstant,
}

impl MonotonicClock {
    #[inline]
    fn new() -> Self {
        Self {
            base: StdInstant::now(),
        }
    }

    #[inline]
    fn now_ticks(&self) -> u64 {
        self.base.elapsed().as_nanos().min(u64::MAX as u128) as u64
    }
}

#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
struct TscClock {
    base_cycle: u64,
    ns_per_cycle_fp: u128,
    cycles_per_ns_fp: u128,
}

#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
impl TscClock {
    fn try_new() -> Option<Self> {
        if !linux_clocksource_is_tsc() || !cpu_has_invariant_tsc() {
            return None;
        }

        let cycles_per_sec = calibrate_cycles_per_second()?;
        if cycles_per_sec == 0 {
            return None;
        }

        let ns_per_cycle_fp =
            ((NANOS_PER_SEC << FIXED_SHIFT) + (cycles_per_sec / 2) as u128) / cycles_per_sec as u128;
        let cycles_per_ns_fp =
            (((cycles_per_sec as u128) << FIXED_SHIFT) + (NANOS_PER_SEC / 2)) / NANOS_PER_SEC;

        Some(Self {
            base_cycle: read_tsc(),
            ns_per_cycle_fp,
            cycles_per_ns_fp,
        })
    }

    #[inline]
    fn now_ticks(&self) -> u64 {
        read_tsc().wrapping_sub(self.base_cycle)
    }

    #[inline]
    fn cycles_to_nanos(&self, cycles: u64) -> u64 {
        let nanos = ((cycles as u128) * self.ns_per_cycle_fp) >> FIXED_SHIFT;
        nanos.min(u64::MAX as u128) as u64
    }

    #[inline]
    fn nanos_to_cycles(&self, nanos: u64) -> u64 {
        let cycles = ((nanos as u128) * self.cycles_per_ns_fp) >> FIXED_SHIFT;
        cycles.min(u64::MAX as u128) as u64
    }
}

#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
fn calibrate_cycles_per_second() -> Option<u64> {
    fn sample() -> Option<u64> {
        let started_at = StdInstant::now();
        let start_cycle = read_tsc();

        while started_at.elapsed().as_millis() < 20 {}

        let elapsed_ns = started_at.elapsed().as_nanos();
        if elapsed_ns == 0 {
            return None;
        }

        let end_cycle = read_tsc();
        let delta_cycles = end_cycle.wrapping_sub(start_cycle) as u128;
        if delta_cycles == 0 {
            return None;
        }

        let cycles_per_sec = delta_cycles.saturating_mul(NANOS_PER_SEC) / elapsed_ns;
        Some(cycles_per_sec.min(u64::MAX as u128) as u64)
    }

    let a = sample()?;
    let b = sample()?;
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    if lo == 0 {
        return None;
    }

    // Reject unstable calibration if samples diverge by more than 1%.
    if (hi - lo) * 100 > hi {
        return None;
    }

    Some((a + b) / 2)
}

#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
fn linux_clocksource_is_tsc() -> bool {
    std::fs::read_to_string("/sys/devices/system/clocksource/clocksource0/current_clocksource")
        .map(|s| s.trim() == "tsc")
        .unwrap_or(false)
}

#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
fn cpu_has_invariant_tsc() -> bool {
    #[cfg(target_arch = "x86")]
    use core::arch::x86::__cpuid;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::__cpuid;

    let max_extended_leaf = unsafe { __cpuid(0x8000_0000).eax };
    if max_extended_leaf < 0x8000_0007 {
        return false;
    }

    let leaf = unsafe { __cpuid(0x8000_0007) };
    (leaf.edx & (1 << 8)) != 0
}

#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
#[inline]
fn read_tsc() -> u64 {
    #[cfg(target_arch = "x86")]
    use core::arch::x86::_rdtsc;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::_rdtsc;

    unsafe { _rdtsc() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instant_is_monotonic() {
        let mut prev = Instant::now();
        for _ in 0..10_000 {
            let now = Instant::now();
            assert!(now >= prev);
            prev = now;
        }
    }

    #[test]
    fn arithmetic_roundtrip() {
        let now = Instant::now();
        let later = now + Duration::from_millis(50);
        assert!(later >= now);

        let back = later - Duration::from_millis(50);
        assert!(back <= later);

        let delta = later.duration_since(now);
        assert!(delta >= Duration::from_millis(49));
    }
}
