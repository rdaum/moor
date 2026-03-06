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

use crate::cache::ANCESTRY_CACHE_STATS;
use crate::cache::stats::{CacheStats, LocalCacheStats};
use ahash::AHasher;
use moor_var::Obj;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, hash_map::Entry},
    hash::BuildHasherDefault,
    sync::Arc,
};

struct AncestryCacheStatsTls(LocalCacheStats);

impl AncestryCacheStatsTls {
    #[inline]
    fn new() -> Self {
        Self(LocalCacheStats::default())
    }

    #[inline]
    fn flush_local(&mut self) {
        ANCESTRY_CACHE_STATS.add_hits(self.0.hits as isize);
        ANCESTRY_CACHE_STATS.add_misses(self.0.misses as isize);
        self.0 = LocalCacheStats::default();
    }
}

impl Drop for AncestryCacheStatsTls {
    fn drop(&mut self) {
        self.flush_local();
    }
}

thread_local! {
    static ANCESTRY_CACHE_STATS_TLS: RefCell<AncestryCacheStatsTls> = RefCell::new(AncestryCacheStatsTls::new());
}

#[inline]
fn ancestry_cache_hit() {
    ANCESTRY_CACHE_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.hits += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
}

#[inline]
fn ancestry_cache_miss() {
    ANCESTRY_CACHE_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.misses += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
}

pub struct AncestryCache {
    #[allow(clippy::type_complexity)]
    inner: AncestryInner,
    stats: &'static CacheStats,
}

impl Default for AncestryCache {
    fn default() -> Self {
        Self {
            inner: AncestryInner {
                orig_version: 0,
                version: 0,
                flushed: false,
                entries: Arc::new(HashMap::default()),
            },
            stats: &ANCESTRY_CACHE_STATS,
        }
    }
}

#[derive(Clone)]
struct AncestryInner {
    orig_version: i64,
    version: i64,
    flushed: bool,

    #[allow(clippy::type_complexity)]
    entries: Arc<HashMap<Obj, Vec<Obj>, BuildHasherDefault<AHasher>>>,
}

impl AncestryInner {
    /// Get a mutable reference to entries, cloning if necessary (copy-on-write)
    fn entries_mut(&mut self) -> &mut HashMap<Obj, Vec<Obj>, BuildHasherDefault<AHasher>> {
        Arc::make_mut(&mut self.entries)
    }
}

impl AncestryCache {
    pub fn fork(&self) -> Self {
        let mut forked_inner = self.inner.clone();
        forked_inner.orig_version = self.inner.version;
        forked_inner.flushed = false;
        Self {
            inner: forked_inner,
            stats: self.stats,
        }
    }

    pub fn lookup(&self, obj: &Obj) -> Option<Vec<Obj>> {
        let result = self.inner.entries.get(obj).cloned();

        if result.is_some() {
            ancestry_cache_hit();
        } else {
            ancestry_cache_miss();
        }

        result
    }

    pub fn flush(&mut self) {
        let entries_count = self.inner.entries.len() as isize;
        self.inner.flushed = true;
        self.inner.version += 1;
        self.inner.entries_mut().clear();
        self.stats.flush();
        self.stats.remove_entries(entries_count);
    }

    pub fn fill(&mut self, obj: &Obj, ancestors: &[Obj]) {
        let obj = *obj;
        self.inner.version += 1;
        let is_new_entry = match self.inner.entries_mut().entry(obj) {
            Entry::Occupied(mut occupied) => {
                occupied.insert(ancestors.to_vec());
                false
            }
            Entry::Vacant(vacant) => {
                vacant.insert(ancestors.to_vec());
                true
            }
        };
        if is_new_entry {
            self.stats.add_entry();
        }
    }

    pub fn has_changed(&self) -> bool {
        self.inner.version > self.inner.orig_version
    }

    pub fn invalidate_objects(&mut self, objects: &[Obj]) {
        if objects.is_empty() {
            return;
        }
        let objs: HashSet<Obj> = objects.iter().copied().collect();
        let removed = {
            let entries = self.inner.entries_mut();
            let before = entries.len();
            entries.retain(|obj, _| !objs.contains(obj));
            before - entries.len()
        };
        if removed == 0 {
            return;
        }
        self.inner.version += 1;
        self.stats.remove_entries(removed as isize);
    }
}
