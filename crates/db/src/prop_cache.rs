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
use arc_swap::ArcSwap;
use lazy_static::lazy_static;
use moor_common::model::PropDef;
use moor_var::{Obj, Symbol};
use std::{
    collections::{HashMap, HashSet},
    hash::BuildHasherDefault,
    sync::Arc,
};

/// Create an optimized cache key by packing Obj and Symbol into a single u64.
/// Upper 32 bits: obj.id(), Lower 32 bits: symbol.compare_id()
fn make_cache_key(obj: &Obj, symbol: &Symbol) -> u64 {
    (obj.as_u64() << 32) | (symbol.compare_id() as u64)
}

fn remove_entries_for_objects(
    entries: &mut HashMap<u64, Option<PropDef>, BuildHasherDefault<AHasher>>,
    obj_ids: &HashSet<u64>,
) -> usize {
    let before = entries.len();
    entries.retain(|key, _| {
        let obj_id = key >> 32;
        !obj_ids.contains(&obj_id)
    });
    before - entries.len()
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
    inner: ArcSwap<Inner>,
}

impl Default for PropResolutionCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PropResolutionCache {
    pub fn new() -> Self {
        Self {
            inner: ArcSwap::new(Arc::new(Inner {
                version: 0,
                orig_version: 0,
                flushed: false,
                entries: Arc::new(HashMap::default()),
                first_parent_with_props_cache: Arc::new(HashMap::default()),
            })),
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
        let inner = self.inner.load_full();
        let mut forked_inner = (*inner).clone();
        forked_inner.orig_version = inner.version;
        forked_inner.flushed = false;
        Box::new(Self {
            inner: ArcSwap::new(Arc::new(forked_inner)),
        })
    }

    pub fn has_changed(&self) -> bool {
        let inner = self.inner.load();
        inner.version > inner.orig_version
    }

    pub fn lookup(&self, obj: &Obj, prop: &Symbol) -> Option<Option<PropDef>> {
        let inner = self.inner.load();
        let key = make_cache_key(obj, prop);
        let result = inner.entries.get(&key).cloned();

        match &result {
            Some(Some(_)) => PROP_CACHE_STATS.hit(),
            Some(None) => PROP_CACHE_STATS.negative_hit(),
            None => PROP_CACHE_STATS.miss(),
        }

        result
    }

    pub fn flush(&self) {
        let entries_count = self.inner.load().entries.len() as isize;
        self.inner.rcu(|inner| {
            let mut new_inner = (**inner).clone();
            new_inner.flushed = true;
            new_inner.version += 1;
            new_inner.entries_mut().clear();
            new_inner.first_parent_cache_mut().clear();
            Arc::new(new_inner)
        });
        PROP_CACHE_STATS.flush();
        PROP_CACHE_STATS.remove_entries(entries_count);
    }

    pub fn fill_hit(&self, obj: &Obj, prop: &Symbol, propd: &PropDef) {
        let key = make_cache_key(obj, prop);
        let propd = propd.clone();
        self.inner.rcu(|inner| {
            let mut new_inner = (**inner).clone();
            new_inner.version += 1;
            let is_new_entry = !new_inner.entries.contains_key(&key);
            new_inner.entries_mut().insert(key, Some(propd.clone()));
            if is_new_entry {
                PROP_CACHE_STATS.add_entry();
            }
            Arc::new(new_inner)
        });
    }

    pub fn fill_miss(&self, obj: &Obj, prop: &Symbol) {
        let key = make_cache_key(obj, prop);
        self.inner.rcu(|inner| {
            let mut new_inner = (**inner).clone();
            new_inner.version += 1;
            let is_new_entry = !new_inner.entries.contains_key(&key);
            new_inner.entries_mut().insert(key, None);
            if is_new_entry {
                PROP_CACHE_STATS.add_entry();
            }
            Arc::new(new_inner)
        });
    }

    pub fn lookup_first_parent_with_props(&self, obj: &Obj) -> Option<Option<Obj>> {
        let inner = self.inner.load();
        inner.first_parent_with_props_cache.get(obj).cloned()
    }

    pub fn fill_first_parent_with_props(&self, obj: &Obj, parent: Option<Obj>) {
        self.inner.rcu(|inner| {
            let mut new_inner = (**inner).clone();
            new_inner.version += 1;
            new_inner.first_parent_cache_mut().insert(*obj, parent);
            Arc::new(new_inner)
        });
    }

    pub fn invalidate_objects(&self, objects: &[Obj]) {
        if objects.is_empty() {
            return;
        }
        let obj_ids: HashSet<u64> = objects.iter().map(|o| o.as_u64()).collect();
        self.inner.rcu(|inner| {
            let mut new_inner = (**inner).clone();
            let mut changed = false;

            let removed = remove_entries_for_objects(new_inner.entries_mut(), &obj_ids);
            if removed > 0 {
                changed = true;
                PROP_CACHE_STATS.remove_entries(removed as isize);
            }

            let first_parent_cache = new_inner.first_parent_cache_mut();
            let before = first_parent_cache.len();
            first_parent_cache.retain(|obj, _| !obj_ids.contains(&obj.as_u64()));
            if before != first_parent_cache.len() {
                changed = true;
            }

            if !changed {
                return inner.clone();
            }

            new_inner.version += 1;
            Arc::new(new_inner)
        });
    }
}
