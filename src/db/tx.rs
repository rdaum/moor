use std::marker::PhantomData;

use itertools::Itertools;
use rkyv::{Archive, Deserialize, Serialize};

use crate::db::relations::TupleValueTraits;
use crate::db::tx::EntryValue::Tombstone;

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
pub struct MvccEntry<V: TupleValueTraits> {
    pub tx_id: u64,
    pub value: EntryValue<V>,
    pub read_timestamp: u64,
    pub write_timestamp: u64,
    pub committed: bool,
}

#[derive(Serialize, Deserialize, Archive)]
pub struct MvccTuple<K: TupleValueTraits, V: TupleValueTraits> {
    pub versions: Vec<MvccEntry<V>>,
    pd: PhantomData<K>,
}

impl<K: TupleValueTraits, V: TupleValueTraits> MvccTuple<K, V> {
    pub fn new(ts_i: u64, initial_value: EntryValue<V>) -> Self {
        MvccTuple {
            versions: vec![MvccEntry {
                tx_id: ts_i,
                value: initial_value,
                read_timestamp: ts_i,
                write_timestamp: ts_i,
                committed: false,
            }],
            pd: PhantomData,
        }
    }
}

impl<K: TupleValueTraits, V: TupleValueTraits> Default for MvccTuple<K, V> {
    fn default() -> Self {
        MvccTuple {
            versions: Vec::new(),
            pd: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Archive)]
pub struct WalValue<K: TupleValueTraits, V: TupleValueTraits> {
    tuple: (K, V),
    wts: u64,
}

pub enum CommitCheckResult {
    None,
    Conflict,
    CanCommit(usize),
}
impl<K: TupleValueTraits, V: TupleValueTraits> MvccTuple<K, V> {
    // Check whether we can commit for a given tuple.
    pub fn can_commit(&self, ts_t: u64) -> CommitCheckResult {
        // Find the read timestamp and position of the version we created.
        let our_version = self.versions.iter().find_position(|v| v.tx_id == ts_t);

        // If we don't have a version of our own, we can just move on (shouldn't get here because
        // we were called because the commit set mentioned us, but still ...)
        let Some(our_version) = our_version else {
            return CommitCheckResult::None;
        };
        let (position, our_rts) = (our_version.0, our_version.1.read_timestamp);

        drop(our_version);

        // verify that the version we're trying to commit is based on a newer or same timestamp than
        // any of the extant versions that are out there.
        for x in self.versions.iter() {
            if !x.committed {
                continue;
            }
            let rts_x = x.read_timestamp;

            if our_rts < rts_x {
                return CommitCheckResult::Conflict;
            }
        }
        CommitCheckResult::CanCommit(position)
    }

    pub fn do_commit(&mut self, ts_t: u64, position: usize) -> Result<(), anyhow::Error> {
        let mut our_version = self.versions.get_mut(position).unwrap();
        our_version.committed = true;
        our_version.read_timestamp = ts_t;
        our_version.write_timestamp = ts_t;

        Ok(())
    }

    pub fn rollback(&mut self, ts_t: u64) -> Result<(), anyhow::Error> {
        let version_position = self.versions.iter().position(|t| t.tx_id == ts_t);
        if let Some(version_position) = version_position {
            self.versions.remove(version_position);
        }
        Ok(())
    }

    pub fn set(&mut self, ts_t: u64, rts: u64, value: &V) {
        // If there's a versions lready for this transaction, get it.
        let version = self.versions.iter_mut().find(|p| p.tx_id == ts_t);
        if let Some(version) = version {
            version.value = EntryValue::Value(value.clone());
            version.write_timestamp = ts_t;
            version.read_timestamp = rts;
            return;
        };

        self.versions.push(MvccEntry {
            tx_id: ts_t,
            value: EntryValue::Value(value.clone()),
            write_timestamp: ts_t,
            read_timestamp: rts,
            committed: false,
        });
    }

    pub fn delete(&mut self, ts_t: u64, rts: u64) {
        // If there's a versions already for this transaction, get it.
        let version = self.versions.iter_mut().find(|p| p.tx_id == ts_t);
        if let Some(version) = version {
            version.value = Tombstone;
            version.write_timestamp = ts_t;
            version.read_timestamp = rts;
            return;
        }
        self.versions.push(MvccEntry {
            tx_id: ts_t,
            value: Tombstone,
            write_timestamp: ts_t,
            read_timestamp: rts,
            committed: false,
        });
    }

    pub fn get(&self, ts_t: u64) -> (u64, Option<V>) {
        // If we have our own tx version, use that.
        let version = self.versions.iter().find(|p| p.tx_id == ts_t);
        if let Some(version) = version {
            let value = match &version.value {
                EntryValue::Value(v) => Some(v.clone()),
                Tombstone => None,
            };
            return (version.read_timestamp, value);
        }

        // Reads see the latest version of an object that was *committed* before our tx started
        // So scan through the versions in reverse order, and return the first one that
        // was committed before our tx started.

        // "Let Xk denote the version of X where for a given txn Ti: W-TS(Xk) ≤ TS(Ti)"
        let x_k = self
            .versions
            .iter()
            .rev()
            .filter(|v| v.write_timestamp <= ts_t && v.committed);

        for x in x_k {
            let rts_x = x.read_timestamp;

            // If our timestamp is greater than the read timestamp of the version we're looking at,
            // then we can return that version.
            if ts_t >= rts_x {
                return (
                    rts_x,
                    match &x.value {
                        EntryValue::Value(v) => Some(v.clone()),
                        EntryValue::Tombstone => None,
                    },
                );
            }
        }

        // the returned read-timestamp needs to be the highest committed timestamp for this tuple.
        // if there is none, we return 0.
        let highest = self
            .versions
            .iter()
            .filter(|v| v.committed)
            .map(|v| v.read_timestamp)
            .max()
            .unwrap_or(0);

        (highest, None)
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
