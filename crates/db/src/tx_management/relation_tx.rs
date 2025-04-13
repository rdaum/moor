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

use crate::tx_management::{Canonical, Error, Timestamp, Tx};
use ahash::AHasher;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::hash::{BuildHasherDefault, Hash};
use std::sync::Arc;

/// A key-value caching store that is scoped for the lifetime of a transaction.
/// When the transaction is completed, it collapses into a WorkingSet which can be applied to the
/// global transactional cache.
pub struct RelationTransaction<Domain, Codomain, Source>
where
    Source: Canonical<Domain, Codomain>,
    Domain: Hash + Eq,
    Codomain: Clone,
{
    tx: Tx,

    // Note: This is RefCell for interior mutability since even get/scan operations can modify the
    //   index.
    index: RefCell<IndexMap<Domain, Entry<Codomain>, BuildHasherDefault<AHasher>>>,

    backing_source: Arc<Source>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum OpType {
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
    pub(crate) size_bytes: usize,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) enum Entry<Datum: Clone> {
    NotPresent(Timestamp),
    Present(Op<Datum>),
}

pub type WorkingSet<Domain, Codomain> = Vec<(Domain, Op<Codomain>)>;

/// Represents the state of a relation in the context of a current transaction.
impl<Domain, Codomain, Source> RelationTransaction<Domain, Codomain, Source>
where
    Source: Canonical<Domain, Codomain>,
    Domain: Clone + Hash + Eq,
    Codomain: Clone,
{
    pub fn new(
        tx: Tx,
        backing_source: Arc<Source>,
    ) -> RelationTransaction<Domain, Codomain, Source> {
        RelationTransaction {
            tx,
            index: RefCell::new(IndexMap::default()),
            backing_source,
        }
    }

    /// Preseed the cache with a set of tuples.
    pub(crate) fn preseed(&mut self, tuples: &[(Timestamp, Domain, Codomain, usize)]) {
        let index = self.index.get_mut();
        for (ts, domain, value, size_bytes) in tuples {
            index.insert(
                domain.clone(),
                Entry::Present(Op {
                    read_ts: *ts,
                    write_ts: *ts,
                    source: DatumSource::Upstream,
                    from_type: OpType::Cached,
                    to_type: OpType::Cached,
                    value: Some(value.clone()),
                    size_bytes: *size_bytes,
                }),
            );
        }
    }

    pub fn insert(
        &mut self,
        domain: Domain,
        value: Codomain,
        size_bytes: usize,
    ) -> Result<(), Error> {
        let mut index = self.index.borrow_mut();

        // Check if the domain is already in the index.
        let old_entry = index.get_mut(&domain);

        if let Some(Entry::Present(old_entry)) = old_entry {
            // If the domain is already in the index and is Delete, we can just turn it back
            // into an Insert or an Update, with the new value.
            old_entry.to_type = match (old_entry.from_type, old_entry.to_type) {
                (OpType::Cached, OpType::Delete) => OpType::Update,
                (from, OpType::Delete) => from,
                _ => {
                    // If the domain is already in the index and is non-Delete, that's a dupe.
                    return Err(Error::Duplicate);
                }
            };
            old_entry.source = DatumSource::Local;
            old_entry.value = Some(value.clone());
            old_entry.size_bytes = size_bytes;
            return Ok(());
        }

        // Not in the index, we check the backing source.
        if let Some((read_ts, backing_value, size_bytes)) = self.backing_source.get(&domain)? {
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
                    size_bytes,
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
                size_bytes,
            }),
        );

        Ok(())
    }

    pub fn update(
        &mut self,
        domain: &Domain,
        value: Codomain,
        size_bytes: usize,
    ) -> Result<Option<Codomain>, Error> {
        let mut index = self.index.borrow_mut();

        // Check if the domain is already in the _local_ index.
        let old_entry = index.get_mut(domain);

        if let Some(Entry::Present(old_entry)) = old_entry {
            let old_value = old_entry.value.clone();

            // Update the "to_type" depending on what the existing to_type was.
            // If it was a "delete", that's an error, you can't update something deleted.
            // If it was an "insert", keep it as an insert, but with the new value.
            // If it was an "update", it stays the same, but with new value.
            // If it was "cached", it now becomes an update.
            old_entry.to_type = match old_entry.to_type {
                OpType::Cached | OpType::Update => OpType::Update,
                OpType::Delete => {
                    return Ok(None);
                }
                OpType::Insert => OpType::Insert,
            };
            old_entry.value = Some(value.clone());
            old_entry.write_ts = self.tx.ts;
            old_entry.size_bytes = size_bytes;
            return Ok(old_value);
        }

        // Not in the index, we check the backing source.
        let Some((read_ts, backing_value, _)) = self.backing_source.get(domain)? else {
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
                size_bytes,
            }),
        );

        Ok(Some(backing_value))
    }

    pub fn upsert(
        &mut self,
        domain: Domain,
        value: Codomain,
        size_bytes: usize,
    ) -> Result<Option<Codomain>, Error> {
        // TODO: We could probably more efficient about this, but there we bugs here before and this
        //   fixed them.
        if self.has_tuple(&domain)? {
            return self.update(&domain, value, size_bytes);
        }
        self.insert(domain, value, size_bytes)?;
        Ok(None)
    }

    pub fn has_tuple(&self, domain: &Domain) -> Result<bool, Error> {
        let mut index = self.index.borrow_mut();

        let entry = index.get(domain);

        if let Some(Entry::Present(entry)) = entry {
            if entry.to_type == OpType::Delete {
                return Ok(false);
            }
            return Ok(true);
        }

        let backing_value = self.backing_source.get(domain)?;
        let Some((read_ts, backing_value, size_bytes)) = backing_value else {
            return Ok(false);
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
                size_bytes,
            }),
        );

        Ok(true)
    }

    pub fn has_domain(&self, domain: &Domain) -> Result<bool, Error> {
        let mut index = self.index.borrow_mut();
        let entry = index.get(domain);

        if let Some(Entry::Present(entry)) = entry {
            if entry.to_type == OpType::Delete {
                return Ok(false);
            }
            return Ok(true);
        }

        let backing_value = self.backing_source.get(domain)?;
        let Some((read_ts, backing_value, size_bytes)) = backing_value else {
            index.insert(domain.clone(), Entry::NotPresent(self.tx.ts));
            return Ok(false);
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
                size_bytes,
            }),
        );

        Ok(true)
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
        let Some((read_ts, backing_value, size_bytes)) = backing_value else {
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
                size_bytes,
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
                match (op.source, op.to_type) {
                    (DatumSource::Local, _) => {
                        // Just remove it by marking it not-present
                        (Entry::NotPresent(self.tx.ts), op.value.clone())
                    }
                    (DatumSource::Upstream, OpType::Delete) => {
                        return Ok(None);
                    }
                    (DatumSource::Upstream, _) => {
                        // Create a Delete entry, unless we have one already?
                        (
                            Entry::Present(Op {
                                read_ts: op.read_ts,
                                write_ts: self.tx.ts,
                                source: DatumSource::Upstream,
                                from_type: op.from_type,
                                to_type: OpType::Delete,
                                value: None,
                                size_bytes: 0,
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
                    Some((read_ts, value, _)) => {
                        let new_entry = Entry::Present(Op {
                            read_ts,
                            write_ts: self.tx.ts,
                            source: DatumSource::Upstream,
                            from_type: OpType::Cached,
                            to_type: OpType::Delete,
                            value: None,
                            size_bytes: 0,
                        });
                        (new_entry, Some(value))
                    }
                }
            }
            None => match self.backing_source.get(domain)? {
                None => (Entry::NotPresent(self.tx.ts), None),
                Some((read_ts, value, _)) => {
                    let new_entry = Entry::Present(Op {
                        read_ts,
                        write_ts: self.tx.ts,
                        source: DatumSource::Upstream,
                        from_type: OpType::Cached,
                        to_type: OpType::Delete,
                        value: None,
                        size_bytes: 0,
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
        for (ts, d, c, size_bytes) in upstream {
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
                            size_bytes,
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
                    if op.to_type == OpType::Cached {
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
        fn get(&self, domain: &u64) -> Result<Option<(Timestamp, u64, usize)>, Error> {
            let store = self.store.lock().unwrap();
            Ok(store.get(domain).cloned().map(|v| (Timestamp(0), v, 16)))
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
        fn apply(&self, working_set: WorkingSet<u64, u64>) {
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
        let mut cache = RelationTransaction::new(tx, backing_store.clone());

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
                size_bytes: 16,
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
                size_bytes: 0,
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
                size_bytes: 0,
                value: None,
            })
        );

        // Insert a new local value is ok because it's been locally tombstoned.
        let result = cache.insert(1, 456, 16);
        assert_eq!(result, Ok(()));
        assert_eq!(
            cache.retrieve_index_entry(&1),
            Entry::Present(Op {
                read_ts: Timestamp(0),
                write_ts: Timestamp(1),
                source: DatumSource::Local,
                from_type: OpType::Cached,
                to_type: OpType::Update,
                size_bytes: 16,
                value: Some(456),
            })
        );

        // But inserting over a present value is a dupe.
        let result = cache.insert(2, 1, 16);
        assert_eq!(result, Err(Error::Duplicate));

        // Updating should work though.
        let old_value = cache.update(&2, 3, 16).unwrap();
        assert_eq!(old_value, Some(2));
        assert_eq!(
            cache.retrieve_index_entry(&2),
            Entry::Present(Op {
                read_ts: Timestamp(0),
                write_ts: Timestamp(1),
                source: DatumSource::Upstream,
                from_type: OpType::Cached,
                to_type: OpType::Update,
                size_bytes: 16,
                value: Some(3),
            })
        );

        // Updating a non-present value should not work.
        let old_value = cache.update(&4, 4, 16).unwrap();
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
        let result = cache.insert(6, 6, 16);
        assert_eq!(result, Ok(()));
        assert_eq!(
            cache.retrieve_index_entry(&6),
            Entry::Present(Op {
                read_ts: Timestamp(1),
                write_ts: Timestamp(1),
                source: DatumSource::Local,
                from_type: OpType::Insert,
                to_type: OpType::Insert,
                size_bytes: 16,
                value: Some(6),
            })
        );

        // Upsert should work for new common and old...

        // Not present local or upstream.
        let old_value = cache.upsert(7, 7, 16).unwrap();
        assert_eq!(old_value, None);
        assert_eq!(
            cache.retrieve_index_entry(&7),
            Entry::Present(Op {
                read_ts: Timestamp(1),
                write_ts: Timestamp(1),
                source: DatumSource::Local,
                from_type: OpType::Insert,
                to_type: OpType::Insert,
                size_bytes: 16,
                value: Some(7),
            })
        );

        // A value that is present local...
        let old_value = cache.upsert(6, 8, 16).unwrap();
        assert_eq!(old_value, Some(6));
        assert_eq!(
            cache.retrieve_index_entry(&6),
            Entry::Present(Op {
                read_ts: Timestamp(1),
                write_ts: Timestamp(1),
                source: DatumSource::Local,
                from_type: OpType::Insert,
                to_type: OpType::Insert,
                size_bytes: 16,
                value: Some(8),
            })
        );

        // A value that is present upstream but not yet seen locally.
        let old_value = cache.upsert(9, 10, 16).unwrap();
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
                size_bytes: 16,
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
