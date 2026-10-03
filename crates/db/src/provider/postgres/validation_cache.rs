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

//! Bounded measurements of validated list versions. No world values are retained.
//! Transaction timestamps are unique within a writer lifetime, so an aborted preparation
//! cannot supply a measurement for another transaction's committed base.
use crate::{ObjAndUUIDHolder, Timestamp};
use moor_var::{ByteSized, Var};
use parking_lot::Mutex;
use std::collections::VecDeque;

const CAPACITY: usize = 4096;
type Key = (ObjAndUUIDHolder, u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ListSize {
    pub json_bytes: usize,
    pub logical_bytes: usize,
    nonempty: bool,
}
impl ListSize {
    /// The caller must have validated this literal under the writer's fixed profile.
    pub fn measured(value: &Var, literal: &str) -> Self {
        Self {
            json_bytes: super::encode::json_string_bytes(literal),
            logical_bytes: value.size_bytes(),
            nonempty: !value.as_list().expect("validated list").is_empty(),
        }
    }

    pub fn append(self, suffix: Self) -> Self {
        // Replace the two inner braces with ", ", then remove one JSON string's
        // pair of quotes. Empty lists contribute neither elements nor separators.
        let json_bytes = if !self.nonempty {
            suffix.json_bytes
        } else if !suffix.nonempty {
            self.json_bytes
        } else {
            self.json_bytes + suffix.json_bytes - 2
        };
        Self {
            json_bytes,
            logical_bytes: self.logical_bytes + suffix.logical_bytes,
            nonempty: self.nonempty || suffix.nonempty,
        }
    }
}

#[derive(Default)]
pub(super) struct ValidationCache(Mutex<State>);
#[derive(Default)]
struct State {
    values: ahash::AHashMap<Key, ListSize>,
    order: VecDeque<Key>,
    hits: u64,
    misses: u64,
}
impl ValidationCache {
    pub fn get(&self, property: &ObjAndUUIDHolder, version: Option<Timestamp>) -> Option<ListSize> {
        let mut state = self.0.lock();
        let value =
            version.and_then(|version| state.values.get(&(property.clone(), version.0)).copied());
        if value.is_some() {
            state.hits += 1;
        } else {
            state.misses += 1;
        }
        value
    }

    pub fn insert(&self, property: &ObjAndUUIDHolder, version: Timestamp, size: ListSize) {
        let mut state = self.0.lock();
        let key = (property.clone(), version.0);
        if state.values.insert(key.clone(), size).is_some() {
            return;
        }
        state.order.push_back(key);
        if state.order.len() > CAPACITY {
            let oldest = state.order.pop_front().unwrap();
            state.values.remove(&oldest);
        }
    }

    pub fn stats(&self) -> (u64, u64) {
        let state = self.0.lock();
        (state.hits, state.misses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_and_eviction_cannot_reuse_another_base_measurement() {
        let cache = ValidationCache::default();
        let key = ObjAndUUIDHolder::new(&moor_var::Obj::mk_id(1), uuid::Uuid::nil());
        let small = ListSize::measured(&moor_var::v_list(&[moor_var::v_int(1)]), "{1}");
        let large = ListSize::measured(&moor_var::v_list(&[moor_var::v_int(1000)]), "{1000}");
        cache.insert(&key, Timestamp(2), large);
        cache.insert(&key, Timestamp(1), small); // An older preparation may finish later.
        assert_eq!(cache.get(&key, Some(Timestamp(2))), Some(large));
        assert_eq!(cache.get(&key, Some(Timestamp(1))), Some(small));
        assert_eq!(cache.get(&key, None), None);
        assert_eq!(cache.get(&key, Some(Timestamp(3))), None);
        for version in 3..=CAPACITY as u64 + 2 {
            cache.insert(&key, Timestamp(version), small);
        }
        assert_eq!(cache.get(&key, Some(Timestamp(2))), None);
        assert_eq!(cache.get(&key, Some(Timestamp(1))), None);
        assert_eq!(cache.0.lock().values.len(), CAPACITY);
        assert_eq!(cache.0.lock().order.len(), CAPACITY);
    }
}
