use crate::tuplebox::base_relation::TupleValue;
use crate::tuplebox::tb::{RelationInfo, TupleBox};
use crate::tuplebox::transaction::{LocalValue, TupleError, TupleOperation};
use crate::tuplebox::RelationId;
use moor_values::util::slice_ref::SliceRef;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The local tx "working set" of mutations to base relations, and consists of the set of operations
/// we will attempt to make permanent when the transaction commits.
/// The working set is also referred to for reads/updates during the lifetime of the transaction.  
/// It effectively "is" the transaction in regards to *base relations*.
pub struct WorkingSet(pub(crate) Vec<TxBaseRelation>);

impl WorkingSet {
    pub(crate) fn new(schema: &[RelationInfo]) -> Self {
        let mut relations = Vec::new();
        for r in schema {
            relations.push(TxBaseRelation {
                domain_index: HashMap::new(),
                codomain_index: if r.secondary_indexed {
                    Some(HashMap::new())
                } else {
                    None
                },
            });
        }
        Self(relations)
    }

    pub(crate) fn clear(&mut self) {
        for rel in self.0.iter_mut() {
            rel.clear();
        }
    }

    pub(crate) async fn seek_by_domain(
        &mut self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        domain: &[u8],
    ) -> Result<SliceRef, TupleError> {
        let relation = &mut self.0[relation_id.0];

        // Check local first.
        if let Some(local_version) = relation.domain_index.get(domain) {
            return match &local_version.t {
                TupleOperation::Insert(v) => Ok(v.clone()),
                TupleOperation::Update(v) => Ok(v.clone()),
                TupleOperation::Value(v) => Ok(v.clone()),
                TupleOperation::Tombstone => Err(TupleError::NotFound),
            };
        }

        let (canon_ts, canon_v) = db
            .with_relation(relation_id, |relation| {
                if let Some(TupleValue { v, ts }) = relation.seek_by_domain(domain) {
                    Ok((*ts, v.clone()))
                } else {
                    Err(TupleError::NotFound)
                }
            })
            .await?;
        relation.domain_index.insert(
            domain.to_vec(),
            LocalValue {
                ts: Some(canon_ts),
                t: TupleOperation::Value(canon_v.clone()),
            },
        );
        if let Some(ref mut codomain_index) = relation.codomain_index {
            codomain_index
                .entry(canon_v.as_slice().to_vec())
                .or_insert_with(HashSet::new)
                .insert(domain.to_vec());
        }
        Ok(canon_v)
    }

    pub(crate) async fn seek_by_codomain(
        &mut self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        codomain: &[u8],
    ) -> Result<HashSet<Vec<u8>>, TupleError> {
        // The codomain index is not guaranteed to be up to date with the working set, so we need
        // to go back to the canonical relation, get the list of domains, then materialize them into
        // our local working set -- which will update the codomain index -- and then actually
        // use the local index.  Complicated enough?
        let domains_for_codomain = {
            let relation = &self.0[relation_id.0];

            // If there's no secondary index, we panic.  You should not have tried this.
            if relation.codomain_index.is_none() {
                panic!("Attempted to seek by codomain on a relation with no secondary index");
            }

            db.with_relation(relation_id, |relation| relation.seek_by_codomain(codomain))
                .await
        };

        for domain in domains_for_codomain {
            self.seek_by_domain(db.clone(), relation_id, &domain)
                .await?;
        }

        let relation = &mut self.0[relation_id.0];
        let codomain_index = relation.codomain_index.as_ref().expect("No codomain index");
        Ok(codomain_index
            .get(codomain)
            .cloned()
            .unwrap_or_else(|| HashSet::new())
            .into_iter()
            .collect())
    }

    pub(crate) async fn insert_tuple(
        &mut self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        domain: &[u8],
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let relation = &mut self.0[relation_id.0];

        // If we already have a local version, that's a dupe, so return an error for that.
        if let Some(_) = relation.domain_index.get(domain) {
            return Err(TupleError::Duplicate);
        }

        db.with_relation(relation_id, |relation| {
            if let Some(TupleValue { .. }) = relation.seek_by_domain(domain) {
                // If there's a canonical version, we can't insert, so return an error.
                return Err(TupleError::Duplicate);
            }
            Ok(())
        })
        .await?;

        // Write into the local copy an insert operation. Net-new timestamp ("None")
        relation.domain_index.insert(
            domain.to_vec(),
            LocalValue {
                ts: None,
                t: TupleOperation::Insert(codomain.clone()),
            },
        );
        relation.update_secondary(domain, None, Some(codomain.clone()));

        Ok(())
    }

    pub(crate) async fn update_tuple(
        &mut self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        domain: &[u8],
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let relation = &mut self.0[relation_id.0];

        // If we have an existing copy, we will update it, but keep its existing derivation
        // timestamp and operation type.
        if let Some(existing) = relation.domain_index.get_mut(domain) {
            let (replacement, old_value) = match &existing.t {
                TupleOperation::Tombstone => return Err(TupleError::NotFound),
                TupleOperation::Insert(ov) => (
                    LocalValue {
                        ts: existing.ts,
                        t: TupleOperation::Insert(codomain.clone()),
                    },
                    ov.clone(),
                ),
                TupleOperation::Update(ov) => (
                    LocalValue {
                        ts: existing.ts,
                        t: TupleOperation::Update(codomain.clone()),
                    },
                    ov.clone(),
                ),
                TupleOperation::Value(ov) => (
                    LocalValue {
                        ts: existing.ts,
                        t: TupleOperation::Update(codomain.clone()),
                    },
                    ov.clone(),
                ),
            };
            *existing = replacement;
            relation.update_secondary(domain, Some(old_value), Some(codomain.clone()));
            return Ok(());
        }

        // Check canonical for an existing value.  And get its timestamp if it exists.
        // We will use the ts on that to determine the derivation timestamp for our own version.
        // If there's nothing there or its tombstoned, that's NotFound, and die.
        let (old, ts) = db
            .with_relation(relation_id, |relation| {
                if let Some(TupleValue { ts, v: ov }) = relation.seek_by_domain(domain) {
                    Ok((ov.clone(), *ts))
                } else {
                    Err(TupleError::NotFound)
                }
            })
            .await?;

        // Write into the local copy an update operation.
        relation.domain_index.insert(
            domain.to_vec(),
            LocalValue {
                ts: Some(ts),
                t: TupleOperation::Update(codomain.clone()),
            },
        );
        relation.update_secondary(domain, Some(old), Some(codomain.clone()));
        Ok(())
    }

    /// Attempt to upsert a tuple in the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub(crate) async fn upsert_tuple(
        &mut self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        domain: &[u8],
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let relation = &mut self.0[relation_id.0];

        // If we have an existing copy, we will update it, but keep its existing derivation
        // timestamp.
        // If it's an insert, we have to keep it an insert, same for update, but if it's a delete,
        // we have to turn it into an update.
        if let Some(existing) = relation.domain_index.get_mut(domain) {
            let (replacement, old) = match &existing.t {
                TupleOperation::Insert(old) => {
                    (TupleOperation::Insert(codomain.clone()), Some(old.clone()))
                }
                TupleOperation::Update(old) => {
                    (TupleOperation::Update(codomain.clone()), Some(old.clone()))
                }
                TupleOperation::Tombstone => (TupleOperation::Update(codomain.clone()), None),
                TupleOperation::Value(old) => {
                    (TupleOperation::Update(codomain.clone()), Some(old.clone()))
                }
            };
            existing.t = replacement;
            relation.update_secondary(domain, old, Some(codomain.clone()));
            return Ok(());
        }

        // Check canonical for an existing value.  And get its timestamp if it exists.
        // We will use the ts on that to determine the derivation timestamp for our own version.
        // If there is no value there, we will use the current transaction timestamp, but it's
        // an insert rather than an update.
        let (operation, old) = db
            .with_relation(relation_id, |relation| {
                if let Some(TupleValue { ts, v: ov }) = relation.seek_by_domain(domain) {
                    (
                        LocalValue {
                            ts: Some(*ts),
                            t: TupleOperation::Update(codomain.clone()),
                        },
                        Some(ov.clone()),
                    )
                } else {
                    (
                        LocalValue {
                            ts: None,
                            t: TupleOperation::Insert(codomain.clone()),
                        },
                        None,
                    )
                }
            })
            .await;
        relation.domain_index.insert(domain.to_vec(), operation);

        // Remove the old codomain->domain index entry if it exists, and then add the new one.
        relation.update_secondary(domain, old, Some(codomain.clone()));
        Ok(())
    }

    /// Attempt to delete a tuple in the transaction's working set, with the intent of eventually
    /// committing the delete to the canonical base relations.
    pub(crate) async fn remove_by_domain(
        &mut self,
        db: Arc<TupleBox>,
        relation_id: RelationId,
        domain: &[u8],
    ) -> Result<(), TupleError> {
        let relation = &mut self.0[relation_id.0];

        // Delete is basically an update but where we stick a Tombstone.
        if let Some(existing) = relation.domain_index.get_mut(domain) {
            let old_v = match &existing.t {
                TupleOperation::Insert(ov)
                | TupleOperation::Update(ov)
                | TupleOperation::Value(ov) => ov.clone(),
                TupleOperation::Tombstone => {
                    return Err(TupleError::NotFound);
                }
            };
            *existing = LocalValue {
                ts: existing.ts,
                t: TupleOperation::Tombstone,
            };
            relation.update_secondary(domain, Some(old_v), None);
            return Ok(());
        }

        let (ts, old) = db
            .with_relation(relation_id, |relation| {
                if let Some(TupleValue { ts, v: old }) = relation.seek_by_domain(domain) {
                    Ok((*ts, old.clone()))
                } else {
                    Err(TupleError::NotFound)
                }
            })
            .await?;

        relation.domain_index.insert(
            domain.to_vec(),
            LocalValue {
                ts: Some(ts),
                t: TupleOperation::Tombstone,
            },
        );
        relation.update_secondary(domain, Some(old), None);
        Ok(())
    }
}

/// The transaction-local storage for tuples in relations derived from base relations.
pub(crate) struct TxBaseRelation {
    domain_index: HashMap<Vec<u8>, LocalValue<SliceRef>>,
    codomain_index: Option<HashMap<Vec<u8>, HashSet<Vec<u8>>>>,
}

impl TxBaseRelation {
    pub fn tuples(&self) -> impl Iterator<Item = (&Vec<u8>, &LocalValue<SliceRef>)> {
        self.domain_index.iter()
    }

    pub(crate) fn clear(&mut self) {
        self.domain_index.clear();
        if let Some(index) = self.codomain_index.as_mut() {
            index.clear();
        }
    }

    /// Update the secondary index.
    pub(crate) fn update_secondary(
        &mut self,
        domain: &[u8],
        old_codomain: Option<SliceRef>,
        new_codomain: Option<SliceRef>,
    ) {
        let Some(index) = self.codomain_index.as_mut() else {
            return;
        };

        // Clear out the old entry, if there was one.
        if let Some(old_codomain) = old_codomain {
            index
                .entry(old_codomain.as_slice().to_vec())
                .or_insert_with(HashSet::new)
                .remove(domain);
        }
        if let Some(new_codomain) = new_codomain {
            index
                .entry(new_codomain.as_slice().to_vec())
                .or_insert_with(HashSet::new)
                .insert(domain.to_vec());
        }
    }
}
