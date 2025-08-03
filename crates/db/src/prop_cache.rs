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

use ahash::AHasher;
use fast_counter::ConcurrentCounter;
use lazy_static::lazy_static;
use moor_common::model::PropDef;
use moor_var::{Obj, Symbol};
use std::hash::BuildHasherDefault;
use std::sync::Mutex;

/// Unified cache statistics structure
pub struct CacheStats {
    hits: ConcurrentCounter,
    misses: ConcurrentCounter,
    flushes: ConcurrentCounter,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheStats {
    pub fn new() -> Self {
        Self {
            hits: ConcurrentCounter::new(0),
            misses: ConcurrentCounter::new(0),
            flushes: ConcurrentCounter::new(0),
        }
    }

    pub fn hit(&self) {
        self.hits.add(1);
    }
    pub fn miss(&self) {
        self.misses.add(1);
    }
    pub fn flush(&self) {
        self.flushes.add(1);
    }

    pub fn hit_count(&self) -> isize {
        self.hits.sum()
    }
    pub fn miss_count(&self) -> isize {
        self.misses.sum()
    }
    pub fn flush_count(&self) -> isize {
        self.flushes.sum()
    }

    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.sum() as f64;
        let misses = self.misses.sum() as f64;
        let total = hits + misses;
        if total > 0.0 {
            (hits / total) * 100.0
        } else {
            0.0
        }
    }
}

lazy_static! {
    /// Global cache statistics for property lookups
    pub static ref PROP_CACHE_STATS: CacheStats = CacheStats::new();
    /// Global cache statistics for verb lookups
    pub static ref VERB_CACHE_STATS: CacheStats = CacheStats::new();
    /// Global cache statistics for ancestry lookups
    pub static ref ANCESTRY_CACHE_STATS: CacheStats = CacheStats::new();
}

pub struct PropResolutionCache {
    inner: Mutex<Inner>,
}

impl Default for PropResolutionCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PropResolutionCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                version: 0,
                orig_version: 0,
                flushed: false,
                entries: im::HashMap::default(),
                first_parent_with_props_cache: im::HashMap::default(),
            }),
        }
    }
}

#[derive(Clone)]
struct Inner {
    orig_version: i64,
    version: i64,
    flushed: bool,

    #[allow(clippy::type_complexity)]
    entries: im::HashMap<(Obj, Symbol), Option<PropDef>, BuildHasherDefault<AHasher>>,
    first_parent_with_props_cache: im::HashMap<Obj, Option<Obj>, BuildHasherDefault<AHasher>>,
}

impl PropResolutionCache {
    pub fn fork(&self) -> Box<Self> {
        let inner = self.inner.lock().unwrap();
        let mut forked_inner = inner.clone();
        forked_inner.orig_version = inner.version;
        forked_inner.flushed = false;
        Box::new(Self {
            inner: Mutex::new(forked_inner),
        })
    }

    pub fn has_changed(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.version > inner.orig_version
    }

    pub fn lookup(&self, obj: &Obj, prop: &Symbol) -> Option<Option<PropDef>> {
        let inner = self.inner.lock().unwrap();
        let result = inner.entries.get(&(*obj, *prop)).cloned();

        if result.is_some() {
            PROP_CACHE_STATS.hit();
        } else {
            PROP_CACHE_STATS.miss();
        }

        result
    }

    pub fn flush(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.flushed = true;
        inner.version += 1;
        inner.entries.clear();
        inner.first_parent_with_props_cache.clear();
        PROP_CACHE_STATS.flush();
    }

    pub fn fill_hit(&self, obj: &Obj, prop: &Symbol, propd: &PropDef) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;

        inner.entries.insert((*obj, *prop), Some(propd.clone()));
    }

    pub fn fill_miss(&self, obj: &Obj, prop: &Symbol) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        inner.entries.insert((*obj, *prop), None);
    }

    pub fn lookup_first_parent_with_props(&self, obj: &Obj) -> Option<Option<Obj>> {
        let inner = self.inner.lock().unwrap();
        inner.first_parent_with_props_cache.get(obj).cloned()
    }

    pub fn fill_first_parent_with_props(&self, obj: &Obj, parent: Option<Obj>) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        inner.first_parent_with_props_cache.insert(*obj, parent);
    }
}
