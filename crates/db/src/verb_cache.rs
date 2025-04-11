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
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::sync::RwLock;

pub(crate) struct VerbResolutionCache {
    inner: RwLock<Inner>,
}

impl VerbResolutionCache {
    pub(crate) fn new() -> Self {
        Self {
            inner: RwLock::new(Inner {
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

    #[allow(clippy::type_complexity)]
    entries: im::HashMap<(Obj, Symbol), Option<Vec<VerbDef>>, BuildHasherDefault<AHasher>>,
    first_parent_with_verbs_cache: im::HashMap<Obj, Option<Obj>, BuildHasherDefault<AHasher>>,
}

impl VerbResolutionCache {
    pub(crate) fn fork(&self) -> Self {
        let inner = self.inner.read().unwrap();
        let mut forked_inner = inner.clone();
        forked_inner.orig_version = inner.version;
        forked_inner.flushed = false;
        Self {
            inner: RwLock::new(forked_inner),
        }
    }

    pub(crate) fn has_changed(&self) -> bool {
        let inner = self.inner.read().unwrap();
        inner.version > inner.orig_version
    }

    pub(crate) fn lookup_first_parent_with_verbs(&self, obj: &Obj) -> Option<Option<Obj>> {
        let inner = self.inner.read().unwrap();
        inner.first_parent_with_verbs_cache.get(obj).cloned()
    }

    pub(crate) fn fill_first_parent_with_verbs(&self, obj: &Obj, parent: Option<Obj>) {
        let mut inner = self.inner.write().unwrap();
        inner.version += 1;
        inner
            .first_parent_with_verbs_cache
            .insert(obj.clone(), parent);
    }

    pub(crate) fn lookup(&self, obj: &Obj, verb: &Symbol) -> Option<Option<Vec<VerbDef>>> {
        let inner = self.inner.read().unwrap();
        inner.entries.get(&(obj.clone(), *verb)).cloned()
    }

    pub(crate) fn flush(&self) {
        let mut inner = self.inner.write().unwrap();
        inner.flushed = true;
        inner.version += 1;
        inner.entries.clear();
        inner.first_parent_with_verbs_cache.clear();
    }

    pub(crate) fn fill_hit(&self, obj: &Obj, verb: &Symbol, verbs: &[VerbDef]) {
        let mut inner = self.inner.write().unwrap();
        inner.version += 1;
        inner.entries.entry((obj.clone(), *verb)).and_modify(|e| {
            if let Some(e) = e {
                e.extend_from_slice(verbs);
            } else {
                *e = Some(verbs.to_vec());
            }
        });
    }

    pub(crate) fn fill_miss(&self, obj: &Obj, verb: &Symbol) {
        let mut inner = self.inner.write().unwrap();
        inner.version += 1;
        inner.entries.insert((obj.clone(), *verb), None);
    }
}

#[derive(Default)]
pub struct AncestryCache {
    #[allow(clippy::type_complexity)]
    entries: RefCell<HashMap<Obj, Vec<Obj>, BuildHasherDefault<AHasher>>>,
}

impl AncestryCache {
    pub(crate) fn lookup(&self, obj: &Obj) -> Option<Vec<Obj>> {
        self.entries.borrow().get(obj).cloned()
    }

    pub(crate) fn flush(&self) {
        self.entries.borrow_mut().clear();
    }

    pub(crate) fn fill(&self, obj: &Obj, ancestors: &[Obj]) {
        self.entries
            .borrow_mut()
            .insert(obj.clone(), ancestors.to_vec());
    }
}
