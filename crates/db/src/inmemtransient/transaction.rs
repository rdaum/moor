use crate::inmemtransient::base_relation::{BaseRelation, TupleValue};
use crate::inmemtransient::tuplebox::{RelationInfo, TupleBox};
use moor_values::util::slice_ref::SliceRef;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// A versioned transaction, which is a fork of the current canonical base relations.
pub struct Transaction {
    /// The timestamp of this transaction, as granted to us by the tuplebox.
    ts: u64,
    /// Where we came from, for referencing back to the base relations.
    db: Arc<TupleBox>,
    working_set: RwLock<WorkingSet>,
}

/// The local "working set" of mutations to the base relations, which is the set of operations
/// we will attempt to commit (and refer to for reads/updates)
pub struct WorkingSet {
    pub(crate) local_relations: Vec<LocalRelation>,
}

pub(crate) struct LocalRelation {
    pub(crate) domain_index: HashMap<Vec<u8>, LocalValue<SliceRef>>,
    codomain_index: Option<HashMap<Vec<u8>, HashSet<Vec<u8>>>>,
}

impl LocalRelation {
    fn clear(&mut self) {
        self.domain_index.clear();
        if let Some(index) = self.codomain_index.as_mut() {
            index.clear();
        }
    }
    /// Update the secondary index.
    fn update_secondary(
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

impl WorkingSet {
    fn new(schema: Vec<RelationInfo>) -> Self {
        let mut relations = Vec::new();
        for r in &schema {
            relations.push(LocalRelation {
                domain_index: HashMap::new(),
                codomain_index: if r.secondary_indexed {
                    Some(HashMap::new())
                } else {
                    None
                },
            });
        }
        Self {
            local_relations: relations,
        }
    }

    fn clear(&mut self) {
        for rel in self.local_relations.iter_mut() {
            rel.clear();
        }
    }
}

/// A local value, which is a tuple operation (insert/update/delete) and a timestamp.
#[derive(Clone)]
pub(crate) struct LocalValue<Codomain: Clone + Eq + PartialEq> {
    pub(crate) ts: Option<u64>,
    pub(crate) t: TupleOperation<Codomain>,
}

/// Possible operations on tuple codomains, in the context of a transaction.
#[derive(Clone)]
pub(crate) enum TupleOperation<Codomain: Clone + Eq + PartialEq> {
    /// Insert T into the tuple.
    Insert(Codomain),
    /// Update T in the tuple.
    Update(Codomain),
    /// Clone/fork T into the tuple from the base relation.
    Value(Codomain),
    /// Delete the tuple.
    Tombstone,
}

#[derive(Debug, Clone, Eq, PartialEq, Error)]
pub enum TupleError {
    #[error("Tuple not found")]
    NotFound,
    #[error("Tuple already exists")]
    Duplicate,
}

/// Errors which can occur during a commit.
#[derive(Debug, Clone, Eq, PartialEq, Error)]
pub enum CommitError {
    /// A version conflict was detected during the preparation of the commit set.
    #[error("Version conflict")]
    TupleVersionConflict,
    /// Multiple writers attempted to modify the same tuple at the same time, and our validated
    /// commit set was potentially invalidated by a concurrent commit.
    #[error("Relation contention conflict")]
    RelationContentionConflict,
}

impl Transaction {
    pub fn new(ts: u64, db: Arc<TupleBox>) -> Self {
        let ws = WorkingSet::new(db.relation_info());
        Self {
            ts,
            db,
            working_set: RwLock::new(ws),
        }
    }
    pub async fn sequence_next(&self, sequence_number: usize) -> u64 {
        self.db.clone().sequence_next(sequence_number).await
    }
    pub async fn sequence_current(&self, sequence_number: usize) -> u64 {
        self.db.clone().sequence_current(sequence_number).await
    }
    pub async fn update_sequence_max(&self, sequence_number: usize, value: u64) {
        self.db
            .clone()
            .update_sequence_max(sequence_number, value)
            .await
    }

    pub async fn commit(&self) -> Result<(), CommitError> {
        let mut tries = 0;
        'retry: loop {
            let working_set = self.working_set.read().await;
            let commit_set = self.db.prepare_commit_set(self.ts, &working_set).await?;
            match self.db.try_commit(commit_set).await {
                Ok(_) => return Ok(()),
                Err(CommitError::RelationContentionConflict) => {
                    tries += 1;
                    if tries > 3 {
                        return Err(CommitError::RelationContentionConflict);
                    } else {
                        // Release the lock and retry the commit set.
                        continue 'retry;
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub async fn rollback(&self) -> Result<(), CommitError> {
        self.working_set.write().await.clear();
        // Clear out the active transaction.
        self.db.abort_transaction(self.ts).await;
        Ok(())
    }

    /// Attempt to retrieve a tuple from the transaction's working set by its domain, or from the
    /// canonical base relations if it's not found in the working set.
    pub async fn seek_by_domain(
        &self,
        relation_id: usize,
        domain: &[u8],
    ) -> Result<SliceRef, TupleError> {
        let ws = &mut self.working_set.write().await.local_relations[relation_id];

        // Check local first.
        if let Some(local_version) = ws.domain_index.get(domain) {
            return match &local_version.t {
                TupleOperation::Insert(v) => Ok(v.clone()),
                TupleOperation::Update(v) => Ok(v.clone()),
                TupleOperation::Value(v) => Ok(v.clone()),
                TupleOperation::Tombstone => Err(TupleError::NotFound),
            };
        }

        let (canon_ts, canon_v) = self
            .db
            .with_relation(relation_id, |relation| {
                if let Some(TupleValue { v, ts }) = relation.seek_by_domain(domain) {
                    Ok((*ts, v.clone()))
                } else {
                    Err(TupleError::NotFound)
                }
            })
            .await?;
        ws.domain_index.insert(
            domain.to_vec(),
            LocalValue {
                ts: Some(canon_ts),
                t: TupleOperation::Value(canon_v.clone()),
            },
        );
        if let Some(ref mut codomain_index) = ws.codomain_index {
            codomain_index
                .entry(canon_v.as_slice().to_vec())
                .or_insert_with(HashSet::new)
                .insert(domain.to_vec());
        }
        Ok(canon_v)
    }

    pub async fn seek_by_codomain(
        &self,
        relation_id: usize,
        codomain: &[u8],
    ) -> Result<HashSet<Vec<u8>>, TupleError> {
        // The codomain index is not guaranteed to be up to date with the working set, so we need
        // to go back to the canonical relation, get the list of domains, then materialize them into
        // our local working set -- which will update the codomain index -- and then actually
        // use the local index.  Complicated enough?

        let domains_for_codomain = {
            let ws = &self.working_set.read().await.local_relations[relation_id];

            // If there's no secondary index, we panic.  You should not have tried this.
            if ws.codomain_index.is_none() {
                panic!("Attempted to seek by codomain on a relation with no secondary index");
            }

            self.db
                .with_relation(relation_id, |relation| relation.seek_by_codomain(codomain))
                .await
        };

        // TODO: the write-lock is lost here between all these phases, so potential for a race.
        //    We should probably do this in a single phase somehow by sharing the lock.
        for domain in domains_for_codomain {
            self.seek_by_domain(relation_id, &domain).await?;
        }

        let ws = &mut self.working_set.write().await.local_relations[relation_id];
        let codomain_index = ws.codomain_index.as_ref().expect("No codomain index");
        Ok(codomain_index
            .get(codomain)
            .cloned()
            .unwrap_or_else(|| HashSet::new())
            .into_iter()
            .collect())
    }

    /// Attempt to insert a tuple into the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub async fn insert_tuple(
        &self,
        relation_id: usize,
        domain: &[u8],
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let ws = &mut self.working_set.write().await.local_relations[relation_id];

        // If we already have a local version, that's a dupe, so return an error for that.
        if let Some(_) = ws.domain_index.get(domain) {
            return Err(TupleError::Duplicate);
        }

        self.db
            .with_relation(relation_id, |relation| {
                if let Some(TupleValue { .. }) = relation.seek_by_domain(domain) {
                    // If there's a canonical version, we can't insert, so return an error.
                    return Err(TupleError::Duplicate);
                }
                Ok(())
            })
            .await?;

        // Write into the local copy an insert operation. Net-new timestamp ("None")
        ws.domain_index.insert(
            domain.to_vec(),
            LocalValue {
                ts: None,
                t: TupleOperation::Insert(codomain.clone()),
            },
        );
        ws.update_secondary(domain, None, Some(codomain.clone()));

        Ok(())
    }

    /// Attempt to update a tuple in the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub async fn update_tuple(
        &self,
        relation_id: usize,
        domain: &[u8],
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let ws = &mut self.working_set.write().await.local_relations[relation_id];

        // If we have an existing copy, we will update it, but keep its existing derivation
        // timestamp and operation type.
        if let Some(existing) = ws.domain_index.get_mut(domain) {
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
            ws.update_secondary(domain, Some(old_value), Some(codomain.clone()));
            return Ok(());
        }

        // Check canonical for an existing value.  And get its timestamp if it exists.
        // We will use the ts on that to determine the derivation timestamp for our own version.
        // If there's nothing there or its tombstoned, that's NotFound, and die.
        let (old, ts) = self
            .db
            .with_relation(relation_id, |relation| {
                if let Some(TupleValue { ts, v: ov }) = relation.seek_by_domain(domain) {
                    Ok((ov.clone(), *ts))
                } else {
                    Err(TupleError::NotFound)
                }
            })
            .await?;

        // Write into the local copy an update operation.
        ws.domain_index.insert(
            domain.to_vec(),
            LocalValue {
                ts: Some(ts),
                t: TupleOperation::Update(codomain.clone()),
            },
        );
        ws.update_secondary(domain, Some(old), Some(codomain.clone()));
        Ok(())
    }

    /// Attempt to upsert a tuple in the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub async fn upsert_tuple(
        &self,
        relation_id: usize,
        domain: &[u8],
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let ws = &mut self.working_set.write().await.local_relations[relation_id];

        // If we have an existing copy, we will update it, but keep its existing derivation
        // timestamp.
        // If it's an insert, we have to keep it an insert, same for update, but if it's a delete,
        // we have to turn it into an update.
        if let Some(existing) = ws.domain_index.get_mut(domain) {
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
            ws.update_secondary(domain, old, Some(codomain.clone()));
            return Ok(());
        }

        // Check canonical for an existing value.  And get its timestamp if it exists.
        // We will use the ts on that to determine the derivation timestamp for our own version.
        // If there is no value there, we will use the current transaction timestamp, but it's
        // an insert rather than an update.
        let (operation, old) = self
            .db
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
        ws.domain_index.insert(domain.to_vec(), operation);

        // Remove the old codomain->domain index entry if it exists, and then add the new one.
        ws.update_secondary(domain, old, Some(codomain.clone()));
        Ok(())
    }

    /// Attempt to delete a tuple in the transaction's working set, with the intent of eventually
    /// committing the delete to the canonical base relations.
    pub async fn remove_by_domain(
        &self,
        relation_id: usize,
        domain: &[u8],
    ) -> Result<(), TupleError> {
        let ws = &mut self.working_set.write().await.local_relations[relation_id];

        // Delete is basically an update but where we stick a Tombstone.
        if let Some(existing) = ws.domain_index.get_mut(domain) {
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
            ws.update_secondary(domain, Some(old_v), None);
            return Ok(());
        }

        let (ts, old) = self
            .db
            .with_relation(relation_id, |relation| {
                if let Some(TupleValue { ts, v: old }) = relation.seek_by_domain(domain) {
                    Ok((*ts, old.clone()))
                } else {
                    Err(TupleError::NotFound)
                }
            })
            .await?;

        ws.domain_index.insert(
            domain.to_vec(),
            LocalValue {
                ts: Some(ts),
                t: TupleOperation::Tombstone,
            },
        );
        ws.update_secondary(domain, Some(old), None);
        Ok(())
    }
}

/// A set of tuples to be committed to the canonical base relations, based on a transaction's
/// working set.
pub(crate) struct CommitSet {
    pub(crate) ts: u64,
    pub(crate) relations: Vec<Option<BaseRelation>>,
}

impl CommitSet {
    pub(crate) fn new(ts: u64, width: usize) -> Self {
        Self {
            ts,
            relations: vec![None; width],
        }
    }

    /// Fork the given base relation into the commit set, if it's not already there.
    pub(crate) fn fork(
        &mut self,
        relation_id: usize,
        canonical: &BaseRelation,
    ) -> &mut BaseRelation {
        if self.relations[relation_id].is_none() {
            let r = canonical.clone();
            self.relations[relation_id] = Some(r);
        }
        self.relations[relation_id].as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::inmemtransient::transaction::{CommitError, TupleError};
    use crate::inmemtransient::tuplebox::{RelationInfo, TupleBox};
    use moor_values::util::slice_ref::SliceRef;
    use std::sync::Arc;

    fn test_db() -> Arc<TupleBox> {
        let db = TupleBox::new(
            &[RelationInfo {
                name: "test".to_string(),
                domain_type_id: 0,
                codomain_type_id: 0,
                secondary_indexed: false,
            }],
            0,
        );
        db
    }

    /// Verifies that base relations ("canonical") get updated when successful commits happen.
    #[tokio::test]
    async fn basic_commit() {
        let db = test_db();
        let tx = db.clone().start_tx();
        tx.insert_tuple(0, b"abc", SliceRef::from_bytes(b"def"))
            .await
            .unwrap();
        tx.insert_tuple(0, b"abc", SliceRef::from_bytes(b"def"))
            .await
            .expect_err("Expected insert to fail");
        tx.update_tuple(0, b"abc", SliceRef::from_bytes(b"123"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"123")
        );
        tx.commit().await.expect("Expected commit to succeed");

        // Verify canonical state
        {
            let relation = &db.canonical.read().await[0];
            let tuple = relation
                .seek_by_domain(b"abc")
                .expect("Expected tuple to exist");
            assert_eq!(tuple.ts, 0);
            assert_eq!(tuple.v, SliceRef::from_bytes(b"123"));
        }
    }

    /// Tests basic serial/sequential logic, where transactions mutate the same tuple but do so
    /// sequentially without potential for conflict.
    #[tokio::test]
    async fn serial_insert_update_tx() {
        let db = test_db();
        let tx = db.clone().start_tx();
        tx.insert_tuple(0, b"abc", SliceRef::from_bytes(b"def"))
            .await
            .unwrap();
        tx.insert_tuple(0, b"abc", SliceRef::from_bytes(b"def"))
            .await
            .expect_err("Expected insert to fail");
        tx.update_tuple(0, b"abc", SliceRef::from_bytes(b"123"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"123")
        );
        tx.commit().await.expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.seek_by_domain(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"123")
        );
        tx.insert_tuple(0, b"abc", SliceRef::from_bytes(b"def"))
            .await
            .expect_err("Expected insert to fail");
        tx.upsert_tuple(0, b"abc", SliceRef::from_bytes(b"321"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"321")
        );
        tx.commit().await.expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.seek_by_domain(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"321")
        );
        tx.upsert_tuple(0, b"abc", SliceRef::from_bytes(b"666"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"666")
        );
        tx.commit().await.expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.seek_by_domain(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"666")
        );
    }

    /// Much the same as above, but test for deletion logic instead of update.
    #[tokio::test]
    async fn serial_insert_delete_tx() {
        let db = test_db();
        let tx = db.clone().start_tx();
        tx.insert_tuple(0, b"abc", SliceRef::from_bytes(b"def"))
            .await
            .unwrap();
        tx.remove_by_domain(0, b"abc")
            .await
            .expect("Expected delete to succeed");
        assert_eq!(
            tx.seek_by_domain(0, b"abc").await.unwrap_err(),
            TupleError::NotFound
        );
        tx.commit().await.expect("Expected commit to succeed");

        let tx = db.start_tx();
        assert_eq!(
            tx.seek_by_domain(0, b"abc").await.unwrap_err(),
            TupleError::NotFound
        );
        tx.insert_tuple(0, b"abc", SliceRef::from_bytes(b"def"))
            .await
            .unwrap();
        tx.update_tuple(0, b"abc", SliceRef::from_bytes(b"321"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"321")
        );
        tx.commit().await.expect("Expected commit to succeed");
    }

    /// Two transactions both starting with nothing present for a tuple.
    /// Both insert and then commit. The second transaction should fail because the first commit
    /// got there first, creating a tuple where we thought none would be.
    /// The insert is not expected to fail until commit time, as we are fully isolated, but when
    /// commit happens, we should detect the conflict and fail.
    #[tokio::test]
    async fn parallel_insert_new_conflict() {
        let db = test_db();
        let tx1 = db.clone().start_tx();

        tx1.insert_tuple(0, b"abc", SliceRef::from_bytes(b"def"))
            .await
            .unwrap();

        let tx2 = db.clone().start_tx();
        tx2.insert_tuple(0, b"abc", SliceRef::from_bytes(b"zzz"))
            .await
            .unwrap();

        assert!(tx1.commit().await.is_ok());
        assert_eq!(
            tx2.commit().await.expect_err("Expected conflict"),
            CommitError::TupleVersionConflict
        );
    }

    #[tokio::test]
    async fn parallel_get_update_conflict() {
        let db = test_db();

        // 1. Initial transaction creates value, commits.
        let init_tx = db.clone().start_tx();
        init_tx
            .insert_tuple(0, b"abc", SliceRef::from_bytes(b"def"))
            .await
            .unwrap();
        init_tx.commit().await.unwrap();

        // 2. Two transactions get the value, and then update it, in "parallel".
        let tx1 = db.clone().start_tx();
        let tx2 = db.clone().start_tx();
        tx1.update_tuple(0, b"abc", SliceRef::from_bytes(b"123"))
            .await
            .unwrap();
        assert_eq!(
            tx1.seek_by_domain(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"123")
        );

        tx2.update_tuple(0, b"abc", SliceRef::from_bytes(b"321"))
            .await
            .unwrap();
        assert_eq!(
            tx2.seek_by_domain(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"321")
        );

        // 3. First transaction commits with success but second transaction fails with Conflict,
        // because it is younger than the first transaction, and the first transaction committed
        // a change to the tuple before we could get to it.
        assert!(tx1.commit().await.is_ok());
        assert_eq!(
            tx2.commit().await.expect_err("Expected conflict"),
            CommitError::TupleVersionConflict
        );
    }

    // TODO: Loom tests
    // TODO: Test sequences
    // TODO: Consistency across multiple relations
}
