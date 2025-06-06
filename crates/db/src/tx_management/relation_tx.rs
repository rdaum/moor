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

use crate::tx_management::relation::{Entry, RelationIndex};
use crate::tx_management::{Canonical, Error, Timestamp, Tx};
use ahash::AHasher;
use indexmap::IndexMap;
use std::cell::RefCell;
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
    index: RefCell<Inner<Domain, Codomain>>,
    backing_source: Arc<Source>,
}

struct Inner<Domain, Codomain>
where
    Domain: Clone + Send + Sync + 'static,
    Codomain: Clone + PartialEq + Send + Sync + 'static,
{
    operations: IndexMap<Domain, Op, BuildHasherDefault<AHasher>>,
    entries: Box<dyn RelationIndex<Domain, Codomain>>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum OpType {
    Insert,
    Update,
    Delete,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Op {
    pub(crate) read_ts: Timestamp,
    pub(crate) write_ts: Timestamp,
    pub(crate) operation: OpType,
}

pub type WorkingSet<Domain, Codomain> = Vec<(Domain, Op, Option<Entry<Codomain>>)>;

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
            operations: IndexMap::default(),
            entries: canonical,
        };
        RelationTransaction {
            tx,
            index: RefCell::new(inner),
            backing_source: backing_source.into(),
        }
    }

    pub fn insert(
        &mut self,
        domain: Domain,
        value: Codomain,
    ) -> Result<(), Error> {
        let mut index = self.index.borrow_mut();
        // If we or upstream has already inserted this domain, we can't insert it again.
        if index.entries.index_lookup(&domain).is_some() {
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
        index.entries.insert_entry(self.tx.ts, domain.clone(), value);
        index.operations.insert(
            domain.clone(),
            Op {
                read_ts: self.tx.ts,
                write_ts: self.tx.ts,
                operation: OpType::Insert,
            },
        );

        Ok(())
    }

    pub fn update(
        &mut self,
        domain: &Domain,
        value: Codomain,
    ) -> Result<Option<Codomain>, Error> {
        let mut index = self.index.borrow_mut();

        // Is this already in the index?
        if let Some(entry) = index.entries.index_lookup(domain) {
            if entry.ts > self.tx.ts {
                // We can't update it, it's too new.
                return Ok(None);
            }

            let old_value = entry.value.clone();
            let read_ts = entry.ts;
            // Update the entry with new value
            index.entries.insert_entry(self.tx.ts, domain.clone(), value.clone());

            // Check to see if we already have an operations-log entry for this domain.
            // If we do, we can update it to its new state.
            if let Some(old_entry) = index.operations.get_mut(domain) {
                old_entry.operation = match old_entry.operation {
                    OpType::Update => OpType::Update,
                    OpType::Delete => {
                        return Ok(None);
                    }
                    OpType::Insert => OpType::Insert,
                };
                old_entry.write_ts = self.tx.ts;
            } else {
                // We need to entry in the ops log which has to be "update" since we're updating.
                index.operations.insert(
                    domain.clone(),
                    Op {
                        read_ts,
                        write_ts: self.tx.ts,
                        operation: OpType::Update,
                    },
                );
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

        // Remember for later.
        index.entries.insert_entry(self.tx.ts, domain.clone(), value);

        // If the timestamp is greater than our own, we can't update it, we shouldn't have seen it.
        if read_ts > self.tx.ts {
            return Ok(None);
        }

        // Put in operations log
        // Copy into the local cache, but with updated value.
        index.operations.insert(
            domain.clone(),
            Op {
                read_ts,
                write_ts: self.tx.ts,
                operation: OpType::Update,
            },
        );

        Ok(Some(backing_value))
    }

    pub fn upsert(
        &mut self,
        domain: Domain,
        value: Codomain,
    ) -> Result<Option<Codomain>, Error> {
        // TODO: We could probably more efficient about this, but there we bugs here before and this
        //   fixed them.
        if self.has_domain(&domain)? {
            return self.update(&domain, value);
        }
        self.insert(domain, value)?;
        Ok(None)
    }

    pub fn has_domain(&self, domain: &Domain) -> Result<bool, Error> {
        Ok(self.get(domain)?.is_some())
    }

    pub fn get(&self, domain: &Domain) -> Result<Option<Codomain>, Error> {
        let mut index = self.index.borrow_mut();

        // Check entries
        if let Some(entry) = index.entries.index_lookup(domain) {
            return Ok(Some(entry.value.clone()));
        }

        // Try upstream.
        match self.backing_source.get(domain)? {
            Some((read_ts, value)) if read_ts < self.tx.ts => {
                // Shove in local index.
                let entry = Entry {
                    ts: read_ts,
                    value,
                };
                let value = entry.value.clone();
                index.entries.insert_entry(read_ts, domain.clone(), value.clone());
                Ok(Some(value))
            }
            _ => Ok(None),
        }
    }

    pub fn delete(&mut self, domain: &Domain) -> Result<Option<Codomain>, Error> {
        // This is like update, but we're removing.
        let mut index = self.index.borrow_mut();

        if let Some(entry) = index.entries.remove_entry(domain) {
            let old_value = entry.value.clone();

            // Check to see if we already have an operations-log entry for this domain.
            // If we do, we can update it to its new state.
            if let Some(old_entry) = index.operations.get_mut(domain) {
                old_entry.operation = match old_entry.operation {
                    OpType::Update => OpType::Delete,
                    OpType::Delete => {
                        return Ok(None);
                    }
                    OpType::Insert => OpType::Delete,
                };
                old_entry.write_ts = self.tx.ts;
                return Ok(Some(old_value));
            } else {
                // Upstream may or may not have this key to delete, but we'll log the operation
                // anyways.
                let read_ts = entry.ts;
                index.operations.insert(
                    domain.clone(),
                    Op {
                        read_ts,
                        write_ts: self.tx.ts,
                        operation: OpType::Delete,
                    },
                );
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
        index.operations.insert(
            domain.clone(),
            Op {
                read_ts,
                write_ts: self.tx.ts,
                operation: OpType::Delete,
            },
        );

        Ok(Some(backing_value))
    }

    pub fn scan<F>(&self, predicate: &F) -> Result<Vec<(Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        // Scan in the upstream first, and then merge the set with local changes.
        let upstream = self.backing_source.scan(predicate)?;

        let mut index = self.index.borrow_mut();

        // Feed in everything from upstream that we don't have locally, as long as the timestamp
        // is less than our own.
        for (ts, d, c) in upstream {
            if index.entries.index_lookup(&d).is_some() {
                continue;
            };
            if ts > self.tx.ts {
                continue;
            }
            index.entries.insert_entry(ts, d.clone(), c.clone());
        }

        let mut results = Vec::new();

        // Now scan the merged local.
        for (domain, entry) in index.entries.iter() {
            if predicate(domain, &entry.value) {
                results.push((domain.clone(), entry.value.clone()));
            }
        }
        Ok(results)
    }

    pub fn working_set(self) -> WorkingSet<Domain, Codomain> {
        let mut index = self.index.into_inner();
        let mut working_set = Vec::new();
        for (domain, op) in index.operations {
            let codomain = index.entries.remove_entry(&domain);
            working_set.push((domain, op, codomain));
        }
        working_set
    }
}
