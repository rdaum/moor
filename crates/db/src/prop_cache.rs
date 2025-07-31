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

use crate::tx_management::indexes::ToRartKey;
use moor_common::model::PropDef;
use moor_var::{Obj, Symbol};
use rart::{ArrayKey, VersionedAdaptiveRadixTree};
use std::sync::Mutex;

pub(crate) struct PropResolutionCache {
    inner: Mutex<Inner>,
}

impl PropResolutionCache {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                version: 0,
                orig_version: 0,
                flushed: false,
                entries: VersionedAdaptiveRadixTree::new(),
                first_parent_with_props_cache: VersionedAdaptiveRadixTree::new(),
            }),
        }
    }
}

#[derive(Clone)]
struct Inner {
    orig_version: i64,
    version: i64,
    flushed: bool,

    entries: VersionedAdaptiveRadixTree<ArrayKey<8>, Option<PropDef>>,
    first_parent_with_props_cache: VersionedAdaptiveRadixTree<ArrayKey<4>, Option<Obj>>,
}

impl PropResolutionCache {
    pub(crate) fn fork(&self) -> Box<Self> {
        let inner = self.inner.lock().unwrap();
        let forked_inner = Inner {
            orig_version: inner.version,
            version: inner.version,
            flushed: false,
            entries: inner.entries.snapshot(),
            first_parent_with_props_cache: inner.first_parent_with_props_cache.snapshot(),
        };
        Box::new(Self {
            inner: Mutex::new(forked_inner),
        })
    }

    pub(crate) fn has_changed(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.version > inner.orig_version
    }

    pub(crate) fn lookup(&self, obj: &Obj, prop: &Symbol) -> Option<Option<PropDef>> {
        let inner = self.inner.lock().unwrap();
        let key = (*obj, *prop).to_rart_key();
        inner.entries.get(key).cloned()
    }

    pub(crate) fn flush(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.flushed = true;
        inner.version += 1;
        inner.entries = VersionedAdaptiveRadixTree::new();
        inner.first_parent_with_props_cache = VersionedAdaptiveRadixTree::new();
    }

    pub(crate) fn fill_hit(&self, obj: &Obj, prop: &Symbol, propd: &PropDef) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        let key = (*obj, *prop).to_rart_key();
        inner.entries.insert_k(&key, Some(propd.clone()));
    }

    pub(crate) fn fill_miss(&self, obj: &Obj, prop: &Symbol) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        let key = (*obj, *prop).to_rart_key();
        inner.entries.insert_k(&key, None);
    }

    pub(crate) fn lookup_first_parent_with_props(&self, obj: &Obj) -> Option<Option<Obj>> {
        let inner = self.inner.lock().unwrap();
        let key = obj.to_rart_key();
        inner.first_parent_with_props_cache.get(key).cloned()
    }

    pub(crate) fn fill_first_parent_with_props(&self, obj: &Obj, parent: Option<Obj>) {
        let mut inner = self.inner.lock().unwrap();
        inner.version += 1;
        let key = obj.to_rart_key();
        inner.first_parent_with_props_cache.insert_k(&key, parent);
    }
}
