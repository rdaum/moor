use crate::inmemtransient::tuplebox::{Relation, TupleBox, TupleValue};
use moor_values::util::slice_ref::SliceRef;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// A versioned transaction, which is a fork of the current canonical base relations.
pub struct Transaction {
    /// The timestamp of this transaction, as granted to us by the tuplebox.
    ts: u64,
    /// Where we came from, for referencing back to the base relations.
    pub(crate) db: Arc<TupleBox>,
    /// Our working set of mutations to the base relations; the set of operations we will attempt
    /// to commit (and refer to for reads/updates).
    working_set: RwLock<Vec<HashMap<Vec<u8>, LocalValue<SliceRef>>>>,
}

/// A local value, which is a tuple operation (insert/update/delete) and a timestamp.
#[derive(Clone)]
struct LocalValue<Codomain: Clone + Eq + PartialEq> {
    ts: Option<u64>,
    t: TupleOperation<Codomain>,
}

/// Possible operations on tuple codomains, in the context of a transaction.
#[derive(Clone)]
enum TupleOperation<Codomain: Clone + Eq + PartialEq> {
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
        let mut relations = Vec::new();
        for _ in 0..db
            .number_relations
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            relations.push(HashMap::new());
        }

        Self {
            ts,
            db: db,
            working_set: RwLock::new(relations),
        }
    }

    pub async fn commit(&self) -> Result<(), CommitError> {
        let mut tries = 0;
        'retry: loop {
            let commitset = self.prepare_commit_set().await?;
            // swap the active canonical state with the new one. but only if the timestamps have not
            // changed in the interim; we have to hold a lock while this is done. If any relations have
            // had their ts change, we need to retry.
            // We have to hold a lock during the duration of this. If we fail, we will loop back
            // and retry.
            let mut canonical = self.db.canonical.write().await;
            for (relation_id, relation) in commitset.relations.iter().enumerate() {
                if let Some(relation) = relation {
                    // Did the relation get committed to by someone else in the interim? If so, we
                    // have to abort and retry our commit-set preparation. If that still doesn't
                    // work after 3 tries, we give up and abort with a Conflict.
                    if relation.ts != canonical[relation_id].ts {
                        tries += 1;
                        if tries > 3 {
                            return Err(CommitError::RelationContentionConflict);
                        } else {
                            // Release the lock and retry the commit set.
                            continue 'retry;
                        }
                    }
                }
            }

            // Everything passed, so we can commit the changes by swapping in the new canonical
            // before releasing the lock.
            for (relation_id, relation) in commitset.relations.into_iter().enumerate() {
                if let Some(relation) = relation {
                    canonical[relation_id] = relation;
                    // And update the timestamp on the canonical relation.
                    canonical[relation_id].ts = self.ts;
                }
            }
            // Clear out the active transaction.
            self.db.active_transactions.write().await.remove(&self.ts);

            // TODO: write to WAL here.

            return Ok(());
        }
    }

    pub async fn rollback(&self) -> Result<(), CommitError> {
        self.working_set.write().await.clear();
        // Clear out the active transaction.
        self.db.active_transactions.write().await.remove(&self.ts);
        Ok(())
    }

    /// Attempt to retrieve a tuple from the transaction's working set, or from the canonical
    /// base relations if it's not found in the working set.
    pub async fn get_tuple(&self, relation_id: usize, k: &[u8]) -> Result<SliceRef, TupleError> {
        let rel = &mut self.working_set.write().await[relation_id];

        // Check local first.
        if let Some(local_version) = rel.get(k) {
            return match &local_version.t {
                TupleOperation::Insert(v) => Ok(v.clone()),
                TupleOperation::Update(v) => Ok(v.clone()),
                TupleOperation::Value(v) => Ok(v.clone()),
                TupleOperation::Tombstone => Err(TupleError::NotFound),
            };
        }

        let canonical = &self.db.canonical.read().await[relation_id];

        // Check canonical; we'll build a local fork based on the timestamp there, and then return
        // its value. Subsequent operations will work against this local copy.
        if let Some(canonical_version) = canonical.tuples.get(k) {
            rel.insert(
                k.to_vec(),
                LocalValue {
                    ts: Some(canonical_version.ts),
                    t: TupleOperation::Value(canonical_version.v.clone()),
                },
            );
            return Ok(canonical_version.v.clone());
        }

        Err(TupleError::NotFound)
    }

    /// Attempt to insert a tuple into the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub async fn insert_tuple(
        &self,
        relation_id: usize,
        k: &[u8],
        v: SliceRef,
    ) -> Result<(), TupleError> {
        let rel = &mut self.working_set.write().await[relation_id];

        // If we already have a local version, that's a dupe, so return an error for that.
        if let Some(_) = rel.get(k) {
            return Err(TupleError::Duplicate);
        }

        let canonical = &self.db.canonical.read().await[relation_id];

        if let Some(TupleValue { .. }) = canonical.tuples.get(k) {
            // If there's a canonical version, we can't insert, so return an error.
            return Err(TupleError::Duplicate);
        }

        // Write into the local copy an insert operation. Net-new timestamp ("None")
        rel.insert(
            k.to_vec(),
            LocalValue {
                ts: None,
                t: TupleOperation::Insert(v),
            },
        );

        Ok(())
    }

    /// Attempt to update a tuple in the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub async fn update_tuple(
        &self,
        relation_id: usize,
        k: &[u8],
        v: SliceRef,
    ) -> Result<(), TupleError> {
        let rel = &mut self.working_set.write().await[relation_id];

        // If we have an existing copy, we will update it, but keep its existing derivation
        // timestamp and operation type.
        if let Some(existing) = rel.get_mut(k) {
            *existing = match &existing.t {
                TupleOperation::Tombstone => return Err(TupleError::NotFound),
                TupleOperation::Insert(_) => LocalValue {
                    ts: existing.ts,
                    t: TupleOperation::Insert(v),
                },
                TupleOperation::Update(_) => LocalValue {
                    ts: existing.ts,
                    t: TupleOperation::Update(v),
                },
                TupleOperation::Value(_) => LocalValue {
                    ts: existing.ts,
                    t: TupleOperation::Update(v),
                },
            };
            return Ok(());
        }

        // Check canonical for an existing value.  And get its timestamp if it exists.
        // We will use the ts on that to determine the derivation timestamp for our own version.
        // If there's nothing there or its tombstoned, that's NotFound, and die.
        let canonical = &self.db.canonical.read().await[relation_id];

        let ts = match canonical.tuples.get(k) {
            Some(TupleValue { ts, .. }) => *ts,
            None => return Err(TupleError::NotFound),
        };

        // Write into the local copy an update operation.
        rel.insert(
            k.to_vec(),
            LocalValue {
                ts: Some(ts),
                t: TupleOperation::Update(v.clone()),
            },
        );
        Ok(())
    }

    /// Attempt to upsert a tuple in the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub async fn upsert_tuple(
        &self,
        relation_id: usize,
        k: &[u8],
        v: SliceRef,
    ) -> Result<(), TupleError> {
        let rel = &mut self.working_set.write().await[relation_id];

        // If we have an existing copy, we will update it, but keep its existing derivation
        // timestamp.
        // If it's an insert, we have to keep it an insert, same for update, but if it's a delete,
        // we have to turn it into an update.
        if let Some(existing) = rel.get_mut(k) {
            existing.t = match &existing.t {
                TupleOperation::Insert(_) => TupleOperation::Insert(v.clone()),
                TupleOperation::Update(_) | TupleOperation::Tombstone => {
                    TupleOperation::Update(v.clone())
                }
                TupleOperation::Value(_) => TupleOperation::Update(v.clone()),
            };
            return Ok(());
        }

        // Check canonical for an existing value.  And get its timestamp if it exists.
        // We will use the ts on that to determine the derivation timestamp for our own version.
        // If there is no value there, we will use the current transaction timestamp, but it's
        // an insert rather than an update.
        let canonical = &self.db.canonical.read().await[relation_id];

        match canonical.tuples.get(k) {
            Some(TupleValue { ts, .. }) => {
                rel.insert(
                    k.to_vec(),
                    LocalValue {
                        ts: Some(*ts),
                        t: TupleOperation::Update(v.clone()),
                    },
                );
            }
            None => {
                rel.insert(
                    k.to_vec(),
                    LocalValue {
                        ts: None,
                        t: TupleOperation::Insert(v.clone()),
                    },
                );
            }
        };

        Ok(())
    }

    /// Attempt to delete a tuple in the transaction's working set, with the intent of eventually
    /// committing the delete to the canonical base relations.
    pub async fn delete_tuple(&self, relation_id: usize, k: &[u8]) -> Result<(), TupleError> {
        let rel = &mut self.working_set.write().await[relation_id];

        // Delete is basically an update but where we stick a Tombstone.

        if let Some(existing) = rel.get_mut(k) {
            *existing = LocalValue {
                ts: existing.ts,
                t: TupleOperation::Tombstone,
            };
            return Ok(());
        }

        let canonical = &self.db.canonical.read().await[relation_id];

        let ts = match canonical.tuples.get(k) {
            Some(TupleValue { ts, .. }) => *ts,
            None => return Err(TupleError::NotFound),
        };

        rel.insert(
            k.to_vec(),
            LocalValue {
                ts: Some(ts),
                t: TupleOperation::Tombstone,
            },
        );

        Ok(())
    }
}

struct CommitSet {
    relations: Vec<Option<Relation>>,
}

impl CommitSet {
    fn new(width: usize) -> Self {
        Self {
            relations: vec![None; width],
        }
    }
    fn fork(&mut self, relation_id: usize, canonical: &Relation) -> &mut Relation {
        if self.relations[relation_id].is_none() {
            let r = canonical.clone();
            self.relations[relation_id] = Some(r);
        }
        self.relations[relation_id].as_mut().unwrap()
    }
}

impl Transaction {
    /// Checks to see if the given transaction can safely commit against the current canonical state
    /// of all base relations. During the process, produces a new canonical state of all base
    /// relation trees.
    async fn prepare_commit_set<'a>(&self) -> Result<CommitSet, CommitError> {
        let local = self.working_set.write().await;

        let mut commitset = CommitSet::new(local.len());

        for (relation_id, local_relation) in local.iter().enumerate() {
            // scan through the local relation, and for each tuple, check to see if it's safe to
            // commit. If it is, then we'll add it to the commit set.
            // note we're not actually committing yet, just producing a candidate commit set
            let canonical = &self.db.canonical.read().await[relation_id];
            for (k, v) in local_relation.iter() {
                let cv = canonical.tuples.get(k);

                // If there's no value there, and our local is not tombstoned and we're not doing
                // an insert that's already a conflict.
                // Otherwise we have to straight-away insert into the canonical base relation.
                // TODO: it should be possible to do this without having the fork logic exist twice
                //   here.
                let Some(cv) = cv else {
                    match &v.t {
                        TupleOperation::Insert(value) => {
                            let forked_relation = commitset.fork(relation_id, &canonical);
                            forked_relation.tuples.insert(
                                k.clone(),
                                TupleValue {
                                    ts: self.ts,
                                    v: value.clone(),
                                },
                            );
                            continue;
                        }
                        TupleOperation::Tombstone => {
                            // We let this pass, as this must be a delete of something we inserted
                            // temporarily previously in our transaction.
                            continue;
                        }
                        TupleOperation::Update(_) | TupleOperation::Value(_) => {
                            return Err(CommitError::TupleVersionConflict);
                        }
                    }
                };

                // If there's no timestamp on the value in ours, but there *is* a value in
                // canonical, that's a conflict, because it means someone else has already
                // committed a change to this tuple that we thought was net-new.
                let Some(ts) = v.ts else {
                    return Err(CommitError::TupleVersionConflict);
                };

                // Check the timestamp on the value, if it's newer than the read-timestamp
                // we have for this tuple (or if we don't have one because net-new), then
                // that's conflict, because it means someone else has already committed a
                // change to this key.
                if cv.ts > ts {
                    return Err(CommitError::TupleVersionConflict);
                }

                // Otherwise apply the change into a new canonical relation, which is a CoW
                // branching of the old one.
                let forked_relation = commitset.fork(relation_id, &canonical);
                match &v.t {
                    TupleOperation::Insert(val) | TupleOperation::Update(val) => {
                        forked_relation.tuples.insert(
                            k.clone(),
                            TupleValue {
                                ts: self.ts,
                                v: val.clone(),
                            },
                        );
                    }
                    TupleOperation::Value(_) => {}
                    TupleOperation::Tombstone => {
                        forked_relation.tuples.remove(k);
                    }
                }
            }
        }
        Ok(commitset)
    }
}

#[cfg(test)]
mod tests {
    use crate::inmemtransient::transaction::{CommitError, TupleError};
    use crate::inmemtransient::tuplebox::TupleBox;
    use moor_values::util::slice_ref::SliceRef;

    /// Verifies that base relations ("canonical") get updated when successful commits happen.
    #[tokio::test]
    async fn basic_commit() {
        let db = TupleBox::new(1, 0);
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
            tx.get_tuple(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"123")
        );
        tx.commit().await.expect("Expected commit to succeed");

        // Verify canonical state
        {
            let relation = &db.canonical.read().await[0];
            let tuple = relation
                .tuples
                .get(&b"abc"[..].to_vec())
                .expect("Expected tuple to exist");
            assert_eq!(tuple.ts, 0);
            assert_eq!(tuple.v, SliceRef::from_bytes(b"123"));
        }
    }

    /// Tests basic serial/sequential logic, where transactions mutate the same tuple but do so
    /// sequentially without potential for conflict.
    #[tokio::test]
    async fn serial_insert_update_tx() {
        let db = TupleBox::new(1, 0);
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
            tx.get_tuple(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"123")
        );
        tx.commit().await.expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.get_tuple(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"123")
        );
        tx.insert_tuple(0, b"abc", SliceRef::from_bytes(b"def"))
            .await
            .expect_err("Expected insert to fail");
        tx.upsert_tuple(0, b"abc", SliceRef::from_bytes(b"321"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.get_tuple(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"321")
        );
        tx.commit().await.expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.get_tuple(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"321")
        );
        tx.upsert_tuple(0, b"abc", SliceRef::from_bytes(b"666"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.get_tuple(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"666")
        );
        tx.commit().await.expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.get_tuple(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"666")
        );
    }

    /// Much the same as above, but test for deletion logic instead of update.
    #[tokio::test]
    async fn serial_insert_delete_tx() {
        let db = TupleBox::new(1, 0);
        let tx = db.clone().start_tx();
        tx.insert_tuple(0, b"abc", SliceRef::from_bytes(b"def"))
            .await
            .unwrap();
        tx.delete_tuple(0, b"abc")
            .await
            .expect("Expected delete to succeed");
        assert_eq!(
            tx.get_tuple(0, b"abc").await.unwrap_err(),
            TupleError::NotFound
        );
        tx.commit().await.expect("Expected commit to succeed");

        let tx = db.start_tx();
        assert_eq!(
            tx.get_tuple(0, b"abc").await.unwrap_err(),
            TupleError::NotFound
        );
        tx.insert_tuple(0, b"abc", SliceRef::from_bytes(b"def"))
            .await
            .unwrap();
        tx.update_tuple(0, b"abc", SliceRef::from_bytes(b"321"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.get_tuple(0, b"abc").await.unwrap(),
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
        let db = TupleBox::new(1, 0);
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
        let db = TupleBox::new(1, 0);

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
            tx1.get_tuple(0, b"abc").await.unwrap(),
            SliceRef::from_bytes(b"123")
        );

        tx2.update_tuple(0, b"abc", SliceRef::from_bytes(b"321"))
            .await
            .unwrap();
        assert_eq!(
            tx2.get_tuple(0, b"abc").await.unwrap(),
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
