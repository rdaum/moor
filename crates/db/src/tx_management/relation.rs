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

use crate::tx_management::relation_tx::{OpType, RelationTransaction, WorkingSet};
use crate::tx_management::{Canonical, Error, Provider, SizedCache, Timestamp, Tx};
use ahash::AHasher;
use minstant::Instant;
use moor_var::Symbol;
use std::hash::{BuildHasherDefault, Hash};
use std::sync::{Arc, RwLock, RwLockWriteGuard};
use std::time::Duration;
use tracing::warn;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry<T: Clone + PartialEq> {
    pub ts: Timestamp,
    pub hits: usize,
    pub value: T,
    pub size_bytes: usize,
}

/// Represents the current "canonical" state of a relation.
#[derive(Clone)]
pub struct Relation<Domain, Codomain, Source>
where
    Source: Provider<Domain, Codomain>,
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
{
    relation_name: Symbol,

    index: Arc<RwLock<RelationIndex<Domain, Codomain>>>,

    source: Arc<Source>,
}

impl<Domain, Codomain, Source> Relation<Domain, Codomain, Source>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
    Source: Provider<Domain, Codomain>,
{
    pub fn new(relation_name: Symbol, provider: Arc<Source>) -> Self {
        Self {
            relation_name,
            index: Arc::new(RwLock::new(RelationIndex {
                entries: Default::default(),
                used_bytes: 0,
            })),
            source: provider,
        }
    }

    pub fn write_lock(&self) -> RwLockWriteGuard<RelationIndex<Domain, Codomain>> {
        self.index.write().unwrap()
    }
}

#[derive(Clone)]
pub struct RelationIndex<Domain, Codomain>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
{
    /// Internal index of the cache.
    entries: im::HashMap<Domain, Entry<Codomain>, BuildHasherDefault<AHasher>>,

    /// Total bytes used by the cache.
    used_bytes: usize,
}

/// Holds a lock on the cache while a transaction commit is in progress.
/// (Just wraps the lock to avoid leaking the Inner type.)
pub struct CheckRelation<Domain, Codomain, P>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
    P: Provider<Domain, Codomain>,
{
    index: RelationIndex<Domain, Codomain>,
    relation_name: Symbol,
    source: Arc<P>,
    dirty: bool,
}

impl<Domain, Codomain, P> CheckRelation<Domain, Codomain, P>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
    P: Provider<Domain, Codomain>,
{
    pub fn num_entries(&self) -> usize {
        self.index.entries.len()
    }

    pub fn used_bytes(&self) -> usize {
        self.index.used_bytes
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// Check the cache for conflicts with the given working set.
    /// Holds a lock on the cache while checking.
    /// This is the first phase of transaction commit, and does not mutate the contents of
    /// the cache.
    pub fn check(&mut self, working_set: &WorkingSet<Domain, Codomain>) -> Result<(), Error> {
        let start_time = Instant::now();
        let mut last_check_time = start_time;
        let total_ops = working_set.len();
        self.dirty = !working_set.is_empty();
        // Check phase first.
        for (n, (domain, op, _)) in working_set.iter().enumerate() {
            if last_check_time.elapsed() > Duration::from_secs(5) {
                warn!(
                    "Long check time for {}; running for {}s; {n}/{total_ops} checked",
                    self.relation_name,
                    start_time.elapsed().as_secs_f32()
                );
                last_check_time = Instant::now();
            }
            // Check local to see if we have one first, to see if there's a conflict.
            if let Some(local_entry) = self.index.entries.get(domain) {
                // If what we have is an insert, and there's something already there, that's a
                // a conflict.
                if op.operation == OpType::Insert {
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
                let entry = Entry {
                    ts,
                    hits: 0,
                    value: codomain.clone(),
                    size_bytes,
                };
                self.index.entries.insert(domain.clone(), entry);

                // If what we have is an insert, and there's something already there, that's also
                // a conflict.
                if op.operation == OpType::Insert || ts > op.read_ts {
                    return Err(Error::Conflict);
                }
            } else {
                // If upstream doesn't have it, and it's not an insert or delete, that's a conflict, this
                // should not have happened.
                if op.operation == OpType::Update {
                    return Err(Error::Conflict);
                }
            }
        }
        Ok(())
    }

    /// Apply the given working set to the cache.
    /// This is the final phase of the transaction commit process, and mutates the cache and
    /// requests mutation into the Source.
    pub fn apply(&mut self, working_set: WorkingSet<Domain, Codomain>) -> Result<(), Error> {
        let start_time = Instant::now();
        let mut last_check_time = start_time;
        let total_ops = working_set.len();
        // Apply phase.
        for (n, (domain, op, codomain)) in working_set.into_iter().enumerate() {
            if last_check_time.elapsed() > Duration::from_secs(5) {
                warn!(
                    "Long apply time for {}; running for {}s; {n}/{total_ops} checked",
                    self.relation_name,
                    start_time.elapsed().as_secs_f32()
                );
                last_check_time = Instant::now();
            }
            match op.operation {
                OpType::Insert | OpType::Update => {
                    let entry = codomain
                        .expect("Codomain should be non-None for insert or update operation");
                    self.source.put(op.write_ts, &domain, &entry.value).ok();
                    self.index.insert_entry(
                        op.write_ts,
                        domain.clone(),
                        entry.value,
                        entry.size_bytes,
                    );
                }
                OpType::Delete => {
                    self.index.insert_tombstone(op.write_ts, domain.clone());
                    self.source.del(op.write_ts, &domain).unwrap();
                }
            }
        }
        Ok(())
    }

    pub fn commit(self, inner: Option<RwLockWriteGuard<RelationIndex<Domain, Codomain>>>) {
        if let Some(mut inner) = inner {
            *inner = self.index;
        }
    }
}

impl<Domain, Codomain, Source> Relation<Domain, Codomain, Source>
where
    Source: Provider<Domain, Codomain>,
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
{
    pub fn start(&self, tx: &Tx) -> RelationTransaction<Domain, Codomain, Self> {
        let index = self.index.read().unwrap();
        RelationTransaction::new(*tx, index.entries.clone(), self.clone())
    }

    pub fn begin_check(&self) -> CheckRelation<Domain, Codomain, Source> {
        let index = self.index.read().unwrap();
        CheckRelation {
            index: index.clone(),
            relation_name: self.relation_name,
            source: self.source.clone(),
            dirty: false,
        }
    }

    #[allow(dead_code)]
    pub fn cache_usage_bytes(&self) -> usize {
        let index = self.index.read().unwrap();
        index.used_bytes
    }
}

impl<Domain, Codomain> RelationIndex<Domain, Codomain>
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
        match self.entries.insert(
            domain.clone(),
            Entry {
                ts,
                hits: 0,
                value: codomain,
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
    }

    fn insert_tombstone(&mut self, _ts: Timestamp, domain: Domain) {
        match self.entries.remove(&domain) {
            None => {}
            Some(oe) => {
                self.used_bytes -= oe.size_bytes;
            }
        }
    }

    fn index_lookup(&mut self, domain: &Domain) -> Option<&mut Entry<Codomain>> {
        let mut entry = self.entries.get_mut(domain);
        if let Some(e) = &mut entry {
            e.hits += 1;
        }
        entry
    }
}

impl<Domain, Codomain, Source> Canonical<Domain, Codomain> for Relation<Domain, Codomain, Source>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
    Source: Provider<Domain, Codomain>,
{
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain, usize)>, Error> {
        let mut inner = self.index.write().unwrap();
        if let Some(entry) = inner.index_lookup(domain) {
            Ok(Some((entry.ts, entry.value.clone(), entry.size_bytes)))
        } else {
            // Pull from backing store.
            if let Some((ts, codomain, bytes)) = self.source.get(domain)? {
                inner.insert_entry(ts, domain.clone(), codomain.clone(), bytes);
                inner.used_bytes += bytes;
                Ok(Some((ts, codomain, bytes)))
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
            let mut index = self.index.write().unwrap();
            index.insert_entry(*ts, domain.clone(), codomain.clone(), *size);
            index.used_bytes += *size;
        }

        Ok(results)
    }
}
impl<Domain, Codomain, Source> Relation<Domain, Codomain, Source>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
    Source: Provider<Domain, Codomain>,
{
    pub fn stop_provider(&self) -> Result<(), Error> {
        self.source.stop()
    }
}

impl<Domain, Codomain, Source> SizedCache for Relation<Domain, Codomain, Source>
where
    Domain: Hash + PartialEq + Eq + Clone,
    Codomain: Clone + PartialEq + Eq,
    Source: Provider<Domain, Codomain>,
{
    fn select_victims(&self) {
        // Noop
    }

    fn process_cache_evictions(&self) -> (usize, usize) {
        // Noop
        (0, 0)
    }

    fn cache_usage_bytes(&self) -> usize {
        self.cache_usage_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::tx_management::Tx;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestDomain(u64);

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestCodomain(u64);

    #[derive(Clone)]
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
            domain: &TestDomain,
            codomain: &TestCodomain,
        ) -> Result<(), Error> {
            let mut data = self.data.lock().unwrap();
            data.insert(domain.clone(), codomain.clone());
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

        fn stop(&self) -> Result<(), Error> {
            Ok(())
        }
    }

    #[test]
    fn test_basic() {
        let mut backing = HashMap::new();
        backing.insert(TestDomain(0), TestCodomain(0));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let relation = Arc::new(Relation::new(Symbol::mk("test"), provider));

        let domain = TestDomain(1);
        let codomain = TestCodomain(1);

        let tx = Tx { ts: Timestamp(1) };
        let mut lc = relation.clone().start(&tx);
        lc.insert(domain.clone(), codomain.clone(), 16).unwrap();
        assert_eq!(lc.get(&domain).unwrap(), Some(codomain.clone()));
        assert_eq!(lc.get(&TestDomain(0)).unwrap(), Some(TestCodomain(0)));
        let ws = lc.working_set();

        let mut cr = relation.begin_check();
        cr.check(&ws).unwrap();
        cr.apply(ws).unwrap();
        assert_eq!(relation.get(&domain).unwrap().unwrap().1, codomain.clone());
    }

    #[test]
    fn test_serializable_initial_insert_conflict() {
        let mut backing = HashMap::new();
        backing.insert(TestDomain(0), TestCodomain(0));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let relation = Arc::new(Relation::new(Symbol::mk("test"), provider));

        let domain = TestDomain(1);
        let codomain_a = TestCodomain(1);
        let codomain_b = TestCodomain(2);

        let tx_a = Tx { ts: Timestamp(0) };
        let tx_b = Tx { ts: Timestamp(1) };

        let mut r_tx_a = relation.clone().start(&tx_a);

        r_tx_a.insert(domain.clone(), codomain_a, 16).unwrap();
        let mut r_tx_b = relation.clone().start(&tx_b);
        r_tx_b.insert(domain.clone(), codomain_b, 16).unwrap();
        let ws_a = r_tx_a.working_set();
        let ws_b = r_tx_b.working_set();
        {
            let mut cr_a = relation.begin_check();
            cr_a.check(&ws_a).unwrap();
            cr_a.apply(ws_a).unwrap();
            let mut r = relation.index.write().unwrap();
            *r = cr_a.index;
        }
        {
            let mut cr_b = relation.begin_check();

            // This should fail because the first insert has already happened.
            let check_result = cr_b.check(&ws_b);
            assert!(matches!(check_result, Err(Error::Conflict)));
        }
    }
}
