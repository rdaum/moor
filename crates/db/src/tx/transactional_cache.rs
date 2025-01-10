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

//! Global cache is a cache that acts as an origin for all local caches.

use crate::tx::tx_table::{OpType, TransactionalTable, WorkingSet};
use crate::tx::{Canonical, Error, Provider, SizedCache, Timestamp, Tx};
use indexmap::IndexMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Datum<T: Clone + PartialEq> {
    Value(T),
    Tombstone,
}

#[derive(Debug, Clone, PartialEq)]
struct Entry<T: Clone + PartialEq> {
    ts: Timestamp,
    hits: usize,
    datum: Datum<T>,
    size_bytes: usize,
}

pub struct TransactionalCache<Domain, Codomain, Source>
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

impl<Domain, Codomain, Source> TransactionalCache<Domain, Codomain, Source>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
    Source: Provider<Domain, Codomain>,
{
    pub fn new(provider: Arc<Source>, threshold_bytes: usize) -> Self {
        Self {
            preseed: HashSet::new(),
            index: Mutex::new(Inner {
                index: IndexMap::new(),
                evict_q: vec![],
                used_bytes: 0,
                threshold_bytes,
            }),
            source: provider,
        }
    }
}

struct Inner<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
{
    /// Internal index of the cache.
    index: IndexMap<Domain, Entry<Codomain>>,

    /// Eviction queue, a place where entries go to die, unless they are given a second chance.
    /// Entry is Domain, hits & time of insertion. If hits during eviction is the same as hits
    /// during insertion, it's evicted.
    evict_q: Vec<(Domain, usize)>,

    /// Total bytes used by the cache.
    used_bytes: usize,

    /// Threshold for eviction.
    threshold_bytes: usize,
}

/// Holds a lock on the cache while a transaction commit is in progress.
/// (Just wraps the lock to avoid leaking the Inner type.)
pub struct CacheLock<'a, Domain, Codomain>(MutexGuard<'a, Inner<Domain, Codomain>>)
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq;

impl<Domain, Codomain, Source> TransactionalCache<Domain, Codomain, Source>
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
            if let Some(e) = lock.0.index.get(d) {
                match &e.datum {
                    Datum::Value(c) => {
                        preseed_tuples.push((e.ts, d.clone(), c.clone()));
                    }
                    Datum::Tombstone => continue,
                }
            }
        }
        lc.preseed(&preseed_tuples);
        lc
    }

    /// Check the cache for conflicts with the given working set.
    /// Holds a lock on the cache while checking.
    /// This is the first phase of transaction commit, and does not mutate the contents of
    /// the cache.
    pub fn check<'a>(
        &self,
        mut cache_lock: CacheLock<'a, Domain, Codomain>,
        working_set: &WorkingSet<Domain, Codomain>,
    ) -> Result<CacheLock<'a, Domain, Codomain>, Error> {
        let inner = &mut cache_lock.0;
        // Check phase first.
        for (domain, op) in working_set {
            // Check local to see if we have one first, to see if there's a conflict.
            if let Some(local_entry) = inner.index.get(domain) {
                // If what we have is an insert, and there's something already there, that's a
                // a conflict.
                if op.to_type == OpType::Insert {
                    return Err(Error::Conflict);
                }

                let ts = local_entry.ts;
                // If the ts there is greater than the read-ts of our own op, that's a conflict
                // Someone got to it first.
                if ts > op.read_ts {
                    return Err(Error::Conflict);
                }
                continue;
            }

            // Otherwise, pull from upstream and fetch to cache and check for conflict.
            if let Some((ts, codomain, size_bytes)) = self.source.get(domain)? {
                inner.insert_entry(ts, domain.clone(), codomain.clone(), size_bytes);

                // If what we have is an insert, and there's something already there, that's also
                // a conflict.
                if op.to_type == OpType::Insert || ts > op.read_ts {
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
        Ok(cache_lock)
    }

    /// Apply the given working set to the cache.
    /// This is the final phase of the transaction commit process, and mutates the cache and
    /// requests mutation into the Source.
    pub fn apply<'a>(
        &self,
        mut lock: CacheLock<'a, Domain, Codomain>,
        working_set: WorkingSet<Domain, Codomain>,
    ) -> Result<CacheLock<'a, Domain, Codomain>, Error> {
        let inner = &mut lock.0;
        // Apply phase.
        for (domain, op) in working_set {
            match op.to_type {
                OpType::Insert | OpType::Update => {
                    let codomain = op.value.unwrap();
                    inner.insert_entry(op.write_ts, domain.clone(), codomain.clone(), 0);
                    self.source.put(op.write_ts, domain.clone(), codomain).ok();
                }
                OpType::Delete => {
                    inner.insert_tombstone(op.write_ts, domain.clone());
                    self.source.del(op.write_ts, &domain).unwrap();
                }
                _ => continue,
            }
        }
        Ok(lock)
    }

    pub fn lock(&self) -> CacheLock<Domain, Codomain> {
        CacheLock(self.index.lock().unwrap())
    }

    #[allow(dead_code)]
    pub fn cache_usage_bytes(&self) -> usize {
        self.index.lock().unwrap().used_bytes
    }

    /// Scavenge the cache for victims that haven't been hit in the last round.
    pub fn process_cache_evictions(&self) -> (usize, usize) {
        let mut inner = self.lock();
        inner.0.process_evictions()
    }
}

impl<Domain, Codomain> Inner<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
{
    fn insert_entry(
        &mut self,
        ts: Timestamp,
        domain: Domain,
        codomain: Codomain,
        entry_size_bytes: usize,
    ) {
        match self.index.insert(
            domain.clone(),
            Entry {
                ts,
                hits: 0,
                datum: Datum::Value(codomain),
                size_bytes: entry_size_bytes,
            },
        ) {
            None => {
                self.used_bytes += entry_size_bytes;
            }
            Some(oe) => {
                self.used_bytes -= oe.size_bytes;
                self.used_bytes += entry_size_bytes;
            }
        }

        self.select_victims();
    }

    fn insert_tombstone(&mut self, ts: Timestamp, domain: Domain) {
        match self.index.insert(
            domain.clone(),
            Entry {
                ts,
                hits: 0,
                datum: Datum::Tombstone,
                // TODO: this really should be a constant size of what a zero-size entry is, which
                //  is actually a few bytes
                size_bytes: 0,
            },
        ) {
            None => {}
            Some(oe) => {
                self.used_bytes -= oe.size_bytes;
            }
        }
        self.select_victims();
    }

    fn index_lookup(&mut self, domain: &Domain) -> Option<&mut Entry<Codomain>> {
        let mut entry = self.index.get_mut(domain);
        if let Some(e) = &mut entry {
            e.hits += 1;
        }
        entry
    }

    fn select_victims(&mut self) {
        // If we've hit a bytes threshold, we pick some entries at random to put into an eviction
        // victims list. It can then be given a second chance, or if still seen in the next
        // eviction round, it will be evicted.
        if self.used_bytes > self.threshold_bytes {
            let mut total_candidate_bytes = 0;
            let eviction_bytes_needed = self.used_bytes - self.threshold_bytes;
            while total_candidate_bytes < eviction_bytes_needed {
                let random_index = rand::random::<usize>() % self.index.len();
                let entry = self.index.get_index(random_index).unwrap();
                total_candidate_bytes += entry.1.size_bytes;
                self.evict_q.push((entry.0.clone(), entry.1.hits));
            }
        }
    }

    fn process_evictions(&mut self) -> (usize, usize) {
        // Go through the eviction queue and evict entries that haven't been hit in the last round.
        let mut num_evicted = 0;
        let before_eviction = self.used_bytes;
        let evict_q = std::mem::take(&mut self.evict_q);
        let mut victims = Vec::new();
        for (domain, hits) in evict_q {
            if let Some(e) = self.index.get(&domain) {
                if e.hits == hits {
                    victims.push(domain);
                    self.used_bytes -= e.size_bytes;
                    num_evicted += 1;
                }
            }
        }
        for v in victims {
            self.index.swap_remove(&v);
        }
        (num_evicted, before_eviction - self.used_bytes)
    }
}

impl<Domain, Codomain, Source> Canonical<Domain, Codomain>
    for TransactionalCache<Domain, Codomain, Source>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
    Source: Provider<Domain, Codomain>,
{
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error> {
        let mut inner = self.index.lock().unwrap();
        if let Some(entry) = inner.index_lookup(domain) {
            match &entry.datum {
                Datum::Value(codomain) => Ok(Some((entry.ts, codomain.clone()))),
                Datum::Tombstone => Ok(None),
            }
        } else {
            // Pull from backing store.
            if let Some((ts, codomain, bytes)) = self.source.get(domain)? {
                inner.insert_entry(ts, domain.clone(), codomain.clone(), bytes);
                Ok(Some((ts, codomain)))
            } else {
                Ok(None)
            }
        }
    }

    fn scan<F>(&self, predicate: &F) -> Result<Vec<(Timestamp, Domain, Codomain, usize)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        let results = self.source.scan(&predicate)?;
        for (ts, domain, codomain, size) in &results {
            let mut index = self.index.lock().unwrap();
            index.insert_entry(*ts, domain.clone(), codomain.clone(), *size);
        }

        Ok(results)
    }
}

impl<Domain, Codomain, Source> SizedCache for TransactionalCache<Domain, Codomain, Source>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
    Source: Provider<Domain, Codomain>,
{
    fn process_cache_evictions(&self) -> (usize, usize) {
        self.process_cache_evictions()
    }

    fn cache_usage_bytes(&self) -> usize {
        self.cache_usage_bytes()
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
        fn get(
            &self,
            domain: &TestDomain,
        ) -> Result<Option<(Timestamp, TestCodomain, usize)>, Error> {
            let data = self.data.lock().unwrap();
            if let Some(codomain) = data.get(domain) {
                Ok(Some((Timestamp(0), codomain.clone(), 8)))
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
        ) -> Result<Vec<(Timestamp, TestDomain, TestCodomain, usize)>, Error>
        where
            F: Fn(&TestDomain, &TestCodomain) -> bool,
        {
            let data = self.data.lock().unwrap();
            Ok(data
                .iter()
                .filter(|(k, v)| predicate(k, v))
                .map(|(k, v)| (Timestamp(0), k.clone(), v.clone(), 16))
                .collect())
        }
    }

    #[test]
    fn test_basic() {
        let mut backing = HashMap::new();
        backing.insert(TestDomain(0), TestCodomain(0));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let global_cache = Arc::new(TransactionalCache::new(provider, 2048));

        let domain = TestDomain(1);
        let codomain = TestCodomain(1);

        let tx = Tx { ts: Timestamp(0) };
        let mut lc = global_cache.clone().start(&tx);
        lc.insert(domain.clone(), codomain.clone()).unwrap();
        assert_eq!(lc.get(&domain).unwrap(), Some(codomain.clone()));
        assert_eq!(lc.get(&TestDomain(0)).unwrap(), Some(TestCodomain(0)));
        let ws = lc.working_set();

        let lock = global_cache.lock();
        let lock = global_cache.check(lock, &ws).unwrap();
        let lock = global_cache.apply(lock, ws).unwrap();
        assert_eq!(
            lock.0.index.get(&domain).unwrap().datum,
            Datum::Value(codomain.clone())
        );
    }

    #[test]
    fn test_serializable_initial_insert_conflict() {
        let mut backing = HashMap::new();
        backing.insert(TestDomain(0), TestCodomain(0));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let global_cache = Arc::new(TransactionalCache::new(provider, 2048));

        let domain = TestDomain(1);
        let codomain_a = TestCodomain(1);
        let codomain_b = TestCodomain(2);

        let tx_a = Tx { ts: Timestamp(0) };
        let tx_b = Tx { ts: Timestamp(1) };

        let mut lc_a = global_cache.clone().start(&tx_a);

        lc_a.insert(domain.clone(), codomain_a).unwrap();
        let mut lc_b = global_cache.clone().start(&tx_b);
        lc_b.insert(domain.clone(), codomain_b).unwrap();
        let ws_a = lc_a.working_set();
        let ws_b = lc_b.working_set();
        {
            let lock_a = global_cache.lock();
            let lock_a = global_cache.check(lock_a, &ws_a).unwrap();
            let _lock_a = global_cache.apply(lock_a, ws_a).unwrap();
        }
        {
            let lock_b = global_cache.lock();

            // This should fail because the first insert has already happened.
            let check_result = global_cache.check(lock_b, &ws_b);
            assert!(matches!(check_result, Err(Error::Conflict)));
        }
    }
}
