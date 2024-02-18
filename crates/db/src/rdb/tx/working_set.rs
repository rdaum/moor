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

use moor_values::util::{BitArray, Bitset64};
use moor_values::util::{PhantomUnsend, PhantomUnsync, SliceRef};

use crate::rdb::index::{HashIndex, Index};
use crate::rdb::paging::TupleBox;
use crate::rdb::relbox::{RelBox, RelationInfo};
use crate::rdb::tuples::{TupleId, TupleRef};
use crate::rdb::tx::tx_tuple::{DataSource, OpSource, TupleApply, TxTupleEvent, TxTupleOp};
use crate::rdb::{RelationError, RelationId};

/// The local tx "working set" of mutations to base relations, and consists of the set of operations
/// we will attempt to make permanent when the transaction commits.
/// The working set is also referred to for reads/updates during the lifetime of the transaction.  
/// It effectively "is" the transaction in regards to *base relations*.
pub struct WorkingSet {
    pub(crate) ts: u64,
    pub(crate) schema: Vec<RelationInfo>,
    pub(crate) tuplebox: Arc<TupleBox>,
    pub(crate) relations: Box<BitArray<TxBaseRelation, 64, Bitset64<1>>>,

    unsend: PhantomUnsend,
    unsync: PhantomUnsync,
}

impl WorkingSet {
    pub(crate) fn new(slotbox: Arc<TupleBox>, schema: &[RelationInfo], ts: u64) -> Self {
        let relations = Box::new(BitArray::new());
        Self {
            ts,
            tuplebox: slotbox,
            schema: schema.to_vec(),
            relations,
            unsend: Default::default(),
            unsync: Default::default(),
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
            relation_info: r.clone(),
            tx_tuple_events: HashMap::new(),
            index: Box::new(HashIndex::new(r.clone())),
            unsend: Default::default(),
            unsync: Default::default(),
        };

        relations.set(relation_id.0, new_relation);
        relations.get_mut(relation_id.0).unwrap()
    }

    pub(crate) fn seek_by_domain(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        domain: SliceRef,
    ) -> Result<HashSet<TupleRef>, RelationError> {
        let relation = Self::get_relation_mut(relation_id, &self.schema, self.relations.as_mut());

        // Get the list of matches from the base relation, and then apply the local working set overtop.
        let tuples = db.with_relation(relation_id, |relation| {
            relation.seek_by_domain(domain.clone())
        });

        // Stash local references to the tuple we've seen, in case updates happen upstream.
        for t in tuples {
            let apply = TupleApply {
                data_source: DataSource::Base,
                op_source: OpSource::Seek,
                replacement_op: Some(TxTupleOp::Value(t.clone())),
                add_tuple: Some(t.clone()),
                del_tuple: None,
            };
            relation.tuple_apply(apply)?;
        }

        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);
        let domain_tuples = relation.index.seek_domain(&domain);
        let tuples = domain_tuples.filter_map(|tid| {
            let t = relation.tx_tuple_events.get(&tid).unwrap();
            match &t.op {
                TxTupleOp::Insert(t)
                | TxTupleOp::Update { to_tuple: t, .. }
                | TxTupleOp::Value(t) => Some(t.clone()),
                TxTupleOp::Tombstone { .. } => None,
            }
        });

        Ok(tuples.collect())
    }

    pub(crate) fn seek_unique_by_domain(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        domain: SliceRef,
    ) -> Result<TupleRef, RelationError> {
        let relation = Self::get_relation_mut(relation_id, &self.schema, self.relations.as_mut());

        // Check local first.
        {
            let mut tuple_ids = relation.index.seek_domain(&domain);
            if let Some(tid) = tuple_ids.next() {
                if tuple_ids.next().is_some() {
                    return Err(RelationError::AmbiguousTuple);
                }
                let local_version_op = relation.tx_tuple_events.get(&tid).unwrap();
                return match &local_version_op.op {
                    TxTupleOp::Insert(t)
                    | TxTupleOp::Update { to_tuple: t, .. }
                    | TxTupleOp::Value(t) => Ok(t.clone()),
                    TxTupleOp::Tombstone { .. } => Err(RelationError::TupleNotFound),
                };
            }
        }
        let canon_t = db.with_relation(relation_id, |relation| {
            let tuples = relation.seek_by_domain(domain.clone());
            if tuples.is_empty() {
                return Err(RelationError::TupleNotFound);
            }
            if tuples.len() > 1 {
                // We expected a unique value, but got more than one.
                return Err(RelationError::AmbiguousTuple);
            }
            Ok(tuples.into_iter().next().unwrap())
        })?;

        // Stash a local reference to the tuple we've seen, in case updates happen upstream.
        let apply = TupleApply {
            data_source: DataSource::Base,
            op_source: OpSource::Seek,
            replacement_op: Some(TxTupleOp::Value(canon_t.clone())),
            add_tuple: Some(canon_t.clone()),
            del_tuple: None,
        };
        relation.tuple_apply(apply)?;
        Ok(canon_t)
    }

    pub(crate) fn seek_by_codomain(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        codomain: SliceRef,
    ) -> Result<HashSet<TupleRef>, RelationError> {
        // The codomain index is not guaranteed to be up to date with the working set, so we need
        // to go back to the canonical relation, get the list of tuples for the codomain, then materialize
        // them into our local working set -- which will update the codomain index -- and then actually
        // use the local index.  Complicated enough?
        let tuples_for_codomain = {
            let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);

            // If there's no secondary index, we panic.  You should not have tried this.
            if !relation.relation_info.secondary_indexed {
                panic!("Attempted to seek by codomain on a relation with no secondary index");
            }

            db.with_relation(relation_id, |relation| {
                relation.seek_by_codomain(codomain.clone())
            })
        };
        // By performing the seek, we'll materialize the tuples into our local working set, which
        // will in turn update the codomain index for those tuples.
        for tuple in tuples_for_codomain {
            let _ = self.seek_unique_by_domain(db, relation_id, tuple.domain());
        }

        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);

        let codomain_tuples = relation.index.seek_codomain(&codomain);
        let tuples = codomain_tuples.filter_map(|tid| {
            let t = relation.tx_tuple_events.get(&tid).unwrap();
            match &t.op {
                TxTupleOp::Insert(t)
                | TxTupleOp::Update { to_tuple: t, .. }
                | TxTupleOp::Value(t) => Some(t.clone()),
                TxTupleOp::Tombstone { .. } => None,
            }
        });
        Ok(tuples.collect())
    }

    pub(crate) fn insert_tuple(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), RelationError> {
        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);

        // Enforce unique domain constraint before doing anything else
        relation.index.check_domain_constraints(&domain)?;
        db.with_relation(relation_id, |relation| {
            relation.check_domain_constraints(&domain)?;
            Ok(())
        })?;

        let new_t = TupleRef::allocate(
            relation_id,
            self.tuplebox.clone(),
            self.ts,
            domain.as_slice(),
            codomain.as_slice(),
        )
        .unwrap();
        let apply = TupleApply {
            data_source: DataSource::Base,
            op_source: OpSource::Insert,
            replacement_op: Some(TxTupleOp::Insert(new_t.clone())),
            add_tuple: Some(new_t),
            del_tuple: None,
        };
        relation.tuple_apply(apply)?;
        Ok(())
    }

    pub(crate) fn predicate_scan<F: Fn(&TupleRef) -> bool>(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        f: F,
    ) -> Result<Vec<TupleRef>, RelationError> {
        // First collect all the tuples from the canonical relation that match.
        let mut tuples: HashMap<TupleId, TupleRef> = db
            .with_relation(relation_id, |relation| relation.predicate_scan(&f))
            .iter()
            .map(|t| (t.id(), t.clone()))
            .collect();

        // Group by domain...
        let mut by_domain: HashMap<SliceRef, HashSet<TupleRef>> =
            tuples.iter().fold(HashMap::new(), |mut acc, (_, t)| {
                acc.entry(t.domain()).or_default().insert(t.clone());
                acc
            });

        // Now pull in the local working set and apply overtop.
        // Apply any changes to the tuples we've already collected, and add in any inserts, and
        // remove anything tombstoned.
        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);
        for (tr, t) in &relation.tx_tuple_events {
            if t.op.ts() > self.ts {
                // Not visible to us.  Prune it out.
                tuples.remove(tr);
                continue;
            }
            match &t.op {
                TxTupleOp::Insert(new_tuple) => {
                    if f(new_tuple) {
                        // If we have a unique domain constraint, we
                        // need to remove any existing tuples with the
                        // same domain, as we're going to replace them
                        // with this one.
                        if relation.relation_info.unique_domain {
                            let existing = by_domain.get_mut(&new_tuple.domain());
                            if let Some(existing) = existing {
                                for existing in existing.iter() {
                                    tuples.remove(&existing.id());
                                }
                                existing.clear();
                            }
                        }
                        tuples.insert(new_tuple.id(), new_tuple.clone());
                    }
                }
                TxTupleOp::Update {
                    to_tuple: new_tuple,
                    from_tuple: old_tuple,
                } => {
                    tuples.remove(&old_tuple.id());
                    if f(new_tuple) {
                        tuples.insert(new_tuple.id(), new_tuple.clone());
                    }
                }
                TxTupleOp::Tombstone(tref, _) => {
                    tuples.remove(&tref.id());
                }
                TxTupleOp::Value(_) => continue,
            }
        }
        Ok(tuples.values().cloned().collect())
    }

    pub(crate) fn update_by_domain(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), RelationError> {
        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);

        // If we have existing copies, we will update each, but keep their existing derivation
        // timestamps and operation types.
        // As we're doing this, track the tuple ids for updates, values, and tombstones so we don't look at them again when we go to
        // the canonical relation.
        let mut skip_ids = HashSet::new();
        let unique_constraint = relation.relation_info.unique_domain;
        if unique_constraint {
            relation.index.check_for_update(&domain)?;
        }
        // if let Some(tuple_indexes) = relation.index.seek_domain(&domain) {
        let tuple_indexes = relation.index.seek_domain(&domain);
        let mut applications = vec![];
        for tuple_ref in tuple_indexes {
            let existing = relation
                .tx_tuple_events
                .get(&tuple_ref)
                .expect("Tuple not found");
            let Some(apply) = existing.transform_to_update(
                &domain,
                &codomain,
                self.tuplebox.clone(),
                relation_id,
                unique_constraint,
            )?
            else {
                continue;
            };
            applications.push(apply);
        }
        for apply in applications {
            if let Some(id) = relation.tuple_apply(apply)? {
                skip_ids.insert(id);
            }
            // If we have a unique domain constraint, we can stop here.
            if unique_constraint {
                return Ok(());
            }
        }

        // Check canonical for existing values.  And get timestamps for each...
        // We will use the ts on that to determine the derivation timestamp for our own version.
        // If there's nothing there or its tombstoned, that's NotFound, and die.
        let canon_tuples = db.with_relation(relation_id, |relation| {
            let tuples = relation.seek_by_domain(domain.clone());

            Ok(tuples)
        })?;
        if unique_constraint && canon_tuples.len() > 1 {
            return Err(RelationError::AmbiguousTuple);
        }
        for old_tup in canon_tuples {
            // Skip any tuples we've already updated locally.
            if skip_ids.contains(&old_tup.id()) {
                continue;
            }
            // Write into the local copy an update operation.
            let new_t = TupleRef::allocate(
                relation_id,
                self.tuplebox.clone(),
                old_tup.ts(),
                domain.as_slice(),
                codomain.as_slice(),
            )
            .unwrap();
            // Update was already done in canonical, so we can just move on.
            if relation.has_tuple(&new_t) {
                continue;
            }

            let apply = TupleApply {
                data_source: DataSource::Base,
                op_source: OpSource::Update,
                replacement_op: Some(TxTupleOp::Update {
                    from_tuple: old_tup.clone(),
                    to_tuple: new_t.clone(),
                }),
                add_tuple: Some(new_t),
                del_tuple: None,
            };
            relation.tuple_apply(apply)?;

            // If we have a unique domain constraint, we can stop here.
            if unique_constraint {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Attempt to upsert a tuple in the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub(crate) fn upsert_by_domain(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), RelationError> {
        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);

        // If we have an existing copy, we will update it, but keep its existing derivation
        // timestamp.
        // If it's an insert, we have to keep it an insert, same for update, but if it's a delete,
        // we have to turn it into an update.
        // An upsert by definition should only have one value for the domain.
        relation.index.check_for_update(&domain)?;
        let domain_tuples = relation.index.seek_domain(&domain);
        let apply = if let Some(existing_tuple_id) = domain_tuples.into_iter().next() {
            let existing_tuple_op = relation
                .tx_tuple_events
                .get(&existing_tuple_id)
                .expect("Tuple not found");

            Some(existing_tuple_op.transform_to_upsert(
                &domain,
                &codomain,
                self.tuplebox.clone(),
                relation_id,
            )?)
        } else {
            None
        };
        if let Some(apply) = apply {
            relation.tuple_apply(apply)?;
            return Ok(());
        }

        // Nothing, local, do canonical...
        let apply = db.with_relation(relation_id, |relation| {
            let old_tuples = relation.seek_by_domain(domain.clone());
            // If there's more than one value for this domain, this operation makes no sense, so raise an
            // ambig error.
            if old_tuples.len() > 1 {
                return Err(RelationError::AmbiguousTuple);
            }
            let result = if old_tuples.is_empty() {
                let new_t = TupleRef::allocate(
                    relation_id,
                    self.tuplebox.clone(),
                    self.ts,
                    domain.as_slice(),
                    codomain.as_slice(),
                )
                .unwrap();
                Ok(Some(TupleApply {
                    data_source: DataSource::Base,
                    op_source: OpSource::Upsert,
                    replacement_op: Some(TxTupleOp::Insert(new_t.clone())),
                    add_tuple: Some(new_t),
                    del_tuple: None,
                }))
            } else {
                let old_tuple = old_tuples.into_iter().next().unwrap();
                // If old domain & codomain are the same as the upsert, we're done, nothing to apply.
                if old_tuple.domain() == domain && old_tuple.codomain() == codomain {
                    return Ok(None);
                }

                let new_t = TupleRef::allocate(
                    relation_id,
                    self.tuplebox.clone(),
                    self.ts,
                    domain.as_slice(),
                    codomain.as_slice(),
                )
                .unwrap();
                Ok(Some(TupleApply {
                    data_source: DataSource::Base,
                    op_source: OpSource::Upsert,
                    replacement_op: Some(TxTupleOp::Update {
                        from_tuple: old_tuple.clone(),
                        to_tuple: new_t.clone(),
                    }),
                    add_tuple: Some(new_t),
                    del_tuple: None,
                }))
            };
            result
        })?;

        if let Some(apply) = apply {
            relation.tuple_apply(apply)?;
        }
        Ok(())
    }

    /// Attempt to delete tuples in the transaction's working set, with the intent of eventually
    /// committing the delete to the canonical base relation.
    pub(crate) fn remove_by_domain(
        &mut self,
        db: &Arc<RelBox>,
        relation_id: RelationId,
        domain: SliceRef,
    ) -> Result<(), RelationError> {
        let relation = Self::get_relation_mut(relation_id, &self.schema, &mut self.relations);

        let mut found = false;
        // Delete is basically an update but where we stick a Tombstone in there.

        relation.index.check_for_update(&domain)?;
        let trefs = relation.index.seek_domain(&domain);
        let mut applications = vec![];
        for tref in trefs {
            let tuple_op = relation
                .tx_tuple_events
                .get_mut(&tref)
                .expect("Tuple op not found for indexed tuple");
            if let Some(apply) = tuple_op.transform_to_remove()? {
                applications.push(apply);
                found = true;
            }
        }
        for apply in applications {
            relation.tuple_apply(apply)?;
        }

        if relation.relation_info.unique_domain && found {
            return Ok(());
        }

        let old_tuples = db.with_relation(relation_id, |relation| {
            let tuples = relation.seek_by_domain(domain.clone());

            if relation.unique_domain {
                if tuples.is_empty() {
                    return Err(RelationError::TupleNotFound);
                }
                if tuples.len() > 1 {
                    return Err(RelationError::AmbiguousTuple);
                }
            }
            Ok(tuples)
        })?;

        for old_tuple in old_tuples {
            let apply = TupleApply {
                data_source: DataSource::Base,
                op_source: OpSource::Remove,
                replacement_op: Some(TxTupleOp::Tombstone(old_tuple.clone(), self.ts)),
                add_tuple: Some(old_tuple.clone()),
                del_tuple: None,
            };
            relation.tuple_apply(apply)?;
        }
        Ok(())
    }
}

/// The transaction-local storage for tuples in relations, originally derived from base relations.
pub(crate) struct TxBaseRelation {
    pub id: RelationId,
    pub relation_info: RelationInfo,

    tx_tuple_events: HashMap<TupleId, TxTupleEvent>,
    index: Box<dyn Index>,

    unsend: PhantomUnsend,
    unsync: PhantomUnsync,
}

impl TxBaseRelation {
    pub fn tuples(&self) -> impl Iterator<Item = &TxTupleEvent> {
        self.tx_tuple_events.values()
    }

    pub fn tuples_iter_mut(&mut self) -> impl Iterator<Item = &mut TxTupleEvent> {
        self.tx_tuple_events.values_mut()
    }

    pub(crate) fn clear(&mut self) {
        self.tx_tuple_events.clear();
        self.index.clear();
    }

    // Check for dupes for the given domain value.
    fn has_tuple(&self, tuple: &TupleRef) -> bool {
        if let Some(existing) = self.tx_tuple_events.get(&tuple.id()) {
            return !matches!(&existing.op, TxTupleOp::Tombstone { .. });
        }
        false
    }

    /// Apply a tuple apply event to the working set relation, storing it in our set, and updating indexes appropriately.
    /// This is done to have a consistent process for applying updates to the working set, to avoid having to
    /// do the index updates in multiple places.
    fn tuple_apply(&mut self, apply: TupleApply) -> Result<Option<TupleId>, RelationError> {
        let TupleApply {
            replacement_op,
            add_tuple,
            del_tuple,
            ..
        } = apply;
        // Perform deletes as appropriate.
        if let Some(del_tuple) = &del_tuple {
            self.index.unindex_tuple(del_tuple);
            self.tx_tuple_events.remove(&del_tuple.id()).unwrap();
        }
        // Then add the new tuple, if there is one.
        if let Some(add_tuple) = add_tuple {
            self.index.check_domain_constraints(&add_tuple.domain())?;

            // We can't accommodate a new tuple if it's already in the relation, we're a set not a bag.
            if self.has_tuple(&add_tuple) {
                return Err(RelationError::UniqueConstraintViolation);
            }

            // Insert the new tuple & operation into the indexes and tuple op map.
            self.index.index_tuple(&add_tuple)?;

            // Shove in the operation at the new locale.
            if let Some(replacement_op) = replacement_op {
                self.tx_tuple_events.insert(
                    add_tuple.id(),
                    TxTupleEvent {
                        op: replacement_op,
                        op_source: apply.op_source,
                        data_source: apply.data_source,
                    },
                );
            }
        }
        Ok(del_tuple.map(|t| t.id()))
    }
}
