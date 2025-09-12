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

use crate::CacheStats;
use ahash::AHasher;
use lazy_static::lazy_static;
use moor_common::model::PropDef;
use moor_var::{Obj, Symbol};
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::sync::{Arc, Mutex};

/// Create an optimized cache key by packing Obj and Symbol into a single u64.
/// Upper 32 bits: obj.id(), Lower 32 bits: symbol.compare_id()
fn make_cache_key(obj: &Obj, symbol: &Symbol) -> u64 {
    (obj.as_u64() << 32) | (symbol.compare_id() as u64)
}

lazy_static! {
    /// Global cache statistics for property lookups
    pub static ref PROP_CACHE_STATS: CacheStats = CacheStats::new();
    /// Global cache statistics for verb lookups
    pub static ref VERB_CACHE_STATS: CacheStats = CacheStats::new();
    /// Global cache statistics for ancestry lookups
    pub static ref ANCESTRY_CACHE_STATS: CacheStats = CacheStats::new();
    /// Global cache statistics for sysobj name lookups
    pub static ref SYSOBJ_NAME_CACHE_STATS: CacheStats = CacheStats::new();
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
                entries: Arc::new(HashMap::default()),
                first_parent_with_props_cache: Arc::new(HashMap::default()),
            }),
        }
    }
}

#[derive(Clone)]
struct Inner {
    orig_version: i64,
    version: i64,
    flushed: bool,

    entries: Arc<HashMap<u64, Option<PropDef>, BuildHasherDefault<AHasher>>>,
    first_parent_with_props_cache: Arc<HashMap<Obj, Option<Obj>, BuildHasherDefault<AHasher>>>,
}

impl Inner {
    /// Get a mutable reference to entries, cloning if necessary (copy-on-write)
    fn entries_mut(&mut self) -> &mut HashMap<u64, Option<PropDef>, BuildHasherDefault<AHasher>> {
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
        let key = make_cache_key(obj, prop);
        let result = inner.entries.get(&key).cloned();

        if result.is_some() {
            PROP_CACHE_STATS.hit();
        } else {
            PROP_CACHE_STATS.miss();
        }

        result
    }

    pub fn flush(&self) {
        let mut inner = self.inner.lock().unwrap();
        let entries_count = inner.entries.len() as isize;
        inner.flushed = true;
        inner.version += 1;
        inner.entries_mut().clear();
        inner.first_parent_cache_mut().clear();
        PROP_CACHE_STATS.flush();
        PROP_CACHE_STATS.remove_entries(entries_count);
    }

    pub fn fill_hit(&self, obj: &Obj, prop: &Symbol, propd: &PropDef) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;

        let key = make_cache_key(obj, prop);
        let is_new_entry = !inner.entries.contains_key(&key);
        inner.entries_mut().insert(key, Some(propd.clone()));
        if is_new_entry {
            PROP_CACHE_STATS.add_entry();
        }
    }

    pub fn fill_miss(&self, obj: &Obj, prop: &Symbol) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        let key = make_cache_key(obj, prop);
        let is_new_entry = !inner.entries.contains_key(&key);
        inner.entries_mut().insert(key, None);
        if is_new_entry {
            PROP_CACHE_STATS.add_entry();
        }
    }

    pub fn lookup_first_parent_with_props(&self, obj: &Obj) -> Option<Option<Obj>> {
        let inner = self.inner.lock().unwrap();
        inner.first_parent_with_props_cache.get(obj).cloned()
    }

    pub fn fill_first_parent_with_props(&self, obj: &Obj, parent: Option<Obj>) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        inner.first_parent_cache_mut().insert(*obj, parent);
    }
}

pub struct SysobjNameCache {
    inner: Mutex<SysobjInner>,
}

impl Default for SysobjNameCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SysobjNameCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(SysobjInner {
                version: 0,
                orig_version: 0,
                flushed: false,
                cached_data: Arc::new(None),
            }),
        }
    }

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

    pub fn lookup(&self) -> Option<std::collections::HashMap<Obj, Vec<Symbol>>> {
        let inner = self.inner.lock().unwrap();
        let result = inner.cached_data.as_ref().as_ref().cloned();

        if result.is_some() {
            SYSOBJ_NAME_CACHE_STATS.hit();
        } else {
            SYSOBJ_NAME_CACHE_STATS.miss();
        }

        result
    }

    pub fn flush(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.flushed = true;
        inner.version += 1;
        inner.cached_data = Arc::new(None);
        SYSOBJ_NAME_CACHE_STATS.flush();
        SYSOBJ_NAME_CACHE_STATS.remove_entries(1);
    }

    pub fn fill(&self, data: std::collections::HashMap<Obj, Vec<Symbol>>) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        let was_empty = inner.cached_data.is_none();
        inner.cached_data = Arc::new(Some(data));
        if was_empty {
            SYSOBJ_NAME_CACHE_STATS.add_entry();
        }
    }
}

#[derive(Clone)]
struct SysobjInner {
    orig_version: i64,
    version: i64,
    flushed: bool,
    cached_data: Arc<Option<std::collections::HashMap<Obj, Vec<Symbol>>>>,
}
