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

use crate::Timestamp;
use ahash::AHasher;
use std::any::Any;
use std::hash::{BuildHasherDefault, Hash};

#[derive(Debug, Clone, PartialEq)]
pub struct Entry<T: Clone + PartialEq> {
    pub ts: Timestamp,
    pub value: T,
}

/// Trait for different indexing strategies for relation caches
pub trait RelationIndex<Domain, Codomain>: Send + Sync
where
    Domain: Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    fn insert_entry(&mut self, ts: Timestamp, domain: Domain, codomain: Codomain);
    fn insert_tombstone(&mut self, _ts: Timestamp, domain: Domain);
    fn index_lookup(&self, domain: &Domain) -> Option<&Entry<Codomain>>;
    fn remove_entry(&mut self, domain: &Domain) -> Option<Entry<Codomain>>;
    fn iter(&self) -> Box<dyn Iterator<Item = (&Domain, &Entry<Codomain>)> + '_>;
    fn len(&self) -> usize;
    fn fork(&self) -> Box<dyn RelationIndex<Domain, Codomain>>;
    fn as_any(&self) -> &dyn Any;
}

/// Hash-based implementation of RelationIndex using im::HashMap
#[derive(Clone)]
pub struct HashRelationIndex<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq,
{
    /// Internal index of the cache.
    pub entries: im::HashMap<Domain, Entry<Codomain>, BuildHasherDefault<AHasher>>,
}

impl<Domain, Codomain> HashRelationIndex<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            entries: Default::default(),
        }
    }
}

impl<Domain, Codomain> RelationIndex<Domain, Codomain> for HashRelationIndex<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    fn insert_entry(&mut self, ts: Timestamp, domain: Domain, codomain: Codomain) {
        self.entries.insert(
            domain.clone(),
            Entry {
                ts,
                value: codomain,
            },
        );
    }

    fn insert_tombstone(&mut self, _ts: Timestamp, domain: Domain) {
        self.entries.remove(&domain);
    }

    fn index_lookup(&self, domain: &Domain) -> Option<&Entry<Codomain>> {
        self.entries.get(domain)
    }

    fn remove_entry(&mut self, domain: &Domain) -> Option<Entry<Codomain>> {
        self.entries.remove(domain)
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (&Domain, &Entry<Codomain>)> + '_> {
        Box::new(self.entries.iter())
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn fork(&self) -> Box<dyn RelationIndex<Domain, Codomain>> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
