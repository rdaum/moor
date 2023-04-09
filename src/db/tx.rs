/// MVCC transaction mgmt
use std::collections::BTreeMap;
use std::marker::PhantomData;

use hybrid_lock::HybridLock;
use rkyv::{Archive, Deserialize, Serialize};

use crate::db::CommitResult;
use crate::db::CommitResult::ConflictRetry;
use crate::db::relations::TupleValueTraits;
use crate::db::tx::CommitResult::Success;

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
pub struct WALEntry<V: TupleValueTraits> {
    value: EntryValue<V>,
    wts: u64,
    rts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Archive)]
pub struct WAL<K, V: TupleValueTraits> {
    pub entries: BTreeMap<K, WALEntry<V>>,
}

impl<K: TupleValueTraits, V: TupleValueTraits> WAL<K, V> {
    pub fn set(&mut self, key: K, value: EntryValue<V>, ts_i: u64) {
        self.entries.insert(
            key,
            WALEntry {
                value,
                wts: ts_i,
                rts: ts_i,
            },
        );
    }
}

impl<K: TupleValueTraits, V: TupleValueTraits> Default for WAL<K, V> {
    fn default() -> Self {
        WAL {
            entries: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Archive)]
pub struct MvccEntry<V: TupleValueTraits> {
    value: EntryValue<V>,
    read_timestamp: u64,
    write_timestmap: u64,
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

    pub fn commit(&mut self, ts_t: u64, value: &WALEntry<V>) -> CommitResult {
        // If ts_t is younger than the read stamp of any of our values, then we have a conflict
        // abort and restart the transaction.
        // If ts_t == wts_x, then we can overwrite.
        let mut versions = self.versions.write();
        for x in versions.iter_mut().rev() {
            let rts_x = x.read_timestamp;
            let wts_x = x.write_timestmap;
            if ts_t < rts_x {
                return ConflictRetry;
            }
            if ts_t == wts_x {
                x.value = value.value.clone();
                x.write_timestmap = ts_t;
                return Success;
            }
        }

        // Otherwise create a new version of the value
        versions.push(MvccEntry {
            value: value.value.clone(),
            read_timestamp: ts_t,
            write_timestmap: ts_t,
        });

        Success
    }

    pub fn set(&mut self, ts_t: u64, key: &K, value: &V, tx_wal: &mut WAL<K, V>) {
        // Set a value in the WAL for this transaction.
        tx_wal.entries.insert(
            key.clone(),
            WALEntry {
                value: EntryValue::Value(value.clone()),
                wts: ts_t,
                rts: ts_t,
            },
        );
    }

    pub fn delete(&mut self, ts_t: u64, key: &K, tx_wal: &mut WAL<K, V>) {
        // Set a value in the WAL for this transaction.
        // TODO: is this correct?
        tx_wal.entries.insert(
            key.clone(),
            WALEntry {
                value: EntryValue::Tombstone,
                wts: ts_t,
                rts: ts_t,
            },
        );
    }

    pub fn get(&self, ts_t: u64, key: &K, tx_wal: &mut WAL<K, V>) -> Option<V> {
        // If the transaction WAL has a value for this key, return that.
        if let Some(wal_local_val) = tx_wal.entries.get(key) {
            return match &wal_local_val.value {
                EntryValue::Value(v) => Some(v.clone()),
                EntryValue::Tombstone => None,
            };
        };

        // Reads see the latest version of an object that was committed before our tx started
        // So scan through the versions in reverse order, and return the first one that
        // was committed before our tx started.

        // "Let Xk denote the version of X where for a given txn Ti: W-TS(Xk) ≤ TS(Ti)"
        let versions = self.versions.write();

        let x_k = versions.iter().rev().filter(|v| v.write_timestmap <= ts_t);

        for x in x_k {
            let rts_x = x.read_timestamp;
            if ts_t >= rts_x {
                // We make a copy of the value and shove it into our WAL.
                tx_wal.entries.insert(
                    key.clone(),
                    WALEntry {
                        value: x.value.clone(),
                        wts: x.write_timestmap,
                        rts: ts_t,
                    },
                );
                return match &x.value {
                    EntryValue::Value(v) => Some(v.clone()),
                    EntryValue::Tombstone => None,
                };
            }
        }

        None
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
