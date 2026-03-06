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

use crate::{
    api::world_state::db_counters,
    provider::Provider,
    tx::{ConflictInfo, ConflictType, Error, RelationCodomain, RelationDomain, Timestamp},
};
use moor_common::util::Instant;
use moor_var::Symbol;
use std::{sync::Arc, time::Duration};
use tracing::warn;

use super::{
    indexes::RelationIndex,
    resolve::{ConflictResolver, Resolution},
    transaction::{Op, OpType, WorkingSet},
};

/// Represents a detected conflict during transaction commit that may be resolvable.
///
/// Contains all information needed for 3-way merge conflict resolution:
/// - **base**: What we saw when the transaction started ("where I came from")
/// - **theirs**: What's currently in canonical state ("what replaced where I came from")
/// - **mine**: What we want to write ("my value")
#[derive(Debug, Clone)]
pub struct PotentialConflict<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    /// The conflict metadata (relation name, domain key string, conflict type)
    pub info: ConflictInfo,
    /// The domain (key) that is in conflict
    pub domain: Domain,
    /// BASE: The value at transaction start - "where I came from" (None if didn't exist)
    pub base: Option<(Timestamp, Codomain)>,
    /// THEIRS: The current canonical value - "what replaced where I came from" (None if doesn't exist)
    pub theirs: Option<(Timestamp, Codomain)>,
    /// MINE: Our proposed operation that caused the conflict
    pub mine: ProposedOp<Codomain>,
    /// The timestamp we read at
    pub read_ts: Timestamp,
    /// The timestamp we're trying to write at
    pub write_ts: Timestamp,
}

/// The operation we're proposing that conflicts with canonical state.
#[derive(Debug, Clone)]
pub enum ProposedOp<Codomain>
where
    Codomain: RelationCodomain,
{
    /// We want to insert this value, but something already exists
    Insert(Codomain),
    /// We want to update to this value, but canonical has changed
    Update(Codomain),
    /// We want to delete, but canonical has changed
    Delete,
}

impl<Codomain: RelationCodomain> ProposedOp<Codomain> {
    /// Extract the proposed value, if any
    pub fn value(&self) -> Option<&Codomain> {
        match self {
            ProposedOp::Insert(v) | ProposedOp::Update(v) => Some(v),
            ProposedOp::Delete => None,
        }
    }
}

pub struct CheckRelation<Domain, Codomain, P>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
    P: Provider<Domain, Codomain>,
{
    pub(crate) index: Box<dyn RelationIndex<Domain, Codomain>>,
    pub(crate) relation_name: Symbol,
    pub(crate) source: Arc<P>,
    pub(crate) dirty: bool,
}

impl<Domain, Codomain, P> CheckRelation<Domain, Codomain, P>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
    P: Provider<Domain, Codomain>,
{
    pub fn num_entries(&self) -> usize {
        self.index.len()
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// Returns the name of this relation (for conflict reporting).
    pub fn relation_name(&self) -> Symbol {
        self.relation_name
    }

    /// Helper to create a ConflictInfo for this relation.
    fn make_conflict_info(&self, domain: &Domain, conflict_type: ConflictType) -> ConflictInfo {
        ConflictInfo {
            relation_name: self.relation_name,
            domain_key: format!("{}", domain),
            conflict_type,
        }
    }

    /// Check the forked index for conflicts with the given working set.
    /// Operates on a lock-free snapshot, so does not block concurrent transaction starts.
    /// This is the first phase of transaction commit, and does not mutate the contents of
    /// the canonical index.
    ///
    /// By default, accepts conflicts where both transactions wrote identical values
    /// (no real conflict) AND attempts smart merging for supported types.
    /// For custom resolution, use `check_with_resolver`.
    pub fn check(&mut self, working_set: &mut WorkingSet<Domain, Codomain>) -> Result<(), Error> {
        self.check_with_smart_merge(working_set)
    }

    fn try_resolve_smart_merge(
        &self,
        conflict_type: ConflictType,
        domain: &Domain,
        base: Option<&Codomain>,
        theirs: Option<&Codomain>,
        op: &mut Op<Codomain>,
        rewrite_op: impl FnOnce(Codomain) -> OpType<Codomain>,
    ) -> Result<(), Error> {
        let counters = db_counters();

        let identical = match (theirs, &op.operation) {
            (Some(theirs_val), OpType::Insert(mine_val) | OpType::Update(mine_val)) => {
                theirs_val == mine_val
            }
            (None, OpType::Delete) => true,
            _ => false,
        };

        if identical {
            counters.crdt_resolve_success.invocations().add(1);
            return Ok(());
        }

        if let Some(base_val) = base
            && let Some(theirs_val) = theirs
            && let OpType::Insert(mine_val) | OpType::Update(mine_val) = &op.operation
            && let Some(merged) = mine_val.try_merge(base_val, theirs_val)
        {
            counters.crdt_resolve_success.invocations().add(1);
            op.operation = rewrite_op(merged);
            return Ok(());
        }

        counters.crdt_resolve_fail.invocations().add(1);
        Err(Error::Conflict(
            self.make_conflict_info(domain, conflict_type),
        ))
    }

    fn check_with_smart_merge(
        &mut self,
        working_set: &mut WorkingSet<Domain, Codomain>,
    ) -> Result<(), Error> {
        let start_time = Instant::now();
        let mut last_check_time = start_time;
        let total_ops = working_set.len();
        self.dirty = !working_set.is_empty();

        let (tuples, base_index) = working_set.parts_mut();
        for (n, (domain, op)) in tuples.iter_mut().enumerate() {
            if (n & 1023) == 0 && last_check_time.elapsed() > Duration::from_secs(5) {
                warn!(
                    "Long check time for {}; running for {}s; {n}/{total_ops} checked",
                    self.relation_name,
                    start_time.elapsed().as_secs_f32()
                );
                last_check_time = Instant::now();
            }

            if op.guaranteed_unique {
                continue;
            }

            if let Some(local_entry) = self.index.index_lookup(domain) {
                let theirs = Some(&local_entry.value);
                if op.operation.is_insert() {
                    self.try_resolve_smart_merge(
                        ConflictType::InsertDuplicate,
                        domain,
                        base_index.index_lookup(domain).map(|entry| &entry.value),
                        theirs,
                        op,
                        OpType::Insert,
                    )?;
                    continue;
                }

                if local_entry.ts > op.read_ts {
                    self.try_resolve_smart_merge(
                        ConflictType::ConcurrentWrite,
                        domain,
                        base_index.index_lookup(domain).map(|entry| &entry.value),
                        theirs,
                        op,
                        OpType::Update,
                    )?;
                    continue;
                }

                if op.read_ts > op.write_ts {
                    self.try_resolve_smart_merge(
                        ConflictType::StaleRead,
                        domain,
                        base_index.index_lookup(domain).map(|entry| &entry.value),
                        theirs,
                        op,
                        OpType::Update,
                    )?;
                    continue;
                }
                continue;
            }

            if let Some((ts, codomain)) = self.source.get(domain)? {
                self.index
                    .insert_entry(ts, domain.clone(), codomain.clone());
                let theirs = Some(&codomain);

                if op.operation.is_insert() {
                    self.try_resolve_smart_merge(
                        ConflictType::InsertDuplicate,
                        domain,
                        base_index.index_lookup(domain).map(|entry| &entry.value),
                        theirs,
                        op,
                        OpType::Insert,
                    )?;
                    continue;
                }

                if ts > op.read_ts {
                    self.try_resolve_smart_merge(
                        ConflictType::ConcurrentWrite,
                        domain,
                        base_index.index_lookup(domain).map(|entry| &entry.value),
                        theirs,
                        op,
                        OpType::Update,
                    )?;
                }
                continue;
            }

            if op.operation.is_update() {
                self.try_resolve_smart_merge(
                    ConflictType::UpdateNonExistent,
                    domain,
                    base_index.index_lookup(domain).map(|entry| &entry.value),
                    None,
                    op,
                    OpType::Insert,
                )?;
            }
        }
        Ok(())
    }

    /// Check the forked index for conflicts with the given working set, calling
    /// the resolver for each detected conflict.
    ///
    /// This enables conflict resolution algorithms to attempt reconciliation on
    /// each conflict as it's detected. The resolver receives full 3-way merge context:
    /// - **base**: What we saw at transaction start (from working_set.base_value)
    /// - **theirs**: Current canonical state
    /// - **mine**: Our proposed operation
    ///
    /// The resolver can either resolve the conflict (return Ok) or abort (return Err).
    ///
    /// Operates on a lock-free snapshot, so does not block concurrent transaction starts.
    /// This is the first phase of transaction commit, and does not mutate the contents of
    /// the canonical index.
    pub fn check_with_resolver<R>(
        &mut self,
        working_set: &mut WorkingSet<Domain, Codomain>,
        mut resolver: R,
    ) -> Result<(), Error>
    where
        R: ConflictResolver<Domain, Codomain>,
    {
        let start_time = Instant::now();
        let mut last_check_time = start_time;
        let total_ops = working_set.len();
        self.dirty = !working_set.is_empty();

        // Check phase first.
        // We use tuples_mut() because resolution might rewrite the operation
        let (tuples, base_index) = working_set.parts_mut();
        for (n, (domain, op)) in tuples.iter_mut().enumerate() {
            if (n & 1023) == 0 && last_check_time.elapsed() > Duration::from_secs(5) {
                warn!(
                    "Long check time for {}; running for {}s; {n}/{total_ops} checked",
                    self.relation_name,
                    start_time.elapsed().as_secs_f32()
                );
                last_check_time = Instant::now();
            }

            // Skip conflict checking for guaranteed unique operations
            if op.guaranteed_unique {
                continue;
            }

            // Check local to see if we have one first, to see if there's a conflict.
            if let Some(local_entry) = self.index.index_lookup(domain) {
                // If what we have is an insert, and there's something already there, that's a
                // conflict.
                if op.operation.is_insert() {
                    let base = base_index
                        .index_lookup(domain)
                        .map(|e| (e.ts, e.value.clone()));
                    let theirs = Some((local_entry.ts, local_entry.value.clone()));
                    let conflict = self.make_potential_conflict(
                        domain,
                        ConflictType::InsertDuplicate,
                        base,
                        theirs,
                        op,
                    );
                    match resolver.resolve(&conflict)? {
                        Resolution::Accept => continue,
                        Resolution::Rewrite(new_val) => {
                            op.operation = OpType::Insert(new_val);
                            continue;
                        }
                    }
                }

                let ts = local_entry.ts;
                // If the ts there is greater than the read-ts of our own op, that's a conflict
                // Someone got to it first.
                if ts > op.read_ts {
                    let base = base_index
                        .index_lookup(domain)
                        .map(|e| (e.ts, e.value.clone()));
                    let theirs = Some((local_entry.ts, local_entry.value.clone()));
                    let conflict = self.make_potential_conflict(
                        domain,
                        ConflictType::ConcurrentWrite,
                        base,
                        theirs,
                        op,
                    );
                    match resolver.resolve(&conflict)? {
                        Resolution::Accept => continue,
                        Resolution::Rewrite(new_val) => {
                            op.operation = OpType::Update(new_val);
                            continue;
                        }
                    }
                }
                // If the transactions *write stamp* is earlier than the read stamp, that's a
                // conflict indicating that the transaction is trying to update something
                // it should not have read.
                // (This only happens because we're not able to early-bail on update operations
                // like this, so there's some waste here.)
                if op.read_ts > op.write_ts {
                    let base = base_index
                        .index_lookup(domain)
                        .map(|e| (e.ts, e.value.clone()));
                    let theirs = Some((local_entry.ts, local_entry.value.clone()));
                    let conflict = self.make_potential_conflict(
                        domain,
                        ConflictType::StaleRead,
                        base,
                        theirs,
                        op,
                    );
                    match resolver.resolve(&conflict)? {
                        Resolution::Accept => continue,
                        Resolution::Rewrite(new_val) => {
                            // If we rewrite a stale read, we assume the new value is valid for the current state
                            op.operation = OpType::Update(new_val);
                            continue;
                        }
                    }
                }
                continue;
            }

            // Otherwise, pull from upstream and fetch to cache and check for conflict.
            if let Some((ts, codomain)) = self.source.get(domain)? {
                self.index
                    .insert_entry(ts, domain.clone(), codomain.clone());

                // If what we have is an insert, and there's something already there, that's also
                // a conflict.
                if op.operation.is_insert() {
                    let base = base_index
                        .index_lookup(domain)
                        .map(|e| (e.ts, e.value.clone()));
                    let theirs = Some((ts, codomain.clone()));
                    let conflict = self.make_potential_conflict(
                        domain,
                        ConflictType::InsertDuplicate,
                        base,
                        theirs,
                        op,
                    );
                    match resolver.resolve(&conflict)? {
                        Resolution::Accept => continue,
                        Resolution::Rewrite(new_val) => {
                            op.operation = OpType::Insert(new_val);
                            continue;
                        }
                    }
                }
                if ts > op.read_ts {
                    let base = base_index
                        .index_lookup(domain)
                        .map(|e| (e.ts, e.value.clone()));
                    let theirs = Some((ts, codomain));
                    let conflict = self.make_potential_conflict(
                        domain,
                        ConflictType::ConcurrentWrite,
                        base,
                        theirs,
                        op,
                    );
                    match resolver.resolve(&conflict)? {
                        Resolution::Accept => continue,
                        Resolution::Rewrite(new_val) => {
                            op.operation = OpType::Update(new_val);
                            continue;
                        }
                    }
                }
            } else {
                // If upstream doesn't have it, and it's not an insert or delete, that's a conflict, this
                // should not have happened.
                if op.operation.is_update() {
                    let base = base_index
                        .index_lookup(domain)
                        .map(|e| (e.ts, e.value.clone()));
                    let conflict = self.make_potential_conflict(
                        domain,
                        ConflictType::UpdateNonExistent,
                        base,
                        None, // theirs doesn't exist
                        op,
                    );
                    match resolver.resolve(&conflict)? {
                        Resolution::Accept => continue,
                        Resolution::Rewrite(new_val) => {
                            // The tuple does not exist in canonical state, so rewriting an
                            // update here must materialize as an insert.
                            op.operation = OpType::Insert(new_val);
                            continue;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Helper to construct a PotentialConflict from the current state.
    fn make_potential_conflict(
        &self,
        domain: &Domain,
        conflict_type: ConflictType,
        base: Option<(Timestamp, Codomain)>,
        theirs: Option<(Timestamp, Codomain)>,
        op: &Op<Codomain>,
    ) -> PotentialConflict<Domain, Codomain> {
        let mine = match &op.operation {
            OpType::Insert(v) => ProposedOp::Insert(v.clone()),
            OpType::Update(v) => ProposedOp::Update(v.clone()),
            OpType::Delete => ProposedOp::Delete,
        };
        PotentialConflict {
            info: self.make_conflict_info(domain, conflict_type),
            domain: domain.clone(),
            base,
            theirs,
            mine,
            read_ts: op.read_ts,
            write_ts: op.write_ts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx::{
        Tx,
        indexes::HashRelationIndex,
        transaction::{Op, OpType, RelationTransaction},
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

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct MergeCodomain(u64);
    impl RelationCodomain for MergeCodomain {
        fn try_merge(&self, base: &Self, theirs: &Self) -> Option<Self> {
            Some(MergeCodomain(
                self.0.wrapping_add(theirs.0).wrapping_sub(base.0),
            ))
        }
    }

    #[derive(Clone)]
    struct TestProvider {
        data: Arc<Mutex<HashMap<TestDomain, TestCodomain>>>,
    }

    impl Provider<TestDomain, TestCodomain> for TestProvider {
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
    fn test_update_nonexistent_rewrite_becomes_insert() {
        struct RewriteToInsert;
        impl ConflictResolver<TestDomain, TestCodomain> for RewriteToInsert {
            fn resolve(
                &mut self,
                _conflict: &PotentialConflict<TestDomain, TestCodomain>,
            ) -> Result<Resolution<TestCodomain>, Error> {
                Ok(Resolution::Rewrite(TestCodomain(901)))
            }
        }

        let data = Arc::new(Mutex::new(HashMap::new()));
        let provider = Arc::new(TestProvider { data });
        let relation = crate::tx::Relation::new(Symbol::mk("test"), provider);
        let domain = TestDomain(42);
        let mut tuples = HashMap::default();
        tuples.insert(
            domain.clone(),
            Op {
                read_ts: Timestamp(1),
                write_ts: Timestamp(10),
                operation: OpType::Update(TestCodomain(900)),
                guaranteed_unique: false,
            },
        );
        let mut ws = WorkingSet::new(Box::new(tuples), Box::new(HashRelationIndex::new()));
        let mut cr = relation.begin_check_from_index(&HashRelationIndex::new());

        cr.check_with_resolver(&mut ws, RewriteToInsert).unwrap();

        let rewritten = ws.tuples_ref().get(&domain).unwrap();
        assert_eq!(rewritten.operation, OpType::Insert(TestCodomain(901)));
    }

    #[test]
    fn test_check_default_path_merge_rewrites_update() {
        let mut data = HashMap::new();
        let domain = TestDomain(7);
        data.insert(domain.clone(), MergeCodomain(10));

        #[derive(Clone)]
        struct MergeProvider {
            data: Arc<Mutex<HashMap<TestDomain, MergeCodomain>>>,
        }

        impl Provider<TestDomain, MergeCodomain> for MergeProvider {
            fn get(
                &self,
                domain: &TestDomain,
            ) -> Result<Option<(Timestamp, MergeCodomain)>, Error> {
                let data = self.data.lock().unwrap();
                Ok(data.get(domain).cloned().map(|v| (Timestamp(0), v)))
            }

            fn put(
                &self,
                _timestamp: Timestamp,
                _domain: &TestDomain,
                _codomain: &MergeCodomain,
            ) -> Result<(), Error> {
                Ok(())
            }

            fn del(&self, _timestamp: Timestamp, _domain: &TestDomain) -> Result<(), Error> {
                Ok(())
            }

            fn scan<F>(
                &self,
                predicate: &F,
            ) -> Result<Vec<(Timestamp, TestDomain, MergeCodomain)>, Error>
            where
                F: Fn(&TestDomain, &MergeCodomain) -> bool,
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

        let provider = Arc::new(MergeProvider {
            data: Arc::new(Mutex::new(data)),
        });
        let relation = crate::tx::Relation::new(Symbol::mk("merge"), provider);
        let base_index = relation.seeded_index().unwrap();

        let tx = Tx {
            ts: Timestamp(1),
            snapshot_version: 0,
        };
        let mut rt: RelationTransaction<TestDomain, MergeCodomain, _> =
            relation.start_from_index(&tx, base_index.as_ref());
        rt.update(&domain, MergeCodomain(11)).unwrap();
        let mut ws = rt.working_set().unwrap();

        let mut checker_index = base_index.fork();
        checker_index.insert_entry(Timestamp(2), domain.clone(), MergeCodomain(20));
        let mut checker = relation.begin_check_from_index(checker_index.as_ref());
        checker.check(&mut ws).unwrap();

        let rewritten = ws.tuples_ref().get(&domain).unwrap();
        assert_eq!(rewritten.operation, OpType::Update(MergeCodomain(21)));
    }
}
