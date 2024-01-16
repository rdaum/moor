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

use moor_values::util::{BitArray, Bitset64};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use moor_values::util::SliceRef;

use crate::rdb::paging::TupleBox;
use crate::rdb::relbox::{RelBox, RelationInfo};
use crate::rdb::tuples::{TupleRef, TxTuple};
use crate::rdb::{RelationId, TupleError};

/// The local tx "working set" of mutations to base relations, and consists of the set of operations
/// we will attempt to make permanent when the transaction commits.
/// The working set is also referred to for reads/updates during the lifetime of the transaction.  
/// It effectively "is" the transaction in regards to *base relations*.
// TODO: see comments on BaseRelation, changes there will reqiure changes here.
pub struct WorkingSet {
    pub(crate) ts: u64,
    pub(crate) schema: Vec<RelationInfo>,
    pub(crate) slotbox: Arc<TupleBox>,
    pub(crate) relations: BitArray<TxBaseRelation, 64, Bitset64<1>>,
}

impl WorkingSet {
    pub(crate) fn new(slotbox: Arc<TupleBox>, schema: &[RelationInfo], ts: u64) -> Self {
        let relations = BitArray::new();
        Self {
            ts,
            slotbox,
            schema: schema.to_vec(),
            relations,
        }
    }

    pub(crate) fn clear(&mut self) {
        for rel in self.relations.iter_mut() {
            // let Some(rel) = rel else { continue };
            rel.1.clear();
        }
    }

    fn get_relation_mut<'a>(
        relation_id: RelationId,
        schema: &[RelationInfo],
        relations: &'a mut BitArray<TxBaseRelation, 64, Bitset64<1>>,
    ) -> &'a mut TxBaseRelation {
        if relations.check(relation_id.0) {
            return relations.get_mut(relation_id.0).unwrap();
        }
        let r = &schema[relation_id.0];
        let new_relation = TxBaseRelation {
            id: relation_id,
            tuples: Vec::new(),
            domain_index: HashMap::new(),
            codomain_index: if r.secondary_indexed {
                Some(HashMap::new())
            } else {
                None
            },
        };

        relations.set(relation_id.0, new_relation);
        relations.get_mut(relation_id.0).unwrap()
    }

    pub(crate) async fn seek_by_domain(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        domain: SliceRef,
    ) -> Result<TupleRef, TupleError> {
        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);

        // Check local first.
        if let Some(tuple_idx) = relation.domain_index.get(&domain) {
            let local_version = relation.tuples.get(*tuple_idx).unwrap();
            return match &local_version {
                TxTuple::Insert(t) | TxTuple::Update(_, t) | TxTuple::Value(_, t) => Ok(t.clone()),
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
        let tuple_idx = relation.tuples.len();
        relation
            .tuples
            .push(TxTuple::Value(canon_t.id(), canon_t.clone()));
        relation.domain_index.insert(domain, tuple_idx);
        if let Some(ref mut codomain_index) = relation.codomain_index {
            codomain_index
                .entry(canon_t.codomain())
                .or_insert_with(HashSet::new)
                .insert(tuple_idx);
        }
        Ok(canon_t)
    }

    pub(crate) async fn seek_by_codomain(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        codomain: SliceRef,
    ) -> Result<HashSet<TupleRef>, TupleError> {
        // The codomain index is not guaranteed to be up to date with the working set, so we need
        // to go back to the canonical relation, get the list of domains, then materialize them into
        // our local working set -- which will update the codomain index -- and then actually
        // use the local index.  Complicated enough?
        // TODO: There is likely a way to optimize this so we're not doing this when not necessary.
        //   but we'll need a round of really good coherence tests before we can do that.
        let tuples_for_codomain = {
            let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);

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
        for tuple in tuples_for_codomain {
            let _ = self.seek_by_domain(db, relation_id, tuple.domain()).await;
        }

        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);
        let codomain_index = relation.codomain_index.as_ref().expect("No codomain index");
        let tuple_indexes = codomain_index
            .get(&codomain)
            .cloned()
            .unwrap_or_else(HashSet::new)
            .into_iter();
        let tuples = tuple_indexes.filter_map(|tid| {
            let t = relation.tuples.get(tid).expect("Tuple not found");
            match &t {
                TxTuple::Insert(t) | TxTuple::Update(_, t) | TxTuple::Value(_, t) => {
                    Some(t.clone())
                }
                TxTuple::Tombstone { .. } => None,
            }
        });
        Ok(tuples.collect())
    }

    pub(crate) async fn insert_tuple(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);

        // If we already have a local version, that's a dupe, so return an error for that.
        if relation.domain_index.get(&domain).is_some() {
            return Err(TupleError::Duplicate);
        }

        db.with_relation(relation_id, |relation| {
            if relation.seek_by_domain(domain.clone()).is_some() {
                // If there's a canonical version, we can't insert, so return an error.
                return Err(TupleError::Duplicate);
            }
            Ok(())
        })
        .await?;

        let tuple_idx = relation.tuples.len();
        let new_t = TupleRef::allocate(
            relation_id,
            self.slotbox.clone(),
            self.ts,
            domain.as_slice(),
            codomain.as_slice(),
        );
        relation.tuples.push(TxTuple::Insert(new_t.unwrap()));
        relation.domain_index.insert(domain, tuple_idx);
        relation.update_secondary(tuple_idx, None, Some(codomain.clone()));

        Ok(())
    }

    pub(crate) async fn predicate_scan<F: Fn(&TupleRef) -> bool>(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        f: F,
    ) -> Result<Vec<TupleRef>, TupleError> {
        // First collect all the tuples from the canonical relation, indexing them by domain.
        let tuples = db
            .with_relation(relation_id, |relation| relation.predicate_scan(&f))
            .await;

        let mut by_domain = HashMap::new();
        for t in tuples {
            by_domain.insert(t.domain().as_slice().to_vec(), t);
        }

        // Now pull in the local working set.
        // Apply any changes to the tuples we've already collected, and add in any inserts, and
        // remove any tombstones.
        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);
        for t in &relation.tuples {
            if t.ts() > self.ts {
                continue;
            }
            match t {
                TxTuple::Insert(t) | TxTuple::Update(_, t) | TxTuple::Value(_, t) => {
                    if f(t) {
                        by_domain.insert(t.domain().as_slice().to_vec(), t.clone());
                    } else {
                        by_domain.remove(t.domain().as_slice());
                    }
                }
                TxTuple::Tombstone { domain, .. } => {
                    by_domain.remove(domain.as_slice());
                }
            }
        }

        // Now we have a map of domain -> tuple, so we can just pull out the tuples and return them.
        Ok(by_domain.into_values().collect())
    }

    pub(crate) async fn update_tuple(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);

        // If we have an existing copy, we will update it, but keep its existing derivation
        // timestamp and operation type.
        if let Some(tuple_idx) = relation.domain_index.get_mut(&domain).cloned() {
            let existing = relation.tuples.get_mut(tuple_idx).expect("Tuple not found");
            let (replacement, old_value) = match &existing {
                TxTuple::Tombstone { .. } => return Err(TupleError::NotFound),
                TxTuple::Insert(t) => {
                    let new_t = TupleRef::allocate(
                        relation_id,
                        self.slotbox.clone(),
                        t.ts(),
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (TxTuple::Insert(new_t.unwrap()), (t.domain(), t.codomain()))
                }
                TxTuple::Update(_id, t) | TxTuple::Value(_id, t) => {
                    let new_t = TupleRef::allocate(
                        relation_id,
                        self.slotbox.clone(),
                        t.ts(),
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    let tuple_ref = new_t.unwrap();
                    (
                        TxTuple::Update(tuple_ref.id(), tuple_ref),
                        (t.domain(), t.codomain()),
                    )
                }
            };
            *existing = replacement;
            relation.update_secondary(tuple_idx, Some(old_value.1), Some(codomain.clone()));
            return Ok(());
        }

        // Check canonical for an existing value.  And get its timestamp if it exists.
        // We will use the ts on that to determine the derivation timestamp for our own version.
        // If there's nothing there or its tombstoned, that's NotFound, and die.
        let old_tuple = db
            .with_relation(relation_id, |relation| {
                if let Some(tuple) = relation.seek_by_domain(domain.clone()) {
                    Ok(tuple)
                } else {
                    Err(TupleError::NotFound)
                }
            })
            .await?;

        // Write into the local copy an update operation.
        let tuple_idx = relation.tuples.len();
        let new_t = TupleRef::allocate(
            relation_id,
            self.slotbox.clone(),
            old_tuple.ts(),
            domain.as_slice(),
            codomain.as_slice(),
        );
        let new_t = new_t.unwrap();
        relation.tuples.push(TxTuple::Update(old_tuple.id(), new_t));
        relation.domain_index.insert(domain, tuple_idx);
        relation.update_secondary(
            tuple_idx,
            Some(old_tuple.codomain()),
            Some(codomain.clone()),
        );
        Ok(())
    }

    /// Attempt to upsert a tuple in the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub(crate) async fn upsert_tuple(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);

        // If we have an existing copy, we will update it, but keep its existing derivation
        // timestamp.
        // If it's an insert, we have to keep it an insert, same for update, but if it's a delete,
        // we have to turn it into an update.
        if let Some(tuple_idx) = relation.domain_index.get_mut(&domain).cloned() {
            let existing = relation.tuples.get_mut(tuple_idx).expect("Tuple not found");
            let (replacement, old) = match &existing {
                TxTuple::Insert(t) => {
                    let new_t = TupleRef::allocate(
                        relation_id,
                        self.slotbox.clone(),
                        t.ts(),
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (
                        TxTuple::Insert(new_t.unwrap()),
                        Some((t.domain(), t.codomain())),
                    )
                }
                TxTuple::Tombstone { ts, tuple_id, .. } => {
                    // We need to allocate a new tuple...
                    let new_t = TupleRef::allocate(
                        relation_id,
                        self.slotbox.clone(),
                        *ts,
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (TxTuple::Update(*tuple_id, new_t.unwrap()), None)
                }
                TxTuple::Update(id, tuple) | TxTuple::Value(id, tuple) => {
                    let new_t = TupleRef::allocate(
                        relation_id,
                        self.slotbox.clone(),
                        tuple.ts(),
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (
                        TxTuple::Update(*id, new_t.unwrap()),
                        Some((tuple.domain(), tuple.codomain())),
                    )
                }
            };
            *existing = replacement;
            relation.update_secondary(tuple_idx, old.map(|o| o.1), Some(codomain.clone()));
            return Ok(());
        }

        // Check canonical for an existing value.  And get its timestamp if it exists.
        // We will use the ts on that to determine the derivation timestamp for our own version.
        // If there is no value there, we will use the current transaction timestamp, but it's
        // an insert rather than an update.
        let (operation, old) = db
            .with_relation(relation_id, |relation| {
                if let Some(old_tuple) = relation.seek_by_domain(domain.clone()) {
                    let new_t = TupleRef::allocate(
                        relation_id,
                        self.slotbox.clone(),
                        old_tuple.ts(),
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (
                        TxTuple::Update(old_tuple.id(), new_t.unwrap()),
                        Some(old_tuple),
                    )
                } else {
                    let new_t = TupleRef::allocate(
                        relation_id,
                        self.slotbox.clone(),
                        self.ts,
                        domain.as_slice(),
                        codomain.as_slice(),
                    );
                    (TxTuple::Insert(new_t.unwrap()), None)
                }
            })
            .await;
        let tuple_idx = relation.tuples.len();
        relation.tuples.push(operation);
        relation.domain_index.insert(domain, tuple_idx);

        // Remove the old codomain->domain index entry if it exists, and then add the new one.
        relation.update_secondary(tuple_idx, old.map(|o| o.domain()), Some(codomain.clone()));
        Ok(())
    }

    /// Attempt to delete a tuple in the transaction's working set, with the intent of eventually
    /// committing the delete to the canonical base relations.
    pub(crate) async fn remove_by_domain(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        domain: SliceRef,
    ) -> Result<(), TupleError> {
        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);

        // Delete is basically an update but where we stick a Tombstone.
        if let Some(tuple_index) = relation.domain_index.get_mut(&domain).cloned() {
            let tuple_v = relation
                .tuples
                .get_mut(tuple_index)
                .expect("Tuple not found");

            // If the tuple was an insert, we should just remove it.
            // If it was an update, we can just replace it with a tombstone.
            // If it was a tombstone, we can just return NotFound.

            let old_v = match &tuple_v {
                TxTuple::Insert(_) => {
                    relation.tuples.remove(tuple_index);
                    relation.domain_index.remove(&domain);
                    relation.update_secondary(tuple_index, None, None);
                    return Ok(());
                }
                TxTuple::Update(_, t) | TxTuple::Value(_, t) => t.clone(),
                TxTuple::Tombstone { .. } => {
                    return Err(TupleError::NotFound);
                }
            };
            *tuple_v = TxTuple::Tombstone {
                ts: tuple_v.ts(),
                tuple_id: tuple_v.origin_tuple_id(),
                domain: domain.clone(),
            };
            relation.update_secondary(tuple_index, Some(old_v.codomain()), None);
            return Ok(());
        }

        let (ts, old_codomain, tuple) = db
            .with_relation(relation_id, |relation| {
                if let Some(tuple) = relation.seek_by_domain(domain.clone()) {
                    Ok((tuple.ts(), tuple.codomain().clone(), tuple))
                } else {
                    Err(TupleError::NotFound)
                }
            })
            .await?;

        let local_tuple_idx = relation.tuples.len();
        relation.tuples.push(TxTuple::Tombstone {
            ts,
            domain: domain.clone(),
            tuple_id: tuple.id(),
        });
        relation.domain_index.insert(domain, local_tuple_idx);
        relation.update_secondary(local_tuple_idx, Some(old_codomain), None);
        Ok(())
    }
}

/// The transaction-local storage for tuples in relations derived from base relations.
pub(crate) struct TxBaseRelation {
    pub id: RelationId,
    tuples: Vec<TxTuple>,
    domain_index: HashMap<SliceRef, usize>,
    codomain_index: Option<HashMap<SliceRef, HashSet<usize>>>,
}

impl TxBaseRelation {
    pub fn tuples(&self) -> impl Iterator<Item = &TxTuple> {
        self.tuples.iter()
    }

    pub fn tuples_mut(&mut self) -> impl Iterator<Item = &mut TxTuple> {
        self.tuples.iter_mut()
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
            codomain_index
                .entry(old_codomain)
                .or_insert_with(HashSet::new)
                .remove(&tuple_id);
        }
        if let Some(new_codomain) = new_codomain {
            codomain_index
                .entry(new_codomain)
                .or_insert_with(HashSet::new)
                .insert(tuple_id);
        }
    }
}
