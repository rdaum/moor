use crate::db::tx::{EntryValue, MvccTuple, Tx, WAL};
use slotmap::{new_key_type, SlotMap};
use std::collections::{BTreeMap, Bound, HashMap, HashSet};
use thiserror::Error;
use crate::db::CommitResult;

#[derive(Error, Debug)]
pub enum Error {
    #[error("tuple not found for key")]
    NotFound,
    #[error("duplicate tuple")]
    Duplicate,
    #[error("commit conflict, abort transaction & retry")]
    Conflict,
}

pub trait OrderedKeyTraits: Clone + Eq + PartialEq + Ord {}
impl<T: Clone + Eq + PartialEq + Ord> OrderedKeyTraits for T {}

// Describes a sort of specialized 2-ary relation, where K and T are the types of the two 'columns'.
// Indexes can exist for both K and T columns, but must always exist for K.
// The tuple values are stored in the indexes.

new_key_type! {struct TupleId;}

pub struct Relation<K: OrderedKeyTraits, T: OrderedKeyTraits> {
    values: SlotMap<TupleId, MvccTuple<TupleId, (K, T)>>,
    k_index: BTreeMap<K, TupleId>,
    t_index: Option<BTreeMap<T, HashSet<TupleId>>>,
    wals: HashMap<u64, WAL<TupleId, (K, T)>>,
}

impl<K: OrderedKeyTraits, T: OrderedKeyTraits> Default for Relation<K, T> {
    fn default() -> Self {
        Relation::new()
    }
}

impl<K: OrderedKeyTraits, T: OrderedKeyTraits> Relation<K, T> {
    pub fn new() -> Self {
        Relation {
            values: Default::default(),
            k_index: Default::default(),
            t_index: None,
            wals: Default::default(),
        }
    }

    pub fn new_bidrectional() -> Self {
        Relation {
            values: Default::default(),
            k_index: Default::default(),
            t_index: Some(Default::default()),
            wals: Default::default(),
        }
    }

    fn has_k(&mut self, tx: &mut Tx, k: &K, wal: &mut WAL<TupleId, (K, T)>) -> bool {
        if let Some(tuple_id) = self.k_index.get(k) {
            let value = self
                .values
                .get_mut(*tuple_id)
                .unwrap()
                .get(tx.tx_start_ts, tuple_id, wal);
            return value.is_some();
        }
        false
    }

    pub fn insert(&mut self, tx: &mut Tx, k: &K, t: &T) -> Result<(), Error> {
        // If there's already a tuple for this row, then we need to check if it's visible to us.
        if let Some(tuple_id) = self.k_index.get(k) {
            let tuple = self.values.get_mut(*tuple_id).unwrap();
            let wal = self
                .wals
                .entry(tx.tx_id)
                .or_insert_with(Default::default);
            let value = tuple.get(tx.tx_start_ts, tuple_id, wal);

            // There's a value visible to us that's not deleted.
            if let Some(_value) = value {
                return Err(Error::Duplicate);
            }

            // The value for the tuple at this index is either tombstoned for us, or invisible, so we can add a new version.
            tuple.set(tx.tx_start_ts, tuple_id, &(k.clone(), t.clone()), wal);
        } else {
            // Didn't exist for any transaction, so create a new version.
            let tuple_id = self.values.insert(MvccTuple::new(
                tx.tx_start_ts,
                EntryValue::Value((k.clone(), t.clone())),
            ));

            self.k_index.insert(k.clone(), tuple_id);

            if let Some(t_index) = &mut self.t_index {
                t_index
                    .entry(t.clone())
                    .or_insert_with(Default::default)
                    .insert(tuple_id);
            }
        }

        Ok(())
    }

    pub fn upsert(&mut self, tx: &mut Tx, k: &K, t: &T) -> Result<(), Error> {
        if let Some(tuple_id) = self.k_index.get(k) {
            let tuple = self.values.get_mut(*tuple_id).unwrap();
            let wal = self
                .wals
                .entry(tx.tx_id)
                .or_insert_with(Default::default);

            // There's a tuple there, either invisible to us or not. But we'll set it on our
            // WAL regardless.
            tuple.set(tx.tx_start_ts, tuple_id, &((k.clone(), t.clone())), wal);
        } else {
            // Didn't exist for any transaction, so create a new version.
            let tuple_id = self.values.insert(MvccTuple::new(
                tx.tx_start_ts,
                EntryValue::Value((k.clone(), t.clone())),
            ));

            self.k_index.insert(k.clone(), tuple_id);

            if let Some(t_index) = &mut self.t_index {
                t_index
                    .entry(t.clone())
                    .or_insert_with(Default::default)
                    .insert(tuple_id);
            }
        }

        // TODO secondary index
        Ok(())
    }

    pub fn remove(&mut self, tx: &mut Tx, k: &K) -> Result<(), Error> {
        if let Some(tuple_id) = self.k_index.get(k) {
            let tuple = self.values.get_mut(*tuple_id).unwrap();
            let wal = self
                .wals
                .entry(tx.tx_id)
                .or_insert_with(Default::default);
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

    pub fn update_k(&mut self, tx: &mut Tx, k: &K, new_k: &K) -> Result<(), Error> {
        if let Some(tuple_id) = self.k_index.get(k) {
            let tuple = self.values.get_mut(*tuple_id).unwrap();
            let wal = self
                .wals
                .entry(tx.tx_id)
                .or_insert_with(Default::default);
            let value = tuple.get(tx.tx_start_ts, tuple_id, wal);

            // If it's deleted by us or invisible to us, we can't update it, can we.
            let Some(value) = value else {
                return Err(Error::NotFound);
            };

            // There's a value there in some fashion. Tombstone it.
            tuple.set(
                tx.tx_start_ts,
                tuple_id,
                &(new_k.clone(), value.1),
                wal,
            );

            return Ok(());
        }

        Err(Error::NotFound)

        // TODO secondary index
    }

    pub fn update_t(&mut self, tx: &mut Tx, k: &K, new_t: &T) -> Result<(), Error> {
        if let Some(tuple_id) = self.k_index.get(k) {
            let tuple = self.values.get_mut(*tuple_id).unwrap();
            let wal = self
                .wals
                .entry(tx.tx_id)
                .or_insert_with(Default::default);
            let value = tuple.get(tx.tx_start_ts, tuple_id, wal);

            // If it's deleted by us or invisible to us, we can't update it, can we.
            let Some(value) = value else {
                return Err(Error::NotFound);
            };

            // There's a value there in some fashion. Tombstone it.
            tuple.set(
                tx.tx_start_ts,
                tuple_id,
                &(value.0, new_t.clone()),
                wal,
            );

            return Ok(());
        }

        Err(Error::NotFound)
    }

    pub fn find_t(&mut self, tx: &mut Tx, k: &K) -> Option<T> {
        if let Some(tuple_id) = self.k_index.get(k) {
            let tuple = self.values.get_mut(*tuple_id).unwrap();
            let wal = self
                .wals
                .entry(tx.tx_id)
                .or_insert_with(Default::default);
            return tuple
                .get(tx.tx_start_ts, tuple_id, wal)
                .map(|v| v.1);
        }
        None
    }

    pub fn range_t(&mut self, tx: &mut Tx, range: (Bound<&K>, Bound<&K>)) -> Vec<(K, T)> {
        let wal = self
            .wals
            .entry(tx.tx_id)
            .or_insert_with(Default::default);
        let tuple_range = self.k_index.range(range);
        let visible_tuples = tuple_range.filter_map(|(k, tuple_id)| {
            let tuple = self.values.get(*tuple_id);
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

    pub fn find_k(&mut self, tx: &mut Tx, t: &T) -> Vec<K> {
        let Some(t_index) = &self.t_index else {
            panic!("secondary index query without index");
        };

        let wal = self
            .wals
            .entry(tx.tx_id)
            .or_insert_with(Default::default);
        match t_index.get(t) {
            None => vec![],
            Some(tuples) => {
                let visible_tuples = tuples.iter().filter_map(|tuple_id| {
                    let tuple = self.values.get(*tuple_id);
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
            let tuple = self.values.get_mut(*tuple_id).unwrap();
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
