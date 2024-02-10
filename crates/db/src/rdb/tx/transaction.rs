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

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;
use std::thread::yield_now;

use thiserror::Error;

use moor_values::util::{BitArray, Bitset64};
use moor_values::util::{PhantomUnsend, PhantomUnsync, SliceRef};

use crate::rdb::base_relation::BaseRelation;
use crate::rdb::paging::TupleBox;
use crate::rdb::relbox::RelBox;
use crate::rdb::tuples::{TupleError, TupleRef};
use crate::rdb::tx::relvar::RelVar;
use crate::rdb::tx::working_set::WorkingSet;
use crate::rdb::RelationId;

/// A versioned transaction, which is a fork of the current canonical base relations.
pub struct Transaction {
    /// Where we came from, for referencing back to the base relations.
    db: Arc<RelBox>,
    /// The "working set" is the set of retrieved and/or modified tuples from base relations, known
    /// to the transaction, and represents the set of values that will be committed to the base
    /// relations at commit time.
    pub(crate) working_set: RefCell<Option<WorkingSet>>,

    unsend: PhantomUnsend,
    unsync: PhantomUnsync,
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
    pub fn new(ts: u64, slotbox: Arc<TupleBox>, db: Arc<RelBox>) -> Self {
        let ws = WorkingSet::new(slotbox.clone(), &db.relation_info(), ts);

        Self {
            db,
            working_set: RefCell::new(Some(ws)),
            unsend: Default::default(),
            unsync: Default::default(),
        }
    }

    pub fn increment_sequence(&self, sequence_number: usize) -> u64 {
        self.db.clone().increment_sequence(sequence_number)
    }

    pub fn sequence_current(&self, sequence_number: usize) -> u64 {
        self.db.clone().sequence_current(sequence_number)
    }
    pub fn update_sequence_max(&self, sequence_number: usize, value: u64) {
        self.db.clone().update_sequence_max(sequence_number, value)
    }
    pub fn commit(&self) -> Result<(), CommitError> {
        let mut tries = 0;
        'retry: loop {
            tries += 1;
            let commit_ts = self.db.clone().next_ts();
            let mut working_set = self.working_set.borrow_mut();
            let (commit_set, commit_guard) = self
                .db
                .prepare_commit_set(commit_ts, working_set.as_mut().unwrap())?;
            match self.db.try_commit(commit_set, commit_guard) {
                Ok(()) => {
                    let working_set = working_set.take().unwrap();
                    self.db.sync(commit_ts, working_set);
                    return Ok(());
                }
                Err(CommitError::RelationContentionConflict) => {
                    if tries > 50 {
                        return Err(CommitError::RelationContentionConflict);
                    } else {
                        // Release the lock, pause a bit, and retry the commit set.
                        yield_now();
                        continue 'retry;
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub fn db_usage_bytes(&self) -> usize {
        self.db.db_usage_bytes()
    }

    pub fn rollback(&self) -> Result<(), CommitError> {
        self.working_set.borrow_mut().as_mut().unwrap().clear();
        Ok(())
    }

    /// Grab a handle to a relation, which can be used to perform operations on it in the context
    /// of this transaction.
    pub fn relation(&self, relation_id: RelationId) -> RelVar {
        RelVar {
            tx: self,
            id: relation_id,
        }
    }

    /// Attempt to retrieve a tuple from the transaction's working set by its domain, or from the
    /// canonical base relations if it's not found in the working set.
    pub(crate) fn seek_by_domain(
        &self,
        relation_id: RelationId,
        domain: SliceRef,
    ) -> Result<TupleRef, TupleError> {
        let mut ws = self.working_set.borrow_mut();
        ws.as_mut()
            .unwrap()
            .seek_by_domain(&self.db, relation_id, domain)
    }

    pub(crate) fn seek_by_codomain(
        &self,
        relation_id: RelationId,
        codomain: SliceRef,
    ) -> Result<HashSet<TupleRef>, TupleError> {
        let mut ws = self.working_set.borrow_mut();
        ws.as_mut()
            .unwrap()
            .seek_by_codomain(&self.db, relation_id, codomain)
    }

    /// Attempt to insert a tuple into the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub(crate) fn insert_tuple(
        &self,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let mut ws = self.working_set.borrow_mut();
        ws.as_mut()
            .unwrap()
            .insert_tuple(&self.db, relation_id, domain, codomain)
    }

    pub(crate) fn predicate_scan<F: Fn(&TupleRef) -> bool>(
        &self,
        relation_id: RelationId,
        f: &F,
    ) -> Result<Vec<TupleRef>, TupleError> {
        let mut ws = self.working_set.borrow_mut();
        ws.as_mut()
            .unwrap()
            .predicate_scan(&self.db, relation_id, f)
    }

    /// Attempt to update a tuple in the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub(crate) fn update_tuple(
        &self,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let mut ws = self.working_set.borrow_mut();
        ws.as_mut()
            .unwrap()
            .update_tuple(&self.db, relation_id, domain, codomain)
    }

    /// Attempt to upsert a tuple in the transaction's working set, with the intent of eventually
    /// committing it to the canonical base relations.
    pub(crate) fn upsert_tuple(
        &self,
        relation_id: RelationId,
        domain: SliceRef,
        codomain: SliceRef,
    ) -> Result<(), TupleError> {
        let mut ws = self.working_set.borrow_mut();
        ws.as_mut()
            .unwrap()
            .upsert_tuple(&self.db, relation_id, domain, codomain)
    }

    /// Attempt to delete a tuple in the transaction's working set, with the intent of eventually
    /// committing the delete to the canonical base relations.
    pub(crate) fn remove_by_domain(
        &self,
        relation_id: RelationId,
        domain: SliceRef,
    ) -> Result<(), TupleError> {
        let mut ws = self.working_set.borrow_mut();
        ws.as_mut()
            .unwrap()
            .remove_by_domain(&self.db, relation_id, domain)
    }
}

/// A set of tuples to be committed to the canonical base relations, based on a transaction's
/// working set.
pub struct CommitSet {
    pub(crate) ts: u64,
    relations: Box<BitArray<BaseRelation, 64, Bitset64<1>>>,

    unsend: PhantomUnsend,
    unsync: PhantomUnsync,
}

impl CommitSet {
    pub(crate) fn new(ts: u64) -> Self {
        Self {
            ts,
            relations: Box::new(BitArray::new()),
            unsend: Default::default(),
            unsync: Default::default(),
        }
    }

    /// Returns an iterator over the modified relations in the commit set.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (usize, &BaseRelation)> {
        return self.relations.iter();
    }

    /// Returns an iterator over the modified relations in the commit set, moving and consuming the
    /// commit set in the process.
    pub(crate) fn into_iter(self) -> impl IntoIterator<Item = (usize, BaseRelation)> {
        self.relations.take_all().into_iter()
    }

    /// Fork the given base relation into the commit set, if it's not already there.
    pub(crate) fn fork(
        &mut self,
        relation_id: RelationId,
        canonical: &BaseRelation,
    ) -> &mut BaseRelation {
        if self.relations.get(relation_id.0).is_none() {
            let r = canonical.clone();
            self.relations.set(relation_id.0, r);
        }
        self.relations.get_mut(relation_id.0).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rand::Rng;

    use moor_values::util::SliceRef;

    use crate::rdb::relbox::{RelBox, RelationInfo};
    use crate::rdb::tuples::{TupleError, TupleRef};
    use crate::rdb::tx::transaction::CommitError;
    use crate::rdb::{RelationId, Transaction};

    fn attr(slice: &[u8]) -> SliceRef {
        SliceRef::from_bytes(slice)
    }

    fn test_db() -> Arc<RelBox> {
        RelBox::new(
            1 << 24,
            None,
            &[RelationInfo {
                name: "test".to_string(),
                domain_type_id: 0,
                codomain_type_id: 0,
                secondary_indexed: true,
            }],
            0,
        )
    }

    /// Verifies that base relations ("canonical") get updated when successful commits happen.
    #[test]
    fn basic_commit() {
        let db = test_db();
        let tx = db.clone().start_tx();
        let rid = RelationId(0);
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def")).unwrap();
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .expect_err("Expected insert to fail");
        tx.update_tuple(rid, attr(b"abc"), attr(b"123"))
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc")).unwrap().codomain(),
            attr(b"123")
        );
        let t = tx
            .seek_by_codomain(rid, attr(b"123"))
            .expect("Expected secondary seek to succeed");
        let compare_t = t
            .iter()
            .map(|t| (t.domain().clone(), t.codomain().clone()))
            .collect::<Vec<_>>();
        assert_eq!(compare_t, vec![(attr(b"abc"), attr(b"123"))]);

        tx.commit().expect("Expected commit to succeed");

        // Verify canonical state.
        {
            let relation = &db.canonical[0].load();
            let tuple = relation
                .seek_by_domain(attr(b"abc"))
                .expect("Expected tuple to exist");
            assert_eq!(tuple.codomain().as_slice(), b"123");

            let tuples = relation.seek_by_codomain(attr(b"123"));
            assert_eq!(tuples.len(), 1);
            let tuple = tuples.iter().next().unwrap();
            assert_eq!(tuple.domain().as_slice(), b"abc");
            assert_eq!(tuple.codomain().as_slice(), b"123");
        }
    }

    /// Tests basic serial/sequential logic, where transactions mutate the same tuple but do so
    /// sequentially without potential for conflict.
    #[test]
    fn serial_insert_update_tx() {
        let db = test_db();
        let tx = db.clone().start_tx();
        let rid = RelationId(0);
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def")).unwrap();
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .expect_err("Expected insert to fail");
        tx.update_tuple(rid, attr(b"abc"), attr(b"123"))
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc"))
                .unwrap()
                .codomain()
                .as_slice(),
            b"123"
        );
        tx.commit().expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc"))
                .unwrap()
                .codomain()
                .as_slice(),
            b"123"
        );
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .expect_err("Expected insert to fail");
        tx.upsert_tuple(rid, attr(b"abc"), attr(b"321"))
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc"))
                .unwrap()
                .codomain()
                .as_slice(),
            b"321"
        );
        tx.commit().expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc"))
                .unwrap()
                .codomain()
                .as_slice(),
            b"321"
        );
        tx.upsert_tuple(rid, attr(b"abc"), attr(b"666"))
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc"))
                .unwrap()
                .codomain()
                .as_slice(),
            b"666"
        );
        tx.commit().expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc"))
                .unwrap()
                .codomain()
                .as_slice(),
            b"666"
        );

        // And verify secondary index...
        let t = tx
            .seek_by_codomain(rid, attr(b"666"))
            .expect("Expected secondary seek to succeed");
        let compare_t = t
            .iter()
            .map(|t| (t.domain().clone(), t.codomain().clone()))
            .collect::<Vec<_>>();
        assert_eq!(compare_t, vec![(attr(b"abc"), attr(b"666"))]);
    }

    /// Much the same as above, but test for deletion logic instead of update.
    #[test]
    fn serial_insert_delete_tx() {
        let db = test_db();
        let tx = db.clone().start_tx();
        let rid = RelationId(0);
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def")).unwrap();
        tx.remove_by_domain(rid, attr(b"abc"))
            .expect("Expected delete to succeed");
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc")).unwrap_err(),
            TupleError::NotFound
        );
        tx.commit().expect("Expected commit to succeed");

        let tx = db.clone().start_tx();
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc")).unwrap_err(),
            TupleError::NotFound
        );
        tx.insert_tuple(rid, attr(b"abc"), attr(b"def")).unwrap();
        tx.update_tuple(rid, attr(b"abc"), attr(b"321"))
            .expect("Expected update to succeed");
        assert_eq!(
            tx.seek_by_domain(rid, attr(b"abc")).unwrap().codomain(),
            attr(b"321")
        );
        tx.commit().expect("Expected commit to succeed");

        // And verify primary & secondary index after the commit.
        let tx = db.start_tx();
        let tuple = tx
            .seek_by_domain(rid, attr(b"abc"))
            .expect("Expected tuple to exist");
        assert_eq!(tuple.codomain().as_slice(), b"321");

        let t = tx
            .seek_by_codomain(rid, attr(b"321"))
            .expect("Expected secondary seek to succeed");
        let compare_t = t
            .iter()
            .map(|t| (t.domain().clone(), t.codomain().clone()))
            .collect::<Vec<_>>();
        assert_eq!(compare_t, vec![(attr(b"abc"), attr(b"321"))]);
    }

    /// Two transactions both starting with nothing present for a tuple.
    /// Both insert and then commit. The second transaction should fail because the first commit
    /// got there first, creating a tuple where we thought none would be.
    /// The insert is not expected to fail until commit time, as we are fully isolated, but when
    /// commit happens, we should detect the conflict and fail.
    #[test]
    fn parallel_insert_new_conflict() {
        let db = test_db();
        let tx1 = db.clone().start_tx();
        let rid = RelationId(0);
        tx1.insert_tuple(rid, attr(b"abc"), attr(b"def")).unwrap();

        let tx2 = db.clone().start_tx();
        tx2.insert_tuple(rid, attr(b"abc"), attr(b"zzz")).unwrap();

        assert!(tx1.commit().is_ok());
        assert_eq!(
            tx2.commit().expect_err("Expected conflict"),
            CommitError::TupleVersionConflict
        );
    }

    #[test]
    fn parallel_get_update_conflict() {
        let db = test_db();
        let rid = RelationId(0);

        // 1. Initial transaction creates value, commits.
        let init_tx = db.clone().start_tx();
        init_tx
            .insert_tuple(rid, attr(b"abc"), attr(b"def"))
            .unwrap();
        init_tx.commit().unwrap();

        // 2. Two transactions get the value, and then update it, in "parallel".
        let tx1 = db.clone().start_tx();
        let tx2 = db.clone().start_tx();
        tx1.update_tuple(rid, attr(b"abc"), attr(b"123")).unwrap();
        assert_eq!(
            tx1.seek_by_domain(rid, attr(b"abc"))
                .unwrap()
                .codomain()
                .as_slice(),
            b"123"
        );

        tx2.update_tuple(rid, attr(b"abc"), attr(b"321")).unwrap();
        assert_eq!(
            tx2.seek_by_domain(rid, attr(b"abc"))
                .unwrap()
                .codomain()
                .as_slice(),
            b"321"
        );

        // 3. First transaction commits with success but second transaction fails with Conflict,
        // because it is younger than the first transaction, and the first transaction committed
        // a change to the tuple before we could get to it.
        assert!(tx1.commit().is_ok());
        assert_eq!(
            tx2.commit().expect_err("Expected conflict"),
            CommitError::TupleVersionConflict
        );
    }

    fn random_tuple() -> (Vec<u8>, Vec<u8>) {
        let mut rng = rand::thread_rng();
        let domain = (0..16).map(|_| rng.gen::<u8>()).collect::<Vec<u8>>();
        let codomain = (0..16).map(|_| rng.gen::<u8>()).collect::<Vec<u8>>();
        (domain, codomain)
    }

    fn assert_same(tuples: &[TupleRef], items: &[(Vec<u8>, Vec<u8>)]) {
        assert_eq!(tuples.len(), items.len());
        for t in tuples {
            let (domain, codomain) = (t.domain().clone(), t.codomain().clone());
            let idx = items
                .iter()
                .position(|(d, _)| d == domain.as_slice())
                .unwrap();
            assert_eq!(codomain.as_slice(), items[idx].1.as_slice());
        }
    }

    /// Test some few secondary index scenarios:
    ///     a->b, b->b, c->b = b->{a,b,c} -- before and after commit
    #[test]
    fn secondary_indices() {
        let db = test_db();
        let rid = RelationId(0);
        let tx = db.clone().start_tx();
        tx.insert_tuple(rid, attr(b"a"), attr(b"b")).unwrap();
        tx.insert_tuple(rid, attr(b"b"), attr(b"b")).unwrap();
        tx.insert_tuple(rid, attr(b"c"), attr(b"b")).unwrap();

        fn verify(tx: &Transaction, expected: Vec<&[u8]>) {
            let b_results = tx.seek_by_codomain(RelationId(0), attr(b"b")).unwrap();

            let mut domains = b_results
                .iter()
                .map(|d| d.clone().domain().as_slice().to_vec())
                .collect::<Vec<_>>();
            domains.sort();
            assert_eq!(domains, expected);
        }
        verify(&tx, vec![b"a", b"b", b"c"]);

        tx.commit().unwrap();

        let tx = db.clone().start_tx();
        verify(&tx, vec![b"a", b"b", b"c"]);

        // Add another one, in our new transaction
        tx.insert_tuple(rid, attr(b"d"), attr(b"b")).unwrap();
        verify(&tx, vec![b"a", b"b", b"c", b"d"]);

        // And remove one
        tx.remove_by_domain(rid, attr(b"c")).unwrap();
        verify(&tx, vec![b"a", b"b", b"d"]);
    }

    #[test]
    fn predicate_scan_with_predicate() {
        let db = test_db();
        let rid = RelationId(0);

        // Scan an empty relation in a transaction
        let tx = db.clone().start_tx();
        let tuples = tx.predicate_scan(rid, &|_| true).unwrap();
        assert_eq!(tuples.len(), 0);
        tx.commit().unwrap();

        // Scan the same empty relation in a brand new transaction.
        let tx = db.clone().start_tx();
        let tuples = tx.predicate_scan(rid, &|_| true).unwrap();
        assert_eq!(tuples.len(), 0);

        // Then insert a pile of of random tuples into the relation, and scan it again.
        let tx = db.clone().start_tx();
        let mut items = vec![];
        for _ in 0..1000 {
            let (domain, codomain) = random_tuple();
            items.push((domain.clone(), codomain.clone()));
            tx.insert_tuple(rid, attr(&domain), attr(&codomain))
                .unwrap();
        }
        // Scan the local relation, and verify that we get back the same number of tuples.
        let tuples = tx.predicate_scan(rid, &|_| true).unwrap();
        assert_same(&tuples, &items);
        tx.commit().unwrap();

        // Scan the same relation in a brand new transaction, and verify that we get back the same
        // number of tuples.
        let tx = db.clone().start_tx();
        let tuples = tx.predicate_scan(rid, &|_| true).unwrap();
        assert_eq!(tuples.len(), 1000);

        // Randomly delete tuples from the relation, and verify that the scan returns the correct
        // tuples.
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let (domain, _) = items.remove(rng.gen_range(0..items.len()));
            tx.remove_by_domain(rid, attr(&domain)).unwrap();
        }
        let tuples = tx.predicate_scan(rid, &|_| true).unwrap();
        assert_same(&tuples, &items);
        tx.commit().unwrap();

        // Scan the same relation in a brand new transaction, and verify that we get back the same
        // tuples.
        let tx = db.clone().start_tx();
        let tuples = tx.predicate_scan(rid, &|_| true).unwrap();
        assert_same(&tuples, &items);

        // Randomly update tuples in the relation, and verify that the scan returns the correct
        // values.
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let (domain, _) = items[rng.gen_range(0..items.len())].clone();
            let new_codomain = (0..16).map(|_| rng.gen::<u8>()).collect::<Vec<u8>>();
            tx.update_tuple(rid, attr(&domain), attr(&new_codomain))
                .unwrap();
            // Update in `items`
            let idx = items.iter().position(|(d, _)| d == &domain).unwrap();
            items[idx] = (domain, new_codomain);
        }
        // Verify ...
        let tuples = tx.predicate_scan(rid, &|_| true).unwrap();
        assert_same(&tuples, &items);

        // And commit and verify in a new tx.
        tx.commit().unwrap();
        let tx = db.clone().start_tx();
        let tuples = tx.predicate_scan(rid, &|_| true).unwrap();
        assert_same(&tuples, &items);

        // Now insert some new random values.
        for _ in 0..100 {
            let (domain, codomain) = random_tuple();
            // Update in `items` and insert, but only if we don't already have this same domain.
            if items.iter().any(|(d, _)| d == &domain) {
                continue;
            }
            items.push((domain.clone(), codomain.clone()));
            tx.insert_tuple(rid, attr(&domain), attr(&codomain))
                .unwrap();
        }
        // And verify that the scan returns the correct number of tuples and values.
        let tuples = tx.predicate_scan(rid, &|_| true).unwrap();
        assert_same(&tuples, &items);

        // Commit and verify in a new tx.
        tx.commit().unwrap();
        let tx = db.clone().start_tx();
        let tuples = tx.predicate_scan(rid, &|_| true).unwrap();
        assert_same(&tuples, &items);
    }

    // TODO(rdaum): More tests for transaction.rs and transactions generally
    //    Loom tests? Stateright tests?
    //    Test sequences & their behaviour
    //    Consistency across multiple relations
    //    Index consistency, secondary index consistency
}
