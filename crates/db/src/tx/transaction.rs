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

use crate::tx::{Error, RelationCodomain, RelationDomain, RelationIndex, Timestamp, Tx};
use ahash::AHasher;
use moor_common::model::WorldStateError;
use moor_var::Symbol;
use std::collections::HashSet;
use std::{collections::HashMap, hash::BuildHasherDefault, sync::Arc};

mod local_operations;
use local_operations::LocalOperations;

/// A key-value caching store that is scoped for the lifetime of a transaction.
/// When the transaction is completed, it collapses into a WorkingSet which can be applied to the
/// global transactional cache.
pub struct RelationTransaction<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    tx: Tx,
    relation_name: Symbol,

    index: Inner<Domain, Codomain>,
}

struct Inner<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    local_operations: LocalOperations<Domain, Codomain>,
    master_entries: Arc<dyn RelationIndex<Domain, Codomain>>,
    fully_resident: bool,
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

#[derive(Clone)]
pub struct WorkingSet<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    tuples: WorkingSetTuples<Domain, Codomain>,
    /// The base index - a snapshot of the canonical state when the transaction started.
    /// Used for 3-way merge during conflict resolution:
    /// - base (this) = what we saw at transaction start
    /// - mine = our operation in tuples
    /// - theirs = current canonical state at commit time
    base_index: Arc<dyn RelationIndex<Domain, Codomain>>,
}

impl<Domain, Codomain> WorkingSet<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    pub fn new(
        tuples: WorkingSetTuples<Domain, Codomain>,
        base_index: Box<dyn RelationIndex<Domain, Codomain>>,
    ) -> WorkingSet<Domain, Codomain> {
        Self::new_shared(tuples, Arc::from(base_index))
    }

    fn new_shared(
        tuples: WorkingSetTuples<Domain, Codomain>,
        base_index: Arc<dyn RelationIndex<Domain, Codomain>>,
    ) -> WorkingSet<Domain, Codomain> {
        WorkingSet { tuples, base_index }
    }

    pub fn len(&self) -> usize {
        self.tuples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tuples.is_empty()
    }

    pub fn tuples(self) -> WorkingSetTuples<Domain, Codomain> {
        self.tuples
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        WorkingSetTuples<Domain, Codomain>,
        Arc<dyn RelationIndex<Domain, Codomain>>,
    ) {
        (self.tuples, self.base_index)
    }

    pub fn tuples_ref(&self) -> &WorkingSetTuples<Domain, Codomain> {
        &self.tuples
    }

    /// Iterate this working set's mutations as explicit writes.
    ///
    /// `None` means the key is deleted; `Some` means it is inserted or updated.
    /// Persistence adapters and test sinks use this typed view instead of
    /// reaching into tuple internals.
    pub fn mutations(&self) -> impl Iterator<Item = (Timestamp, &Domain, Option<&Codomain>)> + '_ {
        self.tuples.iter().map(|(domain, op)| {
            let value = match &op.operation {
                OpType::Insert(value) | OpType::Update(value) => Some(value),
                OpType::Delete => None,
            };
            (op.write_ts, domain, value)
        })
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
}

/// Represents the state of a relation in the context of a current transaction.
impl<Domain, Codomain> RelationTransaction<Domain, Codomain>
where
    Domain: RelationDomain,
    Codomain: RelationCodomain,
{
    #[inline]
    fn visible_ts(&self) -> Timestamp {
        self.tx.visible_ts
    }

    #[inline]
    fn write_ts(&self) -> Timestamp {
        self.tx.ts
    }

    pub fn new(
        tx: Tx,
        relation_name: Symbol,
        canonical: Box<dyn RelationIndex<Domain, Codomain>>,
    ) -> RelationTransaction<Domain, Codomain> {
        Self::new_shared(tx, relation_name, Arc::from(canonical))
    }

    pub(crate) fn new_shared(
        tx: Tx,
        relation_name: Symbol,
        canonical: Arc<dyn RelationIndex<Domain, Codomain>>,
    ) -> RelationTransaction<Domain, Codomain> {
        let fully_resident = canonical.is_fully_resident();
        let inner = Inner {
            local_operations: LocalOperations::new(),
            master_entries: canonical,
            fully_resident,
        };
        RelationTransaction {
            tx,
            relation_name,
            index: inner,
        }
    }

    pub fn insert(&mut self, domain: Domain, value: Codomain) -> Result<(), Error> {
        let visible_ts = self.visible_ts();
        let write_ts = self.write_ts();

        // Common fast path: this transaction has not mutated anything yet.
        if self.index.local_operations.is_empty() {
            // If we or upstream has already inserted this domain, we can't insert it again.
            if self.index.master_entries.index_lookup(&domain).is_some() {
                return Err(Error::Duplicate);
            }

            self.require_complete()?;

            self.index.local_operations.insert(
                domain,
                Op {
                    read_ts: write_ts,
                    write_ts,
                    operation: OpType::Insert(value),
                    guaranteed_unique: false,
                },
            );

            return Ok(());
        }

        // Check our own local index to see if we have an entry for this domain.
        if let Some(entry) = self.index.local_operations.get_mut(&domain) {
            if entry.operation.is_delete() {
                // Recreating a locally deleted entry in this transaction:
                // - if this key existed when we read it (read_ts < tx.ts), this is an update
                // - otherwise it's re-inserting a locally-created key
                entry.write_ts = write_ts;
                entry.operation = if entry.read_ts <= visible_ts {
                    OpType::Update(value)
                } else {
                    entry.read_ts = write_ts;
                    OpType::Insert(value)
                };
                return Ok(());
            }

            // Already have an insert or update for this domain.
            return Err(Error::Duplicate);
        }

        if self.index.master_entries.index_lookup(&domain).is_some() {
            return Err(Error::Duplicate);
        }

        self.require_complete()?;

        // Local index + also the operations log.
        self.index.local_operations.insert(
            domain,
            Op {
                read_ts: write_ts,
                write_ts,
                operation: OpType::Insert(value),
                guaranteed_unique: false,
            },
        );

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
                read_ts: self.write_ts(),
                write_ts: self.write_ts(),
                operation: OpType::Insert(value),
                guaranteed_unique: true,
            },
        );

        Ok(())
    }

    pub fn update(&mut self, domain: &Domain, value: Codomain) -> Result<Option<Codomain>, Error> {
        let visible_ts = self.visible_ts();
        let write_ts = self.write_ts();

        // Check our local index first, but only if we have mutations.
        // If we have an entry for this domain, we can update it.
        if !self.index.local_operations.is_empty()
            && let Some(entry) = self.index.local_operations.get_mut(domain)
        {
            // If the operation is a delete, we can't update it.
            if entry.operation.is_delete() {
                return Ok(None);
            }
            entry.write_ts = write_ts;
            let old_value = match &mut entry.operation {
                // Keep an insert as insert; only the value changes.
                OpType::Insert(current) => std::mem::replace(current, value),
                // Keep an update as update; only the value changes.
                OpType::Update(current) => std::mem::replace(current, value),
                OpType::Delete => return Ok(None),
            };

            return Ok(Some(old_value));
        }

        // Is this already in the *master* index?
        if let Some(entry) = self.index.master_entries.index_lookup(domain) {
            if entry.ts > visible_ts {
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
                    write_ts,
                    operation: OpType::Update(value),
                    guaranteed_unique: false,
                },
            );

            // Update local secondary index

            return Ok(Some(old_value));
        }

        self.require_complete()?;
        Ok(None)
    }

    pub fn upsert(&mut self, domain: Domain, value: Codomain) -> Result<Option<Codomain>, Error> {
        let visible_ts = self.visible_ts();
        let write_ts = self.write_ts();

        // Check local operations first - single lookup that handles all cases, but only if we have mutations
        if !self.index.local_operations.is_empty()
            && let Some(entry) = self.index.local_operations.get_mut(&domain)
        {
            match &entry.operation {
                OpType::Delete => {
                    // Reuse delete-path provenance:
                    // - read_ts < tx.ts means the key existed at transaction start, so this is
                    //   an update of an existing tuple.
                    // - read_ts == tx.ts means this key only existed locally, so this remains
                    //   an insert.
                    if entry.read_ts <= visible_ts {
                        entry.write_ts = write_ts;
                        entry.operation = OpType::Update(value);
                    } else {
                        entry.read_ts = write_ts;
                        entry.write_ts = write_ts;
                        entry.operation = OpType::Insert(value);
                    }

                    // Update local secondary index
                    return Ok(None);
                }
                OpType::Insert(_) | OpType::Update(_) => {
                    // Update existing entry
                    entry.write_ts = write_ts;
                    let old_value = match &mut entry.operation {
                        OpType::Insert(current) | OpType::Update(current) => {
                            std::mem::replace(current, value)
                        }
                        OpType::Delete => unreachable!(), // Already handled above
                    };

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
                    write_ts,
                    operation: OpType::Update(value),
                    guaranteed_unique: false,
                },
            );

            // Update local secondary index
            return Ok(Some(old_value));
        }

        self.require_complete()?;

        self.index.local_operations.insert(
            domain,
            Op {
                read_ts: write_ts,
                write_ts,
                operation: OpType::Insert(value),
                guaranteed_unique: false,
            },
        );

        Ok(None)
    }

    /// Upsert using a closure that derives the new value from the current visible value.
    ///
    /// The closure receives:
    /// - `None` when the domain is not currently visible in this transaction.
    /// - `Some(&Codomain)` when the domain has a visible value.
    ///
    /// Returning:
    /// - `Some(new_value)` performs an upsert and returns the previous value (if any).
    /// - `None` performs no mutation and returns the current value (if any).
    pub fn upsert_with<F>(&mut self, domain: Domain, f: F) -> Result<Option<Codomain>, Error>
    where
        F: FnOnce(Option<&Codomain>) -> Option<Codomain>,
    {
        let visible_ts = self.visible_ts();
        let write_ts = self.write_ts();

        if !self.index.local_operations.is_empty()
            && let Some(entry) = self.index.local_operations.get_mut(&domain)
        {
            match &mut entry.operation {
                OpType::Delete => {
                    if let Some(new_value) = f(None) {
                        if entry.read_ts <= visible_ts {
                            entry.write_ts = write_ts;
                            entry.operation = OpType::Update(new_value);
                        } else {
                            entry.read_ts = write_ts;
                            entry.write_ts = write_ts;
                            entry.operation = OpType::Insert(new_value);
                        }
                    }
                    return Ok(None);
                }
                OpType::Insert(current) | OpType::Update(current) => {
                    let old_value = current.clone();
                    if let Some(new_value) = f(Some(&old_value)) {
                        entry.write_ts = write_ts;
                        *current = new_value;
                    }
                    return Ok(Some(old_value));
                }
            }
        }

        if let Some(entry) = self.index.master_entries.index_lookup(&domain) {
            let old_value = entry.value.clone();
            if let Some(new_value) = f(Some(&entry.value)) {
                self.index.local_operations.insert(
                    domain,
                    Op {
                        read_ts: entry.ts,
                        write_ts: self.write_ts(),
                        operation: OpType::Update(new_value),
                        guaranteed_unique: false,
                    },
                );
            }
            return Ok(Some(old_value));
        }

        self.require_complete()?;

        if let Some(new_value) = f(None) {
            self.index.local_operations.insert(
                domain,
                Op {
                    read_ts: self.write_ts(),
                    write_ts: self.write_ts(),
                    operation: OpType::Insert(new_value),
                    guaranteed_unique: false,
                },
            );
        }

        Ok(None)
    }

    /// Update using a closure that derives a new value from the current value.
    ///
    /// This is update-only: if the domain does not currently exist, no mutation is performed.
    /// Returning `None` from the closure leaves the current value unchanged.
    pub fn update_with<F>(&mut self, domain: &Domain, f: F) -> Result<Option<Codomain>, Error>
    where
        F: FnOnce(&Codomain) -> Option<Codomain>,
    {
        self.upsert_with(domain.clone(), |current| current.and_then(f))
    }

    pub fn has_domain(&self, domain: &Domain) -> Result<bool, Error> {
        // Existence-only path: avoid cloning codomain values from `get()`.
        if !self.index.local_operations.is_empty()
            && let Some(op) = self.index.local_operations.get(domain)
        {
            return Ok(!op.operation.is_delete());
        }

        if self.index.master_entries.index_lookup(domain).is_some() {
            return Ok(true);
        }

        if self.index.fully_resident {
            return Ok(false);
        }

        Err(Error::IncompleteIndex(self.relation_name))
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
            if !self.index.local_operations.is_empty()
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

            if self.index.fully_resident {
                continue;
            }

            return Err(Error::IncompleteIndex(self.relation_name));
        }

        Ok(valid_domains)
    }

    pub fn get_by_codomain(&self, codomain: &Codomain) -> Vec<Domain> {
        let mut results = Vec::new();
        self.for_each_by_codomain(codomain, |domain| results.push(domain.clone()));
        results
    }

    /// Visit each domain that maps to the given codomain in this transaction's view.
    /// Applies local operation overlays over the base index without materializing
    /// the base reverse-lookup vector.
    pub fn for_each_by_codomain<F>(&self, codomain: &Codomain, mut f: F)
    where
        F: FnMut(&Domain),
    {
        self.index
            .master_entries
            .for_each_by_codomain(codomain, &mut |domain| {
                // Any local op shadows base state for this domain.
                if !self.index.local_operations.contains_key(domain) {
                    f(domain);
                }
            });

        if self.index.local_operations.is_empty() {
            return;
        }

        self.index
            .local_operations
            .for_each_by_codomain(codomain, f);
    }

    pub fn get(&self, domain: &Domain) -> Result<Option<Codomain>, Error> {
        // Fast path: no local mutations means no local-ops lookup needed.
        if self.index.local_operations.is_empty() {
            if let Some(entry) = self.index.master_entries.index_lookup(domain) {
                return Ok(Some(entry.value.clone()));
            }

            if self.index.fully_resident {
                return Ok(None);
            }

            return Err(Error::IncompleteIndex(self.relation_name));
        }

        if let Some(op) = self.index.local_operations.get(domain) {
            match &op.operation {
                OpType::Delete => return Ok(None),
                OpType::Insert(value) | OpType::Update(value) => {
                    return Ok(Some(value.clone()));
                }
            }
        }

        if let Some(entry) = self.index.master_entries.index_lookup(domain) {
            return Ok(Some(entry.value.clone()));
        }

        if self.index.fully_resident {
            return Ok(None);
        }

        Err(Error::IncompleteIndex(self.relation_name))
    }

    /// Invoke `f` with the tuple value for `domain` if one is visible to this transaction.
    ///
    /// This mirrors `get()` visibility/precedence semantics while avoiding codomain cloning
    /// when data is already available in local operations or the in-memory index.
    pub fn with_domain_value<R, F>(&self, domain: &Domain, f: F) -> Result<Option<R>, Error>
    where
        F: FnOnce(&Codomain) -> R,
    {
        // Fast path: no local mutations means no local-ops lookup needed.
        if self.index.local_operations.is_empty() {
            if let Some(entry) = self.index.master_entries.index_lookup(domain) {
                return Ok(Some(f(&entry.value)));
            }

            if self.index.fully_resident {
                return Ok(None);
            }

            return Err(Error::IncompleteIndex(self.relation_name));
        }

        if let Some(op) = self.index.local_operations.get(domain) {
            match &op.operation {
                OpType::Delete => return Ok(None),
                OpType::Insert(value) | OpType::Update(value) => {
                    return Ok(Some(f(value)));
                }
            }
        }

        if let Some(entry) = self.index.master_entries.index_lookup(domain) {
            return Ok(Some(f(&entry.value)));
        }

        if self.index.fully_resident {
            return Ok(None);
        }

        Err(Error::IncompleteIndex(self.relation_name))
    }

    pub fn delete(&mut self, domain: &Domain) -> Result<Option<Codomain>, Error> {
        let write_ts = self.write_ts();

        // This is like update, but we're removing.
        // Check our local index first, but only if we have mutations.
        // If we have an entry for this domain, we can delete it and move on
        if !self.index.local_operations.is_empty()
            && let Some(entry) = self.index.local_operations.get_mut(domain)
        {
            // If the operation is a delete, we can't delete it again.
            if entry.operation.is_delete() {
                return Ok(None);
            }
            // If it's an insert or update, we can delete it.
            entry.write_ts = write_ts;
            let old_value = match std::mem::replace(&mut entry.operation, OpType::Delete) {
                OpType::Insert(value) | OpType::Update(value) => {
                    // Update local secondary index (remove from old codomain)
                    value
                }
                OpType::Delete => return Ok(None),
            };

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
                    write_ts,
                    operation: OpType::Delete,
                    guaranteed_unique: false,
                },
            );

            return Ok(Some(old_value));
        }

        self.require_complete()?;
        Ok(None)
    }

    pub fn scan<F>(&self, predicate: &F) -> Result<Vec<(Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        let mut results: HashMap<_, _> = HashMap::new();

        self.require_complete()?;
        for (domain, entry) in self.index.master_entries.iter() {
            if entry.ts <= self.visible_ts() && predicate(domain, &entry.value) {
                results.insert(domain.clone(), entry.value.clone());
            }
        }

        // Apply local operations to get the final view
        for (domain, op) in self.index.local_operations.iter() {
            match &op.operation {
                OpType::Insert(value) | OpType::Update(value) => {
                    if predicate(domain, value) {
                        results.insert(domain.clone(), value.clone());
                    } else {
                        results.remove(domain);
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
    pub fn get_all(&mut self) -> Result<Vec<(Domain, Codomain)>, Error> {
        self.require_complete()?;

        // Now we can just merge master_entries + local_operations
        let mut results = HashMap::new();

        // Add all master entries that are visible to this transaction
        for (domain, entry) in self.index.master_entries.iter() {
            if entry.ts <= self.visible_ts() {
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
        let mut results = HashMap::with_capacity(domains.len());
        for domain in domains {
            // Check local operations first (if we have mutations)
            if !self.index.local_operations.is_empty()
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
                && entry.ts <= self.visible_ts()
            {
                results.insert(domain.clone(), entry.value.clone());
                continue;
            }

            if self.index.fully_resident {
                continue;
            }

            return Err(Error::IncompleteIndex(self.relation_name));
        }

        Ok(results.into_iter().collect())
    }

    /// The fault insertion point for a future version-aware pager. Until then a
    /// missing resident seed is an invariant failure, never a latest-store read.
    #[inline]
    fn require_complete(&self) -> Result<(), Error> {
        if !self.index.fully_resident {
            return Err(Error::IncompleteIndex(self.relation_name));
        }
        Ok(())
    }

    pub(crate) fn has_mutations(&self) -> bool {
        !self.index.local_operations.is_empty()
    }

    pub fn working_set(self) -> Result<WorkingSet<Domain, Codomain>, WorldStateError> {
        let Inner {
            local_operations,
            master_entries,
            ..
        } = self.index;
        Ok(WorkingSet::new_shared(
            local_operations.into_map(),
            master_entries,
        ))
    }
}
