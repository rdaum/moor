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

//! Reverse lookup cache for sysobj references.
//! Maintains a mapping from objects to their property names on the system object (#0).

use ahash::AHasher;
use moor_var::{Obj, Symbol};
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::sync::{Arc, Mutex};

/// Inner cache data protected by mutex.
#[derive(Debug)]
struct Inner {
    cache: HashMap<Obj, Symbol, BuildHasherDefault<AHasher>>,
    populated: bool,
}

/// Cache for reverse sysobj lookups.
/// Maps objects that are values of properties on #0 back to their property names.
#[derive(Debug)]
pub struct SysobjReverseCache {
    inner: Mutex<Inner>,
    changed: Arc<std::sync::atomic::AtomicBool>,
}

impl Default for SysobjReverseCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SysobjReverseCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                cache: HashMap::with_hasher(BuildHasherDefault::<AHasher>::default()),
                populated: false,
            }),
            changed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Fork the cache for a new transaction.
    /// Creates a new cache that shares the same underlying data structure.
    pub fn fork(&self) -> Box<Self> {
        let inner = self.inner.lock().unwrap();
        Box::new(Self {
            inner: Mutex::new(Inner {
                cache: inner.cache.clone(),
                populated: inner.populated,
            }),
            changed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// Check if this cache instance has been modified.
    pub fn has_changed(&self) -> bool {
        self.changed.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Look up an object in the cache.
    /// Returns None if not found or if cache hasn't been populated yet.
    pub fn lookup(&self, obj: &Obj) -> Option<Symbol> {
        let inner = self.inner.lock().unwrap();
        if !inner.populated {
            return None;
        }

        inner.cache.get(obj).copied()
    }

    /// Populate the cache with a complete mapping.
    /// This is called when we scan #0's properties for the first time.
    pub fn populate(&self, mappings: Vec<(Obj, Symbol)>) {
        let mut inner = self.inner.lock().unwrap();
        inner.cache.clear();
        for (obj, symbol) in mappings {
            inner.cache.insert(obj, symbol);
        }
        inner.populated = true;
        self.changed
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Add or update a single entry in the cache.
    pub fn insert(&self, obj: Obj, symbol: Symbol) {
        let mut inner = self.inner.lock().unwrap();
        inner.cache.insert(obj, symbol);
        self.changed
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Remove an entry from the cache.
    pub fn remove(&self, obj: &Obj) {
        let mut inner = self.inner.lock().unwrap();
        inner.cache.remove(obj);
        self.changed
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Mark the cache as needing to be repopulated.
    /// This is called when properties on #0 are modified in ways we can't track incrementally.
    pub fn flush(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.cache.clear();
        inner.populated = false;
        self.changed
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Check if the cache has been populated.
    pub fn is_populated(&self) -> bool {
        self.inner.lock().unwrap().populated
    }
}
