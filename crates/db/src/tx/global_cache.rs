// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! Global cache is a cache that acts as an origin for all local caches.

use crate::tx::tx_table::{Canonical, OpType, TransactionalTable, WorkingSet};
use crate::tx::{Error, Timestamp, Tx};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{Arc, Mutex, MutexGuard};

pub trait Provider<Domain, Codomain> {
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error>;
    fn put(&self, timestamp: Timestamp, domain: Domain, codomain: Codomain) -> Result<(), Error>;
    fn del(&self, timestamp: Timestamp, domain: &Domain) -> Result<(), Error>;

    /// Scan the database for all keys match the given predicate
    fn scan<F>(&self, predicate: &F) -> Result<Vec<(Timestamp, Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool;
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Datum<T: Clone + PartialEq> {
    Entry(T),
    Tombstone,
}

#[derive(Debug, Clone, PartialEq)]
struct Entry<T: Clone + PartialEq> {
    ts: Timestamp,
    datum: Datum<T>,
}

pub struct GlobalCache<Domain, Codomain, Source>
where
    Source: Provider<Domain, Codomain>,
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
{
    /// A series of common that local caches should be pre-seeded with.
    preseed: HashSet<Domain>,

    index: Mutex<Inner<Domain, Codomain>>,

    source: Arc<Source>,
}

impl<Domain, Codomain, MyProvider> GlobalCache<Domain, Codomain, MyProvider>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
    MyProvider: Provider<Domain, Codomain>,
{
    pub(crate) fn new(provider: Arc<MyProvider>) -> Self {
        Self {
            preseed: HashSet::new(),
            index: Mutex::new(Inner {
                index: HashMap::new(),
            }),
            source: provider,
        }
    }
}

pub struct Inner<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
{
    index: HashMap<Domain, Entry<Codomain>>,
}

impl<Domain, Codomain, Source> GlobalCache<Domain, Codomain, Source>
where
    Source: Provider<Domain, Codomain>,
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
{
    pub fn start(self: Arc<Self>, tx: &Tx) -> TransactionalTable<Domain, Codomain, Self> {
        let mut lc = TransactionalTable::new(*tx, self.clone());
        let lock = self.lock();
        let mut preseed_tuples = vec![];
        for d in &self.preseed {
            if let Some(e) = lock.index.get(d) {
                match &e.datum {
                    Datum::Entry(c) => {
                        preseed_tuples.push((e.ts, d.clone(), c.clone()));
                    }
                    Datum::Tombstone => continue,
                }
            }
        }
        lc.preseed(&preseed_tuples);
        lc
    }

    pub fn check<'a>(
        &self,
        mut inner: MutexGuard<'a, Inner<Domain, Codomain>>,
        working_set: &WorkingSet<Domain, Codomain>,
    ) -> Result<MutexGuard<'a, Inner<Domain, Codomain>>, Error> {
        // Check phase first.
        for (domain, op) in working_set {
            // Check local to see if we have one first, to see if there's a conflict.
            if let Some(local_entry) = inner.index.get(domain) {
                let ts = local_entry.ts;
                // If the ts there is greater than the read-ts in the update_op, that's a conflict
                // Someone got to it first.
                if ts > op.read_ts {
                    return Err(Error::Conflict);
                }
                continue;
            }

            // Otherwise, pull from upstream and fetch to cache and check for conflict.
            if let Some((ts, codomain)) = self.source.get(domain)? {
                inner.index.insert(
                    domain.clone(),
                    Entry {
                        ts,
                        datum: Datum::Entry(codomain),
                    },
                );
                if ts > op.read_ts {
                    return Err(Error::Conflict);
                }
            } else {
                // If upstream doesn't have it, and it's not an insert, that's a conflict, this
                // should not have happened.
                if op.to_type != OpType::Insert {
                    return Err(Error::Conflict);
                }
            }
        }
        Ok(inner)
    }

    pub fn lock(&self) -> MutexGuard<Inner<Domain, Codomain>> {
        self.index.lock().unwrap()
    }

    pub fn apply<'a>(
        &self,
        mut inner: MutexGuard<'a, Inner<Domain, Codomain>>,
        working_set: WorkingSet<Domain, Codomain>,
    ) -> Result<MutexGuard<'a, Inner<Domain, Codomain>>, Error> {
        // Apply phase.
        for (domain, op) in working_set {
            match op.to_type {
                OpType::Insert | OpType::Update => {
                    let codomain = op.value.unwrap();
                    inner.index.insert(
                        domain.clone(),
                        Entry {
                            ts: op.write_ts,
                            datum: Datum::Entry(codomain.clone()),
                        },
                    );

                    self.source.put(op.write_ts, domain.clone(), codomain).ok();
                }
                OpType::Delete => {
                    inner.index.insert(
                        domain.clone(),
                        Entry {
                            ts: op.write_ts,
                            datum: Datum::Tombstone,
                        },
                    );

                    self.source.del(op.write_ts, &domain).unwrap();
                }
                _ => continue,
            }
        }
        Ok(inner)
    }
}

impl<Domain, Codomain, Source> Canonical<Domain, Codomain> for GlobalCache<Domain, Codomain, Source>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
    Source: Provider<Domain, Codomain>,
{
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error> {
        let mut index = self.index.lock().unwrap();
        if let Some(entry) = index.index.get(domain) {
            match &entry.datum {
                Datum::Entry(codomain) => Ok(Some((entry.ts, codomain.clone()))),
                Datum::Tombstone => Ok(None),
            }
        } else {
            // Pull from backing store.
            if let Some((ts, codomain)) = self.source.get(domain)? {
                index.index.insert(
                    domain.clone(),
                    Entry {
                        ts,
                        datum: Datum::Entry(codomain.clone()),
                    },
                );
                Ok(Some((ts, codomain)))
            } else {
                Ok(None)
            }
        }
    }

    fn scan<F>(&self, predicate: &F) -> Result<Vec<(Timestamp, Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        let results = self.source.scan(&predicate)?;
        for (ts, domain, codomain) in &results {
            let mut index = self.index.lock().unwrap();
            index.index.insert(
                domain.clone(),
                Entry {
                    ts: *ts,
                    datum: Datum::Entry(codomain.clone()),
                },
            );
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::tx::Tx;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestDomain(u64);

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestCodomain(u64);

    struct TestProvider {
        data: Arc<Mutex<HashMap<TestDomain, TestCodomain>>>,
    }

    impl Provider<TestDomain, TestCodomain> for TestProvider {
        fn get(&self, domain: &TestDomain) -> Result<Option<(Timestamp, TestCodomain)>, Error> {
            let data = self.data.lock().unwrap();
            if let Some(codomain) = data.get(domain) {
                Ok(Some((Timestamp(0), codomain.clone())))
            } else {
                Ok(None)
            }
        }

        fn put(
            &self,
            _timestamp: Timestamp,
            domain: TestDomain,
            codomain: TestCodomain,
        ) -> Result<(), Error> {
            let mut data = self.data.lock().unwrap();
            data.insert(domain, codomain);
            Ok(())
        }

        fn del(&self, _timestamp: Timestamp, domain: &TestDomain) -> Result<(), Error> {
            let mut data = self.data.lock().unwrap();
            data.remove(domain);
            Ok(())
        }

        fn scan<F>(
            &self,
            predicate: &F,
        ) -> Result<Vec<(Timestamp, TestDomain, TestCodomain)>, Error>
        where
            F: Fn(&TestDomain, &TestCodomain) -> bool,
        {
            let data = self.data.lock().unwrap();
            Ok(data
                .iter()
                .filter(|(k, v)| predicate(k, v))
                .map(|(k, v)| (Timestamp(0), k.clone(), v.clone()))
                .collect())
        }
    }

    #[test]
    fn test_basic() {
        let mut backing = HashMap::new();
        backing.insert(TestDomain(0), TestCodomain(0));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let global_cache = Arc::new(GlobalCache {
            preseed: HashSet::new(),
            index: Mutex::new(Inner {
                index: HashMap::new(),
            }),
            source: provider,
        });

        let domain = TestDomain(1);
        let codomain = TestCodomain(1);

        let tx = Tx { ts: Timestamp(0) };
        let lc = global_cache.clone().start(&tx);
        lc.insert(domain.clone(), codomain.clone()).unwrap();
        assert_eq!(lc.get(&domain).unwrap(), Some(codomain.clone()));
        assert_eq!(lc.get(&TestDomain(0)).unwrap(), Some(TestCodomain(0)));
        let ws = lc.working_set();

        let lock = global_cache.lock();
        let lock = global_cache.check(lock, &ws).unwrap();
        let lock = global_cache.apply(lock, ws).unwrap();
        assert_eq!(
            lock.index.get(&domain).unwrap().datum,
            Datum::Entry(codomain.clone())
        );
    }
}
