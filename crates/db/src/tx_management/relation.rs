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
    tx_management::{
        Canonical, ConflictInfo, ConflictType, Error, RelationCodomain, RelationCodomainHashable,
        RelationDomain, Timestamp, Tx,
        indexes::{HashRelationIndex, RelationIndex},
        relation_tx::{Op, OpType, RelationTransaction, WorkingSet},
    },
};
use arc_swap::ArcSwap;
use minstant::Instant;
use moor_common::util::PerfTimerGuard;
use moor_var::Symbol;
use std::{sync::Arc, time::Duration};
use tracing::warn;

use crate::db_worldstate::db_counters;

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

/// The result of a conflict resolution attempt.
pub enum Resolution<Codomain> {
    /// Accept the proposed operation as-is (continue checking)
    Accept,
    /// Rewrite the proposed operation with a new value
    Rewrite(Codomain),
}

/// Trait for conflict resolution strategies.
///
/// Implement this to provide custom conflict resolution logic. The resolver
/// is called for each detected conflict and can either:
/// - Return `Ok(Resolution::Accept)` to accept/resolve the conflict and continue checking
/// - Return `Ok(Resolution::Rewrite(new_val))` to resolve by changing the written value
/// - Return `Err(Error::Conflict(...))` to abort with that conflict
pub trait ConflictResolver<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    /// Attempt to resolve a conflict.
    ///
    /// Called when a conflict is detected during the check phase.
    fn resolve(
        &mut self,
        conflict: &PotentialConflict<Domain, Codomain>,
    ) -> Result<Resolution<Codomain>, Error>;
}

/// A resolver that always fails on conflict (the default behavior).
pub struct FailOnConflict;

impl<Domain, Codomain> ConflictResolver<Domain, Codomain> for FailOnConflict
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    fn resolve(
        &mut self,
        conflict: &PotentialConflict<Domain, Codomain>,
    ) -> Result<Resolution<Codomain>, Error> {
        Err(Error::Conflict(conflict.info.clone()))
    }
}

/// A resolver that accepts conflicts where theirs == mine (identical values).
/// If both transactions wrote the same value, there's no real conflict.
pub struct AcceptIdentical;

impl<Domain, Codomain> ConflictResolver<Domain, Codomain> for AcceptIdentical
where
    Domain: RelationDomain,
    Codomain: RelationCodomain, // PartialEq is already required by RelationCodomain
{
    fn resolve(
        &mut self,
        conflict: &PotentialConflict<Domain, Codomain>,
    ) -> Result<Resolution<Codomain>, Error> {
        // Check if theirs == mine (using PartialEq)
        let identical = match (&conflict.theirs, &conflict.mine) {
            // Both have values - compare them
            (
                Some((_, theirs_val)),
                ProposedOp::Insert(mine_val) | ProposedOp::Update(mine_val),
            ) => theirs_val == mine_val,
            // Both are effectively "no value" (theirs doesn't exist, we're deleting)
            (None, ProposedOp::Delete) => true,
            // Otherwise, real conflict
            _ => false,
        };

        if identical {
            Ok(Resolution::Accept)
        } else {
            Err(Error::Conflict(conflict.info.clone()))
        }
    }
}

/// A resolver that attempts smart merges using RelationCodomain::try_merge.
/// If merging fails, it falls back to AcceptIdentical logic.
pub struct SmartMergeResolver;

impl<Domain, Codomain> ConflictResolver<Domain, Codomain> for SmartMergeResolver
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    fn resolve(
        &mut self,
        conflict: &PotentialConflict<Domain, Codomain>,
    ) -> Result<Resolution<Codomain>, Error> {
        // 1. Try AcceptIdentical logic (idempotency)
        let identical = match (&conflict.theirs, &conflict.mine) {
            (
                Some((_, theirs_val)),
                ProposedOp::Insert(mine_val) | ProposedOp::Update(mine_val),
            ) => theirs_val == mine_val,
            (None, ProposedOp::Delete) => true,
            _ => false,
        };

        if identical {
            return Ok(Resolution::Accept);
        }

        // 2. Try Smart Merge (CRDT-like)
        // Only applicable if we have Base, Theirs, and Mine (Update/Insert conflict)
        if let Some((_, base_val)) = &conflict.base
            && let Some((_, theirs_val)) = &conflict.theirs
            && let Some(mine_val) = conflict.mine.value()
        {
            if let Some(merged) = mine_val.try_merge(base_val, theirs_val) {
                return Ok(Resolution::Rewrite(merged));
            }
        }

        // 3. Fail
        Err(Error::Conflict(conflict.info.clone()))
    }
}

/// Implement ConflictResolver for closures for convenience.
impl<Domain, Codomain, F> ConflictResolver<Domain, Codomain> for F
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
    F: FnMut(&PotentialConflict<Domain, Codomain>) -> Result<Resolution<Codomain>, Error>,
{
    fn resolve(
        &mut self,
        conflict: &PotentialConflict<Domain, Codomain>,
    ) -> Result<Resolution<Codomain>, Error> {
        self(conflict)
    }
}

/// Represents the current "canonical" state of a relation.
#[derive(Clone)]
pub struct Relation<Domain, Codomain, Source>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    index: Arc<ArcSwap<Box<dyn RelationIndex<Domain, Codomain>>>>,
    relation_name: Symbol,
    source: Arc<Source>,
}

pub struct CheckRelation<Domain, Codomain, P>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
    P: Provider<Domain, Codomain>,
{
    index: Box<dyn RelationIndex<Domain, Codomain>>,
    relation_name: Symbol,
    source: Arc<P>,
    dirty: bool,
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
        self.check_with_resolver(working_set, SmartMergeResolver)
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
            if last_check_time.elapsed() > Duration::from_secs(5) {
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

            // Look up the base value (what we saw at transaction start) for 3-way merge
            let base = base_index
                .index_lookup(domain)
                .map(|e| (e.ts, e.value.clone()));

            // Check local to see if we have one first, to see if there's a conflict.
            if let Some(local_entry) = self.index.index_lookup(domain) {
                let theirs = Some((local_entry.ts, local_entry.value.clone()));

                // If what we have is an insert, and there's something already there, that's a
                // conflict.
                if op.operation.is_insert() {
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

                let theirs = Some((ts, codomain));

                // If what we have is an insert, and there's something already there, that's also
                // a conflict.
                if op.operation.is_insert() {
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
                            // If we rewrite an update on non-existent, it becomes an insert?
                            // Or we assume the resolver knows what it's doing.
                            // If theirs is None, Update is invalid. If resolver says Rewrite, maybe they want Insert?
                            // But OpType::Update implies we thought it existed.
                            // Let's assume Update -> Update(new_val) is what was asked, but really this is weird.
                            // If it doesn't exist, we can't update it. We should probably Insert it.
                            // But OpType doesn't track this semantic well.
                            // Let's just update the value.
                            op.operation = OpType::Update(new_val);
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
        for (domain, op) in working_set.tuples().into_iter() {
            match op.operation {
                OpType::Insert(codomain) | OpType::Update(codomain) => {
                    {
                        let _t = PerfTimerGuard::new(&counters.apply_source_put);
                        self.source.put(op.write_ts, &domain, &codomain).ok();
                    }
                    {
                        let _t = PerfTimerGuard::new(&counters.apply_index_insert);
                        self.index
                            .insert_entry(op.write_ts, domain.clone(), codomain);
                    }
                }
                OpType::Delete => {
                    {
                        let _t = PerfTimerGuard::new(&counters.apply_index_insert);
                        self.index.insert_tombstone(op.write_ts, domain.clone());
                    }
                    {
                        let _t = PerfTimerGuard::new(&counters.apply_source_put);
                        self.source.del(op.write_ts, &domain).unwrap();
                    }
                }
            }
        }
        Ok(())
    }

    pub fn commit(self, index_swap: &Arc<ArcSwap<Box<dyn RelationIndex<Domain, Codomain>>>>) {
        index_swap.store(Arc::new(self.index));
    }
}

impl<Domain, Codomain, Source> Relation<Domain, Codomain, Source>
where
    Source: Provider<Domain, Codomain>,
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    pub fn new(relation_name: Symbol, source: Arc<Source>) -> Self {
        Self {
            index: Arc::new(ArcSwap::new(Arc::new(Box::new(HashRelationIndex::new())))),
            relation_name,
            source,
        }
    }

    pub fn new_with_secondary(relation_name: Symbol, source: Arc<Source>) -> Self
    where
        Codomain: RelationCodomainHashable,
    {
        Self {
            index: Arc::new(ArcSwap::new(Arc::new(Box::new(
                crate::tx_management::indexes::SecondaryIndexRelation::new(),
            )))),
            relation_name,
            source,
        }
    }

    pub fn index(&self) -> &Arc<ArcSwap<Box<dyn RelationIndex<Domain, Codomain>>>> {
        &self.index
    }

    pub fn source(&self) -> &Arc<Source> {
        &self.source
    }

    pub fn start(&self, tx: &Tx) -> RelationTransaction<Domain, Codomain, Self> {
        let index = self.index.load();
        RelationTransaction::new(*tx, self.relation_name, (**index).fork(), self.clone())
    }

    pub fn begin_check(&self) -> CheckRelation<Domain, Codomain, Source> {
        let index = self.index.load();
        CheckRelation {
            index: (**index).fork(),
            relation_name: self.relation_name,
            source: self.source.clone(),
            dirty: false,
        }
    }

    /// Mark this relation as fully loaded from its backing provider.
    /// After this call, scans will skip provider I/O and use only cached data.
    pub fn mark_fully_loaded(&self) {
        let index = self.index.load();
        let mut new_index = (**index).fork();
        new_index.set_provider_fully_loaded(true);
        self.index.store(Arc::new(new_index));
    }
}

impl<Domain, Codomain, Source> Canonical<Domain, Codomain> for Relation<Domain, Codomain, Source>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
    Source: Provider<Domain, Codomain>,
{
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error> {
        // Try read path first
        let index = self.index.load();
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
            self.index.store(Arc::new(new_index));
            Ok(Some((ts, codomain)))
        } else {
            Ok(None)
        }
    }

    fn scan<F>(&self, predicate: &F) -> Result<Vec<(Timestamp, Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        let index = self.index.load();

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

        self.index.store(Arc::new(new_index));
        Ok(results)
    }

    fn get_by_codomain(&self, codomain: &Codomain) -> Vec<Domain> {
        let index = self.index.load();
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

        // Now T1 tries to update based on its read - this should conflict
        let check_result = r_tx_1.update(&domain, TestCodomain(11));
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

        // Now older transaction tries to update - should conflict due to newer timestamp in cache
        let check_result = r_tx_older.update(&domain, TestCodomain(300));
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
        use crate::tx_management::indexes::SecondaryIndexRelation;

        let backing = HashMap::new();
        let data = Arc::new(Mutex::new(backing));
        let provider = Arc::new(TestProvider { data });

        // Create relation with secondary index support
        let relation = Arc::new(Relation {
            relation_name: Symbol::mk("test"),
            index: Arc::new(ArcSwap::new(Arc::new(Box::new(
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
