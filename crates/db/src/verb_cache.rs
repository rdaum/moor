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

use crate::CacheStats;
use crate::prop_cache::{ANCESTRY_CACHE_STATS, VERB_CACHE_STATS};
use ahash::AHasher;
use moor_common::model::VerbDef;
use moor_var::{Obj, Symbol};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    hash::BuildHasherDefault,
    sync::{Arc, Mutex},
};

/// Create an optimized cache key by packing Obj and Symbol into a single u128.
/// Upper 64 bits: obj.as_u64(), Lower 64 bits: symbol.compare_id()
fn make_cache_key(obj: &Obj, symbol: &Symbol) -> u128 {
    ((obj.as_u64() as u128) << 64) | (symbol.compare_id() as u128)
}

fn remove_entries_for_objects(
    entries: &mut HashMap<u128, Option<VerbDef>, BuildHasherDefault<AHasher>>,
    obj_ids: &HashSet<u64>,
) -> usize {
    let before = entries.len();
    entries.retain(|key, _| {
        let obj_id = (key >> 64) as u64;
        !obj_ids.contains(&obj_id)
    });
    before - entries.len()
}

const LOCAL_STATS_BATCH_SIZE: u32 = 128;

#[derive(Default)]
struct LocalCacheStats {
    hits: u32,
    negative_hits: u32,
    misses: u32,
}

impl LocalCacheStats {
    #[inline]
    fn should_flush(&self) -> bool {
        self.hits + self.negative_hits + self.misses >= LOCAL_STATS_BATCH_SIZE
    }
}

struct VerbCacheStatsTls(LocalCacheStats);

impl VerbCacheStatsTls {
    #[inline]
    fn new() -> Self {
        Self(LocalCacheStats::default())
    }

    #[inline]
    fn flush_local(&mut self) {
        VERB_CACHE_STATS.add_hits(self.0.hits as isize);
        VERB_CACHE_STATS.add_negative_hits(self.0.negative_hits as isize);
        VERB_CACHE_STATS.add_misses(self.0.misses as isize);
        self.0 = LocalCacheStats::default();
    }
}

impl Drop for VerbCacheStatsTls {
    fn drop(&mut self) {
        self.flush_local();
    }
}

#[derive(Default)]
struct LocalAncestryStats {
    hits: u32,
    misses: u32,
}

impl LocalAncestryStats {
    #[inline]
    fn should_flush(&self) -> bool {
        self.hits + self.misses >= LOCAL_STATS_BATCH_SIZE
    }
}

struct AncestryCacheStatsTls(LocalAncestryStats);

impl AncestryCacheStatsTls {
    #[inline]
    fn new() -> Self {
        Self(LocalAncestryStats::default())
    }

    #[inline]
    fn flush_local(&mut self) {
        ANCESTRY_CACHE_STATS.add_hits(self.0.hits as isize);
        ANCESTRY_CACHE_STATS.add_misses(self.0.misses as isize);
        self.0 = LocalAncestryStats::default();
    }
}

impl Drop for AncestryCacheStatsTls {
    fn drop(&mut self) {
        self.flush_local();
    }
}

thread_local! {
    static VERB_CACHE_STATS_TLS: RefCell<VerbCacheStatsTls> = RefCell::new(VerbCacheStatsTls::new());
    static ANCESTRY_CACHE_STATS_TLS: RefCell<AncestryCacheStatsTls> = RefCell::new(AncestryCacheStatsTls::new());
}

#[inline]
fn verb_cache_hit() {
    VERB_CACHE_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.hits += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
}

#[inline]
fn verb_cache_negative_hit() {
    VERB_CACHE_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.negative_hits += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
}

#[inline]
fn verb_cache_miss() {
    VERB_CACHE_STATS_TLS.with(|tls| {
        let mut tls = tls.borrow_mut();
        tls.0.misses += 1;
        if tls.0.should_flush() {
            tls.flush_local();
        }
    });
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

pub struct VerbResolutionCache {
    inner: Mutex<Inner>,
    stats: &'static CacheStats,
}

impl Default for VerbResolutionCache {
    fn default() -> Self {
        Self::new()
    }
}

impl VerbResolutionCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                version: 0,
                orig_version: 0,
                flushed: false,
                entries: Arc::new(HashMap::default()),
                first_parent_with_verbs_cache: Arc::new(HashMap::default()),
            }),
            stats: &VERB_CACHE_STATS,
        }
    }
}

#[derive(Clone)]
struct Inner {
    orig_version: i64,
    version: i64,
    flushed: bool,

    entries: Arc<HashMap<u128, Option<VerbDef>, BuildHasherDefault<AHasher>>>,
    first_parent_with_verbs_cache: Arc<HashMap<Obj, Option<Obj>, BuildHasherDefault<AHasher>>>,
}

impl Inner {
    /// Get a mutable reference to entries, cloning if necessary (copy-on-write)
    fn entries_mut(&mut self) -> &mut HashMap<u128, Option<VerbDef>, BuildHasherDefault<AHasher>> {
        Arc::make_mut(&mut self.entries)
    }

    /// Get a mutable reference to first_parent_with_verbs_cache, cloning if necessary (copy-on-write)
    fn first_parent_cache_mut(
        &mut self,
    ) -> &mut HashMap<Obj, Option<Obj>, BuildHasherDefault<AHasher>> {
        Arc::make_mut(&mut self.first_parent_with_verbs_cache)
    }
}

impl VerbResolutionCache {
    pub fn fork(&self) -> Box<Self> {
        let inner = self.inner.lock().expect("verb cache mutex poisoned");
        let mut forked_inner = inner.clone();
        forked_inner.orig_version = inner.version;
        forked_inner.flushed = false;
        Box::new(Self {
            inner: Mutex::new(forked_inner),
            stats: self.stats,
        })
    }

    pub fn has_changed(&self) -> bool {
        let inner = self.inner.lock().expect("verb cache mutex poisoned");
        inner.version > inner.orig_version
    }

    pub(crate) fn lookup_first_parent_with_verbs(&self, obj: &Obj) -> Option<Option<Obj>> {
        let inner = self.inner.lock().expect("verb cache mutex poisoned");
        inner.first_parent_with_verbs_cache.get(obj).cloned()
    }

    pub(crate) fn fill_first_parent_with_verbs(&self, obj: &Obj, parent: Option<Obj>) {
        let mut inner = self.inner.lock().expect("verb cache mutex poisoned");
        inner.version += 1;
        inner.first_parent_cache_mut().insert(*obj, parent);
    }

    pub fn lookup(&self, obj: &Obj, verb: &Symbol) -> Option<Option<VerbDef>> {
        let inner = self.inner.lock().expect("verb cache mutex poisoned");
        let key = make_cache_key(obj, verb);
        let result = inner.entries.get(&key).cloned();

        match &result {
            Some(Some(_)) => verb_cache_hit(),
            Some(None) => verb_cache_negative_hit(),
            None => verb_cache_miss(),
        }

        result
    }

    pub fn flush(&self) {
        let mut inner = self.inner.lock().expect("verb cache mutex poisoned");
        let entries_count = inner.entries.len() as isize;
        inner.flushed = true;
        inner.version += 1;
        inner.entries_mut().clear();
        inner.first_parent_cache_mut().clear();
        self.stats.flush();
        self.stats.remove_entries(entries_count);
    }

    pub fn fill_hit(&self, obj: &Obj, verb: &Symbol, verbdef: &VerbDef) {
        let key = make_cache_key(obj, verb);
        let mut inner = self.inner.lock().expect("verb cache mutex poisoned");
        inner.version += 1;
        let is_new_entry = !inner.entries.contains_key(&key);
        inner.entries_mut().insert(key, Some(verbdef.clone()));
        if is_new_entry {
            self.stats.add_entry();
        }
    }

    pub fn fill_miss(&self, obj: &Obj, verb: &Symbol) {
        let key = make_cache_key(obj, verb);
        let mut inner = self.inner.lock().expect("verb cache mutex poisoned");
        inner.version += 1;
        let is_new_entry = !inner.entries.contains_key(&key);
        inner.entries_mut().insert(key, None);
        if is_new_entry {
            self.stats.add_entry();
        }
    }

    pub fn invalidate_objects(&self, objects: &[Obj]) {
        if objects.is_empty() {
            return;
        }
        let obj_ids: HashSet<u64> = objects.iter().map(|o| o.as_u64()).collect();
        let mut inner = self.inner.lock().expect("verb cache mutex poisoned");
        let mut changed = false;

        let removed = remove_entries_for_objects(inner.entries_mut(), &obj_ids);
        if removed > 0 {
            changed = true;
            self.stats.remove_entries(removed as isize);
        }

        let first_parent_cache = inner.first_parent_cache_mut();
        let before = first_parent_cache.len();
        first_parent_cache.retain(|obj, _| !obj_ids.contains(&obj.as_u64()));
        if before != first_parent_cache.len() {
            changed = true;
        }

        if changed {
            inner.version += 1;
        }
    }
}

pub struct AncestryCache {
    #[allow(clippy::type_complexity)]
    inner: Mutex<AncestryInner>,
    stats: &'static CacheStats,
}

impl Default for AncestryCache {
    fn default() -> Self {
        Self {
            inner: Mutex::new(AncestryInner {
                orig_version: 0,
                version: 0,
                flushed: false,
                entries: Arc::new(HashMap::default()),
            }),
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
    pub fn fork(&self) -> Box<Self> {
        let inner = self.inner.lock().expect("ancestry cache mutex poisoned");
        let mut forked_inner = inner.clone();
        forked_inner.orig_version = inner.version;
        forked_inner.flushed = false;
        Box::new(Self {
            inner: Mutex::new(forked_inner),
            stats: self.stats,
        })
    }
    pub fn lookup(&self, obj: &Obj) -> Option<Vec<Obj>> {
        let inner = self.inner.lock().expect("ancestry cache mutex poisoned");
        let result = inner.entries.get(obj).cloned();

        // Ancestry cache doesn't use Option wrapping, so we only have hits and misses
        if result.is_some() {
            ancestry_cache_hit();
        } else {
            ancestry_cache_miss();
        }

        result
    }

    pub fn flush(&self) {
        let mut inner = self.inner.lock().expect("ancestry cache mutex poisoned");
        let entries_count = inner.entries.len() as isize;
        inner.flushed = true;
        inner.version += 1;
        inner.entries_mut().clear();
        self.stats.flush();
        self.stats.remove_entries(entries_count);
    }

    pub fn fill(&self, obj: &Obj, ancestors: &[Obj]) {
        let obj = *obj;
        let mut inner = self.inner.lock().expect("ancestry cache mutex poisoned");
        inner.version += 1;
        let is_new_entry = !inner.entries.contains_key(&obj);
        inner.entries_mut().insert(obj, ancestors.to_vec());
        if is_new_entry {
            self.stats.add_entry();
        }
    }

    pub fn has_changed(&self) -> bool {
        let inner = self.inner.lock().expect("ancestry cache mutex poisoned");
        inner.version > inner.orig_version
    }

    pub fn invalidate_objects(&self, objects: &[Obj]) {
        if objects.is_empty() {
            return;
        }
        let objs: HashSet<Obj> = objects.iter().copied().collect();
        let mut inner = self.inner.lock().expect("ancestry cache mutex poisoned");
        let removed = {
            let entries = inner.entries_mut();
            let before = entries.len();
            entries.retain(|obj, _| !objs.contains(obj));
            before - entries.len()
        };
        if removed == 0 {
            return;
        }
        inner.version += 1;
        self.stats.remove_entries(removed as isize);
    }
}
