use crate::db::tx::{EntryValue, MvccEntry, MvccTuple, Tx, WAL};
use crate::db::CommitResult;
use bytes::BytesMut;
use rkyv::ser::Serializer;
use rkyv::with::ArchiveWith;
use rkyv::{Archive, Archived, Deserialize, Resolver, Serialize};
use std::collections::{BTreeMap, Bound, HashMap, HashSet};
use std::sync::atomic::AtomicUsize;
use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum Error {
    #[error("tuple not found for key")]
    NotFound,
    #[error("duplicate tuple")]
    Duplicate,
    #[error("commit conflict, abort transaction & retry")]
    Conflict,
}

pub trait OrderedKeyTraits: Clone + Eq + PartialEq + Ord + Archive {}
impl<T: Clone + Eq + PartialEq + Ord + Archive> OrderedKeyTraits for T {}

#[derive(
    Debug, Serialize, Deserialize, Archive, Ord, PartialOrd, Copy, Clone, Eq, PartialEq, Hash,
)]
pub struct TupleId(usize);

// Describes a sort of specialized 2-ary relation, where L and R are the types of the two 'columns'.
// Indexes can exist for both K and T columns, but must always exist for K.
// The tuple values are stored in the indexes.
pub struct Relation<L: OrderedKeyTraits, R: OrderedKeyTraits> {
    // Tuple storage for this relation.
    // Right now this is a hash mapping tuple IDs to the tuple values.
    // There are likely much faster data structures for this.
    values: HashMap<TupleId, MvccTuple<TupleId, (L, R)>>,
    next_tuple_id: AtomicUsize,

    // Indexes for the L and (optionally) R attributes.
    l_index: BTreeMap<L, TupleId>,
    r_index: Option<BTreeMap<R, HashSet<TupleId>>>,

    // The set of current active write-ahead-log entries for transactions that are currently active
    // on this relation.
    wals: HashMap<u64, WAL<TupleId, (L, R)>>,
}

impl<L: OrderedKeyTraits, R: OrderedKeyTraits> Default for Relation<L, R> {
    fn default() -> Self {
        Relation::new()
    }
}

impl<L: OrderedKeyTraits, R: OrderedKeyTraits> Relation<L, R> {
    pub fn new() -> Self {
        Relation {
            values: Default::default(),
            next_tuple_id: Default::default(),
            l_index: Default::default(),
            r_index: None,
            wals: Default::default(),
        }
    }

    pub fn new_bidrectional() -> Self {
        Relation {
            values: Default::default(),
            next_tuple_id: Default::default(),
            l_index: Default::default(),
            r_index: Some(Default::default()),
            wals: Default::default(),
        }
    }

    fn has_with_l(&mut self, tx: &mut Tx, k: &L, wal: &mut WAL<TupleId, (L, R)>) -> bool {
        if let Some(tuple_id) = self.l_index.get(k) {
            let value = self
                .values
                .get_mut(tuple_id)
                .unwrap()
                .get(tx.tx_start_ts, tuple_id, wal);
            return value.is_some();
        }
        false
    }

    pub fn insert(&mut self, tx: &mut Tx, l: &L, r: &R) -> Result<(), Error> {
        // If there's already a tuple for this row, then we need to check if it's visible to us.
        if let Some(tuple_id) = self.l_index.get(l) {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            let value = tuple.get(tx.tx_start_ts, tuple_id, wal);

            // There's a value visible to us that's not deleted.
            if let Some(_value) = value {
                return Err(Error::Duplicate);
            }

            // The value for the tuple at this index is either tombstoned for us, or invisible, so we can add a new version.
            tuple.set(tx.tx_start_ts, tuple_id, &(l.clone(), r.clone()), wal);
        } else {
            // Didn't exist for any transaction, so create a new version, stick in our WAL.
            let tuple_id = TupleId(
                self.next_tuple_id
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            );
            self.values.insert(tuple_id, Default::default());
            self.l_index.insert(l.clone(), tuple_id);

            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            wal.set(
                tuple_id,
                EntryValue::Value((l.clone(), r.clone())),
                tx.tx_start_ts,
            );

            if let Some(t_index) = &mut self.r_index {
                t_index
                    .entry(r.clone())
                    .or_insert_with(Default::default)
                    .insert(tuple_id);
            }
        }

        Ok(())
    }

    pub fn upsert(&mut self, tx: &mut Tx, l: &L, r: &R) -> Result<(), Error> {
        if let Some(tuple_id) = self.l_index.get(l) {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);

            // There's a tuple there, either invisible to us or not. But we'll set it on our
            // WAL regardless.
            tuple.set(tx.tx_start_ts, tuple_id, &((l.clone(), r.clone())), wal);
        } else {
            // Didn't exist for any transaction, so create a new version, stick in our WAL.
            let tuple_id = TupleId(
                self.next_tuple_id
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            );
            self.values.insert(tuple_id, Default::default());

            self.l_index.insert(l.clone(), tuple_id);

            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            wal.set(
                tuple_id,
                EntryValue::Value((l.clone(), r.clone())),
                tx.tx_start_ts,
            );

            if let Some(t_index) = &mut self.r_index {
                t_index
                    .entry(r.clone())
                    .or_insert_with(Default::default)
                    .insert(tuple_id);
            }
        }

        // TODO secondary index
        Ok(())
    }

    pub fn remove_for_l(&mut self, tx: &mut Tx, l: &L) -> Result<(), Error> {
        if let Some(tuple_id) = self.l_index.get(l) {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            let value = tuple.get(tx.tx_start_ts, tuple_id, wal);

            // If we already deleted it or it's not visible to us, we can't delete it.
            if value.is_none() {
                return Err(Error::NotFound);
            }

            // There's a value there in some fashion. Tombstone it.
            tuple.delete(tx.tx_start_ts, tuple_id, wal);

            return Ok(());
        }

        Err(Error::NotFound)

        // TODO secondary index
    }

    pub fn update_l(&mut self, tx: &mut Tx, l: &L, new_l: &L) -> Result<(), Error> {
        if let Some(tuple_id) = self.l_index.get(l) {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            let value = tuple.get(tx.tx_start_ts, tuple_id, wal);

            // If it's deleted by us or invisible to us, we can't update it, can we.
            let Some(value) = value else {
                return Err(Error::NotFound);
            };

            // There's a value there in some fashion. Tombstone it.
            tuple.set(tx.tx_start_ts, tuple_id, &(new_l.clone(), value.1), wal);

            return Ok(());
        }

        Err(Error::NotFound)

        // TODO secondary index
    }

    pub fn update_r(&mut self, tx: &mut Tx, l: &L, new_r: &R) -> Result<(), Error> {
        if let Some(tuple_id) = self.l_index.get(l) {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            let value = tuple.get(tx.tx_start_ts, tuple_id, wal);

            // If it's deleted by us or invisible to us, we can't update it, can we.
            let Some(value) = value else {
                return Err(Error::NotFound);
            };

            // There's a value there in some fashion. Tombstone it.
            tuple.set(tx.tx_start_ts, tuple_id, &(value.0, new_r.clone()), wal);

            return Ok(());
        }

        Err(Error::NotFound)
    }

    pub fn seek_for_l_eq(&mut self, tx: &mut Tx, k: &L) -> Option<R> {
        if let Some(tuple_id) = self.l_index.get(k) {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            return tuple.get(tx.tx_start_ts, tuple_id, wal).map(|v| v.1);
        }
        None
    }

    pub fn range_r(&mut self, tx: &mut Tx, range: (Bound<&L>, Bound<&L>)) -> Vec<(L, R)> {
        let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
        let tuple_range = self.l_index.range(range);
        let visible_tuples = tuple_range.filter_map(|(k, tuple_id)| {
            let tuple = self.values.get(tuple_id);
            if let Some(tuple) = tuple {
                let value = tuple.get(tx.tx_start_ts, tuple_id, wal);
                if let Some(value) = value {
                    return Some((k.clone(), value.1));
                }
            };
            None
        });
        visible_tuples.collect()
    }

    pub fn seek_for_r_eq(&mut self, tx: &mut Tx, t: &R) -> Vec<L> {
        let Some(t_index) = &self.r_index else {
            panic!("secondary index query without index");
        };

        let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
        match t_index.get(t) {
            None => vec![],
            Some(tuples) => {
                let visible_tuples = tuples.iter().filter_map(|tuple_id| {
                    let tuple = self.values.get(tuple_id);
                    if let Some(tuple) = tuple {
                        let value = tuple.get(tx.tx_start_ts, tuple_id, wal);
                        if let Some(value) = value {
                            return Some(value.0);
                        }
                    };
                    None
                });
                visible_tuples.collect()
            }
        }
    }

    pub fn commit(&mut self, tx: &mut Tx) -> Result<(), Error> {
        // Flush the Tx's WAL writes to the main data structures.
        let Some(wal) = self.wals.get(&tx.tx_id) else {
            return Ok(());
        };
        for (tuple_id, wal_entry) in wal.entries.iter() {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let result = tuple.commit(tx.tx_start_ts, wal_entry);
            match result {
                CommitResult::Success => continue,
                CommitResult::ConflictRetry => return Err(Error::Conflict),
            }
        }
        Ok(())
    }

    pub fn rollback(&mut self, tx: &mut Tx) -> Result<(), Error> {
        // Rollback should be throwing away the WAL without applying its changes.
        self.wals.remove(&tx.tx_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn insert_new_tuple() {
        let mut relation = Relation::<String, i32>::new();

        let mut tx1 = Tx::new(1, 1);
        assert_eq!(relation.insert(&mut tx1, &"hello".to_string(), &1), Ok(()));
        assert_eq!(relation.insert(&mut tx1, &"world".to_string(), &2), Ok(()));
    }

    #[test]
    fn insert_existing_tuple() {
        let mut relation = Relation::<String, i32>::new();

        let mut tx1 = Tx::new(1, 1);
        assert_eq!(relation.insert(&mut tx1, &"hello".to_string(), &1), Ok(()));
        assert_eq!(
            relation.insert(&mut tx1, &"hello".to_string(), &2),
            Err(Error::Duplicate)
        );
    }

    #[test]
    fn upsert_new_tuple() {
        let mut relation = Relation::<String, i32>::new();

        let mut tx1 = Tx::new(1, 1);
        assert_eq!(relation.upsert(&mut tx1, &"hello".to_string(), &1), Ok(()));
    }

    #[test]
    fn upsert_existing_tuple() {
        let mut relation = Relation::<String, i32>::new();

        let mut tx1 = Tx::new(1, 1);
        assert_eq!(relation.insert(&mut tx1, &"hello".to_string(), &1), Ok(()));
        assert_eq!(relation.upsert(&mut tx1, &"hello".to_string(), &2), Ok(()));
    }

    #[test]
    fn remove_existing_tuple() {
        let mut relation = Relation::<String, i32>::new();

        let mut tx1 = Tx::new(1, 1);
        assert_eq!(relation.insert(&mut tx1, &"hello".to_string(), &1), Ok(()));
        assert_eq!(
            relation.remove_for_l(&mut tx1, &"hello".to_string()),
            Ok(())
        );
    }

    #[test]
    fn remove_nonexistent_tuple() {
        let mut relation = Relation::<String, i32>::new();

        let mut tx1 = Tx::new(1, 1);
        assert_eq!(
            relation.remove_for_l(&mut tx1, &"hello".to_string()),
            Err(Error::NotFound)
        );
    }

    #[test]
    fn insert_transactional() {
        let mut a = Relation::<String, i32>::new();

        let mut s = Tx::new(1, 1);
        assert_eq!(a.insert(&mut s, &"hello".to_string(), &1), Ok(()));
        assert_eq!(a.commit(&mut s), Ok(()));

        let mut t1 = Tx::new(2, 2);
        assert_eq!(a.update_r(&mut t1, &"hello".to_string(), &2), Ok(()));
        let mut t2 = Tx::new(3, 3);
        assert_eq!(a.update_r(&mut t2, &"hello".to_string(), &3), Ok(()));
        assert_eq!(a.commit(&mut t2), Ok(()));
        assert_eq!(a.commit(&mut t1), Err(Error::Conflict));
    }

    #[test]
    fn delete_transactional() {
        let mut a = Relation::<String, i32>::new();

        let mut s = Tx::new(1, 1);
        assert_eq!(a.insert(&mut s, &"hello".to_string(), &1), Ok(()));
        assert_eq!(a.commit(&mut s), Ok(()));

        let mut t1 = Tx::new(2, 2);
        assert_eq!(a.remove_for_l(&mut t1, &"hello".to_string()), Ok(()));

        let mut t2 = Tx::new(3, 3);
        assert_eq!(a.remove_for_l(&mut t2, &"hello".to_string()), Ok(()));

        assert_eq!(a.commit(&mut t2), Ok(()));
        assert_eq!(a.commit(&mut t1), Err(Error::Conflict));
    }

    #[test]
    fn insert_delete_transactional() {
        let mut a = Relation::<String, i32>::new();

        let mut s = Tx::new(1, 1);
        assert_eq!(a.insert(&mut s, &"hello".to_string(), &1), Ok(()));
        assert_eq!(a.commit(&mut s), Ok(()));

        let mut t1 = Tx::new(2, 2);
        assert_eq!(a.remove_for_l(&mut t1, &"hello".to_string()), Ok(()));

        // the delete done by t1 hasn't been committed yet, so this is a duplicate and can't
        // be inserted.
        let mut t2 = Tx::new(3, 3);
        assert_eq!(
            a.insert(&mut t2, &"hello".to_string(), &3),
            Err(Error::Duplicate)
        );
        assert!(a.rollback(&mut t2).is_ok());
        assert_eq!(a.commit(&mut t1), Ok(()));

        // now that t1 has been committed, this insert should succeed.
        let mut t3 = Tx::new(4, 4);
        assert_eq!(a.insert(&mut t3, &"hello".to_string(), &3), Ok(()));
        assert_eq!(a.commit(&mut t3), Ok(()));
    }

    #[test]
    fn update_delete_transactional() {
        let mut a = Relation::<String, i32>::new();

        let mut s = Tx::new(1, 1);
        assert_eq!(a.insert(&mut s, &"hello".to_string(), &1), Ok(()));
        assert_eq!(a.commit(&mut s), Ok(()));

        let mut t1 = Tx::new(2, 2);
        assert_eq!(a.update_r(&mut t1, &"hello".to_string(), &2), Ok(()));

        let mut t2 = Tx::new(3, 3);
        assert_eq!(a.remove_for_l(&mut t2, &"hello".to_string()), Ok(()));

        assert_eq!(a.commit(&mut t2), Ok(()));
        assert_eq!(a.commit(&mut t1), Err(Error::Conflict));
    }

    #[test]
    fn insert_parallel() {
        let mut a = Relation::<String, i32>::new();

        let mut t1 = Tx::new(1, 1);
        assert_eq!(a.insert(&mut t1, &"hello".to_string(), &1), Ok(()));

        let mut t2 = Tx::new(2, 2);
        assert_eq!(a.insert(&mut t2, &"world".to_string(), &2), Ok(()));

        assert_eq!(a.commit(&mut t1), Ok(()));
        assert_eq!(a.commit(&mut t2), Ok(()));
    }

    #[test]
    fn delete_insert_parallel() {
        let mut a = Relation::<String, i32>::new();

        let mut t1 = Tx::new(1, 1);
        assert_eq!(a.insert(&mut t1, &"hello".to_string(), &1), Ok(()));

        let mut t2 = Tx::new(2, 2);
        assert_eq!(
            a.remove_for_l(&mut t2, &"hello".to_string()),
            Err(Error::NotFound)
        );

        let mut t3 = Tx::new(3, 3);
        assert_eq!(a.insert(&mut t3, &"hello".to_string(), &3), Ok(()));

        assert_eq!(a.commit(&mut t1), Err(Error::Conflict));
        assert_eq!(a.commit(&mut t2), Ok(()));
        assert_eq!(a.commit(&mut t3), Ok(()));
    }

    #[test]
    fn update_delete_parallel() {
        let mut a = Relation::<String, i32>::new();

        let mut t1 = Tx::new(1, 1);
        assert_eq!(a.insert(&mut t1, &"hello".to_string(), &1), Ok(()));

        let mut t2 = Tx::new(2, 2);
        assert_eq!(
            a.remove_for_l(&mut t2, &"hello".to_string()),
            Err(Error::NotFound)
        );

        assert_eq!(a.commit(&mut t1), Ok(()));
        let mut t3 = Tx::new(3, 3);
        assert_eq!(a.update_r(&mut t3, &"hello".to_string(), &3), Ok(()));
        assert_eq!(a.commit(&mut t2), Ok(()));
        assert_eq!(a.commit(&mut t3), Ok(()));
    }
}
