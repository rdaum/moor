/// MVCC transaction mgmt
use std::collections::BTreeMap;
use std::marker::PhantomData;

use hybrid_lock::HybridLock;
use itertools::Itertools;
use rkyv::{Archive, Deserialize, Serialize};

use crate::db::relations::TupleValueTraits;
use crate::db::tx::CommitResult::Success;
use crate::db::CommitResult;
use crate::db::CommitResult::ConflictRetry;

// Each base relation carries a WAL for each transaction that is currently active.
// The WAL has a copy of each tuple that was modified by the transaction.
// At commit, an attempt is amde to apply the commit to the shared tree.
// If there's a conflict, the transaction is aborted and asked to be restarted.
// Otherwise, the new version is committed to the shared tree.
// Committing consists of appending the new version to the end of the vector of versions.

// TODO verify correctness and thread safety
// TODO WAL flush to disk (and, uhh, relations, too)
// TODO versions GC
// TODO optimize
// TODO clean up what's in here vs relations.rs

#[derive(Debug, Clone, Serialize, Deserialize, Archive)]
pub enum EntryValue<V: TupleValueTraits> {
    Value(V),
    Tombstone,
}

#[derive(Debug, Clone, Serialize, Deserialize, Archive)]
pub struct WAL<K, V: TupleValueTraits> {
    pub ts: u64,
    pub entries: BTreeMap<K, MvccEntry<V>>,
}

impl<K: TupleValueTraits, V: TupleValueTraits> WAL<K, V> {
    pub fn new(ts: u64) -> Self {
        Self {
            ts,
            entries: Default::default(),
        }
    }
    pub fn set(&mut self, tx: &mut Tx, key: K, value: EntryValue<V>, ts_i: u64) {
        self.entries.insert(
            key,
            MvccEntry {
                value,
                write_timestmap: tx.tx_start_ts,
                read_timestamp: ts_i,
            },
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Archive)]
pub struct MvccEntry<V: TupleValueTraits> {
    pub value: EntryValue<V>,
    pub read_timestamp: u64,
    pub write_timestmap: u64,
}

#[derive(Serialize, Deserialize, Archive)]
pub struct MvccTuple<K: TupleValueTraits, V: TupleValueTraits> {
    pub versions: HybridLock<Vec<MvccEntry<V>>>,
    pd: PhantomData<K>,
}

impl<K: TupleValueTraits, V: TupleValueTraits> MvccTuple<K, V> {
    pub fn new(ts_i: u64, initial_value: EntryValue<V>) -> Self {
        MvccTuple {
            versions: HybridLock::new(vec![MvccEntry {
                value: initial_value,
                read_timestamp: ts_i,
                write_timestmap: ts_i,
            }]),
            pd: PhantomData,
        }
    }
}

impl<K: TupleValueTraits, V: TupleValueTraits> Default for MvccTuple<K, V> {
    fn default() -> Self {
        MvccTuple {
            versions: HybridLock::new(Vec::new()),
            pd: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Archive)]
pub struct WalValue<K: TupleValueTraits, V: TupleValueTraits> {
    tuple: (K, V),
    wts: u64,
}

impl<K: TupleValueTraits, V: TupleValueTraits> MvccTuple<K, V> {
    // TODO: vacuum/GC of old versions

    // Try to commit a value to the tuple.
    pub fn commit(&mut self, ts_t: u64, value: &MvccEntry<V>) -> CommitResult {
        let mut versions = self.versions.write();

        let iter_versions = versions.iter_mut().sorted_by_key(|v| v.read_timestamp);

        // verify that the version we're trying to commit is based on a newer or same timestamp
        for x in iter_versions {
            let rts_x = x.read_timestamp;
            let wts_x = x.write_timestmap;

            // We can't commit a new version if there's extant versions that have a timestamp less
            // than ours.
            if rts_x < ts_t {
                return ConflictRetry;
            }

            // If this is our version, we can just overwrite it.
            if ts_t == wts_x {
                *x = value.clone();
                return Success;
            }
        }

        // Otherwise create a new version of the value.
        versions.push(MvccEntry {
            value: value.value.clone(),
            read_timestamp: ts_t,
            write_timestmap: ts_t,
        });

        Success
    }

    pub fn set(&mut self, ts_t: u64, rts: u64, key: &K, value: &V, tx_wal: &mut WAL<K, V>) {
        // Set a tuple value in the WAL for this transaction.
        // The read-timestamp should be the version of the tuple that we're basing it off.
        tx_wal.entries.insert(
            key.clone(),
            MvccEntry {
                value: EntryValue::Value(value.clone()),
                write_timestmap: ts_t,
                read_timestamp: rts,
            },
        );
    }

    pub fn delete(&mut self, ts_t: u64, rts: u64, key: &K, tx_wal: &mut WAL<K, V>) {
        // Set a value in the WAL for this transaction.
        // The read-timestamp should be the version of the tuple that we're basing it off.
        tx_wal.entries.insert(
            key.clone(),
            MvccEntry {
                value: EntryValue::Tombstone,
                write_timestmap: ts_t,
                read_timestamp: rts,
            },
        );
    }

    pub fn get(&self, ts_t: u64, key: &K, tx_wal: &mut WAL<K, V>) -> (u64, Option<V>) {
        // If the transaction WAL has a value for this key, return that.
        if let Some(wal_local_val) = tx_wal.entries.get(key) {
            return (
                wal_local_val.read_timestamp,
                match &wal_local_val.value {
                    EntryValue::Value(v) => Some(v.clone()),
                    EntryValue::Tombstone => None,
                },
            );
        };

        // Reads see the latest version of an object that was committed before our tx started
        // So scan through the versions in reverse order, and return the first one that
        // was committed before our tx started.

        // "Let Xk denote the version of X where for a given txn Ti: W-TS(Xk) ≤ TS(Ti)"
        let versions = self.versions.write();

        let x_k = versions.iter().rev().filter(|v| v.write_timestmap <= ts_t);

        for x in x_k {
            let rts_x = x.read_timestamp;

            // If our timestamp is greater than the read timestamp of the version we're looking at,
            // then we can return that version.
            if ts_t >= rts_x {
                // We make a copy of the value and shove it into our WAL.
                tx_wal.entries.insert(key.clone(), x.clone());
                return (
                    rts_x,
                    match &x.value {
                        EntryValue::Value(v) => Some(v.clone()),
                        EntryValue::Tombstone => None,
                    },
                );
            }
        }

        (ts_t, None)
    }
}

pub struct Tx {
    pub tx_id: u64,
    pub tx_start_ts: u64,
}

impl Tx {
    pub fn new(tx_id: u64, tx_start_ts: u64) -> Tx {
        Self { tx_id, tx_start_ts }
    }
}
