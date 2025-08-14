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
use moor_common::model::VerbDef;
use moor_var::{Obj, Symbol};
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::sync::{Arc, Mutex};

/// Create an optimized cache key by packing Obj and Symbol into a single u64.
/// Upper 32 bits: obj.id(), Lower 32 bits: symbol.compare_id()
fn make_cache_key(obj: &Obj, symbol: &Symbol) -> u64 {
    ((obj.id().0 as u64) << 32) | (symbol.compare_id() as u64)
}

use crate::prop_cache::{ANCESTRY_CACHE_STATS, VERB_CACHE_STATS};

pub struct VerbResolutionCache {
    inner: Mutex<Inner>,
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
    fn first_parent_cache_mut(&mut self) -> &mut HashMap<Obj, Option<Obj>, BuildHasherDefault<AHasher>> {
        Arc::make_mut(&mut self.first_parent_with_verbs_cache)
    }
}

impl VerbResolutionCache {
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

    pub(crate) fn lookup_first_parent_with_verbs(&self, obj: &Obj) -> Option<Option<Obj>> {
        let inner = self.inner.lock().unwrap();
        inner.first_parent_with_verbs_cache.get(obj).cloned()
    }

    pub(crate) fn fill_first_parent_with_verbs(&self, obj: &Obj, parent: Option<Obj>) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        inner.first_parent_cache_mut().insert(*obj, parent);
    }

    pub fn lookup(&self, obj: &Obj, verb: &Symbol) -> Option<Option<VerbDef>> {
        let inner = self.inner.lock().unwrap();
        let key = make_cache_key(obj, verb);
        let entry = inner.entries.get(&key);
        let result = entry.cloned();

        if result.is_some() {
            VERB_CACHE_STATS.hit();
        } else {
            VERB_CACHE_STATS.miss();
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
        VERB_CACHE_STATS.flush();
        VERB_CACHE_STATS.remove_entries(entries_count);
    }

    pub fn fill_hit(&self, obj: &Obj, verb: &Symbol, verbdef: &VerbDef) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        let key = make_cache_key(obj, verb);
        let is_new_entry = !inner.entries.contains_key(&key);
        inner.entries_mut().insert(key, Some(verbdef.clone()));
        if is_new_entry {
            VERB_CACHE_STATS.add_entry();
        }
    }

    pub fn fill_miss(&self, obj: &Obj, verb: &Symbol) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        let key = make_cache_key(obj, verb);
        let is_new_entry = !inner.entries.contains_key(&key);
        inner.entries_mut().insert(key, None);
        if is_new_entry {
            VERB_CACHE_STATS.add_entry();
        }
    }
}

pub struct AncestryCache {
    #[allow(clippy::type_complexity)]
    inner: Mutex<AncestryInner>,
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
        let inner = self.inner.lock().unwrap();
        let mut forked_inner = inner.clone();
        forked_inner.orig_version = inner.version;
        forked_inner.flushed = false;
        Box::new(Self {
            inner: Mutex::new(forked_inner),
        })
    }
    pub fn lookup(&self, obj: &Obj) -> Option<Vec<Obj>> {
        let inner = self.inner.lock().unwrap();
        let result = inner.entries.get(obj).cloned();

        if result.is_some() {
            ANCESTRY_CACHE_STATS.hit();
        } else {
            ANCESTRY_CACHE_STATS.miss();
        }

        result
    }

    pub fn flush(&self) {
        let mut inner = self.inner.lock().unwrap();
        let entries_count = inner.entries.len() as isize;
        inner.flushed = true;
        inner.version += 1;
        inner.entries_mut().clear();
        ANCESTRY_CACHE_STATS.flush();
        ANCESTRY_CACHE_STATS.remove_entries(entries_count);
    }

    pub fn fill(&self, obj: &Obj, ancestors: &[Obj]) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        let is_new_entry = !inner.entries.contains_key(obj);
        inner.entries_mut().insert(*obj, ancestors.to_vec());
        if is_new_entry {
            ANCESTRY_CACHE_STATS.add_entry();
        }
    }

    pub fn has_changed(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.version > inner.orig_version
    }
}
