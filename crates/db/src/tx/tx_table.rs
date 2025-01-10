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

use crate::tx::{Canonical, Error, Timestamp, Tx};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::hash::Hash;
use std::sync::Arc;

/// A key-value caching store that is scoped for the lifetime of a transaction.
/// When the transaction is completed, it collapses into a WorkingSet which can be applied to the
/// global transactional cache.
pub struct TransactionalTable<Domain, Codomain, Source>
where
    Source: Canonical<Domain, Codomain>,
    Domain: Hash + Eq,
    Codomain: Clone,
{
    tx: Tx,

    // Note: This is RefCell for interior mutability since even get/scan operations can modify the
    //   index.
    index: RefCell<IndexMap<Domain, Entry<Codomain>>>,

    backing_source: Arc<Source>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum OpType {
    None,
    Cached,
    Insert,
    Update,
    Delete,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum DatumSource {
    Upstream,
    Local,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Op<Datum: Clone> {
    pub(crate) read_ts: Timestamp,
    pub(crate) write_ts: Timestamp,
    pub(crate) source: DatumSource,
    pub(crate) from_type: OpType,
    pub(crate) to_type: OpType,
    pub(crate) value: Option<Datum>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) enum Entry<Datum: Clone> {
    NotPresent(Timestamp),
    Present(Op<Datum>),
}

pub type WorkingSet<Domain, Codomain> = Vec<(Domain, Op<Codomain>)>;

impl<Domain, Codomain, Source> TransactionalTable<Domain, Codomain, Source>
where
    Source: Canonical<Domain, Codomain>,
    Domain: Clone + Hash + Eq,
    Codomain: Clone,
{
    pub fn new(
        tx: Tx,
        backing_source: Arc<Source>,
    ) -> TransactionalTable<Domain, Codomain, Source> {
        TransactionalTable {
            tx,
            index: RefCell::new(IndexMap::new()),
            backing_source,
        }
    }

    /// Preseed the cache with a set of tuples.
    pub(crate) fn preseed(&mut self, tuples: &[(Timestamp, Domain, Codomain)]) {
        let index = self.index.get_mut();
        for (ts, domain, value) in tuples {
            index.insert(
                domain.clone(),
                Entry::Present(Op {
                    read_ts: *ts,
                    write_ts: *ts,
                    source: DatumSource::Upstream,
                    from_type: OpType::Cached,
                    to_type: OpType::Cached,
                    value: Some(value.clone()),
                }),
            );
        }
    }

    pub fn insert(&mut self, domain: Domain, value: Codomain) -> Result<(), Error> {
        let mut index = self.index.borrow_mut();

        // Check if the domain is already in the index.
        let old_entry = index.get_mut(&domain);

        // If the domain is already in the index and is non-Delete, that's a dupe.
        if let Some(Entry::Present(old_entry)) = old_entry {
            // If the domain is already in the index and is Delete, we can just turn it back
            // into an Insert or an Update, with the new value.
            old_entry.to_type = match (old_entry.from_type, old_entry.to_type) {
                (OpType::Cached, OpType::Delete) => OpType::Update,
                (from, OpType::Delete) => from,
                _ => {
                    return Err(Error::Duplicate);
                }
            };
            old_entry.source = DatumSource::Local;
            old_entry.value = Some(value.clone());

            return Ok(());
        }

        // Not in the index, we check the backing source.
        if let Some((read_ts, backing_value)) = self.backing_source.get(&domain)? {
            // If the backing source has a value, we can't insert.
            // But let's cache this value in our local.
            index.insert(
                domain.clone(),
                Entry::Present(Op {
                    read_ts,
                    write_ts: self.tx.ts,
                    source: DatumSource::Upstream,
                    from_type: OpType::Cached,
                    to_type: OpType::Cached,
                    value: Some(backing_value.clone()),
                }),
            );
            return Err(Error::Duplicate);
        }

        // Not in the index, not in the backing source, we can insert freely.
        index.insert(
            domain.clone(),
            Entry::Present(Op {
                read_ts: self.tx.ts,
                write_ts: self.tx.ts,
                source: DatumSource::Local,
                from_type: OpType::Insert,
                to_type: OpType::Insert,
                value: Some(value),
            }),
        );

        Ok(())
    }

    pub fn update(&mut self, domain: &Domain, value: Codomain) -> Result<Option<Codomain>, Error> {
        let mut index = self.index.borrow_mut();

        // Check if the domain is already in the index.
        let old_entry = index.get_mut(domain);

        if let Some(Entry::Present(old_entry)) = old_entry {
            let old_value = old_entry.value.clone();

            old_entry.to_type = match old_entry.to_type {
                OpType::Cached => OpType::Update,
                OpType::Delete => {
                    return Ok(None);
                }
                _ => old_entry.from_type,
            };
            old_entry.value = Some(value.clone());

            return Ok(old_value);
        }

        // Not in the index, we check the backing source.
        let Some((read_ts, backing_value)) = self.backing_source.get(domain)? else {
            index.insert(domain.clone(), Entry::NotPresent(self.tx.ts));
            return Ok(None);
        };

        // Copy into the local cache, but with updated value.
        index.insert(
            domain.clone(),
            Entry::Present(Op {
                read_ts,
                write_ts: self.tx.ts,
                source: DatumSource::Upstream,
                from_type: OpType::Cached,
                to_type: OpType::Update,
                value: Some(value.clone()),
            }),
        );

        Ok(Some(backing_value))
    }

    pub fn upsert(&mut self, domain: Domain, value: Codomain) -> Result<Option<Codomain>, Error> {
        let mut index = self.index.borrow_mut();

        // Check if the domain is already in the index.
        let old_entry = index.get_mut(&domain);

        if let Some(Entry::Present(old_entry)) = old_entry {
            let old_value = old_entry.value.clone();

            old_entry.to_type = match old_entry.to_type {
                OpType::Cached => OpType::Update,
                OpType::Delete | OpType::Insert => OpType::Insert,
                OpType::Update => OpType::Update,
                _ => old_entry.from_type,
            };
            old_entry.value = Some(value.clone());

            return Ok(old_value);
        }
        // Not in the index, we check the backing source.

        if let Some((read_ts, backing_value)) = self.backing_source.get(&domain)? {
            // Already present, this becomes an update
            index.insert(
                domain.clone(),
                Entry::Present(Op {
                    read_ts,
                    write_ts: self.tx.ts,
                    source: DatumSource::Upstream,
                    from_type: OpType::Cached,
                    to_type: OpType::Update,
                    value: Some(value.clone()),
                }),
            );

            return Ok(Some(backing_value.clone()));
        }

        // It's not upstream, create an insert
        index.insert(
            domain.clone(),
            Entry::Present(Op {
                read_ts: self.tx.ts,
                write_ts: self.tx.ts,
                source: DatumSource::Local,
                from_type: OpType::None,
                to_type: OpType::Insert,
                value: Some(value.clone()),
            }),
        );

        Ok(None)
    }

    pub fn get(&self, domain: &Domain) -> Result<Option<Codomain>, Error> {
        let mut index = self.index.borrow_mut();

        let entry = index.get(domain);

        if let Some(Entry::Present(entry)) = entry {
            if entry.to_type == OpType::Delete {
                return Ok(None);
            }
            return Ok(entry.value.clone());
        }

        let backing_value = self.backing_source.get(domain)?;
        let Some((read_ts, backing_value)) = backing_value else {
            index.insert(domain.clone(), Entry::NotPresent(self.tx.ts));
            return Ok(None);
        };

        index.insert(
            domain.clone(),
            Entry::Present(Op {
                read_ts,
                write_ts: self.tx.ts,
                source: DatumSource::Upstream,
                from_type: OpType::Cached,
                to_type: OpType::Cached,
                value: Some(backing_value.clone()),
            }),
        );

        Ok(Some(backing_value))
    }

    pub fn delete(&mut self, domain: &Domain) -> Result<Option<Codomain>, Error> {
        let mut index = self.index.borrow_mut();

        // Check local first to see if we have an entry. If we do, check its source.  If it's
        // upstream, we turn it into delete. If it's local, we can just remove it.
        let entry = index.get_mut(domain);

        let (new_entry, old_value) = match entry {
            Some(Entry::Present(op)) => {
                match op.source {
                    DatumSource::Local => {
                        // Just remove it by marking it not-present
                        (Entry::NotPresent(self.tx.ts), op.value.clone())
                    }
                    DatumSource::Upstream => {
                        // Create a Delete entry
                        (
                            Entry::Present(Op {
                                read_ts: op.read_ts,
                                write_ts: self.tx.ts,
                                source: DatumSource::Upstream,
                                from_type: op.from_type,
                                to_type: OpType::Delete,
                                value: None,
                            }),
                            op.value.clone(),
                        )
                    }
                }
            }
            Some(Entry::NotPresent(read_ts)) => {
                // Fill cache from upstream...
                match self.backing_source.get(domain)? {
                    None => (Entry::NotPresent(*read_ts), None),
                    Some((read_ts, value)) => {
                        let new_entry = Entry::Present(Op {
                            read_ts,
                            write_ts: self.tx.ts,
                            source: DatumSource::Upstream,
                            from_type: OpType::Cached,
                            to_type: OpType::Delete,
                            value: None,
                        });
                        (new_entry, Some(value))
                    }
                }
            }
            None => match self.backing_source.get(domain)? {
                None => (Entry::NotPresent(self.tx.ts), None),
                Some((read_ts, value)) => {
                    let new_entry = Entry::Present(Op {
                        read_ts,
                        write_ts: self.tx.ts,
                        source: DatumSource::Upstream,
                        from_type: OpType::Cached,
                        to_type: OpType::Delete,
                        value: None,
                    });
                    (new_entry, Some(value))
                }
            },
        };

        index.insert(domain.clone(), new_entry);
        Ok(old_value)
    }

    pub fn scan<F>(&self, predicate: &F) -> Result<Vec<(Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        // Scan in the upstream first, and then merge the set with local changes.
        let upstream = self.backing_source.scan(predicate)?;

        let mut index = self.index.borrow_mut();

        // This is basically like going a `get` on each entry, we're filling our cache with
        // all the upstream common.
        for (ts, d, c, _) in upstream {
            let entry = index.get_mut(&d);
            match entry {
                Some(_) => continue,
                None => {
                    index.insert(
                        d.clone(),
                        Entry::Present(Op {
                            read_ts: ts,
                            write_ts: ts,
                            source: DatumSource::Upstream,
                            from_type: OpType::Cached,
                            to_type: OpType::Cached,
                            value: Some(c),
                        }),
                    );
                }
            }
        }

        let mut results = Vec::new();

        // Now scan local
        for (domain, entry) in index.iter() {
            if let Entry::Present(op) = entry {
                if predicate(domain, op.value.as_ref().unwrap()) {
                    results.push((domain.clone(), op.value.as_ref().unwrap().clone()));
                }
            }
        }
        Ok(results)
    }
    pub fn working_set(self) -> WorkingSet<Domain, Codomain> {
        let index = self.index.take();
        index
            .into_iter()
            .filter_map(|(domain, entry)| match entry {
                Entry::NotPresent(_) => None,
                Entry::Present(op) => {
                    if op.to_type == OpType::Cached || op.to_type == OpType::None {
                        return None;
                    }
                    Some((domain, op))
                }
            })
            .collect()
    }

    // Test-only function to retrieve an index entry for inspection.
    #[cfg(test)]
    fn retrieve_index_entry(&self, domain: &Domain) -> Entry<Codomain> {
        let index = self.index.borrow();
        index[domain].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    struct TestBackingStore {
        store: Arc<Mutex<HashMap<u64, u64>>>,
    }

    impl TestBackingStore {
        fn new(values: &[(u64, u64)]) -> Self {
            let mut store = HashMap::new();
            for (k, v) in values {
                store.insert(*k, *v);
            }
            TestBackingStore {
                store: Arc::new(Mutex::new(store)),
            }
        }
    }
    impl Canonical<u64, u64> for TestBackingStore {
        fn get(&self, domain: &u64) -> Result<Option<(Timestamp, u64)>, Error> {
            let store = self.store.lock().unwrap();
            Ok(store.get(domain).cloned().map(|v| (Timestamp(0), v)))
        }

        fn scan<F: Fn(&u64, &u64) -> bool>(
            &self,
            predicate: &F,
        ) -> Result<Vec<(Timestamp, u64, u64, usize)>, Error> {
            let store = self.store.lock().unwrap();
            Ok(store
                .iter()
                .filter(|(k, v)| predicate(k, v))
                .map(|(k, v)| (Timestamp(0), *k, *v, 16))
                .collect())
        }
    }

    impl TestBackingStore {
        fn apply(&self, working_set: Vec<(u64, Op<u64>)>) {
            let mut store = self.store.lock().unwrap();
            for op in working_set {
                match op.1.to_type {
                    OpType::Insert => {
                        store.insert(op.0, op.1.value.unwrap());
                    }
                    OpType::Update => {
                        store.insert(op.0, op.1.value.unwrap());
                    }
                    OpType::Delete => {
                        store.remove(&op.0);
                    }
                    _ => {
                        panic!("Unexpected OpType in working set: {:?}", op.1.to_type);
                    }
                }
            }
        }
    }

    #[test]
    fn it_works() {
        let backing_store = TestBackingStore::new(&[(1, 1), (2, 2), (3, 3), (9, 9)]);

        let backing_store = Arc::new(backing_store);
        let tx = Tx { ts: Timestamp(1) };
        let mut cache = TransactionalTable::new(tx, backing_store.clone());

        let result = cache.get(&1).unwrap();
        assert_eq!(result, Some(1));
        assert_eq!(
            cache.retrieve_index_entry(&1),
            Entry::Present(Op {
                read_ts: Timestamp(0),
                write_ts: Timestamp(1),
                source: DatumSource::Upstream,
                from_type: OpType::Cached,
                to_type: OpType::Cached,
                value: Some(1),
            })
        );

        let removed = cache.delete(&1).unwrap();
        assert_eq!(removed, Some(1));
        assert_eq!(
            cache.retrieve_index_entry(&1),
            Entry::Present(Op {
                read_ts: Timestamp(0),
                write_ts: Timestamp(1),
                source: DatumSource::Upstream,
                from_type: OpType::Cached,
                to_type: OpType::Delete,
                value: None,
            })
        );

        let verified_gone = cache.get(&1).unwrap();
        assert_eq!(verified_gone, None);
        assert_eq!(
            cache.retrieve_index_entry(&1),
            Entry::Present(Op {
                read_ts: Timestamp(0),
                write_ts: Timestamp(1),
                source: DatumSource::Upstream,
                from_type: OpType::Cached,
                to_type: OpType::Delete,
                value: None,
            })
        );

        // Insert a new local value is ok because it's been locally tombstoned.
        let result = cache.insert(1, 456);
        assert_eq!(result, Ok(()));
        assert_eq!(
            cache.retrieve_index_entry(&1),
            Entry::Present(Op {
                read_ts: Timestamp(0),
                write_ts: Timestamp(1),
                source: DatumSource::Local,
                from_type: OpType::Cached,
                to_type: OpType::Update,
                value: Some(456),
            })
        );

        // But inserting over a present value is a dupe.
        let result = cache.insert(2, 1);
        assert_eq!(result, Err(Error::Duplicate));

        // Updating should work though.
        let old_value = cache.update(&2, 3).unwrap();
        assert_eq!(old_value, Some(2));
        assert_eq!(
            cache.retrieve_index_entry(&2),
            Entry::Present(Op {
                read_ts: Timestamp(0),
                write_ts: Timestamp(1),
                source: DatumSource::Upstream,
                from_type: OpType::Cached,
                to_type: OpType::Update,
                value: Some(3),
            })
        );

        // Updating a non-present value should not work.
        let old_value = cache.update(&4, 4).unwrap();
        assert_eq!(old_value, None);
        assert_eq!(
            cache.retrieve_index_entry(&4),
            Entry::NotPresent(Timestamp(1))
        );

        // Likewise removing one.
        let removed = cache.delete(&5).unwrap();
        assert_eq!(removed, None);
        assert_eq!(
            cache.retrieve_index_entry(&5),
            Entry::NotPresent(Timestamp(1))
        );

        // Inserting brand new common...
        let result = cache.insert(6, 6);
        assert_eq!(result, Ok(()));
        assert_eq!(
            cache.retrieve_index_entry(&6),
            Entry::Present(Op {
                read_ts: Timestamp(1),
                write_ts: Timestamp(1),
                source: DatumSource::Local,
                from_type: OpType::Insert,
                to_type: OpType::Insert,
                value: Some(6),
            })
        );

        // Upsert should work for new common and old...

        // Not present local or upstream.
        let old_value = cache.upsert(7, 7).unwrap();
        assert_eq!(old_value, None);
        assert_eq!(
            cache.retrieve_index_entry(&7),
            Entry::Present(Op {
                read_ts: Timestamp(1),
                write_ts: Timestamp(1),
                source: DatumSource::Local,
                from_type: OpType::None,
                to_type: OpType::Insert,
                value: Some(7),
            })
        );

        // A value that is present local...
        let old_value = cache.upsert(6, 8).unwrap();
        assert_eq!(old_value, Some(6));
        assert_eq!(
            cache.retrieve_index_entry(&6),
            Entry::Present(Op {
                read_ts: Timestamp(1),
                write_ts: Timestamp(1),
                source: DatumSource::Local,
                from_type: OpType::Insert,
                to_type: OpType::Insert,
                value: Some(8),
            })
        );

        // A value that is present upstream but not yet seen locally.
        let old_value = cache.upsert(9, 10).unwrap();
        assert_eq!(old_value, Some(9));
        assert_eq!(
            // cache.index[&9],
            cache.retrieve_index_entry(&9),
            Entry::Present(Op {
                read_ts: Timestamp(0),
                write_ts: Timestamp(1),
                source: DatumSource::Upstream,
                from_type: OpType::Cached,
                to_type: OpType::Update,
                value: Some(10),
            })
        );

        // Now apply the working set
        let ws = cache.working_set();
        backing_store.apply(ws);

        // And verify the contents of the backing store
        let store = backing_store.store.lock().unwrap();
        assert_eq!(store.get(&1), Some(&456));
        assert_eq!(store.get(&2), Some(&3));
        assert_eq!(store.get(&3), Some(&3));
        assert_eq!(store.get(&4), None);
        assert_eq!(store.get(&5), None);
        assert_eq!(store.get(&6), Some(&8));
        assert_eq!(store.get(&9), Some(&10));
    }
}
