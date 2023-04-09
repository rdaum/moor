use std::collections::{BTreeMap, BTreeSet, Bound, HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

use rkyv::ser::serializers::{AlignedSerializer, AllocSerializer, CompositeSerializer};
use rkyv::ser::Serializer;
use rkyv::{AlignedVec, Archive, Deserialize, Serialize};
use thiserror::Error;

use crate::db::relations::Error::NotFound;
use crate::db::tx::{EntryValue, MvccEntry, MvccTuple, Tx, WAL};
use crate::db::CommitResult;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum Error {
    #[error("tuple not found for key")]
    NotFound,
    #[error("duplicate tuple")]
    Duplicate,
    #[error("commit conflict, abort transaction & retry")]
    Conflict,
}

pub trait SerializationTraits:
    rkyv::Serialize<
    CompositeSerializer<
        AlignedSerializer<AlignedVec>,
        rkyv::ser::serializers::FallbackScratch<
            rkyv::ser::serializers::HeapScratch<0>,
            rkyv::ser::serializers::AllocScratch,
        >,
        rkyv::ser::serializers::SharedSerializeMap,
    >,
>
{
}
impl<
        T: rkyv::Serialize<
            CompositeSerializer<
                AlignedSerializer<AlignedVec>,
                rkyv::ser::serializers::FallbackScratch<
                    rkyv::ser::serializers::HeapScratch<0>,
                    rkyv::ser::serializers::AllocScratch,
                >,
                rkyv::ser::serializers::SharedSerializeMap,
            >,
        >,
    > SerializationTraits for T
{
}

pub trait TupleValueTraits: Clone + Eq + PartialEq + Ord + Archive + SerializationTraits {}
impl<T: Clone + Eq + PartialEq + Ord + Archive + SerializationTraits> TupleValueTraits for T {}

#[derive(
    Debug, Serialize, Deserialize, Archive, Ord, PartialOrd, Copy, Clone, Eq, PartialEq, Hash,
)]
#[archive(compare(PartialEq), check_bytes)]
#[archive_attr(derive(Ord, PartialOrd, Copy, Clone, Eq, PartialEq, Hash,))]
pub struct TupleId(u64);

// Describes a sort of specialized 2-ary relation, where L and R are the types of the two 'columns'.
// Indexes can exist for both K and T columns, but must always exist for K.
// The tuple values are stored in the indexes.
pub struct Relation<L: TupleValueTraits, R: TupleValueTraits> {
    // Tuple storage for this relation.
    // Right now this is a hash mapping tuple IDs to the tuple values.
    // There are likely much faster data structures for this.
    values: HashMap<TupleId, MvccTuple<TupleId, (L, R)>>,
    next_tuple_id: AtomicU64,

    // Indexes for the L and (optionally) R attributes.
    l_index: BTreeMap<L, TupleId>,
    r_index: Option<BTreeMap<R, HashSet<TupleId>>>,

    // The set of current active write-ahead-log entries for transactions that are currently active
    // on this relation.
    wals: HashMap<u64, WAL<TupleId, (L, R)>>,
}

impl<L: TupleValueTraits, R: TupleValueTraits> Default for Relation<L, R> {
    fn default() -> Self {
        Relation::new()
    }
}

impl<L: TupleValueTraits, R: TupleValueTraits> Relation<L, R> {
    pub fn new() -> Self {
        Relation {
            values: Default::default(),
            next_tuple_id: Default::default(),
            l_index: Default::default(),
            r_index: None,
            wals: Default::default(),
        }
    }

    pub fn new_bidirectional() -> Self {
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
            let (_rts, value) =
                self.values
                    .get_mut(tuple_id)
                    .unwrap()
                    .get(tx.tx_start_ts, tuple_id, wal);
            return value.is_some();
        }
        false
    }

    pub fn insert(&mut self, tx: &mut Tx, l: &L, r: &R) -> Result<(), Error> {
        // If there's already a tuple for this row, then we need to check if it's visible to us.
        let tuple_id = if let Some(tuple_id) = self.l_index.get(l) {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            let (rts, value) = tuple.get(tx.tx_start_ts, tuple_id, wal);

            // There's a value visible to us that's not deleted.
            if let Some(_value) = value {
                return Err(Error::Duplicate);
            }

            // The value for the tuple at this index is either tombstoned for us, or invisible, so we can add a new version.
            tuple.set(tx.tx_start_ts, rts, tuple_id, &(l.clone(), r.clone()), wal);

            *tuple_id
        } else {
            // Didn't exist for any transaction, so create a new version, stick in our WAL.
            let tuple_id = TupleId(
                self.next_tuple_id
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            );
            // Start with a tombstone, just to reserve the slot
            self.values.insert(
                tuple_id,
                MvccTuple::new(tx.tx_start_ts, EntryValue::Tombstone),
            );
            self.l_index.insert(l.clone(), tuple_id);

            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            wal.set(
                tuple_id,
                EntryValue::Value((l.clone(), r.clone())),
                tx.tx_start_ts,
            );

            tuple_id
        };

        if let Some(r_index) = &mut self.r_index {
            r_index
                .entry(r.clone())
                .or_insert_with(Default::default)
                .insert(tuple_id);
        }
        Ok(())
    }

    pub fn upsert(&mut self, tx: &mut Tx, l: &L, r: &R) -> Result<(), Error> {
        let e = self.remove_for_l(tx, l);
        if e != Ok(()) && e != Err(NotFound) {
            return e;
        }
        self.insert(tx, l, r)?;

        Ok(())
    }

    pub fn remove_for_l(&mut self, tx: &mut Tx, l: &L) -> Result<(), Error> {
        if let Some(tuple_id) = self.l_index.get(l) {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            let (rts, value) = tuple.get(tx.tx_start_ts, tuple_id, wal);

            // If we already deleted it or it's not visible to us, we can't delete it.
            if value.is_none() {
                return Err(Error::NotFound);
            }

            // There's a value there in some fashion. Tombstone it.
            tuple.delete(tx.tx_start_ts, rts, tuple_id, wal);

            if let Some(r_index) = &mut self.r_index {
                if let Some(value) = value {
                    r_index.entry(value.1).and_modify(|s| {
                        s.remove(tuple_id);
                    });
                }
            }
            return Ok(());
        }

        Err(Error::NotFound)
    }

    pub fn update_l(&mut self, tx: &mut Tx, l: &L, new_l: &L) -> Result<(), Error> {
        if let Some(tuple_id) = self.l_index.get(l) {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            let (rts, value) = tuple.get(tx.tx_start_ts, tuple_id, wal);

            // If it's deleted by us or invisible to us, we can't update it, can we?
            let Some(value) = value else {
                return Err(Error::NotFound);
            };

            if let Some(r_index) = &mut self.r_index {
                r_index.entry(value.1.clone()).and_modify(|s| {
                    s.remove(tuple_id);
                });
            }

            // There's a value there in some fashion. Update it to tombstone, and then perform an
            // insert-equiv for the new key.
            tuple.delete(tx.tx_start_ts, rts, tuple_id, wal);

            // tuple_id is borrowed from self, ugly drop it here so we can use self to do the
            // insert.
            drop(tuple_id);
            self.insert(tx, new_l, &value.1)?;

            return Ok(());
        }

        Err(Error::NotFound)
    }

    pub fn update_r(&mut self, tx: &mut Tx, l: &L, new_r: &R) -> Result<(), Error> {
        if let Some(tuple_id) = self.l_index.get(l) {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            let (_rts, value) = tuple.get(tx.tx_start_ts, tuple_id, wal);

            // If it's deleted by us or invisible to us, we can't update it, can we.
            let Some(value) = value else {
                return Err(Error::NotFound);
            };

            drop(tuple_id);

            self.remove_for_l(tx, &value.0)?;
            self.insert(tx, l, new_r)?;

            return Ok(());
        }

        Err(Error::NotFound)
    }

    pub fn seek_for_l_eq(&mut self, tx: &mut Tx, k: &L) -> Option<R> {
        if let Some(tuple_id) = self.l_index.get(k) {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
            return tuple.get(tx.tx_start_ts, tuple_id, wal).1.map(|v| v.1);
        }
        None
    }

    pub fn range_for_l_eq(&mut self, tx: &mut Tx, range: (Bound<&L>, Bound<&L>)) -> Vec<(L, R)> {
        let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
        let tuple_range = self.l_index.range(range);
        let visible_tuples = tuple_range.filter_map(|(k, tuple_id)| {
            let tuple = self.values.get(tuple_id);
            if let Some(tuple) = tuple {
                let (_rts, value) = tuple.get(tx.tx_start_ts, tuple_id, wal);
                if let Some(value) = value {
                    return Some((k.clone(), value.1));
                }
            };
            None
        });
        visible_tuples.collect()
    }

    pub fn seek_for_r_eq(&mut self, tx: &mut Tx, t: &R) -> BTreeSet<L> {
        let Some(t_index) = &self.r_index else {
            panic!("secondary index query without index");
        };

        let wal = self.wals.entry(tx.tx_id).or_insert_with(Default::default);
        match t_index.get(t) {
            None => BTreeSet::new(),
            Some(tuples) => {
                let visible_tuples = tuples.iter().filter_map(|tuple_id| {
                    let tuple = self.values.get(tuple_id);
                    if let Some(tuple) = tuple {
                        let (_rts, value) = tuple.get(tx.tx_start_ts, tuple_id, wal);
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
        // we have nothing to commit
        let Some(wal) = self.wals.get(&tx.tx_id) else {
            return Ok(());
        };
        // Flush the Tx's WAL writes to the main data structures.
        for (tuple_id, wal_entry) in wal.entries.iter() {
            let tuple = self.values.get_mut(tuple_id).unwrap();
            let result = tuple.commit(tx.tx_start_ts, wal_entry);
            match result {
                CommitResult::Success => continue,
                CommitResult::ConflictRetry => return Err(Error::Conflict),
            }
        }
        // Delete the WAL.
        self.wals.remove(&tx.tx_id);
        Ok(())
    }

    pub fn rollback(&mut self, tx: &mut Tx) -> Result<(), Error> {
        // Rollback should be throwing away the WAL without applying its changes.
        self.wals.remove(&tx.tx_id);
        Ok(())
    }

    pub fn serialize(&self) -> Result<AlignedVec, Error>
    where
        <L as Archive>::Archived: Ord,
        <R as Archive>::Archived: Ord,
    {
        // First we copy into a PRelation / PMvccTuple, which removes locks, WAL, and any other things
        // that are irrelevant for the on-disk form.
        // Then we serialize that.

        let mut pr = PRelation {
            values: Default::default(),
            next_tuple_id: AtomicU64::new(self.next_tuple_id.load(Ordering::SeqCst)),
            l_index: self.l_index.clone(),
            r_index: self.r_index.clone(),
        };

        for (tuple_id, tuple) in self.values.iter() {
            let versions = tuple.versions.read();
            let pmvcc = PMvccTuple {
                versions: versions.clone(),
                pd: Default::default(),
            };
            pr.values.push((*tuple_id, pmvcc));
        }

        let mut serializer = AllocSerializer::<0>::default();
        serializer.serialize_value(&pr).unwrap();
        let bytes = serializer.into_serializer().into_inner();

        Ok(bytes)
    }
}

#[derive(Serialize, Deserialize, Archive)]
pub struct PMvccTuple<K: TupleValueTraits, V: TupleValueTraits> {
    pub versions: Vec<MvccEntry<V>>,
    pd: PhantomData<K>,
}

#[derive(Serialize, Deserialize, Archive)]
pub struct PRelation<L: TupleValueTraits, R: TupleValueTraits> {
    values: Vec<(TupleId, PMvccTuple<TupleId, (L, R)>)>,
    next_tuple_id: AtomicU64,

    // Indexes for the L and (optionally) R attributes.
    l_index: BTreeMap<L, TupleId>,
    r_index: Option<BTreeMap<R, HashSet<TupleId>>>,
}

#[cfg(test)]
mod tests {
    use std::collections::Bound::{Included, Unbounded};

    use crate::db::relations::Error::Conflict;

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
    fn test_secondary_index() {
        let mut relation = Relation::<String, i32>::new_bidirectional();
        let mut tx1 = Tx::new(1, 1);
        assert_eq!(relation.insert(&mut tx1, &"hello".to_string(), &1), Ok(()));
        assert_eq!(relation.insert(&mut tx1, &"bye".to_string(), &1), Ok(()));
        assert_eq!(
            relation.insert(&mut tx1, &"tomorrow".to_string(), &2),
            Ok(())
        );
        assert_eq!(
            relation.insert(&mut tx1, &"yesterday".to_string(), &2),
            Ok(())
        );

        assert_eq!(
            relation.seek_for_r_eq(&mut tx1, &1),
            BTreeSet::from(["hello".into(), "bye".into()])
        );
        assert_eq!(
            relation.seek_for_r_eq(&mut tx1, &2),
            BTreeSet::from(["tomorrow".into(), "yesterday".into()])
        );

        assert_eq!(
            relation.update_l(&mut tx1, &"hello".to_string(), &"everyday".to_string()),
            Ok(())
        );
        assert_eq!(
            relation.seek_for_r_eq(&mut tx1, &1),
            BTreeSet::from(["everyday".into(), "bye".into()])
        );

        assert_eq!(
            relation.remove_for_l(&mut tx1, &"everyday".to_string()),
            Ok(())
        );
        assert_eq!(
            relation.seek_for_r_eq(&mut tx1, &1),
            BTreeSet::from(["bye".into()])
        );

        assert_eq!(relation.upsert(&mut tx1, &"bye".to_string(), &3), Ok(()));
        assert_eq!(relation.seek_for_r_eq(&mut tx1, &1), BTreeSet::from([]));
        assert_eq!(
            relation.seek_for_r_eq(&mut tx1, &3),
            BTreeSet::from(["bye".into()])
        );
        assert_eq!(relation.update_r(&mut tx1, &"bye".to_string(), &4), Ok(()));
        assert_eq!(
            relation.seek_for_r_eq(&mut tx1, &4),
            BTreeSet::from(["bye".into()])
        );
        assert_eq!(relation.seek_for_r_eq(&mut tx1, &3), BTreeSet::from([]));

        assert_eq!(
            relation.range_for_l_eq(&mut tx1, (Included(&"tomorrow".into()), Unbounded)),
            vec![("tomorrow".into(), 2), ("yesterday".into(), 2)]
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
        assert_eq!(a.commit(&mut t1), Ok(()));

        // should fail because t2 (ts 3) is trying to commit a change based on (ts 1) but the most
        // recent committed change is (ts 2)
        assert_eq!(a.commit(&mut t2), Err(Error::Conflict));
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

        assert_eq!(a.commit(&mut t1), Ok(()));
        assert_eq!(a.commit(&mut t2), Err(Error::Conflict));
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
        assert_eq!(a.commit(&mut t1), Err(Conflict));
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

        assert_eq!(a.commit(&mut t1), Ok(()));
        assert_eq!(a.commit(&mut t2), Ok(()));

        // should fail because t3 (ts 3) is trying to commit a change based on a version where
        // there was no tuple present at all. (ts 1 had not committed yet)
        assert_eq!(a.commit(&mut t3), Err(Error::Conflict));
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

        // Fails with conflict because t2 committed its version before t3 did, and t3 based its
        // version off t1's
        assert_eq!(a.commit(&mut t3), Err(Error::Conflict));
    }
}
