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

use crate::cache::PROP_CACHE_STATS;
use crate::cache::stats::{CacheStats, LocalCacheStats};
use ahash::AHasher;
use moor_common::model::PropDef;
use moor_var::{Obj, Symbol};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, hash_map::Entry},
    hash::BuildHasherDefault,
    sync::Arc,
};

/// Create an optimized cache key by packing Obj and Symbol into a single u128.
/// Upper 64 bits: obj.as_u64(), Lower 64 bits: symbol.compare_id()
fn make_cache_key(obj: &Obj, symbol: &Symbol) -> u128 {
    ((obj.as_u64() as u128) << 64) | (symbol.compare_id() as u128)
}

fn remove_entries_for_objects(
    entries: &mut HashMap<u128, Option<PropDef>, BuildHasherDefault<AHasher>>,
    obj_ids: &HashSet<u64>,
) -> usize {
    let before = entries.len();
    entries.retain(|key, _| {
        let obj_id = (key >> 64) as u64;
        !obj_ids.contains(&obj_id)
    });
    before - entries.len()
}

struct PropCacheStatsTls(LocalCacheStats);

impl PropCacheStatsTls {
    #[inline]
    fn new() -> Self {
        Self(LocalCacheStats::default())
    }

    #[inline]
    fn flush_local(&mut self) {
        PROP_CACHE_STATS.add_hits(self.0.hits as isize);
        PROP_CACHE_STATS.add_negative_hits(self.0.negative_hits as isize);
        PROP_CACHE_STATS.add_misses(self.0.misses as isize);
        self.0 = LocalCacheStats::default();
    }
}

impl Drop for PropCacheStatsTls {
    fn drop(&mut self) {
        self.flush_local();
    }
}

thread_local! {
    static PROP_CACHE_STATS_TLS: RefCell<PropCacheStatsTls> = RefCell::new(PropCacheStatsTls::new());
}

#[inline]
fn prop_cache_hit() {
    PROP_CACHE_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.hits += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
}

#[inline]
fn prop_cache_negative_hit() {
    PROP_CACHE_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.negative_hits += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
}

#[inline]
fn prop_cache_miss() {
    PROP_CACHE_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.misses += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
}

pub struct PropResolutionCache {
    inner: Inner,
    stats: &'static CacheStats,
}

impl Default for PropResolutionCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PropResolutionCache {
    pub fn new() -> Self {
        Self {
            inner: Inner {
                version: 0,
                guard_version: 0,
                orig_version: 0,
                flushed: false,
                entries: Arc::new(HashMap::default()),
                first_parent_with_props_cache: Arc::new(HashMap::default()),
            },
            stats: &PROP_CACHE_STATS,
        }
    }
}

#[derive(Clone)]
struct Inner {
    orig_version: i64,
    version: i64,
    guard_version: i64,
    flushed: bool,

    entries: Arc<HashMap<u128, Option<PropDef>, BuildHasherDefault<AHasher>>>,
    first_parent_with_props_cache: Arc<HashMap<Obj, Option<Obj>, BuildHasherDefault<AHasher>>>,
}

impl Inner {
    /// Get a mutable reference to entries, cloning if necessary (copy-on-write)
    fn entries_mut(&mut self) -> &mut HashMap<u128, Option<PropDef>, BuildHasherDefault<AHasher>> {
        Arc::make_mut(&mut self.entries)
    }

    /// Get a mutable reference to first_parent_with_props_cache, cloning if necessary (copy-on-write)
    fn first_parent_cache_mut(
        &mut self,
    ) -> &mut HashMap<Obj, Option<Obj>, BuildHasherDefault<AHasher>> {
        Arc::make_mut(&mut self.first_parent_with_props_cache)
    }
}

impl PropResolutionCache {
    pub fn fork(&self) -> Self {
        let mut forked_inner = self.inner.clone();
        forked_inner.orig_version = self.inner.version;
        forked_inner.flushed = false;
        Self {
            inner: forked_inner,
            stats: self.stats,
        }
    }

    pub fn has_changed(&self) -> bool {
        self.inner.version > self.inner.orig_version
    }

    #[inline]
    pub fn version(&self) -> i64 {
        self.inner.version
    }

    #[inline]
    pub fn guard_version(&self) -> i64 {
        self.inner.guard_version
    }

    pub fn lookup(&self, obj: &Obj, prop: &Symbol) -> Option<Option<PropDef>> {
        let key = make_cache_key(obj, prop);
        let result = self.inner.entries.get(&key).cloned();

        match &result {
            Some(Some(_)) => prop_cache_hit(),
            Some(None) => prop_cache_negative_hit(),
            None => prop_cache_miss(),
        }

        result
    }

    pub fn flush(&mut self) {
        let entries_count = self.inner.entries.len() as isize;
        self.inner.flushed = true;
        self.inner.version += 1;
        self.inner.guard_version += 1;
        self.inner.entries_mut().clear();
        self.inner.first_parent_cache_mut().clear();
        self.stats.flush();
        self.stats.remove_entries(entries_count);
    }

    pub fn fill_hit(&mut self, obj: &Obj, prop: &Symbol, propd: &PropDef) {
        let key = make_cache_key(obj, prop);
        self.inner.version += 1;
        let is_new_entry = match self.inner.entries_mut().entry(key) {
            Entry::Occupied(mut occupied) => {
                occupied.insert(Some(propd.clone()));
                false
            }
            Entry::Vacant(vacant) => {
                vacant.insert(Some(propd.clone()));
                true
            }
        };
        if is_new_entry {
            self.stats.add_entry();
        }
    }

    pub fn fill_miss(&mut self, obj: &Obj, prop: &Symbol) {
        let key = make_cache_key(obj, prop);
        self.inner.version += 1;
        let is_new_entry = match self.inner.entries_mut().entry(key) {
            Entry::Occupied(mut occupied) => {
                occupied.insert(None);
                false
            }
            Entry::Vacant(vacant) => {
                vacant.insert(None);
                true
            }
        };
        if is_new_entry {
            self.stats.add_entry();
        }
    }

    pub fn lookup_first_parent_with_props(&self, obj: &Obj) -> Option<Option<Obj>> {
        self.inner.first_parent_with_props_cache.get(obj).cloned()
    }

    pub fn fill_first_parent_with_props(&mut self, obj: &Obj, parent: Option<Obj>) {
        self.inner.version += 1;
        self.inner.first_parent_cache_mut().insert(*obj, parent);
    }

    pub fn invalidate_objects(&mut self, objects: &[Obj]) {
        if objects.is_empty() {
            return;
        }
        let obj_ids: HashSet<u64> = objects.iter().map(|o| o.as_u64()).collect();
        let mut changed = false;
        let removed = remove_entries_for_objects(self.inner.entries_mut(), &obj_ids);
        if removed > 0 {
            changed = true;
        }

        let first_parent_cache = self.inner.first_parent_cache_mut();
        let before = first_parent_cache.len();
        first_parent_cache.retain(|obj, _| !obj_ids.contains(&obj.as_u64()));
        if before != first_parent_cache.len() {
            changed = true;
        }

        if changed {
            self.inner.version += 1;
            self.inner.guard_version += 1;
        }

        if removed > 0 {
            self.stats.remove_entries(removed as isize);
        }
    }
}
