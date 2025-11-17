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
use arc_swap::ArcSwap;
use moor_common::model::VerbDef;
use moor_var::{Obj, Symbol};
use std::{
    collections::{HashMap, HashSet},
    hash::BuildHasherDefault,
    sync::Arc,
};

/// Create an optimized cache key by packing Obj and Symbol into a single u64.
/// Upper 32 bits: obj.id(), Lower 32 bits: symbol.compare_id()
fn make_cache_key(obj: &Obj, symbol: &Symbol) -> u64 {
    ((obj.as_u64()) << 32) | (symbol.compare_id() as u64)
}

fn remove_entries_for_objects(
    entries: &mut HashMap<u64, Option<VerbDef>, BuildHasherDefault<AHasher>>,
    obj_ids: &HashSet<u64>,
) -> usize {
    let before = entries.len();
    entries.retain(|key, _| {
        let obj_id = key >> 32;
        !obj_ids.contains(&obj_id)
    });
    before - entries.len()
}

use crate::prop_cache::{ANCESTRY_CACHE_STATS, VERB_CACHE_STATS};

pub struct VerbResolutionCache {
    inner: ArcSwap<Inner>,
}

impl Default for VerbResolutionCache {
    fn default() -> Self {
        Self::new()
    }
}

impl VerbResolutionCache {
    pub fn new() -> Self {
        Self {
            inner: ArcSwap::new(Arc::new(Inner {
                version: 0,
                orig_version: 0,
                flushed: false,
                entries: Arc::new(HashMap::default()),
                first_parent_with_verbs_cache: Arc::new(HashMap::default()),
            })),
        }
    }
}

#[derive(Clone)]
struct Inner {
    orig_version: i64,
    version: i64,
    flushed: bool,

    entries: Arc<HashMap<u64, Option<VerbDef>, BuildHasherDefault<AHasher>>>,
    first_parent_with_verbs_cache: Arc<HashMap<Obj, Option<Obj>, BuildHasherDefault<AHasher>>>,
}

impl Inner {
    /// Get a mutable reference to entries, cloning if necessary (copy-on-write)
    fn entries_mut(&mut self) -> &mut HashMap<u64, Option<VerbDef>, BuildHasherDefault<AHasher>> {
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

    pub(crate) fn lookup_first_parent_with_verbs(&self, obj: &Obj) -> Option<Option<Obj>> {
        let inner = self.inner.load();
        inner.first_parent_with_verbs_cache.get(obj).cloned()
    }

    pub(crate) fn fill_first_parent_with_verbs(&self, obj: &Obj, parent: Option<Obj>) {
        self.inner.rcu(|inner| {
            let mut new_inner = (**inner).clone();
            new_inner.version += 1;
            new_inner.first_parent_cache_mut().insert(*obj, parent);
            Arc::new(new_inner)
        });
    }

    pub fn lookup(&self, obj: &Obj, verb: &Symbol) -> Option<Option<VerbDef>> {
        let inner = self.inner.load();
        let key = make_cache_key(obj, verb);
        let result = inner.entries.get(&key).cloned();

        match &result {
            Some(Some(_)) => VERB_CACHE_STATS.hit(),
            Some(None) => VERB_CACHE_STATS.negative_hit(),
            None => VERB_CACHE_STATS.miss(),
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
        VERB_CACHE_STATS.flush();
        VERB_CACHE_STATS.remove_entries(entries_count);
    }

    pub fn fill_hit(&self, obj: &Obj, verb: &Symbol, verbdef: &VerbDef) {
        let key = make_cache_key(obj, verb);
        let verbdef = verbdef.clone();
        self.inner.rcu(|inner| {
            let mut new_inner = (**inner).clone();
            new_inner.version += 1;
            let is_new_entry = !new_inner.entries.contains_key(&key);
            new_inner.entries_mut().insert(key, Some(verbdef.clone()));
            if is_new_entry {
                VERB_CACHE_STATS.add_entry();
            }
            Arc::new(new_inner)
        });
    }

    pub fn fill_miss(&self, obj: &Obj, verb: &Symbol) {
        let key = make_cache_key(obj, verb);
        self.inner.rcu(|inner| {
            let mut new_inner = (**inner).clone();
            new_inner.version += 1;
            let is_new_entry = !new_inner.entries.contains_key(&key);
            new_inner.entries_mut().insert(key, None);
            if is_new_entry {
                VERB_CACHE_STATS.add_entry();
            }
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
                VERB_CACHE_STATS.remove_entries(removed as isize);
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

pub struct AncestryCache {
    #[allow(clippy::type_complexity)]
    inner: ArcSwap<AncestryInner>,
}

impl Default for AncestryCache {
    fn default() -> Self {
        Self {
            inner: ArcSwap::new(Arc::new(AncestryInner {
                orig_version: 0,
                version: 0,
                flushed: false,
                entries: Arc::new(HashMap::default()),
            })),
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
        let inner = self.inner.load_full();
        let mut forked_inner = (*inner).clone();
        forked_inner.orig_version = inner.version;
        forked_inner.flushed = false;
        Box::new(Self {
            inner: ArcSwap::new(Arc::new(forked_inner)),
        })
    }
    pub fn lookup(&self, obj: &Obj) -> Option<Vec<Obj>> {
        let inner = self.inner.load();
        let result = inner.entries.get(obj).cloned();

        // Ancestry cache doesn't use Option wrapping, so we only have hits and misses
        if result.is_some() {
            ANCESTRY_CACHE_STATS.hit();
        } else {
            ANCESTRY_CACHE_STATS.miss();
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
            Arc::new(new_inner)
        });
        ANCESTRY_CACHE_STATS.flush();
        ANCESTRY_CACHE_STATS.remove_entries(entries_count);
    }

    pub fn fill(&self, obj: &Obj, ancestors: &[Obj]) {
        let obj = *obj;
        let ancestors = ancestors.to_vec();
        self.inner.rcu(|inner| {
            let mut new_inner = (**inner).clone();
            new_inner.version += 1;
            let is_new_entry = !new_inner.entries.contains_key(&obj);
            new_inner.entries_mut().insert(obj, ancestors.clone());
            if is_new_entry {
                ANCESTRY_CACHE_STATS.add_entry();
            }
            Arc::new(new_inner)
        });
    }

    pub fn has_changed(&self) -> bool {
        let inner = self.inner.load();
        inner.version > inner.orig_version
    }

    pub fn invalidate_objects(&self, objects: &[Obj]) {
        if objects.is_empty() {
            return;
        }
        let objs: HashSet<Obj> = objects.iter().copied().collect();
        self.inner.rcu(|inner| {
            let mut new_inner = (**inner).clone();
            let removed = {
                let entries = new_inner.entries_mut();
                let before = entries.len();
                entries.retain(|obj, _| !objs.contains(obj));
                before - entries.len()
            };
            if removed == 0 {
                return inner.clone();
            }
            new_inner.version += 1;
            ANCESTRY_CACHE_STATS.remove_entries(removed as isize);
            Arc::new(new_inner)
        });
    }
}
