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

use crate::util::Instant;
use std::time::Duration;

/// A captured monotonic timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Timestamp(Instant);

impl Timestamp {
    #[inline]
    pub fn now() -> Self {
        Self(Instant::now())
    }

    #[inline]
    pub fn instant(self) -> Instant {
        self.0
    }

    #[inline]
    pub fn elapsed(self) -> Duration {
        self.0.elapsed()
    }

    #[inline]
    pub fn duration_since(self, earlier: Timestamp) -> Duration {
        self.0.duration_since(earlier.0)
    }
}

/// A monotonic future point used for timeout/deadline logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Deadline(Instant);

impl Deadline {
    #[inline]
    pub fn from_now(delay: Duration) -> Self {
        Self(Instant::now() + delay)
    }

    #[inline]
    pub fn at(instant: Instant) -> Self {
        Self(instant)
    }

    #[inline]
    pub fn instant(self) -> Instant {
        self.0
    }

    #[inline]
    pub fn is_expired(self) -> bool {
        self.is_expired_at(Instant::now())
    }

    #[inline]
    pub fn is_expired_at(self, now: Instant) -> bool {
        now >= self.0
    }

    #[inline]
    pub fn remaining(self) -> Option<Duration> {
        self.remaining_at(Instant::now())
    }

    #[inline]
    pub fn remaining_at(self, now: Instant) -> Option<Duration> {
        self.0.checked_duration_since(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_duration_since_is_non_negative() {
        let t0 = Timestamp::now();
        let t1 = Timestamp::now();
        let delta = t1.duration_since(t0);
        assert!(delta <= Duration::from_millis(5));
    }

    #[test]
    fn deadline_remaining_drops_to_none_after_expiry() {
        let now = Instant::now();
        let d = Deadline::at(now + Duration::from_millis(10));
        assert!(d.remaining_at(now).is_some());
        assert!(d.remaining_at(now + Duration::from_millis(20)).is_none());
        assert!(d.is_expired_at(now + Duration::from_millis(20)));
    }
}
