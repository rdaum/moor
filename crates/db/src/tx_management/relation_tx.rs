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

use crate::tx_management::indexes::RelationIndex;
use crate::tx_management::{Canonical, Error, Timestamp, Tx};
use ahash::AHasher;
use im::HashMap;
use indexmap::IndexMap;
use moor_common::model::WorldStateError;
use std::hash::{BuildHasherDefault, Hash};
use std::sync::Arc;

/// A key-value caching store that is scoped for the lifetime of a transaction.
/// When the transaction is completed, it collapses into a WorkingSet which can be applied to the
/// global transactional cache.
pub struct RelationTransaction<Domain, Codomain, Source>
where
    Source: Canonical<Domain, Codomain>,
    Domain: Hash + Eq + Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    tx: Tx,

    // Note: This is RefCell for interior mutability since even get/scan operations can modify the
    //   index.
    index: Inner<Domain, Codomain>,
    backing_source: Arc<Source>,
}

struct Inner<Domain, Codomain>
where
    Domain: Clone + Hash + Eq + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    local_operations: Box<IndexMap<Domain, Op<Codomain>, BuildHasherDefault<AHasher>>>,
    master_entries: Box<dyn RelationIndex<Domain, Codomain>>,
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
}

pub struct WorkingSet<Domain, Codomain>
where
    Domain: Clone + Hash + Eq + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    tuples: Box<IndexMap<Domain, Op<Codomain>, BuildHasherDefault<AHasher>>>,
}

impl<Domain, Codomain> WorkingSet<Domain, Codomain>
where
    Domain: Clone + Hash + Eq + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    pub fn new(
        tuples: Box<IndexMap<Domain, Op<Codomain>, BuildHasherDefault<AHasher>>>,
    ) -> WorkingSet<Domain, Codomain> {
        WorkingSet { tuples }
    }

    pub fn len(&self) -> usize {
        self.tuples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tuples.is_empty()
    }

    pub fn tuples(self) -> Box<IndexMap<Domain, Op<Codomain>, BuildHasherDefault<AHasher>>> {
        self.tuples
    }

    pub fn tuples_ref(&self) -> &IndexMap<Domain, Op<Codomain>, BuildHasherDefault<AHasher>> {
        &self.tuples
    }
}

/// Represents the state of a relation in the context of a current transaction.
impl<Domain, Codomain, Source> RelationTransaction<Domain, Codomain, Source>
where
    Source: Canonical<Domain, Codomain>,
    Domain: Clone + Hash + Eq + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    pub fn new(
        tx: Tx,
        canonical: Box<dyn RelationIndex<Domain, Codomain>>,
        backing_source: Source,
    ) -> RelationTransaction<Domain, Codomain, Source> {
        let inner = Inner {
            local_operations: Box::new(IndexMap::default()),
            master_entries: canonical,
        };
        RelationTransaction {
            tx,
            index: inner,
            backing_source: backing_source.into(),
        }
    }

    pub fn insert(&mut self, domain: Domain, value: Codomain) -> Result<(), Error> {
        // If we or upstream has already inserted this domain, we can't insert it again.
        if self.index.master_entries.index_lookup(&domain).is_some() {
            return Err(Error::Duplicate);
        }

        // We also have to check our own local index to see if we have an entry for this domain.
        // IF we do, we can't insert it.
        if self.index.local_operations.contains_key(&domain) {
            return Err(Error::Duplicate);
        }

        // Not in the index, we check the backing source.
        if let Some((read_ts, _)) = self.backing_source.get(&domain)? {
            if read_ts < self.tx.ts {
                return Err(Error::Duplicate);
            }
        }

        // Not in the index, not in the backing source, we can insert freely.
        // Local index + also the operations log.
        self.index.local_operations.insert(
            domain.clone(),
            Op {
                read_ts: self.tx.ts,
                write_ts: self.tx.ts,
                operation: OpType::Insert(value.clone()),
            },
        );

        Ok(())
    }

    pub fn update(&mut self, domain: &Domain, value: Codomain) -> Result<Option<Codomain>, Error> {
        // Check our local index first.
        // If we have an entry for this domain, we can update it.
        if let Some(entry) = self.index.local_operations.get_mut(domain) {
            // If the operation is a delete, we can't update it.
            if entry.operation.is_delete() {
                return Ok(None);
            }
            // If it's an insert or update, we can update it.
            entry.write_ts = self.tx.ts;
            let is_update = entry.operation.is_update();
            let old_value = match std::mem::replace(
                &mut entry.operation,
                if is_update {
                    OpType::Update(value.clone())
                } else {
                    OpType::Insert(value.clone())
                },
            ) {
                OpType::Insert(old_value) | OpType::Update(old_value) => old_value,
                OpType::Delete => return Ok(None),
            };
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
                    operation: OpType::Update(value.clone()),
                },
            );

            // Update local secondary index

            return Ok(Some(old_value));
        }

        // We had nothing in our local index, so let's ask upstream to attempt to pull in an "old"
        // value for it. We'll then update our own copy, and create an operations log entry for the
        // update

        // Not in the index, we check the backing source.
        let Some((read_ts, backing_value)) = self.backing_source.get(domain)? else {
            // Not in the backing source, we can't update it.
            return Ok(None);
        };

        // If the timestamp is greater than our own, we won't be able to update it when we actually
        // commit, so we may as well mark it as a conflict *now*.
        // (Let's just hope the "upper layers" try to do the right thing here)
        if read_ts >= self.tx.ts {
            return Err(Error::Conflict);
        }

        // Put in operations log
        // Copy into the local cache, but with updated value.
        self.index.local_operations.insert(
            domain.clone(),
            Op {
                read_ts,
                write_ts: self.tx.ts,
                operation: OpType::Update(value.clone()),
            },
        );

        // Update local secondary index

        Ok(Some(backing_value))
    }

    pub fn upsert(&mut self, domain: Domain, value: Codomain) -> Result<Option<Codomain>, Error> {
        // Check local operations first - single lookup that handles all cases
        if let Some(entry) = self.index.local_operations.get_mut(&domain) {
            match &entry.operation {
                OpType::Delete => {
                    // Convert delete to insert - avoids duplicate key errors
                    entry.write_ts = self.tx.ts;
                    entry.operation = OpType::Insert(value.clone());
                    // Update local secondary index (insert after delete)
                    return Ok(None);
                }
                OpType::Insert(_) | OpType::Update(_) => {
                    // Update existing entry
                    entry.write_ts = self.tx.ts;
                    let is_update = matches!(entry.operation, OpType::Update(_));
                    let old_value = match std::mem::replace(
                        &mut entry.operation,
                        if is_update {
                            OpType::Update(value.clone())
                        } else {
                            OpType::Insert(value.clone())
                        },
                    ) {
                        OpType::Insert(old) | OpType::Update(old) => {
                            // Update local secondary index
                            old
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
                domain.clone(),
                Op {
                    read_ts: entry.ts,
                    write_ts: self.tx.ts,
                    operation: OpType::Update(value.clone()),
                },
            );
            // Update local secondary index
            return Ok(Some(old_value));
        }

        // Check backing source for existing data
        if let Some((read_ts, backing_value)) = self.backing_source.get(&domain)? {
            if read_ts < self.tx.ts {
                // Existing entry in backing - do update via local operation
                self.index.local_operations.insert(
                    domain.clone(),
                    Op {
                        read_ts,
                        write_ts: self.tx.ts,
                        operation: OpType::Update(value.clone()),
                    },
                );
                // Update local secondary index
                return Ok(Some(backing_value));
            }
        }

        // No existing entry anywhere - do insert via local operation
        self.index.local_operations.insert(
            domain.clone(),
            Op {
                read_ts: self.tx.ts,
                write_ts: self.tx.ts,
                operation: OpType::Insert(value.clone()),
            },
        );
        Ok(None)
    }

    pub fn has_domain(&self, domain: &Domain) -> Result<bool, Error> {
        Ok(self.get(domain)?.is_some())
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
        // Check local operations first.
        if let Some(op) = self.index.local_operations.get(domain) {
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

        // Try upstream.
        match self.backing_source.get(domain)? {
            Some((read_ts, value)) if read_ts < self.tx.ts => Ok(Some(value)),
            _ => Ok(None),
        }
    }

    pub fn delete(&mut self, domain: &Domain) -> Result<Option<Codomain>, Error> {
        // This is like update, but we're removing.
        // Check our local index first.
        // If we have an entry for this domain, we can delete it and move on
        if let Some(entry) = self.index.local_operations.get_mut(domain) {
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
            return Ok(Some(old_value));
        }

        if let Some(entry) = self.index.master_entries.index_lookup(domain) {
            let old_value = entry.value.clone();

            // Check to see if we already have an operations-log entry for this domain.
            // If we do, we can update it to its new state.
            if let Some(old_entry) = self.index.local_operations.get_mut(domain) {
                let local_old_value = match &old_entry.operation {
                    OpType::Update(value) | OpType::Insert(value) => Some(value.clone()),
                    OpType::Delete => None,
                };

                old_entry.operation = match old_entry.operation {
                    OpType::Update(_) => OpType::Delete,
                    OpType::Delete => {
                        return Ok(None);
                    }
                    OpType::Insert(_) => OpType::Delete,
                };
                old_entry.write_ts = self.tx.ts;

                // Update local secondary index (remove from old codomain)
                if let Some(_local_old) = local_old_value {
                } else {
                    // Fallback to master entry value if local entry was delete
                }

                return Ok(Some(old_value));
            } else {
                // Upstream may or may not have this key to delete, but we'll log the operation
                // anyways.
                let read_ts = entry.ts;
                self.index.local_operations.insert(
                    domain.clone(),
                    Op {
                        read_ts,
                        write_ts: self.tx.ts,
                        operation: OpType::Delete,
                    },
                );

                // Update local secondary index (remove from old codomain)
            }

            return Ok(Some(old_value));
        }

        // We had nothing in our local index, so let's ask upstream to attempt to pull in an "old"
        // value for it. We'll then update our own copy, and create an operations log entry for the
        // update

        // Not in the index, we check the backing source.
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
            },
        );

        // Update local secondary index (remove from old codomain)

        Ok(Some(backing_value))
    }

    pub fn scan<F>(&self, predicate: &F) -> Result<Vec<(Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        // TODO: this is brutally inefficient. we should be able to get the master index to populate
        //   the results for us, and then we can just merge in the local operations, but will require
        //   some refactoring of the RelationIndex trait to allow for that.

        // Bring in the results from the backing source first, indexed by domain, filtering
        // by the predicate.
        let mut results: HashMap<_, _> = self
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

        // Now, we need to merge in the master entries from the index.
        for (domain, entry) in self.index.master_entries.iter() {
            if !results.contains_key(domain)
                && entry.ts <= self.tx.ts
                && predicate(domain, &entry.value)
            {
                results.insert(domain.clone(), entry.value.clone());
            }
        }

        // And then scan our local operations.
        for (domain, op) in self.index.local_operations.iter() {
            match &op.operation {
                OpType::Insert(value) | OpType::Update(value) => {
                    if predicate(domain, value) {
                        results.insert(domain.clone(), value.clone());
                    }
                }
                OpType::Delete => {
                    // We don't include deletes in the results.
                }
            }
        }

        Ok(results.into_iter().collect())
    }

    pub fn working_set(self) -> Result<WorkingSet<Domain, Codomain>, WorldStateError> {
        Ok(WorkingSet::new(self.index.local_operations))
    }
}
