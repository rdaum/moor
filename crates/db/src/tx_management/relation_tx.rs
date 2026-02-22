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

use crate::tx_management::{
    Canonical, ConflictInfo, ConflictType, Error, RelationCodomain, RelationDomain, Timestamp, Tx,
    indexes::RelationIndex,
};
use ahash::AHasher;
use moor_common::model::WorldStateError;
use moor_var::Symbol;
use std::collections::HashSet;
use std::{collections::HashMap, hash::BuildHasherDefault, sync::Arc};

/// A key-value caching store that is scoped for the lifetime of a transaction.
/// When the transaction is completed, it collapses into a WorkingSet which can be applied to the
/// global transactional cache.
pub struct RelationTransaction<Domain, Codomain, Source>
where
    Source: Canonical<Domain, Codomain>,
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    tx: Tx,
    relation_name: Symbol,

    // Note: This is RefCell for interior mutability since even get/scan operations can modify the
    //   index.
    index: Inner<Domain, Codomain>,
    backing_source: Arc<Source>,
}

struct Inner<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    local_operations: HashMap<Domain, Op<Codomain>, BuildHasherDefault<AHasher>>,
    master_entries: Box<dyn RelationIndex<Domain, Codomain>>,
    provider_fully_loaded: bool,
    has_local_mutations: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum OpType<Codomain>
where
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    /// We wish to insert a tuple into the master index for this relation.
    Insert(Codomain),
    /// We wish to update a tuple in the master index for this relation.
    Update(Codomain),
    /// We wish to delete a tuple from the master index for this relation.
    Delete,
}

impl<Codomain> OpType<Codomain>
where
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    pub fn is_insert(&self) -> bool {
        matches!(self, OpType::Insert(_))
    }

    pub fn is_update(&self) -> bool {
        matches!(self, OpType::Update(_))
    }

    pub fn is_delete(&self) -> bool {
        matches!(self, OpType::Delete)
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Op<Codomain>
where
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    pub(crate) read_ts: Timestamp,
    pub(crate) write_ts: Timestamp,
    pub(crate) operation: OpType<Codomain>,
    /// If true, this operation is guaranteed to be unique and can skip conflict checking.
    /// Used for optimizing anonymous object creation and similar operations.
    pub(crate) guaranteed_unique: bool,
}

/// Alias for the internal map of operations in a working set.
pub type WorkingSetTuples<Domain, Codomain> =
    HashMap<Domain, Op<Codomain>, BuildHasherDefault<AHasher>>;

pub struct WorkingSet<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    tuples: Box<WorkingSetTuples<Domain, Codomain>>,
    /// The base index - a snapshot of the canonical state when the transaction started.
    /// Used for 3-way merge during conflict resolution:
    /// - base (this) = what we saw at transaction start
    /// - mine = our operation in tuples
    /// - theirs = current canonical state at commit time
    base_index: Box<dyn RelationIndex<Domain, Codomain>>,
    provider_fully_loaded: bool,
}

impl<Domain, Codomain> WorkingSet<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    pub fn new(
        tuples: Box<WorkingSetTuples<Domain, Codomain>>,
        base_index: Box<dyn RelationIndex<Domain, Codomain>>,
    ) -> WorkingSet<Domain, Codomain> {
        WorkingSet {
            tuples,
            base_index,
            provider_fully_loaded: false,
        }
    }

    pub fn new_with_fully_loaded(
        tuples: Box<WorkingSetTuples<Domain, Codomain>>,
        base_index: Box<dyn RelationIndex<Domain, Codomain>>,
        provider_fully_loaded: bool,
    ) -> WorkingSet<Domain, Codomain> {
        WorkingSet {
            tuples,
            base_index,
            provider_fully_loaded,
        }
    }

    pub fn len(&self) -> usize {
        self.tuples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tuples.is_empty()
    }

    pub fn tuples(self) -> Box<WorkingSetTuples<Domain, Codomain>> {
        self.tuples
    }

    pub fn tuples_ref(&self) -> &WorkingSetTuples<Domain, Codomain> {
        &self.tuples
    }

    pub fn tuples_mut(&mut self) -> &mut WorkingSetTuples<Domain, Codomain> {
        &mut self.tuples
    }

    pub fn parts_mut(
        &mut self,
    ) -> (
        &mut WorkingSetTuples<Domain, Codomain>,
        &dyn RelationIndex<Domain, Codomain>,
    ) {
        (&mut self.tuples, &*self.base_index)
    }

    /// Get the base index for looking up what values existed at transaction start.
    pub fn base_index(&self) -> &dyn RelationIndex<Domain, Codomain> {
        &*self.base_index
    }

    /// Look up the base value for a domain (what we saw at transaction start).
    pub fn base_value(&self, domain: &Domain) -> Option<(Timestamp, Codomain)> {
        self.base_index
            .index_lookup(domain)
            .map(|entry| (entry.ts, entry.value.clone()))
    }

    pub fn provider_fully_loaded(&self) -> bool {
        self.provider_fully_loaded
    }
}

/// Represents the state of a relation in the context of a current transaction.
impl<Domain, Codomain, Source> RelationTransaction<Domain, Codomain, Source>
where
    Source: Canonical<Domain, Codomain>,
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    pub fn new(
        tx: Tx,
        relation_name: Symbol,
        canonical: Box<dyn RelationIndex<Domain, Codomain>>,
        backing_source: Source,
    ) -> RelationTransaction<Domain, Codomain, Source> {
        let provider_fully_loaded = canonical.is_provider_fully_loaded();
        let inner = Inner {
            local_operations: HashMap::default(),
            master_entries: canonical,
            provider_fully_loaded,
            has_local_mutations: false,
        };
        RelationTransaction {
            tx,
            relation_name,
            index: inner,
            backing_source: backing_source.into(),
        }
    }

    /// Helper to create a ConflictInfo for this relation.
    fn make_conflict_info(&self, domain: &Domain, conflict_type: ConflictType) -> ConflictInfo {
        ConflictInfo {
            relation_name: self.relation_name,
            domain_key: format!("{}", domain),
            conflict_type,
        }
    }

    pub fn insert(&mut self, domain: Domain, value: Codomain) -> Result<(), Error> {
        // If we or upstream has already inserted this domain, we can't insert it again.
        if self.index.master_entries.index_lookup(&domain).is_some() {
            return Err(Error::Duplicate);
        }

        // Check our own local index to see if we have an entry for this domain.
        if self.index.has_local_mutations
            && let Some(entry) = self.index.local_operations.get_mut(&domain)
        {
            match &entry.operation {
                OpType::Delete => {
                    // Recreating a locally deleted entry in this transaction becomes an insert.
                    entry.read_ts = self.tx.ts;
                    entry.write_ts = self.tx.ts;
                    entry.operation = OpType::Insert(value);
                    self.index.has_local_mutations = true;
                    return Ok(());
                }
                OpType::Insert(_) | OpType::Update(_) => {
                    // Already have an insert or update for this domain
                    return Err(Error::Duplicate);
                }
            }
        }

        // Not in the index, we check the backing source.
        if let Some((read_ts, _)) = self.backing_source.get(&domain)?
            && read_ts < self.tx.ts
        {
            return Err(Error::Duplicate);
        }

        // Not in the index, not in the backing source, we can insert freely.
        // Local index + also the operations log.
        self.index.local_operations.insert(
            domain,
            Op {
                read_ts: self.tx.ts,
                write_ts: self.tx.ts,
                operation: OpType::Insert(value),
                guaranteed_unique: false,
            },
        );
        self.index.has_local_mutations = true;

        Ok(())
    }

    /// Insert a value that is guaranteed to be unique, skipping conflict checking.
    /// This is an optimization for cases where uniqueness is ensured by the caller,
    /// such as anonymous object creation with UUID-based keys.
    pub fn insert_guaranteed_unique(
        &mut self,
        domain: Domain,
        value: Codomain,
    ) -> Result<(), Error> {
        // Skip all duplicate checking since we're guaranteed unique
        self.index.local_operations.insert(
            domain,
            Op {
                read_ts: self.tx.ts,
                write_ts: self.tx.ts,
                operation: OpType::Insert(value),
                guaranteed_unique: true,
            },
        );
        self.index.has_local_mutations = true;

        Ok(())
    }

    pub fn update(&mut self, domain: &Domain, value: Codomain) -> Result<Option<Codomain>, Error> {
        // Check our local index first, but only if we have mutations.
        // If we have an entry for this domain, we can update it.
        if self.index.has_local_mutations
            && let Some(entry) = self.index.local_operations.get_mut(domain)
        {
            // If the operation is a delete, we can't update it.
            if entry.operation.is_delete() {
                return Ok(None);
            }
            // If it's an insert or update, we can update it.
            entry.write_ts = self.tx.ts;
            let next_op = if entry.operation.is_update() {
                OpType::Update(value)
            } else {
                OpType::Insert(value)
            };
            let old_value = match std::mem::replace(
                &mut entry.operation,
                next_op,
            ) {
                OpType::Insert(old_value) | OpType::Update(old_value) => old_value,
                OpType::Delete => return Ok(None),
            };
            self.index.has_local_mutations = true;
            return Ok(Some(old_value));
        }

        // Is this already in the *master* index?
        if let Some(entry) = self.index.master_entries.index_lookup(domain) {
            if entry.ts > self.tx.ts {
                // We can't update it, it's too new.
                return Ok(None);
            }

            let old_value = entry.value.clone();
            let read_ts = entry.ts;

            // We need to entry in the ops log which has to be "update" since we're updating.
            self.index.local_operations.insert(
                domain.clone(),
                Op {
                    read_ts,
                    write_ts: self.tx.ts,
                    operation: OpType::Update(value),
                    guaranteed_unique: false,
                },
            );
            self.index.has_local_mutations = true;

            // Update local secondary index

            return Ok(Some(old_value));
        }

        // We had nothing in our local index, so let's ask upstream to attempt to pull in an "old"
        // value for it. We'll then update our own copy, and create an operations log entry for the
        // update

        // If provider is fully loaded, not being in master means it doesn't exist
        if self.index.provider_fully_loaded {
            return Ok(None);
        }

        // Provider not fully loaded - check the backing source
        let Some((read_ts, backing_value)) = self.backing_source.get(domain)? else {
            // Not in the backing source, we can't update it.
            return Ok(None);
        };

        // If the timestamp is greater than our own, we won't be able to update it when we actually
        // commit, so we may as well mark it as a conflict *now*.
        // (Let's just hope the "upper layers" try to do the right thing here)
        if read_ts >= self.tx.ts {
            return Err(Error::Conflict(
                self.make_conflict_info(domain, ConflictType::ConcurrentWrite),
            ));
        }

        // Put in operations log
        // Copy into the local cache, but with updated value.
        self.index.local_operations.insert(
            domain.clone(),
            Op {
                read_ts,
                write_ts: self.tx.ts,
                operation: OpType::Update(value),
                guaranteed_unique: false,
            },
        );
        self.index.has_local_mutations = true;

        // Update local secondary index

        Ok(Some(backing_value))
    }

    pub fn upsert(&mut self, domain: Domain, value: Codomain) -> Result<Option<Codomain>, Error> {
        // Check local operations first - single lookup that handles all cases, but only if we have mutations
        if self.index.has_local_mutations
            && let Some(entry) = self.index.local_operations.get_mut(&domain)
        {
            match &entry.operation {
                OpType::Delete => {
                    // Check master index to determine if this should be Insert or Update
                    if let Some(master_entry) = self.index.master_entries.index_lookup(&domain) {
                        // Entry exists in master index, convert to Update
                        entry.read_ts = master_entry.ts;
                        entry.write_ts = self.tx.ts;
                        entry.operation = OpType::Update(value);
                    } else {
                        // No entry in master index, convert to Insert
                        entry.read_ts = self.tx.ts;
                        entry.write_ts = self.tx.ts;
                        entry.operation = OpType::Insert(value);
                    }
                    self.index.has_local_mutations = true;
                    // Update local secondary index
                    return Ok(None);
                }
                OpType::Insert(_) | OpType::Update(_) => {
                    // Update existing entry
                    entry.write_ts = self.tx.ts;
                    let next_op = if matches!(entry.operation, OpType::Update(_)) {
                        OpType::Update(value)
                    } else {
                        OpType::Insert(value)
                    };
                    let old_value = match std::mem::replace(
                        &mut entry.operation,
                        next_op,
                    ) {
                        OpType::Insert(old) | OpType::Update(old) => {
                            // Update local secondary index
                            old
                        }
                        OpType::Delete => unreachable!(), // Already handled above
                    };
                    self.index.has_local_mutations = true;
                    return Ok(Some(old_value));
                }
            }
        }

        // Check master entries for existing data
        if let Some(entry) = self.index.master_entries.index_lookup(&domain) {
            // Existing entry in master - do update via local operation
            let old_value = entry.value.clone();
            self.index.local_operations.insert(
                domain,
                Op {
                    read_ts: entry.ts,
                    write_ts: self.tx.ts,
                    operation: OpType::Update(value),
                    guaranteed_unique: false,
                },
            );
            self.index.has_local_mutations = true;
            // Update local secondary index
            return Ok(Some(old_value));
        }

        // If provider not fully loaded, check backing source for existing data
        if !self.index.provider_fully_loaded
            && let Some((read_ts, backing_value)) = self.backing_source.get(&domain)?
            && read_ts < self.tx.ts
        {
            // Existing entry in backing - do update via local operation
            self.index.local_operations.insert(
                domain,
                Op {
                    read_ts,
                    write_ts: self.tx.ts,
                    operation: OpType::Update(value),
                    guaranteed_unique: false,
                },
            );
            self.index.has_local_mutations = true;
            // Update local secondary index
            return Ok(Some(backing_value));
        }

        // No existing entry anywhere (or provider fully loaded) - do insert via local operation
        self.index.local_operations.insert(
            domain,
            Op {
                read_ts: self.tx.ts,
                write_ts: self.tx.ts,
                operation: OpType::Insert(value),
                guaranteed_unique: false,
            },
        );
        self.index.has_local_mutations = true;
        Ok(None)
    }

    pub fn has_domain(&self, domain: &Domain) -> Result<bool, Error> {
        Ok(self.get(domain)?.is_some())
    }

    /// Bulk check existence of multiple domains efficiently
    /// Returns a Vec of domains that exist (are valid)
    pub fn check_domains<T: Iterator<Item = Domain>>(
        &self,
        domains: T,
    ) -> Result<HashSet<Domain>, Error> {
        let mut valid_domains = HashSet::new();

        for domain in domains {
            // Check local operations first (if we have mutations)
            if self.index.has_local_mutations
                && let Some(op) = self.index.local_operations.get(&domain)
            {
                match &op.operation {
                    OpType::Delete => continue, // Not valid
                    OpType::Insert(_) | OpType::Update(_) => {
                        valid_domains.insert(domain.clone());
                        continue;
                    }
                }
            }

            // Check master entries
            if self.index.master_entries.index_lookup(&domain).is_some() {
                valid_domains.insert(domain.clone());
                continue;
            }

            // If provider fully loaded, not being in master means it doesn't exist
            if self.index.provider_fully_loaded {
                continue;
            }

            // Provider not fully loaded - check backing source
            if let Some((ts, _)) = self.backing_source.get(&domain)?
                && ts <= self.tx.ts
            {
                valid_domains.insert(domain.clone());
            }
        }

        Ok(valid_domains)
    }

    pub fn get_by_codomain(&self, codomain: &Codomain) -> Vec<Domain> {
        // Start with results from master entries
        let mut results = self.index.master_entries.get_by_codomain(codomain);

        // Process local operations to account for uncommitted changes
        for (domain, op) in self.index.local_operations.iter() {
            match &op.operation {
                OpType::Insert(value) | OpType::Update(value) => {
                    if value == codomain {
                        // Add this domain if it maps to the requested codomain
                        if !results.contains(domain) {
                            results.push(domain.clone());
                        }
                    } else {
                        // Remove this domain if it no longer maps to the requested codomain
                        results.retain(|d| d != domain);
                    }
                }
                OpType::Delete => {
                    // Remove this domain since it's been deleted
                    results.retain(|d| d != domain);
                }
            }
        }

        results
    }

    pub fn get(&self, domain: &Domain) -> Result<Option<Codomain>, Error> {
        // Check local operations first, but only if we have mutations.
        if self.index.has_local_mutations
            && let Some(op) = self.index.local_operations.get(domain)
        {
            match &op.operation {
                // If it's a delete, we don't have it.
                OpType::Delete => return Ok(None),
                // If it's an insert or update, we have it.
                OpType::Insert(value) | OpType::Update(value) => {
                    return Ok(Some(value.clone()));
                }
            }
        }

        // Check entries
        if let Some(entry) = self.index.master_entries.index_lookup(domain) {
            return Ok(Some(entry.value.clone()));
        }

        // If provider is fully loaded into master entries, not being there means it doesn't exist
        if self.index.provider_fully_loaded {
            return Ok(None);
        }

        // Provider not fully loaded - need to check backing source
        match self.backing_source.get(domain)? {
            Some((read_ts, value)) if read_ts < self.tx.ts => Ok(Some(value)),
            _ => Ok(None),
        }
    }

    pub fn delete(&mut self, domain: &Domain) -> Result<Option<Codomain>, Error> {
        // This is like update, but we're removing.
        // Check our local index first, but only if we have mutations.
        // If we have an entry for this domain, we can delete it and move on
        if self.index.has_local_mutations
            && let Some(entry) = self.index.local_operations.get_mut(domain)
        {
            // If the operation is a delete, we can't delete it again.
            if entry.operation.is_delete() {
                return Ok(None);
            }
            // If it's an insert or update, we can delete it.
            entry.write_ts = self.tx.ts;
            let old_value = match std::mem::replace(&mut entry.operation, OpType::Delete) {
                OpType::Insert(value) | OpType::Update(value) => {
                    // Update local secondary index (remove from old codomain)
                    value
                }
                OpType::Delete => return Ok(None),
            };
            self.index.has_local_mutations = true;
            return Ok(Some(old_value));
        }

        if let Some(entry) = self.index.master_entries.index_lookup(domain) {
            let old_value = entry.value.clone();
            // Upstream may or may not have this key to delete, but we'll log the operation anyways.
            let read_ts = entry.ts;
            self.index.local_operations.insert(
                domain.clone(),
                Op {
                    read_ts,
                    write_ts: self.tx.ts,
                    operation: OpType::Delete,
                    guaranteed_unique: false,
                },
            );
            self.index.has_local_mutations = true;
            return Ok(Some(old_value));
        }

        // We had nothing in our local index, so let's ask upstream to attempt to pull in an "old"
        // value for it. We'll then update our own copy, and create an operations log entry for the
        // update

        // If provider is fully loaded, not being in master means it doesn't exist
        if self.index.provider_fully_loaded {
            return Ok(None);
        }

        // Provider not fully loaded - check the backing source
        let Some((read_ts, backing_value)) = self.backing_source.get(domain)? else {
            // Not in the backing source, we can't update it.
            return Ok(None);
        };

        // Pretend we didn't see it, it's too new.
        if read_ts >= self.tx.ts {
            return Ok(None);
        }

        // It's there upstream, so log a delete for it in the operations log as something we need
        // to do.
        self.index.local_operations.insert(
            domain.clone(),
            Op {
                read_ts,
                write_ts: self.tx.ts,
                operation: OpType::Delete,
                guaranteed_unique: false,
            },
        );
        self.index.has_local_mutations = true;

        // Update local secondary index (remove from old codomain)

        Ok(Some(backing_value))
    }

    pub fn scan<F>(&self, predicate: &F) -> Result<Vec<(Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        let mut results: HashMap<_, _> = HashMap::new();

        // If we've already fully loaded from the provider, we can skip the expensive backing source scan
        if self.index.provider_fully_loaded {
            // Just use the master entries - they already contain all the provider data
            for (domain, entry) in self.index.master_entries.iter() {
                if entry.ts <= self.tx.ts && predicate(domain, &entry.value) {
                    results.insert(domain.clone(), entry.value.clone());
                }
            }
        } else {
            // Need to hit the backing source to get data not yet loaded into master entries
            let backing_results: HashMap<_, _> = self
                .backing_source
                .scan(predicate)?
                .iter()
                .filter_map(|(ts, domain, value)| {
                    if *ts <= self.tx.ts && predicate(domain, value) {
                        return Some((domain.clone(), value.clone()));
                    }
                    None
                })
                .collect();
            results.extend(backing_results);

            // Also merge in the master entries from the index
            for (domain, entry) in self.index.master_entries.iter() {
                if !results.contains_key(domain)
                    && entry.ts <= self.tx.ts
                    && predicate(domain, &entry.value)
                {
                    results.insert(domain.clone(), entry.value.clone());
                }
            }
        }

        // Apply local operations to get the final view
        for (domain, op) in self.index.local_operations.iter() {
            match &op.operation {
                OpType::Insert(value) | OpType::Update(value) => {
                    if predicate(domain, value) {
                        results.insert(domain.clone(), value.clone());
                    }
                }
                OpType::Delete => {
                    results.remove(domain);
                }
            }
        }

        Ok(results.into_iter().collect())
    }

    /// Optimized method to get all tuples without filtering
    /// Loads from provider once and caches the result for subsequent calls
    pub fn get_all(&mut self) -> Result<Vec<(Domain, Codomain)>, Error> {
        // If we haven't loaded from provider yet, do it now
        if !self.index.provider_fully_loaded {
            self.fully_load_from_provider()?;
            self.index.provider_fully_loaded = true;
        }

        // Now we can just merge master_entries + local_operations
        // without touching the provider again
        let mut results = HashMap::new();

        // Add all master entries that are visible to this transaction
        for (domain, entry) in self.index.master_entries.iter() {
            if entry.ts <= self.tx.ts {
                results.insert(domain.clone(), entry.value.clone());
            }
        }

        // Apply local operations to get the final view
        for (domain, op) in self.index.local_operations.iter() {
            match &op.operation {
                OpType::Insert(value) | OpType::Update(value) => {
                    results.insert(domain.clone(), value.clone());
                }
                OpType::Delete => {
                    results.remove(domain);
                }
            }
        }

        Ok(results.into_iter().collect())
    }

    /// Optimized method to get multiple specific tuples
    /// More efficient than get_all() when you only need a subset of tuples
    pub fn bulk_get(&self, domains: &[Domain]) -> Result<Vec<(Domain, Codomain)>, Error> {
        let mut results = HashMap::new();

        // Process each requested domain
        for domain in domains {
            // Check local operations first (if we have mutations)
            if self.index.has_local_mutations
                && let Some(op) = self.index.local_operations.get(domain)
            {
                match &op.operation {
                    OpType::Delete => continue, // Skip deleted entries
                    OpType::Insert(value) | OpType::Update(value) => {
                        results.insert(domain.clone(), value.clone());
                        continue;
                    }
                }
            }

            // Check master entries
            if let Some(entry) = self.index.master_entries.index_lookup(domain)
                && entry.ts <= self.tx.ts
            {
                results.insert(domain.clone(), entry.value.clone());
                continue;
            }

            // If provider fully loaded, not being in master means it doesn't exist
            if self.index.provider_fully_loaded {
                continue;
            }

            // Provider not fully loaded - check backing source as fallback
            if let Some((ts, value)) = self.backing_source.get(domain)?
                && ts <= self.tx.ts
            {
                results.insert(domain.clone(), value);
            }
        }

        Ok(results.into_iter().collect())
    }

    /// Helper method to fully load all data from the provider into the master index
    fn fully_load_from_provider(&mut self) -> Result<(), Error> {
        // Scan all data from the provider that's visible to this transaction
        let _provider_data = self.backing_source.scan(&|_domain, _codomain| true)?;

        // The scan() call above will have populated the master_entries index as a side effect
        // due to the Canonical::scan() implementation, so we don't need to do anything else here

        // Mark the master entries as fully loaded
        self.index.master_entries.set_provider_fully_loaded(true);

        Ok(())
    }

    pub fn working_set(self) -> Result<WorkingSet<Domain, Codomain>, WorldStateError> {
        Ok(WorkingSet::new_with_fully_loaded(
            Box::new(self.index.local_operations),
            self.index.master_entries,
            self.index.provider_fully_loaded,
        ))
    }
}
