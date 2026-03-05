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

use crate::{
    api::world_state::db_counters,
    tx::{Error, RelationCodomain, RelationDomain},
};
use arc_swap::ArcSwap;
use minstant::Instant;
use std::sync::Arc;

use super::{
    check::CheckRelation,
    indexes::RelationIndex,
    transaction::{OpType, WorkingSet},
};

impl<Domain, Codomain, P> CheckRelation<Domain, Codomain, P>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
    P: crate::provider::Provider<Domain, Codomain>,
{
    /// Apply the given working set to the cache.
    /// This is the final phase of the transaction commit process, and mutates the cache and
    /// requests mutation into the Source.
    pub fn apply(&mut self, working_set: WorkingSet<Domain, Codomain>) -> Result<(), Error> {
        // Update the provider_fully_loaded state first
        if working_set.provider_fully_loaded() {
            self.index.set_provider_fully_loaded(true);
        }

        // Mark as dirty if we have mutations - critical for commit_all to swap the index
        // This is needed when check() was skipped via the conflict-check optimization
        if !working_set.is_empty() {
            self.dirty = true;
        }

        // Apply phase.
        let counters = db_counters();
        let total_ops = working_set.len();
        let mut inserts = Vec::with_capacity(total_ops);
        let mut tombstones = Vec::new();

        for (domain, op) in working_set.tuples().into_iter() {
            match op.operation {
                OpType::Insert(codomain) | OpType::Update(codomain) => {
                    self.source.put(op.write_ts, &domain, &codomain).ok();
                    inserts.push((op.write_ts, domain, codomain));
                }
                OpType::Delete => {
                    self.source.del(op.write_ts, &domain).unwrap();
                    tombstones.push((op.write_ts, domain));
                }
            }
        }

        let index_ops = inserts.len() + tombstones.len();
        if index_ops > 0 {
            let start = Instant::now();
            self.index.apply_batch(inserts, tombstones);
            let elapsed_nanos = isize::try_from(start.elapsed().as_nanos()).unwrap_or(isize::MAX);
            let invocation_count = isize::try_from(index_ops).unwrap_or(isize::MAX);
            counters
                .apply_index_insert
                .invocations()
                .add(invocation_count);
            counters
                .apply_index_insert
                .cumulative_duration_nanos()
                .add(elapsed_nanos);
        }
        Ok(())
    }

    pub fn commit(self, index_swap: &Arc<ArcSwap<Box<dyn RelationIndex<Domain, Codomain>>>>) {
        index_swap.store(Arc::new(self.index));
    }

    pub fn committed_index_or(
        self,
        existing: Arc<dyn RelationIndex<Domain, Codomain>>,
    ) -> Arc<dyn RelationIndex<Domain, Codomain>> {
        if self.dirty {
            return Arc::from(self.index);
        }
        existing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx::{Timestamp, Tx};
    use crate::tx::{
        indexes::HashRelationIndex,
        transaction::{Op, OpType},
    };
    use moor_var::Symbol;
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

    impl crate::provider::Provider<TestDomain, TestCodomain> for TestProvider {
        fn get(&self, domain: &TestDomain) -> Result<Option<(Timestamp, TestCodomain)>, Error> {
            let data = self.data.lock().unwrap();
            Ok(data.get(domain).cloned().map(|v| (Timestamp(0), v)))
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
    fn test_apply_writes_provider_and_marks_dirty() {
        let provider = Arc::new(TestProvider {
            data: Arc::new(Mutex::new(HashMap::new())),
        });
        let relation = crate::tx::Relation::new(Symbol::mk("test"), provider.clone());
        let tx = Tx {
            ts: Timestamp(10),
            snapshot_version: 0,
        };

        let mut tuples = HashMap::default();
        tuples.insert(
            TestDomain(1),
            Op {
                read_ts: tx.ts,
                write_ts: tx.ts,
                operation: OpType::Insert(TestCodomain(99)),
                guaranteed_unique: false,
            },
        );
        let ws = WorkingSet::new(Box::new(tuples), Box::new(HashRelationIndex::new()));
        let mut cr = relation.begin_check_from_index(&HashRelationIndex::new());

        cr.apply(ws).unwrap();

        assert!(cr.dirty());
        assert_eq!(
            provider
                .data
                .lock()
                .unwrap()
                .get(&TestDomain(1))
                .cloned()
                .unwrap(),
            TestCodomain(99)
        );
    }
}
