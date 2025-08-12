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

use crate::tx_management::indexes::{HashRelationIndex, RelationIndex};
use crate::tx_management::relation_tx::{OpType, RelationTransaction, WorkingSet};
use crate::tx_management::{Canonical, Error, Provider, Timestamp, Tx};
use minstant::Instant;
use moor_var::Symbol;
use std::hash::Hash;
use std::sync::{Arc, RwLock, RwLockWriteGuard};
use std::time::Duration;
use tracing::warn;

/// Represents the current "canonical" state of a relation.
pub struct Relation<Domain, Codomain, Source>
where
    Source: Provider<Domain, Codomain>,
    Domain: Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    relation_name: Symbol,

    index: Arc<RwLock<Box<dyn RelationIndex<Domain, Codomain>>>>,

    source: Arc<Source>,
}

impl<Domain, Codomain, Source> Clone for Relation<Domain, Codomain, Source>
where
    Domain: Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
    Source: Provider<Domain, Codomain>,
{
    fn clone(&self) -> Self {
        Self {
            relation_name: self.relation_name,
            index: self.index.clone(),
            source: self.source.clone(),
        }
    }
}

impl<Domain, Codomain, Source> Relation<Domain, Codomain, Source>
where
    Domain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
    Source: Provider<Domain, Codomain>,
{
    pub fn new(relation_name: Symbol, provider: Arc<Source>) -> Self {
        Self {
            relation_name,
            index: Arc::new(RwLock::new(Box::new(HashRelationIndex::new()))),
            source: provider,
        }
    }

    pub fn new_with_secondary(relation_name: Symbol, provider: Arc<Source>) -> Self
    where
        Codomain: Hash + Eq,
    {
        use crate::tx_management::indexes::SecondaryIndexRelation;
        Self {
            relation_name,
            index: Arc::new(RwLock::new(Box::new(SecondaryIndexRelation::new()))),
            source: provider,
        }
    }

    pub fn write_lock(&self) -> RwLockWriteGuard<'_, Box<dyn RelationIndex<Domain, Codomain>>> {
        self.index.write().unwrap()
    }

    pub fn source(&self) -> &Arc<Source> {
        &self.source
    }
}

/// Holds a lock on the cache while a transaction commit is in progress.
/// (Just wraps the lock to avoid leaking the Inner type.)
pub struct CheckRelation<Domain, Codomain, P>
where
    Domain: Clone + Hash + Eq + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
    P: Provider<Domain, Codomain>,
{
    index: Box<dyn RelationIndex<Domain, Codomain>>,
    relation_name: Symbol,
    source: Arc<P>,
    dirty: bool,
}

impl<Domain, Codomain, P> CheckRelation<Domain, Codomain, P>
where
    Domain: Clone + Hash + Eq + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
    P: Provider<Domain, Codomain>,
{
    pub fn num_entries(&self) -> usize {
        self.index.len()
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
        for (n, (domain, op)) in working_set.tuples_ref().iter().enumerate() {
            if last_check_time.elapsed() > Duration::from_secs(5) {
                warn!(
                    "Long check time for {}; running for {}s; {n}/{total_ops} checked",
                    self.relation_name,
                    start_time.elapsed().as_secs_f32()
                );
                last_check_time = Instant::now();
            }
            // Check local to see if we have one first, to see if there's a conflict.
            if let Some(local_entry) = self.index.index_lookup(domain) {
                // If what we have is an insert, and there's something already there, that's a
                // conflict.
                if op.operation.is_insert() {
                    return Err(Error::Conflict);
                }

                let ts = local_entry.ts;
                // If the ts there is greater than the read-ts of our own op, that's a conflict
                // Someone got to it first.
                if ts > op.read_ts {
                    return Err(Error::Conflict);
                }
                // If the transactions *write stamp* is earlier than the read stamp, that's a
                // conflict indicating that the transaction is trying to update something
                // it should not have read.
                // (This only happens because we're not able to early-bail on update operations
                // like this, so there's some waste here.)
                if op.read_ts > op.write_ts {
                    return Err(Error::Conflict);
                }
                continue;
            }

            // Otherwise, pull from upstream and fetch to cache and check for conflict.
            if let Some((ts, codomain)) = self.source.get(domain)? {
                self.index
                    .insert_entry(ts, domain.clone(), codomain.clone());

                // If what we have is an insert, and there's something already there, that's also
                // a conflict.
                if op.operation.is_insert() || ts > op.read_ts {
                    return Err(Error::Conflict);
                }
            } else {
                // If upstream doesn't have it, and it's not an insert or delete, that's a conflict, this
                // should not have happened.
                if op.operation.is_update() {
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
        // Update the provider_fully_loaded state first
        if working_set.provider_fully_loaded() {
            self.index.set_provider_fully_loaded(true);
        }

        // Apply phase.
        for (domain, op) in working_set.tuples().into_iter() {
            match op.operation {
                OpType::Insert(codomain) | OpType::Update(codomain) => {
                    self.source.put(op.write_ts, &domain, &codomain).ok();
                    self.index
                        .insert_entry(op.write_ts, domain.clone(), codomain);
                }
                OpType::Delete => {
                    self.index.insert_tombstone(op.write_ts, domain.clone());
                    self.source.del(op.write_ts, &domain).unwrap();
                }
            }
        }
        Ok(())
    }

    pub fn commit(self, inner: Option<RwLockWriteGuard<Box<dyn RelationIndex<Domain, Codomain>>>>) {
        if let Some(mut inner) = inner {
            *inner = self.index;
        }
    }
}

impl<Domain, Codomain, Source> Relation<Domain, Codomain, Source>
where
    Source: Provider<Domain, Codomain>,
    Domain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    pub fn start(&self, tx: &Tx) -> RelationTransaction<Domain, Codomain, Self> {
        let index = self.index.read().unwrap();
        RelationTransaction::new(*tx, index.fork(), self.clone())
    }

    pub fn begin_check(&self) -> CheckRelation<Domain, Codomain, Source> {
        let index = self.index.read().unwrap();
        CheckRelation {
            index: index.fork(),
            relation_name: self.relation_name,
            source: self.source.clone(),
            dirty: false,
        }
    }
}

impl<Domain, Codomain, Source> Canonical<Domain, Codomain> for Relation<Domain, Codomain, Source>
where
    Domain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
    Source: Provider<Domain, Codomain>,
{
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error> {
        // First try with read lock
        {
            let inner = self.index.read().unwrap();
            if let Some(entry) = inner.index_lookup(domain) {
                return Ok(Some((entry.ts, entry.value.clone())));
            }
        }

        // Not in cache, need write lock to potentially insert from backing store
        let mut inner = self.index.write().unwrap();
        // Double-check since another thread might have inserted while we waited for write lock
        if let Some(entry) = inner.index_lookup(domain) {
            Ok(Some((entry.ts, entry.value.clone())))
        } else {
            // Pull from backing store.
            if let Some((ts, codomain)) = self.source.get(domain)? {
                inner.insert_entry(ts, domain.clone(), codomain.clone());
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
        {
            let mut index = self.index.write().unwrap();
            for (ts, domain, codomain) in &results {
                index.insert_entry(*ts, domain.clone(), codomain.clone());
            }
            // If we're scanning with a predicate that accepts everything, mark as fully loaded
            if self.is_full_scan_predicate(predicate) {
                index.set_provider_fully_loaded(true);
            }
        }

        Ok(results)
    }

    fn get_by_codomain(&self, codomain: &Codomain) -> Vec<Domain> {
        let inner = self.index.read().unwrap();
        inner.get_by_codomain(codomain)
    }
}
impl<Domain, Codomain, Source> Relation<Domain, Codomain, Source>
where
    Domain: Hash + PartialEq + Eq + Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
    Source: Provider<Domain, Codomain>,
{
    pub fn stop_provider(&self) -> Result<(), Error> {
        self.source.stop()
    }

    /// Check if a predicate represents a full scan (accepts everything)
    /// We can detect this by testing with dummy values, but for now we'll use a simpler approach
    fn is_full_scan_predicate<F>(&self, _predicate: &F) -> bool
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        // For now, we'll be conservative and only mark as fully loaded when
        // explicitly called through get_all() in RelationTransaction
        false
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

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestCodomain(u64);

    #[derive(Clone)]
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
        lc.insert(domain.clone(), codomain.clone()).unwrap();
        assert_eq!(lc.get(&domain).unwrap(), Some(codomain.clone()));
        assert_eq!(lc.get(&TestDomain(0)).unwrap(), Some(TestCodomain(0)));
        let ws = lc.working_set().unwrap();

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

        r_tx_a.insert(domain.clone(), codomain_a).unwrap();
        let mut r_tx_b = relation.clone().start(&tx_b);
        r_tx_b.insert(domain.clone(), codomain_b).unwrap();
        let ws_a = r_tx_a.working_set().unwrap();
        let ws_b = r_tx_b.working_set().unwrap();
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

    #[test]
    fn test_concurrent_read_write_conflict() {
        // Test write-after-read dependency: T1 reads, T2 writes, T1 writes
        let mut backing = HashMap::new();
        backing.insert(TestDomain(1), TestCodomain(10));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let relation = Arc::new(Relation::new(Symbol::mk("test"), provider));

        let domain = TestDomain(1);

        let tx_1 = Tx { ts: Timestamp(10) };
        let tx_2 = Tx { ts: Timestamp(20) };

        // T1 reads the value
        let mut r_tx_1 = relation.clone().start(&tx_1);
        let initial_value = r_tx_1.get(&domain).unwrap().unwrap();
        assert_eq!(initial_value, TestCodomain(10));

        // T2 updates the value
        let mut r_tx_2 = relation.clone().start(&tx_2);
        r_tx_2.update(&domain, TestCodomain(20)).unwrap();
        let ws_2 = r_tx_2.working_set().unwrap();

        // Commit T2 first
        {
            let mut cr_2 = relation.begin_check();
            cr_2.check(&ws_2).unwrap();
            cr_2.apply(ws_2).unwrap();
            let mut r = relation.index.write().unwrap();
            *r = cr_2.index;
        }

        // Now T1 tries to update based on its read - this should conflict
        let check_result = r_tx_1.update(&domain, TestCodomain(11));
        assert!(matches!(check_result, Err(Error::Conflict)));
    }

    #[test]
    fn test_write_write_conflict() {
        // Test two transactions updating the same key
        let mut backing = HashMap::new();
        backing.insert(TestDomain(1), TestCodomain(10));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let relation = Arc::new(Relation::new(Symbol::mk("test"), provider));

        let domain = TestDomain(1);

        let tx_1 = Tx { ts: Timestamp(10) };
        let tx_2 = Tx { ts: Timestamp(20) };

        // Both transactions update the same key
        let mut r_tx_1 = relation.clone().start(&tx_1);
        let mut r_tx_2 = relation.clone().start(&tx_2);

        r_tx_1.update(&domain, TestCodomain(100)).unwrap();
        r_tx_2.update(&domain, TestCodomain(200)).unwrap();

        let ws_1 = r_tx_1.working_set().unwrap();
        let ws_2 = r_tx_2.working_set().unwrap();

        // Commit T1 first
        {
            let mut cr_1 = relation.begin_check();
            cr_1.check(&ws_1).unwrap();
            cr_1.apply(ws_1).unwrap();
            let mut r = relation.index.write().unwrap();
            *r = cr_1.index;
        }

        // T2 should conflict
        let mut cr_2 = relation.begin_check();
        let check_result = cr_2.check(&ws_2);
        assert!(matches!(check_result, Err(Error::Conflict)));
    }

    #[test]
    fn test_phantom_read_protection() {
        // Test that inserts are properly serialized to prevent phantom reads
        let backing = HashMap::new();
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let relation = Arc::new(Relation::new(Symbol::mk("test"), provider));

        let domain_1 = TestDomain(1);
        let domain_2 = TestDomain(2);

        let tx_1 = Tx { ts: Timestamp(10) };
        let tx_2 = Tx { ts: Timestamp(20) };

        // T1 scans for all entries (finds none)
        let mut r_tx_1 = relation.clone().start(&tx_1);
        let scan_result = r_tx_1.scan(&|_, _| true).unwrap();
        assert_eq!(scan_result.len(), 0);

        // T2 inserts a new entry
        let mut r_tx_2 = relation.clone().start(&tx_2);
        r_tx_2.insert(domain_1.clone(), TestCodomain(100)).unwrap();
        let ws_2 = r_tx_2.working_set().unwrap();

        // Commit T2
        {
            let mut cr_2 = relation.begin_check();
            cr_2.check(&ws_2).unwrap();
            cr_2.apply(ws_2).unwrap();
            let mut r = relation.index.write().unwrap();
            *r = cr_2.index;
        }

        // T1 now inserts another entry - this should succeed since it's a different key
        r_tx_1.insert(domain_2.clone(), TestCodomain(200)).unwrap();
        let ws_1 = r_tx_1.working_set().unwrap();

        let mut cr_1 = relation.begin_check();
        cr_1.check(&ws_1).unwrap(); // Should not conflict
        cr_1.apply(ws_1).unwrap();
    }

    #[test]
    fn test_delete_insert_sequence() {
        // Test delete in one transaction followed by insert in another
        let mut backing = HashMap::new();
        backing.insert(TestDomain(1), TestCodomain(10));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let relation = Arc::new(Relation::new(Symbol::mk("test"), provider));

        let domain = TestDomain(1);

        let tx_1 = Tx { ts: Timestamp(10) };
        let tx_2 = Tx { ts: Timestamp(20) };

        // T1 deletes the entry
        let mut r_tx_1 = relation.clone().start(&tx_1);
        r_tx_1.delete(&domain).unwrap();
        let ws_1 = r_tx_1.working_set().unwrap();

        // Commit T1 first
        {
            let mut cr_1 = relation.begin_check();
            cr_1.check(&ws_1).unwrap();
            cr_1.apply(ws_1).unwrap();
            let mut r = relation.index.write().unwrap();
            *r = cr_1.index;
        }

        // Now T2 tries to insert the same key - should succeed since key was deleted
        let mut r_tx_2 = relation.clone().start(&tx_2);
        r_tx_2.insert(domain.clone(), TestCodomain(20)).unwrap();
        let ws_2 = r_tx_2.working_set().unwrap();

        let mut cr_2 = relation.begin_check();
        cr_2.check(&ws_2).unwrap();
        cr_2.apply(ws_2).unwrap();

        // Verify final state
        assert_eq!(relation.get(&domain).unwrap().unwrap().1, TestCodomain(20));
    }

    #[test]
    fn test_update_nonexistent_key() {
        // Test updating a key that doesn't exist - the update should return None but not error
        let backing = HashMap::new();
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let relation = Arc::new(Relation::new(Symbol::mk("test"), provider));

        let domain = TestDomain(1);
        let tx = Tx { ts: Timestamp(10) };

        let mut r_tx = relation.clone().start(&tx);
        let result = r_tx.update(&domain, TestCodomain(100)).unwrap();
        assert_eq!(result, None); // Update of nonexistent key returns None

        let ws = r_tx.working_set().unwrap();
        // The working set should be empty since no actual operation occurred
        assert_eq!(ws.len(), 0);
    }

    #[test]
    fn test_serial_execution_order() {
        // Test that transactions maintain serializability when executed in timestamp order
        let mut backing = HashMap::new();
        backing.insert(TestDomain(1), TestCodomain(0));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let relation = Arc::new(Relation::new(Symbol::mk("test"), provider));

        let domain = TestDomain(1);

        // Execute transactions in timestamp order
        for i in 1..=5 {
            let tx = Tx { ts: Timestamp(i) };
            let mut r_tx = relation.clone().start(&tx);

            // Read current value and increment it
            let current = r_tx.get(&domain).unwrap().unwrap();
            r_tx.update(&domain, TestCodomain(current.0 + 1)).unwrap();

            let ws = r_tx.working_set().unwrap();
            let mut cr = relation.begin_check();
            cr.check(&ws).unwrap();
            cr.apply(ws).unwrap();
            let mut r = relation.index.write().unwrap();
            *r = cr.index;
        }

        // Final value should be 5 (0 + 5 increments)
        assert_eq!(relation.get(&domain).unwrap().unwrap().1, TestCodomain(5));
    }

    #[test]
    fn test_mixed_operations_serialization() {
        // Test a complex scenario with inserts, updates, and deletes
        let mut backing = HashMap::new();
        backing.insert(TestDomain(1), TestCodomain(100));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let relation = Arc::new(Relation::new(Symbol::mk("test"), provider));

        let tx_1 = Tx { ts: Timestamp(10) };
        let tx_2 = Tx { ts: Timestamp(20) };
        let tx_3 = Tx { ts: Timestamp(30) };

        // T1: Update existing key and insert new key
        let mut r_tx_1 = relation.clone().start(&tx_1);
        r_tx_1.update(&TestDomain(1), TestCodomain(200)).unwrap();
        r_tx_1.insert(TestDomain(2), TestCodomain(300)).unwrap();
        let ws_1 = r_tx_1.working_set().unwrap();

        // T2: Try to update the same key as T1 but to different value
        let mut r_tx_2 = relation.clone().start(&tx_2);
        r_tx_2.update(&TestDomain(1), TestCodomain(400)).unwrap();
        r_tx_2.insert(TestDomain(3), TestCodomain(500)).unwrap();
        let ws_2 = r_tx_2.working_set().unwrap();

        // Commit T1 first
        {
            let mut cr_1 = relation.begin_check();
            cr_1.check(&ws_1).unwrap();
            cr_1.apply(ws_1).unwrap();
            let mut r = relation.index.write().unwrap();
            *r = cr_1.index;
        }

        // T2 should conflict because it tries to update what T1 already updated
        {
            let mut cr_2 = relation.begin_check();
            let check_result = cr_2.check(&ws_2);
            assert!(matches!(check_result, Err(Error::Conflict)));
        }

        // T3: Should be able to read T1's committed changes and make updates
        let mut r_tx_3 = relation.clone().start(&tx_3);
        let current_val = r_tx_3.get(&TestDomain(1)).unwrap().unwrap();
        assert_eq!(current_val, TestCodomain(200)); // Should see T1's update
        r_tx_3
            .update(&TestDomain(1), TestCodomain(current_val.0 + 100))
            .unwrap();
        let ws_3 = r_tx_3.working_set().unwrap();

        {
            let mut cr_3 = relation.begin_check();
            cr_3.check(&ws_3).unwrap();
            cr_3.apply(ws_3).unwrap();
            let mut r = relation.index.write().unwrap();
            *r = cr_3.index;
        }

        // Verify final state: T1's insert and T3's update should be there
        assert_eq!(
            relation.get(&TestDomain(1)).unwrap().unwrap().1,
            TestCodomain(300)
        );
        assert_eq!(
            relation.get(&TestDomain(2)).unwrap().unwrap().1,
            TestCodomain(300)
        );
        assert!(relation.get(&TestDomain(3)).unwrap().is_none()); // T2 didn't commit
    }

    #[test]
    fn test_timestamp_ordering_enforcement() {
        // Test that operations respect timestamp ordering for conflict detection
        let mut backing = HashMap::new();
        backing.insert(TestDomain(1), TestCodomain(100));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let relation = Arc::new(Relation::new(Symbol::mk("test"), provider));

        let domain = TestDomain(1);

        // Start both transactions, older one reads first
        let tx_older = Tx { ts: Timestamp(10) };
        let tx_newer = Tx { ts: Timestamp(20) };

        let mut r_tx_older = relation.clone().start(&tx_older);
        let _old_val = r_tx_older.get(&domain).unwrap().unwrap(); // Read with ts=10

        // Newer transaction commits first
        let mut r_tx_newer = relation.clone().start(&tx_newer);
        r_tx_newer.update(&domain, TestCodomain(200)).unwrap();
        let ws_newer = r_tx_newer.working_set().unwrap();

        {
            let mut cr_newer = relation.begin_check();
            cr_newer.check(&ws_newer).unwrap();
            cr_newer.apply(ws_newer).unwrap();
            let mut r = relation.index.write().unwrap();
            *r = cr_newer.index;
        }

        // Now older transaction tries to update - should conflict due to newer timestamp in cache
        let check_result = r_tx_older.update(&domain, TestCodomain(300));
        assert!(matches!(check_result, Err(Error::Conflict)));
    }

    #[test]
    fn test_consistent_snapshot_reads() {
        // Test that reads within a transaction see a consistent snapshot
        let mut backing = HashMap::new();
        backing.insert(TestDomain(1), TestCodomain(100));
        backing.insert(TestDomain(2), TestCodomain(200));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let relation = Arc::new(Relation::new(Symbol::mk("test"), provider));

        let tx = Tx { ts: Timestamp(10) };
        let mut r_tx = relation.clone().start(&tx);

        // Read both keys - they should be consistent
        let val1 = r_tx.get(&TestDomain(1)).unwrap().unwrap();
        let val2 = r_tx.get(&TestDomain(2)).unwrap().unwrap();

        assert_eq!(val1, TestCodomain(100));
        assert_eq!(val2, TestCodomain(200));

        // Update one key based on both reads
        r_tx.update(&TestDomain(1), TestCodomain(val1.0 + val2.0))
            .unwrap();

        let ws = r_tx.working_set().unwrap();
        let mut cr = relation.begin_check();
        cr.check(&ws).unwrap();
        cr.apply(ws).unwrap();

        // Commit the changes to the relation
        let guard = relation.index.write().unwrap();
        cr.commit(Some(guard));

        // Verify the update was applied correctly
        assert_eq!(
            relation.get(&TestDomain(1)).unwrap().unwrap().1,
            TestCodomain(300)
        );
    }

    #[test]
    fn test_secondary_index_transaction_integration() {
        use crate::tx_management::indexes::SecondaryIndexRelation;

        let backing = HashMap::new();
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });

        // Create relation with secondary index support
        let relation = Arc::new(Relation {
            relation_name: Symbol::mk("test"),
            index: Arc::new(RwLock::new(Box::new(SecondaryIndexRelation::new()))),
            source: provider,
        });

        let domain1 = TestDomain(1);
        let domain2 = TestDomain(2);
        let domain3 = TestDomain(3);
        let codomain_a = TestCodomain(100);
        let codomain_b = TestCodomain(200);

        let tx = Tx { ts: Timestamp(10) };
        let mut r_tx = relation.clone().start(&tx);

        // Insert entries
        r_tx.insert(domain1.clone(), codomain_a.clone()).unwrap();
        r_tx.insert(domain2.clone(), codomain_a.clone()).unwrap();
        r_tx.insert(domain3.clone(), codomain_b.clone()).unwrap();

        // Test get_by_codomain through transaction interface
        let result_a = r_tx.get_by_codomain(&codomain_a);
        assert_eq!(result_a.len(), 2);
        assert!(result_a.contains(&domain1));
        assert!(result_a.contains(&domain2));

        let result_b = r_tx.get_by_codomain(&codomain_b);
        assert_eq!(result_b.len(), 1);
        assert!(result_b.contains(&domain3));

        // Commit the transaction
        let ws = r_tx.working_set().unwrap();
        let mut cr = relation.begin_check();
        cr.check(&ws).unwrap();
        cr.apply(ws).unwrap();
        let guard = relation.index.write().unwrap();
        cr.commit(Some(guard));

        // Test that committed secondary index state is visible in new transaction
        let tx2 = Tx { ts: Timestamp(20) };
        let r_tx2 = relation.clone().start(&tx2);

        let committed_result_a = r_tx2.get_by_codomain(&codomain_a);
        assert_eq!(committed_result_a.len(), 2);
        assert!(committed_result_a.contains(&domain1));
        assert!(committed_result_a.contains(&domain2));

        let committed_result_b = r_tx2.get_by_codomain(&codomain_b);
        assert_eq!(committed_result_b.len(), 1);
        assert!(committed_result_b.contains(&domain3));
    }
}
