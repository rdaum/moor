// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use crate::{
    provider::Provider,
    tx::{
        Error, RelationCodomain, RelationCodomainHashable, RelationDomain, Tx,
    },
};
use moor_var::Symbol;
use std::sync::Arc;

#[cfg(test)]
use crate::tx::{Canonical, Timestamp};
#[cfg(test)]
use arc_swap::ArcSwap;
use crate::tx::{
    CheckRelation, RelationIndex, RelationTransaction,
    indexes::{HashRelationIndex, SecondaryIndexRelation},
};

/// Represents the current "canonical" state of a relation.
type IndexFactory<Domain, Codomain> = fn() -> Box<dyn RelationIndex<Domain, Codomain>>;

fn primary_index_factory<Domain, Codomain>() -> Box<dyn RelationIndex<Domain, Codomain>>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    Box::new(HashRelationIndex::new())
}

fn secondary_index_factory<Domain, Codomain>() -> Box<dyn RelationIndex<Domain, Codomain>>
where
    Domain: RelationDomain,
    Codomain: RelationCodomainHashable,
{
    Box::new(SecondaryIndexRelation::new())
}

#[derive(Clone)]
pub struct Relation<Domain, Codomain, Source>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    relation_name: Symbol,
    source: Arc<Source>,
    index_factory: IndexFactory<Domain, Codomain>,
    #[cfg(test)]
    test_index: Arc<ArcSwap<Box<dyn RelationIndex<Domain, Codomain>>>>,
}

impl<Domain, Codomain, Source> Relation<Domain, Codomain, Source>
where
    Source: Provider<Domain, Codomain>,
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    pub fn new(relation_name: Symbol, source: Arc<Source>) -> Self {
        Self {
            relation_name,
            source,
            index_factory: primary_index_factory::<Domain, Codomain>,
            #[cfg(test)]
            test_index: Arc::new(ArcSwap::new(Arc::new(primary_index_factory::<
                Domain,
                Codomain,
            >()))),
        }
    }

    pub fn new_with_secondary(relation_name: Symbol, source: Arc<Source>) -> Self
    where
        Codomain: RelationCodomainHashable,
    {
        Self {
            relation_name,
            source,
            index_factory: secondary_index_factory::<Domain, Codomain>,
            #[cfg(test)]
            test_index: Arc::new(ArcSwap::new(Arc::new(secondary_index_factory::<
                Domain,
                Codomain,
            >()))),
        }
    }

    pub fn source(&self) -> &Arc<Source> {
        &self.source
    }

    pub fn seeded_index(&self) -> Result<Box<dyn RelationIndex<Domain, Codomain>>, Error> {
        let mut index = (self.index_factory)();
        let tuples = self.source.scan(&|_, _| true)?;
        for (ts, domain, codomain) in tuples {
            index.insert_entry(ts, domain, codomain);
        }
        index.set_provider_fully_loaded(true);
        Ok(index)
    }

    pub fn start_from_index(
        &self,
        tx: &Tx,
        index: &dyn RelationIndex<Domain, Codomain>,
    ) -> RelationTransaction<Domain, Codomain, Source> {
        RelationTransaction::new(
            *tx,
            self.relation_name,
            index.fork(),
            (*self.source).clone(),
        )
    }

    pub fn begin_check_from_index(
        &self,
        index: &dyn RelationIndex<Domain, Codomain>,
    ) -> CheckRelation<Domain, Codomain, Source> {
        CheckRelation {
            index: index.fork(),
            relation_name: self.relation_name,
            source: self.source.clone(),
            dirty: false,
        }
    }

    #[cfg(test)]
    pub fn index(&self) -> &Arc<ArcSwap<Box<dyn RelationIndex<Domain, Codomain>>>> {
        &self.test_index
    }

    #[cfg(test)]
    pub fn start(&self, tx: &Tx) -> RelationTransaction<Domain, Codomain, Source> {
        let index = self.test_index.load();
        self.start_from_index(tx, index.as_ref().as_ref())
    }

    #[cfg(test)]
    pub fn begin_check(&self) -> CheckRelation<Domain, Codomain, Source> {
        let index = self.test_index.load();
        self.begin_check_from_index(index.as_ref().as_ref())
    }

    #[cfg(test)]
    /// Mark this relation as fully loaded from its backing provider.
    /// After this call, scans will skip provider I/O and use only cached data.
    pub fn mark_fully_loaded(&self) {
        let index = self.test_index.load();
        let mut new_index = (**index).fork();
        new_index.set_provider_fully_loaded(true);
        self.test_index.store(Arc::new(new_index));
    }
}

#[cfg(test)]
impl<Domain, Codomain, Source> Canonical<Domain, Codomain> for Relation<Domain, Codomain, Source>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
    Source: Provider<Domain, Codomain>,
{
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error> {
        // Try read path first
        let index = self.test_index.load();
        if let Some(entry) = index.index_lookup(domain) {
            return Ok(Some((entry.ts, entry.value.clone())));
        }

        // If provider is fully loaded, not being in index means it doesn't exist
        if index.is_provider_fully_loaded() {
            return Ok(None);
        }

        // Provider not fully loaded - need to check backing store
        // Fork the index, insert the new entry, and swap it in
        let mut new_index = (**index).fork();
        if let Some((ts, codomain)) = self.source.get(domain)? {
            new_index.insert_entry(ts, domain.clone(), codomain.clone());
            self.test_index.store(Arc::new(new_index));
            Ok(Some((ts, codomain)))
        } else {
            Ok(None)
        }
    }

    fn scan<F>(&self, predicate: &F) -> Result<Vec<(Timestamp, Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        let index = self.test_index.load();

        // If provider is fully loaded, scan directly from index
        if index.is_provider_fully_loaded() {
            let results: Vec<_> = index
                .iter()
                .filter_map(|(domain, entry)| {
                    if predicate(domain, &entry.value) {
                        Some((entry.ts, domain.clone(), entry.value.clone()))
                    } else {
                        None
                    }
                })
                .collect();
            return Ok(results);
        }

        // Provider not fully loaded - need to scan backing source
        let results = self.source.scan(&predicate)?;
        let mut new_index = (**index).fork();

        for (ts, domain, codomain) in &results {
            new_index.insert_entry(*ts, domain.clone(), codomain.clone());
        }

        // If we're scanning with a predicate that accepts everything, mark as fully loaded
        if self.is_full_scan_predicate(predicate) {
            new_index.set_provider_fully_loaded(true);
        }

        self.test_index.store(Arc::new(new_index));
        Ok(results)
    }

    fn get_by_codomain(&self, codomain: &Codomain) -> Vec<Domain> {
        let index = self.test_index.load();
        index.get_by_codomain(codomain)
    }
}
impl<Domain, Codomain, Source> Relation<Domain, Codomain, Source>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
    Source: Provider<Domain, Codomain>,
{
    pub fn stop_provider(&self) -> Result<(), Error> {
        self.source.stop()
    }

    /// Check if a predicate represents a full scan (accepts everything)
    /// We can detect this by testing with dummy values, but for now we'll use a simpler approach
    #[cfg(test)]
    #[allow(dead_code)]
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

    use crate::tx::Tx;
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestDomain(u64);

    impl std::fmt::Display for TestDomain {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TestDomain({})", self.0)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestCodomain(u64);
    impl RelationCodomain for TestCodomain {}

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

        let tx = Tx {
            ts: Timestamp(1),
            snapshot_version: 0,
        };
        let mut lc = relation.clone().start(&tx);
        lc.insert(domain.clone(), codomain.clone()).unwrap();
        assert_eq!(lc.get(&domain).unwrap(), Some(codomain.clone()));
        assert_eq!(lc.get(&TestDomain(0)).unwrap(), Some(TestCodomain(0)));
        let mut ws = lc.working_set().unwrap();

        let mut cr = relation.begin_check();
        cr.check(&mut ws).unwrap();
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

        let tx_a = Tx {
            ts: Timestamp(0),
            snapshot_version: 0,
        };
        let tx_b = Tx {
            ts: Timestamp(1),
            snapshot_version: 0,
        };

        let mut r_tx_a = relation.clone().start(&tx_a);

        r_tx_a.insert(domain.clone(), codomain_a).unwrap();
        let mut r_tx_b = relation.clone().start(&tx_b);
        r_tx_b.insert(domain.clone(), codomain_b).unwrap();
        let mut ws_a = r_tx_a.working_set().unwrap();
        let mut ws_b = r_tx_b.working_set().unwrap();
        {
            let mut cr_a = relation.begin_check();
            cr_a.check(&mut ws_a).unwrap();
            cr_a.apply(ws_a).unwrap();
            cr_a.commit(relation.index());
        }
        {
            let mut cr_b = relation.begin_check();

            // This should fail because the first insert has already happened.
            let check_result = cr_b.check(&mut ws_b);
            assert!(matches!(check_result, Err(Error::Conflict(_))));
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

        let tx_1 = Tx {
            ts: Timestamp(10),
            snapshot_version: 0,
        };
        let tx_2 = Tx {
            ts: Timestamp(20),
            snapshot_version: 0,
        };

        // T1 reads the value
        let mut r_tx_1 = relation.clone().start(&tx_1);
        let initial_value = r_tx_1.get(&domain).unwrap().unwrap();
        assert_eq!(initial_value, TestCodomain(10));

        // T2 updates the value
        let mut r_tx_2 = relation.clone().start(&tx_2);
        r_tx_2.update(&domain, TestCodomain(20)).unwrap();
        let mut ws_2 = r_tx_2.working_set().unwrap();

        // Commit T2 first
        {
            let mut cr_2 = relation.begin_check();
            cr_2.check(&mut ws_2).unwrap();
            cr_2.apply(ws_2).unwrap();
            cr_2.commit(relation.index());
        }

        // Now T1 tries to update based on its read - conflict should be detected during check.
        r_tx_1.update(&domain, TestCodomain(11)).unwrap();
        let mut ws_1 = r_tx_1.working_set().unwrap();
        let mut cr_1 = relation.begin_check();
        let check_result = cr_1.check(&mut ws_1);
        assert!(matches!(check_result, Err(Error::Conflict(_))));
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

        let tx_1 = Tx {
            ts: Timestamp(10),
            snapshot_version: 0,
        };
        let tx_2 = Tx {
            ts: Timestamp(20),
            snapshot_version: 0,
        };

        // Both transactions update the same key
        let mut r_tx_1 = relation.clone().start(&tx_1);
        let mut r_tx_2 = relation.clone().start(&tx_2);

        r_tx_1.update(&domain, TestCodomain(100)).unwrap();
        r_tx_2.update(&domain, TestCodomain(200)).unwrap();

        let mut ws_1 = r_tx_1.working_set().unwrap();
        let mut ws_2 = r_tx_2.working_set().unwrap();

        // Commit T1 first
        {
            let mut cr_1 = relation.begin_check();
            cr_1.check(&mut ws_1).unwrap();
            cr_1.apply(ws_1).unwrap();
            cr_1.commit(relation.index());
        }

        // T2 should conflict
        let mut cr_2 = relation.begin_check();
        let check_result = cr_2.check(&mut ws_2);
        assert!(matches!(check_result, Err(Error::Conflict(_))));
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

        let tx_1 = Tx {
            ts: Timestamp(10),
            snapshot_version: 0,
        };
        let tx_2 = Tx {
            ts: Timestamp(20),
            snapshot_version: 0,
        };

        // T1 scans for all entries (finds none)
        let mut r_tx_1 = relation.clone().start(&tx_1);
        let scan_result = r_tx_1.scan(&|_, _| true).unwrap();
        assert_eq!(scan_result.len(), 0);

        // T2 inserts a new entry
        let mut r_tx_2 = relation.clone().start(&tx_2);
        r_tx_2.insert(domain_1.clone(), TestCodomain(100)).unwrap();
        let mut ws_2 = r_tx_2.working_set().unwrap();

        // Commit T2
        {
            let mut cr_2 = relation.begin_check();
            cr_2.check(&mut ws_2).unwrap();
            cr_2.apply(ws_2).unwrap();
            cr_2.commit(relation.index());
        }

        // T1 now inserts another entry - this should succeed since it's a different key
        r_tx_1.insert(domain_2.clone(), TestCodomain(200)).unwrap();
        let mut ws_1 = r_tx_1.working_set().unwrap();

        let mut cr_1 = relation.begin_check();
        cr_1.check(&mut ws_1).unwrap(); // Should not conflict
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

        let tx_1 = Tx {
            ts: Timestamp(10),
            snapshot_version: 0,
        };
        let tx_2 = Tx {
            ts: Timestamp(20),
            snapshot_version: 0,
        };

        // T1 deletes the entry
        let mut r_tx_1 = relation.clone().start(&tx_1);
        r_tx_1.delete(&domain).unwrap();
        let mut ws_1 = r_tx_1.working_set().unwrap();

        // Commit T1 first
        {
            let mut cr_1 = relation.begin_check();
            cr_1.check(&mut ws_1).unwrap();
            cr_1.apply(ws_1).unwrap();
            cr_1.commit(relation.index());
        }

        // Now T2 tries to insert the same key - should succeed since key was deleted
        let mut r_tx_2 = relation.clone().start(&tx_2);
        r_tx_2.insert(domain.clone(), TestCodomain(20)).unwrap();
        let mut ws_2 = r_tx_2.working_set().unwrap();

        let mut cr_2 = relation.begin_check();
        cr_2.check(&mut ws_2).unwrap();
        cr_2.apply(ws_2).unwrap();

        // Verify final state
        assert_eq!(relation.get(&domain).unwrap().unwrap().1, TestCodomain(20));
    }

    #[test]
    fn test_delete_then_insert_same_tx_succeeds() {
        let mut backing = HashMap::new();
        backing.insert(TestDomain(1), TestCodomain(10));
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });
        let relation = Arc::new(Relation::new(Symbol::mk("test"), provider));

        let domain = TestDomain(1);
        let tx = Tx {
            ts: Timestamp(10),
            snapshot_version: 0,
        };

        let mut r_tx = relation.clone().start(&tx);
        assert_eq!(r_tx.delete(&domain).unwrap(), Some(TestCodomain(10)));
        r_tx.insert(domain.clone(), TestCodomain(20)).unwrap();

        let mut ws = r_tx.working_set().unwrap();
        let mut cr = relation.begin_check();
        cr.check(&mut ws).unwrap();
        cr.apply(ws).unwrap();
        cr.commit(relation.index());

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
        let tx = Tx {
            ts: Timestamp(10),
            snapshot_version: 0,
        };

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
            let tx = Tx {
                ts: Timestamp(i),
                snapshot_version: 0,
            };
            let mut r_tx = relation.clone().start(&tx);

            // Read current value and increment it
            let current = r_tx.get(&domain).unwrap().unwrap();
            r_tx.update(&domain, TestCodomain(current.0 + 1)).unwrap();

            let mut ws = r_tx.working_set().unwrap();
            let mut cr = relation.begin_check();
            cr.check(&mut ws).unwrap();
            cr.apply(ws).unwrap();
            cr.commit(relation.index());
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

        let tx_1 = Tx {
            ts: Timestamp(10),
            snapshot_version: 0,
        };
        let tx_2 = Tx {
            ts: Timestamp(20),
            snapshot_version: 0,
        };
        let tx_3 = Tx {
            ts: Timestamp(30),
            snapshot_version: 0,
        };

        // T1: Update existing key and insert new key
        let mut r_tx_1 = relation.clone().start(&tx_1);
        r_tx_1.update(&TestDomain(1), TestCodomain(200)).unwrap();
        r_tx_1.insert(TestDomain(2), TestCodomain(300)).unwrap();
        let mut ws_1 = r_tx_1.working_set().unwrap();

        // T2: Try to update the same key as T1 but to different value
        let mut r_tx_2 = relation.clone().start(&tx_2);
        r_tx_2.update(&TestDomain(1), TestCodomain(400)).unwrap();
        r_tx_2.insert(TestDomain(3), TestCodomain(500)).unwrap();
        let mut ws_2 = r_tx_2.working_set().unwrap();

        // Commit T1 first
        {
            let mut cr_1 = relation.begin_check();
            cr_1.check(&mut ws_1).unwrap();
            cr_1.apply(ws_1).unwrap();
            cr_1.commit(relation.index());
        }

        // T2 should conflict because it tries to update what T1 already updated
        {
            let mut cr_2 = relation.begin_check();
            let check_result = cr_2.check(&mut ws_2);
            assert!(matches!(check_result, Err(Error::Conflict(_))));
        }

        // T3: Should be able to read T1's committed changes and make updates
        let mut r_tx_3 = relation.clone().start(&tx_3);
        let current_val = r_tx_3.get(&TestDomain(1)).unwrap().unwrap();
        assert_eq!(current_val, TestCodomain(200)); // Should see T1's update
        r_tx_3
            .update(&TestDomain(1), TestCodomain(current_val.0 + 100))
            .unwrap();
        let mut ws_3 = r_tx_3.working_set().unwrap();

        {
            let mut cr_3 = relation.begin_check();
            cr_3.check(&mut ws_3).unwrap();
            cr_3.apply(ws_3).unwrap();
            cr_3.commit(relation.index());
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
        let tx_older = Tx {
            ts: Timestamp(10),
            snapshot_version: 0,
        };
        let tx_newer = Tx {
            ts: Timestamp(20),
            snapshot_version: 0,
        };

        let mut r_tx_older = relation.clone().start(&tx_older);
        let _old_val = r_tx_older.get(&domain).unwrap().unwrap(); // Read with ts=10

        // Newer transaction commits first
        let mut r_tx_newer = relation.clone().start(&tx_newer);
        r_tx_newer.update(&domain, TestCodomain(200)).unwrap();
        let mut ws_newer = r_tx_newer.working_set().unwrap();

        {
            let mut cr_newer = relation.begin_check();
            cr_newer.check(&mut ws_newer).unwrap();
            cr_newer.apply(ws_newer).unwrap();
            cr_newer.commit(relation.index());
        }

        // Now older transaction tries to update - conflict should be detected during check.
        r_tx_older.update(&domain, TestCodomain(300)).unwrap();
        let mut ws_older = r_tx_older.working_set().unwrap();
        let mut cr_older = relation.begin_check();
        let check_result = cr_older.check(&mut ws_older);
        assert!(matches!(check_result, Err(Error::Conflict(_))));
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

        let tx = Tx {
            ts: Timestamp(10),
            snapshot_version: 0,
        };
        let mut r_tx = relation.clone().start(&tx);

        // Read both keys - they should be consistent
        let val1 = r_tx.get(&TestDomain(1)).unwrap().unwrap();
        let val2 = r_tx.get(&TestDomain(2)).unwrap().unwrap();

        assert_eq!(val1, TestCodomain(100));
        assert_eq!(val2, TestCodomain(200));

        // Update one key based on both reads
        r_tx.update(&TestDomain(1), TestCodomain(val1.0 + val2.0))
            .unwrap();

        let mut ws = r_tx.working_set().unwrap();
        let mut cr = relation.begin_check();
        cr.check(&mut ws).unwrap();
        cr.apply(ws).unwrap();

        // Commit the changes to the relation
        cr.commit(relation.index());

        // Verify the update was applied correctly
        assert_eq!(
            relation.get(&TestDomain(1)).unwrap().unwrap().1,
            TestCodomain(300)
        );
    }

    #[test]
    fn test_secondary_index_transaction_integration() {
        use crate::tx::indexes::SecondaryIndexRelation;

        let backing = HashMap::new();
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });

        // Create relation with secondary index support
        let relation = Arc::new(Relation {
            relation_name: Symbol::mk("test"),
            index_factory: secondary_index_factory::<TestDomain, TestCodomain>,
            test_index: Arc::new(ArcSwap::new(Arc::new(Box::new(
                SecondaryIndexRelation::new(),
            )))),
            source: provider,
        });

        let domain1 = TestDomain(1);
        let domain2 = TestDomain(2);
        let domain3 = TestDomain(3);
        let codomain_a = TestCodomain(100);
        let codomain_b = TestCodomain(200);

        let tx = Tx {
            ts: Timestamp(10),
            snapshot_version: 0,
        };
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
        let mut ws = r_tx.working_set().unwrap();
        let mut cr = relation.begin_check();
        cr.check(&mut ws).unwrap();
        cr.apply(ws).unwrap();
        cr.commit(relation.index());

        // Test that committed secondary index state is visible in new transaction
        let tx2 = Tx {
            ts: Timestamp(20),
            snapshot_version: 0,
        };
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
