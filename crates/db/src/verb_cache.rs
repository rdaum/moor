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
use std::hash::BuildHasherDefault;
use std::sync::Mutex;

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
                entries: im::HashMap::default(),
                first_parent_with_verbs_cache: im::HashMap::default(),
            }),
        }
    }
}

#[derive(Clone)]
struct Inner {
    orig_version: i64,
    version: i64,
    flushed: bool,

    entries: im::HashMap<u64, Option<VerbDef>, BuildHasherDefault<AHasher>>,
    first_parent_with_verbs_cache: im::HashMap<Obj, Option<Obj>, BuildHasherDefault<AHasher>>,
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
        inner.first_parent_with_verbs_cache.insert(*obj, parent);
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
        inner.flushed = true;
        inner.version += 1;
        inner.entries.clear();
        inner.first_parent_with_verbs_cache.clear();
        VERB_CACHE_STATS.flush();
    }

    pub fn fill_hit(&self, obj: &Obj, verb: &Symbol, verbdef: &VerbDef) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        let key = make_cache_key(obj, verb);
        inner.entries.insert(key, Some(verbdef.clone()));
    }

    pub fn fill_miss(&self, obj: &Obj, verb: &Symbol) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        let key = make_cache_key(obj, verb);
        inner.entries.insert(key, None);
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
                entries: im::HashMap::default(),
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
    entries: im::HashMap<Obj, Vec<Obj>, BuildHasherDefault<AHasher>>,
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
        inner.flushed = true;
        inner.version += 1;
        inner.entries.clear();
        ANCESTRY_CACHE_STATS.flush();
    }

    pub fn fill(&self, obj: &Obj, ancestors: &[Obj]) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        inner.entries.insert(*obj, ancestors.to_vec());
    }

    pub fn has_changed(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.version > inner.orig_version
    }
}
