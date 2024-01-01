// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use moor_values::util::slice_ref::SliceRef;

use crate::tuplebox::slots::SlotBox;
use crate::tuplebox::tb::{RelationInfo, TupleBox};
use crate::tuplebox::tuples::{Tuple, TupleError, TxTuple};
use crate::tuplebox::RelationId;

/// The local tx "working set" of mutations to base relations, and consists of the set of operations
/// we will attempt to make permanent when the transaction commits.
/// The working set is also referred to for reads/updates during the lifetime of the transaction.  
/// It effectively "is" the transaction in regards to *base relations*.
pub struct WorkingSet {
    pub(crate) ts: u64,
    pub(crate) slotbox: Arc<SlotBox>,
    pub(crate) relations: Vec<TxBaseRelation>,
}

impl WorkingSet {
    pub(crate) fn new(slotbox: Arc<SlotBox>, schema: &[RelationInfo], ts: u64) -> Self {
        let mut relations = Vec::new();
        for r in schema {
            relations.push(TxBaseRelation {
                tuples: vec![],
                domain_index: HashMap::new(),
                codomain_index: if r.secondary_indexed {
                    Some(HashMap::new())
                } else {
                    None
                },
            });
        }
        Self {
            ts,
            slotbox,
            relations,
        }
    }

    pub(crate) fn clear(&mut self) {
        for rel in self.relations.iter_mut() {
            rel.clear();
        }
    }

    pub(crate) async fn seek_by_domain(
        &mut self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        domain: SliceRef,
    ) -> Result<(SliceRef, SliceRef), TupleError> {
        let relation = &mut self.relations[relation_id.0];

        // Check local first.
        if let Some(tuple_id) = relation.domain_index.get(domain.as_slice()) {
            let local_version = relation.tuples.get(*tuple_id).unwrap();
            return match &local_version {
                TxTuple::Insert(t) | TxTuple::Update(t) | TxTuple::Value(t) => {
                    let t = t.get();
                    Ok((t.domain().clone(), t.codomain().clone()))
                }
                TxTuple::Tombstone { .. } => Err(TupleError::NotFound),
            };
        }

        let canon_t = db
            .with_relation(relation_id, |relation| {
                if let Some(tuple) = relation.seek_by_domain(domain.clone()) {
                    Ok(tuple.clone())
                } else {
                    Err(TupleError::NotFound)
                }
            })
            .await?;
        let tuple_id = relation.tuples.len();
        relation.tuples.push(TxTuple::Value(canon_t.clone()));
        relation
            .domain_index
            .insert(domain.as_slice().to_vec(), tuple_id);
        let t = canon_t.get();
        if let Some(ref mut codomain_index) = relation.codomain_index {
            codomain_index
                .entry(t.codomain().as_slice().to_vec())
                .or_insert_with(HashSet::new)
                .insert(tuple_id);
        }
        Ok((t.domain(), t.codomain()))
    }

    pub(crate) async fn seek_by_codomain(
        &mut self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        codomain: SliceRef,
    ) -> Result<Vec<(SliceRef, SliceRef)>, TupleError> {
        // The codomain index is not guaranteed to be up to date with the working set, so we need
        // to go back to the canonical relation, get the list of domains, then materialize them into
        // our local working set -- which will update the codomain index -- and then actually
        // use the local index.  Complicated enough?
        // TODO: There is likely a way to optimize this so we're not doing this when not necessary.
        //   but we'll need a round of really good coherence tests before we can do that.
        let tuples_for_codomain = {
            let relation = &self.relations[relation_id.0];

            // If there's no secondary index, we panic.  You should not have tried this.
            if relation.codomain_index.is_none() {
                panic!("Attempted to seek by codomain on a relation with no secondary index");
            }

            db.with_relation(relation_id, |relation| {
                relation.seek_by_codomain(codomain.clone())
            })
            .await
        };
        // By performing the seek, we'll materialize the tuples into our local working set, which
        // will in turn update the codomain index for those tuples.
        for tuples in tuples_for_codomain {
            let _ = self
                .seek_by_domain(db.clone(), relation_id, tuples.get().domain())
                .await;
        }

        let relation = &mut self.relations[relation_id.0];
        let codomain_index = relation.codomain_index.as_ref().expect("No codomain index");
        let tuple_ids = codomain_index
            .get(codomain.as_slice())
            .cloned()
            .unwrap_or_else(|| HashSet::new())
            .into_iter();
        let tuples = tuple_ids.filter_map(|tid| {
            let t = relation.tuples.get(tid).expect("Tuple not found");
            match &t {
                TxTuple::Insert(t) | TxTuple::Update(t) | TxTuple::Value(t) => {
                    let t = t.get();
                    Some((t.domain(), t.codomain()))
                }
                TxTuple::Tombstone { .. } => None,
            }
        });
        Ok(tuples.collect())
    }

    pub(crate) async fn insert_tuple(
        &mut self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let relation = &mut self.relations[relation_id.0];

        // If we already have a local version, that's a dupe, so return an error for that.
        if let Some(_) = relation.domain_index.get(domain.as_slice()) {
            return Err(TupleError::Duplicate);
        }

        db.with_relation(relation_id, |relation| {
            if let Some(_) = relation.seek_by_domain(domain.clone()) {
                // If there's a canonical version, we can't insert, so return an error.
                return Err(TupleError::Duplicate);
            }
            Ok(())
        })
        .await?;

        let tuple_id = relation.tuples.len();
        let new_t = Tuple::allocate(
            self.slotbox.clone(),
            self.ts,
            domain.as_slice(),
            codomain.as_slice(),
        );
        relation.tuples.push(TxTuple::Insert(new_t));
        relation
            .domain_index
            .insert(domain.as_slice().to_vec(), tuple_id);
        relation.update_secondary(tuple_id, None, Some(codomain.clone()));

        Ok(())
    }

    pub(crate) async fn predicate_scan<F: Fn(&(SliceRef, SliceRef)) -> bool>(
        &self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        f: F,
    ) -> Result<Vec<(SliceRef, SliceRef)>, TupleError> {
        // First collect all the tuples from the canonical relation, indexing them by domain.
        let tuples = db
            .with_relation(relation_id, |relation| relation.predicate_scan(&f))
            .await;

        let mut by_domain = HashMap::new();
        for t in tuples {
            let t = t.get();
            by_domain.insert(t.domain().as_slice().to_vec(), t);
        }

        // Now pull in the local working set.
        // Apply any changes to the tuples we've already collected, and add in any inserts, and
        // remove any tombstones.
        let relation = &self.relations[relation_id.0];

        for t in &relation.tuples {
            if t.ts() > self.ts {
                continue;
            }
            match t {
                TxTuple::Insert(t) | TxTuple::Update(t) | TxTuple::Value(t) => {
                    let t = t.get();
                    if f(&(t.domain(), t.codomain())) {
                        by_domain.insert(t.domain().as_slice().to_vec(), t);
                    } else {
                        by_domain.remove(t.domain().as_slice());
                    }
                }
                TxTuple::Tombstone { domain, ts: _ } => {
                    by_domain.remove(domain.as_slice());
                }
            }
        }

        // Now we have a map of domain -> tuple, so we can just pull out the tuples and return them.
        Ok(by_domain
            .into_iter()
            .map(|(_, t)| (t.domain(), t.codomain()))
            .collect())
    }

    pub(crate) async fn update_tuple(
        &mut self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let relation = &mut self.relations[relation_id.0];

        // If we have an existing copy, we will update it, but keep its existing derivation
        // timestamp and operation type.
        if let Some(tuple_id) = relation.domain_index.get_mut(domain.as_slice()).cloned() {
            let existing = relation.tuples.get_mut(tuple_id).expect("Tuple not found");
            let (replacement, old_value) = match &existing {
                TxTuple::Tombstone { .. } => return Err(TupleError::NotFound),
                TxTuple::Insert(t) => {
                    let t = t.get();
                    let new_t = Tuple::allocate(
                        self.slotbox.clone(),
                        t.ts(),
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (TxTuple::Insert(new_t), (t.domain(), t.codomain()))
                }
                TxTuple::Update(t) | TxTuple::Value(t) => {
                    let t = t.get();
                    let new_t = Tuple::allocate(
                        self.slotbox.clone(),
                        t.ts(),
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (TxTuple::Update(new_t), (t.domain(), t.codomain()))
                }
            };
            *existing = replacement;
            relation.update_secondary(tuple_id, Some(old_value.1), Some(codomain.clone()));
            return Ok(());
        }

        // Check canonical for an existing value.  And get its timestamp if it exists.
        // We will use the ts on that to determine the derivation timestamp for our own version.
        // If there's nothing there or its tombstoned, that's NotFound, and die.
        let (old_codomain, ts) = db
            .with_relation(relation_id, |relation| {
                if let Some(tuple) = relation.seek_by_domain(domain.clone()) {
                    let tuple = tuple.get();
                    Ok((tuple.codomain().clone(), tuple.ts()))
                } else {
                    Err(TupleError::NotFound)
                }
            })
            .await?;

        // Write into the local copy an update operation.
        let tuple_id = relation.tuples.len();
        let new_t = Tuple::allocate(
            self.slotbox.clone(),
            ts,
            domain.as_slice(),
            codomain.as_slice(),
        );
        relation.tuples.push(TxTuple::Update(new_t));
        relation
            .domain_index
            .insert(domain.as_slice().to_vec(), tuple_id);
        relation.update_secondary(tuple_id, Some(old_codomain), Some(codomain.clone()));
        Ok(())
    }

    /// Attempt to upsert a tuple in the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub(crate) async fn upsert_tuple(
        &mut self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let relation = &mut self.relations[relation_id.0];

        // If we have an existing copy, we will update it, but keep its existing derivation
        // timestamp.
        // If it's an insert, we have to keep it an insert, same for update, but if it's a delete,
        // we have to turn it into an update.
        if let Some(tuple_id) = relation.domain_index.get_mut(domain.as_slice()).cloned() {
            let existing = relation.tuples.get_mut(tuple_id).expect("Tuple not found");
            let (replacement, old) = match &existing {
                TxTuple::Insert(t) => {
                    let t = t.get();
                    let new_t = Tuple::allocate(
                        self.slotbox.clone(),
                        t.ts(),
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (TxTuple::Insert(new_t), Some((t.domain(), t.codomain())))
                }
                TxTuple::Tombstone { ts, .. } => {
                    // We need to allocate a new tuple...
                    let new_t = Tuple::allocate(
                        self.slotbox.clone(),
                        *ts,
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (TxTuple::Update(new_t), None)
                }
                TxTuple::Update(t) | TxTuple::Value(t) => {
                    let tuple = t.get();
                    let new_t = Tuple::allocate(
                        self.slotbox.clone(),
                        tuple.ts(),
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (
                        TxTuple::Update(new_t),
                        Some((tuple.domain(), tuple.codomain())),
                    )
                }
            };
            *existing = replacement;
            relation.update_secondary(tuple_id, old.map(|o| o.1), Some(codomain.clone()));
            return Ok(());
        }

        // Check canonical for an existing value.  And get its timestamp if it exists.
        // We will use the ts on that to determine the derivation timestamp for our own version.
        // If there is no value there, we will use the current transaction timestamp, but it's
        // an insert rather than an update.
        let (operation, old) = db
            .with_relation(relation_id, |relation| {
                if let Some(tuple) = relation.seek_by_domain(domain.clone()) {
                    let tuple = tuple.get();
                    let new_t = Tuple::allocate(
                        self.slotbox.clone(),
                        tuple.ts(),
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (
                        TxTuple::Update(new_t),
                        Some((tuple.domain(), tuple.codomain())),
                    )
                } else {
                    let new_t = Tuple::allocate(
                        self.slotbox.clone(),
                        self.ts,
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (TxTuple::Insert(new_t), None)
                }
            })
            .await;
        let tuple_id = relation.tuples.len();
        relation.tuples.push(operation);
        relation
            .domain_index
            .insert(domain.as_slice().to_vec(), tuple_id);

        // Remove the old codomain->domain index entry if it exists, and then add the new one.
        relation.update_secondary(tuple_id, old.map(|o| o.0), Some(codomain.clone()));
        Ok(())
    }

    /// Attempt to delete a tuple in the transaction's working set, with the intent of eventually
    /// committing the delete to the canonical base relations.
    pub(crate) async fn remove_by_domain(
        &mut self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        domain: SliceRef,
    ) -> Result<(), TupleError> {
        let relation = &mut self.relations[relation_id.0];

        // Delete is basically an update but where we stick a Tombstone.
        if let Some(tuple_index) = relation.domain_index.get_mut(domain.as_slice()).cloned() {
            let tuple_v = relation
                .tuples
                .get_mut(tuple_index)
                .expect("Tuple not found");

            let old_v = match &tuple_v {
                TxTuple::Insert(t) | TxTuple::Update(t) | TxTuple::Value(t) => {
                    let t = t.get();
                    (t.domain(), t.codomain())
                }
                TxTuple::Tombstone { .. } => {
                    return Err(TupleError::NotFound);
                }
            };
            *tuple_v = TxTuple::Tombstone {
                ts: tuple_v.ts(),
                domain: domain.clone(),
            };
            relation.update_secondary(tuple_index, Some(old_v.1), None);
            return Ok(());
        }

        let (ts, old) = db
            .with_relation(relation_id, |relation| {
                if let Some(tuple) = relation.seek_by_domain(domain.clone()) {
                    let tuple = tuple.get();
                    Ok((tuple.ts(), tuple.codomain().clone()))
                } else {
                    Err(TupleError::NotFound)
                }
            })
            .await?;

        let tuple_id = relation.tuples.len();
        relation.tuples.push(TxTuple::Tombstone {
            ts,
            domain: domain.clone(),
        });
        relation
            .domain_index
            .insert(domain.as_slice().to_vec(), tuple_id);
        relation.update_secondary(tuple_id, Some(old), None);
        Ok(())
    }
}

/// The transaction-local storage for tuples in relations derived from base relations.
pub(crate) struct TxBaseRelation {
    tuples: Vec<TxTuple>,
    domain_index: HashMap<Vec<u8>, usize>,
    codomain_index: Option<HashMap<Vec<u8>, HashSet<usize>>>,
}

impl TxBaseRelation {
    pub fn tuples(&self) -> impl Iterator<Item = &TxTuple> {
        self.tuples.iter()
    }

    pub(crate) fn clear(&mut self) {
        self.tuples.clear();
        self.domain_index.clear();
        if let Some(index) = self.codomain_index.as_mut() {
            index.clear();
        }
    }

    /// Update the secondary index for the given tuple.
    pub(crate) fn update_secondary(
        &mut self,
        tuple_id: usize,
        old_codomain: Option<SliceRef>,
        new_codomain: Option<SliceRef>,
    ) {
        let Some(codomain_index) = self.codomain_index.as_mut() else {
            return;
        };

        // Clear out the old entry, if there was one.
        if let Some(old_codomain) = old_codomain {
            let old_codomain_bytes = old_codomain.as_slice().to_vec();
            codomain_index
                .entry(old_codomain_bytes)
                .or_insert_with(HashSet::new)
                .remove(&tuple_id);
        }
        if let Some(new_codomain) = new_codomain {
            let codomain_bytes = new_codomain.as_slice().to_vec();
            codomain_index
                .entry(codomain_bytes)
                .or_insert_with(HashSet::new)
                .insert(tuple_id);
        }
    }
}
