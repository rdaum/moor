use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use sized_chunks::SparseChunk;
use thiserror::Error;
use tokio::sync::RwLock;

use moor_values::util::slice_ref::SliceRef;

use crate::tuplebox::base_relation::BaseRelation;
use crate::tuplebox::slots::SlotBox;
use crate::tuplebox::tb::TupleBox;
use crate::tuplebox::tuples::TupleError;
use crate::tuplebox::tx::relvar::RelVar;
use crate::tuplebox::tx::working_set::WorkingSet;
use crate::tuplebox::RelationId;

/// A versioned transaction, which is a fork of the current canonical base relations.
pub struct Transaction {
    /// The timestamp of this transaction, as granted to us by the tuplebox.
    ts: u64,
    /// Where we came from, for referencing back to the base relations.
    db: Arc<TupleBox>,
    slotbox: Arc<SlotBox>,
    /// The "working set" is the set of retrieved and/or modified tuples from base relations, known
    /// to the transaction, and represents the set of values that will be committed to the base
    /// relations at commit time.
    working_set: RwLock<WorkingSet>,
    /// Local-only relations, which are not directly-derived from or committed to the base relations
    /// (though operations will exist for moving them from a transient relation to a base relation,
    /// and or moving tuples in them into commits in the working set..)
    transient_relations: RwLock<HashMap<RelationId, TransientRelation>>,
    next_transient_relation_id: RelationId,
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
    pub fn new(ts: u64, slotbox: Arc<SlotBox>, db: Arc<TupleBox>) -> Self {
        let ws = WorkingSet::new(slotbox.clone(), &db.relation_info(), ts);
        let next_transient_relation_id = RelationId::transient(db.relation_info().len());

        Self {
            ts,
            slotbox,
            db,
            working_set: RwLock::new(ws),
            transient_relations: RwLock::new(HashMap::new()),
            next_transient_relation_id,
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
            let mut working_set = self.working_set.write().await;
            let commit_set = self.db.prepare_commit_set(self.ts, &working_set).await?;
            match self.db.try_commit(commit_set).await {
                Ok(_) => {
                    let mut blank_ws =
                        WorkingSet::new(self.slotbox.clone(), &self.db.relation_info(), self.ts);
                    std::mem::swap(&mut *working_set, &mut blank_ws);
                    self.db.sync(self.ts, blank_ws).await;
                    return Ok(());
                }
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

    /// Grab a handle to a relation, which can be used to perform operations on it in the context
    /// of this transaction.
    pub async fn relation(&self, relation_id: RelationId) -> RelVar {
        RelVar {
            tx: self,
            id: relation_id,
        }
    }

    /// Create a new (transient) relation in the transaction's local context. The relation will not
    /// persist past the length of the transaction, and will be discarded at commit or rollback.
    pub async fn new_relation(&mut self) -> RelVar {
        let rid = self.next_transient_relation_id;
        self.next_transient_relation_id.0 += 1;
        let mut ts = self.transient_relations.write().await;
        ts.insert(
            rid,
            TransientRelation {
                _id: rid,
                tuples: vec![],
                domain_tuples: HashMap::new(),
                codomain_domain: None,
            },
        );
        RelVar { tx: self, id: rid }
    }

    /// Attempt to retrieve a tuple from the transaction's working set by its domain, or from the
    /// canonical base relations if it's not found in the working set.
    pub(crate) async fn seek_by_domain(
        &self,
        relation_id: RelationId,
        domain: SliceRef,
    ) -> Result<(SliceRef, SliceRef), TupleError> {
        if relation_id.is_base_relation() {
            let mut ws = self.working_set.write().await;
            ws.seek_by_domain(self.db.clone(), relation_id, domain)
                .await
        } else {
            let ts = self.transient_relations.read().await;
            ts.get(&relation_id)
                .ok_or(TupleError::NotFound)?
                .seek_by_domain(domain)
                .await
        }
    }

    pub(crate) async fn seek_by_codomain(
        &self,
        relation_id: RelationId,
        codomain: SliceRef,
    ) -> Result<Vec<(SliceRef, SliceRef)>, TupleError> {
        if relation_id.is_base_relation() {
            let mut ws = self.working_set.write().await;
            ws.seek_by_codomain(self.db.clone(), relation_id, codomain)
                .await
        } else {
            let ts = self.transient_relations.read().await;
            ts.get(&relation_id)
                .ok_or(TupleError::NotFound)?
                .seek_by_codomain(codomain)
                .await
        }
    }

    /// Attempt to insert a tuple into the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub(crate) async fn insert_tuple(
        &self,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        if relation_id.is_base_relation() {
            let mut ws = self.working_set.write().await;
            ws.insert_tuple(self.db.clone(), relation_id, domain, codomain)
                .await
        } else {
            let mut ts = self.transient_relations.write().await;
            ts.get_mut(&relation_id)
                .ok_or(TupleError::NotFound)?
                .insert_tuple(domain, codomain)
                .await
        }
    }

    pub(crate) async fn predicate_scan<F: Fn(&(SliceRef, SliceRef)) -> bool>(
        &self,
        relation_id: RelationId,
        f: &F,
    ) -> Result<Vec<(SliceRef, SliceRef)>, TupleError> {
        if relation_id.is_base_relation() {
            let ws = self.working_set.read().await;
            ws.predicate_scan(self.db.clone(), relation_id, f).await
        } else {
            let ts = self.transient_relations.read().await;
            ts.get(&relation_id)
                .ok_or(TupleError::NotFound)
                .unwrap()
                .predicate_scan(f)
                .await
        }
    }

    /// Attempt to update a tuple in the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub(crate) async fn update_tuple(
        &self,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        if relation_id.is_base_relation() {
            let mut ws = self.working_set.write().await;
            ws.update_tuple(self.db.clone(), relation_id, domain, codomain)
                .await
        } else {
            let mut ts = self.transient_relations.write().await;
            ts.get_mut(&relation_id)
                .ok_or(TupleError::NotFound)?
                .update_tuple(domain, codomain)
                .await
        }
    }

    /// Attempt to upsert a tuple in the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub(crate) async fn upsert_tuple(
        &self,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        if relation_id.is_base_relation() {
            let mut ws = self.working_set.write().await;
            ws.upsert_tuple(self.db.clone(), relation_id, domain, codomain)
                .await
        } else {
            let mut ts = self.transient_relations.write().await;
            ts.get_mut(&relation_id)
                .ok_or(TupleError::NotFound)?
                .upsert_tuple(domain, codomain)
                .await
        }
    }

    /// Attempt to delete a tuple in the transaction's working set, with the intent of eventually
    /// committing the delete to the canonical base relations.
    pub(crate) async fn remove_by_domain(
        &self,
        relation_id: RelationId,
        domain: SliceRef,
    ) -> Result<(), TupleError> {
        if relation_id.is_base_relation() {
            let mut ws = self.working_set.write().await;
            ws.remove_by_domain(self.db.clone(), relation_id, domain)
                .await
        } else {
            let mut ts = self.transient_relations.write().await;
            ts.get_mut(&relation_id)
                .ok_or(TupleError::NotFound)?
                .remove_by_domain(domain)
                .await
        }
    }
}

/// A set of tuples to be committed to the canonical base relations, based on a transaction's
/// working set.
pub(crate) struct CommitSet {
    pub(crate) ts: u64,
    relations: SparseChunk<BaseRelation, 256>,
}

impl CommitSet {
    pub(crate) fn new(ts: u64) -> Self {
        Self {
            ts,
            relations: SparseChunk::new(),
        }
    }

    /// Returns an iterator over the modified relations in the commit set.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &BaseRelation> {
        return self.relations.iter();
    }

    /// Returns an iterator over the modified relations in the commit set, moving and consuming the
    /// commit set in the process.
    pub(crate) fn into_iter(self) -> impl IntoIterator<Item = BaseRelation> {
        return self.relations.into_iter();
    }

    /// Fork the given base relation into the commit set, if it's not already there.
    pub(crate) fn fork(
        &mut self,
        relation_id: RelationId,
        canonical: &BaseRelation,
    ) -> &mut BaseRelation {
        if self.relations.get(relation_id.0).is_none() {
            let r = canonical.clone();
            self.relations.insert(relation_id.0, r);
        }
        self.relations.get_mut(relation_id.0).unwrap()
    }
}

struct TransientRelation {
    _id: RelationId,
    tuples: Vec<(SliceRef, SliceRef)>,
    domain_tuples: HashMap<Vec<u8>, usize>,
    codomain_domain: Option<HashMap<Vec<u8>, HashSet<usize>>>,
}

impl TransientRelation {
    /// Seek for a tuple by its indexed domain value.
    pub async fn seek_by_domain(
        &self,
        domain: SliceRef,
    ) -> Result<(SliceRef, SliceRef), TupleError> {
        let tuple_id = self
            .domain_tuples
            .get(domain.as_slice())
            .map(|v| v.clone())
            .ok_or(TupleError::NotFound);
        tuple_id.and_then(|id| Ok(self.tuples[id].clone()))
    }

    /// Seek for tuples by their indexed codomain value, if there's an index. Panics if there is no
    /// secondary index.
    pub async fn seek_by_codomain(
        &self,
        codomain: SliceRef,
    ) -> Result<Vec<(SliceRef, SliceRef)>, TupleError> {
        // Attempt to seek on codomain without an index is a panic.
        // We could do full-scan, but in this case we're going to assume that the caller knows
        // what they're doing.
        let codomain_domain = self.codomain_domain.as_ref().expect("No codomain index");
        let tuple_ids = codomain_domain
            .get(codomain.as_slice())
            .map(|v| v.clone())
            .ok_or(TupleError::NotFound)?;
        Ok(tuple_ids
            .iter()
            .map(|tid| self.tuples[*tid].clone())
            .collect())
    }

    pub async fn predicate_scan<F: Fn(&(SliceRef, SliceRef)) -> bool>(
        &self,
        f: &F,
    ) -> Result<Vec<(SliceRef, SliceRef)>, TupleError> {
        Ok(self
            .tuples
            .iter()
            .filter(|t| f(t))
            .map(|t| t.clone())
            .collect())
    }

    /// Insert a tuple into the relation.
    pub async fn insert_tuple(
        &mut self,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        if self.domain_tuples.contains_key(domain.as_slice()) {
            return Err(TupleError::Duplicate);
        }
        let tuple_id = self.tuples.len();
        self.tuples.push((domain.clone(), codomain.clone()));
        self.domain_tuples
            .insert(domain.as_slice().to_vec(), tuple_id)
            .map(|_| ())
            .ok_or(TupleError::Duplicate)
    }

    /// Update a tuple in the relation.
    pub async fn update_tuple(
        &mut self,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let tuple_id = self
            .domain_tuples
            .get(domain.as_slice())
            .map(|v| v.clone())
            .ok_or(TupleError::NotFound)?;
        if self.codomain_domain.is_some() {
            self.update_secondary(tuple_id, None, Some(codomain.clone()));
        }
        self.tuples[tuple_id] = (domain, codomain);
        Ok(())
    }

    /// Upsert a tuple into the relation.
    pub async fn upsert_tuple(
        &mut self,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let tuple_id = match self.domain_tuples.get(domain.as_slice()) {
            Some(tuple_id) => {
                self.tuples[*tuple_id] = (domain, codomain.clone());
                *tuple_id
            }
            None => {
                let tuple_id = self.tuples.len();
                self.tuples.push((domain.clone(), codomain.clone()));
                self.domain_tuples
                    .insert(domain.as_slice().to_vec(), tuple_id);
                tuple_id
            }
        };
        self.update_secondary(tuple_id, None, Some(codomain.clone()));

        Ok(())
    }

    /// Remove a tuple from the relation.
    pub async fn remove_by_domain(&mut self, domain: SliceRef) -> Result<(), TupleError> {
        let tuple_id = self
            .domain_tuples
            .remove(domain.as_slice())
            .map(|v| v.clone())
            .ok_or(TupleError::NotFound)?;

        if self.codomain_domain.is_some() {
            self.update_secondary(tuple_id, None, None);
        }
        self.tuples.remove(tuple_id);
        Ok(())
    }

    pub(crate) fn update_secondary(
        &mut self,
        tuple_id: usize,
        old_codomain: Option<SliceRef>,
        new_codomain: Option<SliceRef>,
    ) {
        let Some(index) = self.codomain_domain.as_mut() else {
            return;
        };

        // Clear out the old entry, if there was one.
        if let Some(old_codomain) = old_codomain {
            index
                .entry(old_codomain.as_slice().to_vec())
                .or_insert_with(HashSet::new)
                .remove(&tuple_id);
        }
        if let Some(new_codomain) = new_codomain {
            index
                .entry(new_codomain.as_slice().to_vec())
                .or_insert_with(HashSet::new)
                .insert(tuple_id);
        }
    }
}
#[cfg(test)]
mod tests {
    use rand::Rng;
    use std::sync::Arc;

    use moor_values::util::slice_ref::SliceRef;

    use crate::tuplebox::tb::{RelationInfo, TupleBox};
    use crate::tuplebox::tuples::TupleError;
    use crate::tuplebox::tx::transaction::CommitError;
    use crate::tuplebox::RelationId;

    fn attr(slice: &[u8]) -> SliceRef {
        SliceRef::from_bytes(slice)
    }

    async fn test_db() -> Arc<TupleBox> {
        let db = TupleBox::new(
            None,
            &[RelationInfo {
                name: "test".to_string(),
                domain_type_id: 0,
                codomain_type_id: 0,
                secondary_indexed: true,
            }],
            0,
        )
        .await;
        db
    }

    /// Verifies that base relations ("canonical") get updated when successful commits happen.
    #[tokio::test]
    async fn basic_commit() {
        let db = test_db().await;
        let tx = db.clone().start_tx();
        let rid = RelationId(0);
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .await
            .unwrap();
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .await
            .expect_err("Expected insert to fail");
        tx.update_tuple(rid, attr(b"abc"), attr(b"123"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc")).await.unwrap().1,
            attr(b"123")
        );
        assert_eq!(
            tx.seek_by_codomain(rid, attr(b"123"))
                .await
                .expect("Expected secondary seek to succeed"),
            vec![(attr(b"abc"), attr(b"123"))]
        );

        tx.commit().await.expect("Expected commit to succeed");

        // Verify canonical state.
        {
            let relation = &db.canonical.read().await[0];
            let tref = relation
                .seek_by_domain(attr(b"abc"))
                .expect("Expected tuple to exist");
            let tuple = tref.get();
            assert_eq!(tuple.ts(), 0);
            assert_eq!(tuple.codomain().as_slice(), b"123");

            let tuples = relation.seek_by_codomain(attr(b"123"));
            assert_eq!(tuples.len(), 1);
            let tuple = tuples.iter().next().unwrap().get();
            assert_eq!(tuple.ts(), 0);
            assert_eq!(tuple.domain().as_slice(), b"abc");
            assert_eq!(tuple.codomain().as_slice(), b"123");
        }
    }

    /// Tests basic serial/sequential logic, where transactions mutate the same tuple but do so
    /// sequentially without potential for conflict.
    #[tokio::test]
    async fn serial_insert_update_tx() {
        let db = test_db().await;
        let tx = db.clone().start_tx();
        let rid = RelationId(0);
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .await
            .unwrap();
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .await
            .expect_err("Expected insert to fail");
        tx.update_tuple(rid, attr(b"abc"), attr(b"123"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc"))
                .await
                .unwrap()
                .1
                .as_slice(),
            b"123"
        );
        tx.commit().await.expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc"))
                .await
                .unwrap()
                .1
                .as_slice(),
            b"123"
        );
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .await
            .expect_err("Expected insert to fail");
        tx.upsert_tuple(rid, attr(b"abc"), attr(b"321"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc"))
                .await
                .unwrap()
                .1
                .as_slice(),
            b"321"
        );
        tx.commit().await.expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc"))
                .await
                .unwrap()
                .1
                .as_slice(),
            b"321"
        );
        tx.upsert_tuple(rid, attr(b"abc"), attr(b"666"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc"))
                .await
                .unwrap()
                .1
                .as_slice(),
            b"666"
        );
        tx.commit().await.expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc"))
                .await
                .unwrap()
                .1
                .as_slice(),
            b"666"
        );

        // And verify secondary index...
        assert_eq!(
            tx.seek_by_codomain(rid, attr(b"666"))
                .await
                .expect("Expected secondary seek to succeed"),
            vec![(attr(b"abc"), attr(b"666"))]
        );
    }

    /// Much the same as above, but test for deletion logic instead of update.
    #[tokio::test]
    async fn serial_insert_delete_tx() {
        let db = test_db().await;
        let tx = db.clone().start_tx();
        let rid = RelationId(0);
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .await
            .unwrap();
        tx.remove_by_domain(rid, attr(b"abc"))
            .await
            .expect("Expected delete to succeed");
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc")).await.unwrap_err(),
            TupleError::NotFound
        );
        tx.commit().await.expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc")).await.unwrap_err(),
            TupleError::NotFound
        );
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .await
            .unwrap();
        tx.update_tuple(rid, attr(b"abc"), attr(b"321"))
            .await
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc")).await.unwrap().1,
            attr(b"321")
        );
        tx.commit().await.expect("Expected commit to succeed");

        // And verify primary & secondary index after the commit.
        let tx = db.start_tx();
        let tuple = tx
            .seek_by_domain(rid, attr(b"abc"))
            .await
            .expect("Expected tuple to exist");
        assert_eq!(tuple.1.as_slice(), b"321");
        assert_eq!(
            tx.seek_by_codomain(rid, attr(b"321"))
                .await
                .expect("Expected secondary seek to succeed"),
            vec![(attr(b"abc"), attr(b"321"))]
        );
    }

    /// Two transactions both starting with nothing present for a tuple.
    /// Both insert and then commit. The second transaction should fail because the first commit
    /// got there first, creating a tuple where we thought none would be.
    /// The insert is not expected to fail until commit time, as we are fully isolated, but when
    /// commit happens, we should detect the conflict and fail.
    #[tokio::test]
    async fn parallel_insert_new_conflict() {
        let db = test_db().await;
        let tx1 = db.clone().start_tx();
        let rid = RelationId(0);
        tx1.insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .await
            .unwrap();

        let tx2 = db.clone().start_tx();
        tx2.insert_tuple(rid, attr(b"abc"), attr(b"zzz"))
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
        let db = test_db().await;
        let rid = RelationId(0);

        // 1. Initial transaction creates value, commits.
        let init_tx = db.clone().start_tx();
        init_tx
            .insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .await
            .unwrap();
        init_tx.commit().await.unwrap();

        // 2. Two transactions get the value, and then update it, in "parallel".
        let tx1 = db.clone().start_tx();
        let tx2 = db.clone().start_tx();
        tx1.update_tuple(rid, attr(b"abc"), attr(b"123"))
            .await
            .unwrap();
        assert_eq!(
            tx1.seek_by_domain(rid, attr(b"abc"))
                .await
                .unwrap()
                .1
                .as_slice(),
            b"123"
        );

        tx2.update_tuple(rid, attr(b"abc"), attr(b"321"))
            .await
            .unwrap();
        assert_eq!(
            tx2.seek_by_domain(rid, attr(b"abc"))
                .await
                .unwrap()
                .1
                .as_slice(),
            b"321"
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

    fn random_tuple() -> (Vec<u8>, Vec<u8>) {
        let mut rng = rand::thread_rng();
        let domain = (0..16).map(|_| rng.gen::<u8>()).collect::<Vec<u8>>();
        let codomain = (0..16).map(|_| rng.gen::<u8>()).collect::<Vec<u8>>();
        (domain, codomain)
    }

    fn assert_same(tuples: &[(SliceRef, SliceRef)], items: &[(Vec<u8>, Vec<u8>)]) {
        assert_eq!(tuples.len(), items.len());
        for (domain, codomain) in tuples {
            let idx = items
                .iter()
                .position(|(d, _)| d == domain.as_slice())
                .unwrap();
            assert_eq!(codomain.as_slice(), items[idx].1.as_slice());
        }
    }

    #[tokio::test]
    async fn predicate_scan_with_predicate() {
        let db = test_db().await;
        let rid = RelationId(0);

        // Scan an empty relation in a transaction
        let tx = db.clone().start_tx();
        let tuples = tx.predicate_scan(rid, &|_| true).await.unwrap();
        assert_eq!(tuples.len(), 0);
        tx.commit().await.unwrap();

        // Scan the same empty relation in a brand new transaction.
        let tx = db.clone().start_tx();
        let tuples = tx.predicate_scan(rid, &|_| true).await.unwrap();
        assert_eq!(tuples.len(), 0);

        // Then insert a pile of of random tuples into the relation, and scan it again.
        let tx = db.clone().start_tx();
        let mut items = vec![];
        for _ in 0..1000 {
            let (domain, codomain) = random_tuple();
            items.push((domain.clone(), codomain.clone()));
            tx.insert_tuple(rid, attr(&domain), attr(&codomain))
                .await
                .unwrap();
        }
        // Scan the local relation, and verify that we get back the same number of tuples.
        let tuples = tx.predicate_scan(rid, &|_| true).await.unwrap();
        assert_same(&tuples, &items);
        tx.commit().await.unwrap();

        // Scan the same relation in a brand new transaction, and verify that we get back the same
        // number of tuples.
        let tx = db.clone().start_tx();
        let tuples = tx.predicate_scan(rid, &|_| true).await.unwrap();
        assert_eq!(tuples.len(), 1000);

        // Randomly delete tuples from the relation, and verify that the scan returns the correct
        // tuples.
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let (domain, _) = items.remove(rng.gen_range(0..items.len()));
            tx.remove_by_domain(rid, attr(&domain)).await.unwrap();
        }
        let tuples = tx.predicate_scan(rid, &|_| true).await.unwrap();
        assert_same(&tuples, &items);
        tx.commit().await.unwrap();

        // Scan the same relation in a brand new transaction, and verify that we get back the same
        // tuples.
        let tx = db.clone().start_tx();
        let tuples = tx.predicate_scan(rid, &|_| true).await.unwrap();
        assert_same(&tuples, &items);

        // Randomly update tuples in the relation, and verify that the scan returns the correct
        // values.
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let (domain, _) = items[rng.gen_range(0..items.len())].clone();
            let new_codomain = (0..16).map(|_| rng.gen::<u8>()).collect::<Vec<u8>>();
            tx.update_tuple(rid, attr(&domain), attr(&new_codomain))
                .await
                .unwrap();
            // Update in `items`
            let idx = items.iter().position(|(d, _)| d == &domain).unwrap();
            items[idx] = (domain, new_codomain);
        }
        // Verify ...
        let tuples = tx.predicate_scan(rid, &|_| true).await.unwrap();
        assert_same(&tuples, &items);

        // And commit and verify in a new tx.
        tx.commit().await.unwrap();
        let tx = db.clone().start_tx();
        let tuples = tx.predicate_scan(rid, &|_| true).await.unwrap();
        assert_same(&tuples, &items);

        // Now insert some new random values.
        for _ in 0..100 {
            let (domain, codomain) = random_tuple();
            // Update in `items` and insert, but only if we don't already have this same domain.
            if items.iter().find(|(d, _)| d == &domain).is_some() {
                continue;
            }
            items.push((domain.clone(), codomain.clone()));
            tx.insert_tuple(rid, attr(&domain), attr(&codomain))
                .await
                .unwrap();
        }
        // And verify that the scan returns the correct number of tuples and values.
        let tuples = tx.predicate_scan(rid, &|_| true).await.unwrap();
        assert_same(&tuples, &items);

        // Commit and verify in a new tx.
        tx.commit().await.unwrap();
        let tx = db.clone().start_tx();
        let tuples = tx.predicate_scan(rid, &|_| true).await.unwrap();
        assert_same(&tuples, &items);
    }

    // TODO: Loom tests? Stateright tests?
    // TODO: Test sequences
    // TODO: Consistency across multiple relations
    // TODO: Index consistency, secondary index consistency
}
