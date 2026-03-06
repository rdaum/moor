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

use moor_common::util::ConcurrentCounter;
use std::sync::OnceLock;

pub(crate) const LOCAL_STATS_BATCH_SIZE: u32 = 128;

#[derive(Default)]
pub(crate) struct LocalCacheStats {
    pub(crate) hits: u32,
    pub(crate) negative_hits: u32,
    pub(crate) misses: u32,
}

impl LocalCacheStats {
    #[inline]
    pub(crate) fn should_flush(&self) -> bool {
        self.hits + self.negative_hits + self.misses >= LOCAL_STATS_BATCH_SIZE
    }
}

/// Unified cache statistics structure
pub struct CacheStats {
    hits: ConcurrentCounter,
    negative_hits: ConcurrentCounter,
    misses: ConcurrentCounter,
    flushes: ConcurrentCounter,
    num_entries: ConcurrentCounter,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheStats {
    #[inline]
    fn default_shard_count() -> usize {
        static SHARD_COUNT: OnceLock<usize> = OnceLock::new();
        *SHARD_COUNT.get_or_init(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        })
    }

    pub fn new() -> Self {
        let shard_count = Self::default_shard_count();
        Self {
            hits: ConcurrentCounter::new(shard_count),
            negative_hits: ConcurrentCounter::new(shard_count),
            misses: ConcurrentCounter::new(shard_count),
            flushes: ConcurrentCounter::new(shard_count),
            num_entries: ConcurrentCounter::new(shard_count),
        }
    }

    pub fn hit(&self) {
        self.hits.add(1);
    }
    pub fn negative_hit(&self) {
        self.negative_hits.add(1);
    }
    pub fn miss(&self) {
        self.misses.add(1);
    }
    pub fn flush(&self) {
        self.flushes.add(1);
    }

    pub fn add_entry(&self) {
        self.num_entries.add(1);
    }

    #[inline]
    pub fn add_hits(&self, count: isize) {
        if count != 0 {
            self.hits.add(count);
        }
    }

    #[inline]
    pub fn add_negative_hits(&self, count: isize) {
        if count != 0 {
            self.negative_hits.add(count);
        }
    }

    #[inline]
    pub fn add_misses(&self, count: isize) {
        if count != 0 {
            self.misses.add(count);
        }
    }

    pub fn remove_entries(&self, count: isize) {
        self.num_entries.add(-count);
    }

    pub fn hit_count(&self) -> isize {
        self.hits.sum()
    }
    pub fn negative_hit_count(&self) -> isize {
        self.negative_hits.sum()
    }
    pub fn miss_count(&self) -> isize {
        self.misses.sum()
    }
    pub fn flush_count(&self) -> isize {
        self.flushes.sum()
    }

    pub fn num_entries(&self) -> isize {
        self.num_entries.sum()
    }

    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.sum() as f64;
        let negative_hits = self.negative_hits.sum() as f64;
        let misses = self.misses.sum() as f64;
        let total = hits + negative_hits + misses;
        if total > 0.0 {
            ((hits + negative_hits) / total) * 100.0
        } else {
            0.0
        }
    }
}
