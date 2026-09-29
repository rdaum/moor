// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! A storage batch and its metadata advance together through consuming states.

use super::*;

pub(super) struct PreparedPersistence<'a> {
    db: &'a fjall::Database,
    state: &'a mut WriterState,
    write_batch: fjall::OwnedWriteBatch,
    version: u64,
    transaction: u64,
    property_definition_changes: Box<[PropertyDefinitionChange]>,
    property_value_changes: Vec<PropertyValueChainChange>,
    next_property_value_record_version: Option<u64>,
    encoding: EncodingStats,
    sequences: Vec<SequenceUpdate>,
}

/// Only a successful storage commit can create a completion capability.
pub(super) struct PersistedBatch<'a> {
    db: &'a fjall::Database,
    state: &'a mut WriterState,
    version: u64,
}

impl<'a> PreparedPersistence<'a> {
    pub(super) fn prepare(
        db: &'a fjall::Database,
        batch: EncodedCommitBatch,
        state: &'a mut WriterState,
        rollup_encoder: &RollupEncoder,
    ) -> Result<Self, String> {
        if batch.version != state.next_version {
            return Err(format!(
                "expected persistence version {}, got {}",
                state.next_version, batch.version
            ));
        }
        let EncodedCommitBatch {
            version,
            timestamp,
            operations,
            property_definition_changes,
            sequences,
            admission: _,
            mut encoding,
        } = batch;
        let transaction = timestamp.0;
        let mut write_batch = db.batch();
        let mut property_value_changes = Vec::new();
        let property_value_record_version = state.next_property_value_record_version;

        for op in operations {
            let EncodedBatchOp { partition, op_type } = op;
            match op_type {
                EncodedBatchOpType::Insert { key, value } => {
                    write_batch.insert(&partition, key, value);
                }
                EncodedBatchOpType::Delete { key } => {
                    write_batch.remove(&partition, key);
                }
                EncodedBatchOpType::PropertyValue(op) => {
                    let EncodedPropertyValueOp { property, mutation } = op;
                    let previous_chain = state.property_value_chains.get(&property);
                    match mutation {
                        EncodedPropertyValueMutation::Replace { record } => {
                            let key = encode_property_value_record_key(
                                &property,
                                property_value_record_version,
                            );
                            write_batch.insert(&partition, key, record);
                            if let Some(chain) = previous_chain {
                                for old_version in chain.record_versions() {
                                    let key =
                                        encode_property_value_record_key(&property, old_version);
                                    write_batch.remove(&partition, key);
                                    encoding.encoded_bytes += PROPERTY_RECORD_KEY_BYTES;
                                }
                            }
                            property_value_changes.push(PropertyValueChainChange::Reset {
                                property,
                                full_version: property_value_record_version,
                            });
                        }
                        EncodedPropertyValueMutation::AppendList {
                            record,
                            payload_bytes,
                            final_value,
                        } => {
                            let Some(chain) = previous_chain else {
                                return Err(format!(
                                    "property-value append for {property} has no complete record"
                                ));
                            };
                            if chain.reaches_limit(payload_bytes, state.property_value_limits) {
                                let rollup = rollup_encoder.encode(final_value, timestamp)?;
                                db_counters().timers_rare.record_elapsed(
                                    WorldStateTimerOp::PropertyValueRollupEncode,
                                    rollup.elapsed,
                                );
                                db_counters()
                                    .counters
                                    .inc(WorldStateCountOp::PropertyValueForegroundRollup);
                                db_counters().counters.add(
                                    WorldStateCountOp::PropertyValueFullEncodedBytes,
                                    isize::try_from(rollup.record.len()).unwrap_or(isize::MAX),
                                );
                                encoding.elapsed += rollup.elapsed;
                                encoding.encoded_bytes +=
                                    PROPERTY_RECORD_KEY_BYTES + rollup.record.len();
                                let source = BatchOpSource::Property {
                                    relation: "object_propvalues",
                                    object: property.obj(),
                                    uuid: property.uuid(),
                                };
                                if encoding.slowest.as_ref().is_none_or(
                                    |(_, slowest_elapsed, _)| rollup.elapsed > *slowest_elapsed,
                                ) {
                                    encoding.slowest = Some((
                                        source,
                                        rollup.elapsed,
                                        PROPERTY_RECORD_KEY_BYTES + rollup.record.len(),
                                    ));
                                }

                                let key = encode_property_value_record_key(
                                    &property,
                                    property_value_record_version,
                                );
                                write_batch.insert(&partition, key, rollup.record);
                                for old_version in chain.record_versions() {
                                    let key =
                                        encode_property_value_record_key(&property, old_version);
                                    write_batch.remove(&partition, key);
                                    encoding.encoded_bytes += PROPERTY_RECORD_KEY_BYTES;
                                }
                                property_value_changes.push(PropertyValueChainChange::Reset {
                                    property,
                                    full_version: property_value_record_version,
                                });
                            } else {
                                let key = encode_property_value_record_key(
                                    &property,
                                    property_value_record_version,
                                );
                                write_batch.insert(&partition, key, record);
                                property_value_changes.push(PropertyValueChainChange::Append {
                                    property,
                                    record_version: property_value_record_version,
                                    payload_bytes,
                                });
                            }
                        }
                        EncodedPropertyValueMutation::Delete => {
                            if let Some(chain) = previous_chain {
                                for old_version in chain.record_versions() {
                                    let key =
                                        encode_property_value_record_key(&property, old_version);
                                    write_batch.remove(&partition, key);
                                    encoding.encoded_bytes += PROPERTY_RECORD_KEY_BYTES;
                                }
                            }
                            property_value_changes.push(PropertyValueChainChange::Delete(property));
                        }
                    }
                }
            }
        }

        // Sequence observations are applied monotonically: a reordered submission must never
        // lower a stored high-water mark.
        for update in &sequences {
            let Some(current) = state.confirmed_sequences.get(update.slot).copied() else {
                return Err(format!(
                    "sequence update names out-of-range slot {}",
                    update.slot
                ));
            };
            let next = current.max(update.value);
            if next != current {
                write_batch.insert(
                    &state.sequences_partition,
                    update.slot.to_le_bytes(),
                    next.to_le_bytes(),
                );
            }
        }

        let next_property_value_record_version = if property_value_changes.is_empty() {
            None
        } else {
            Some(
                property_value_record_version
                    .checked_add(1)
                    .ok_or_else(|| "property-value record version exhausted".to_string())?,
            )
        };

        Ok(Self {
            db,
            state,
            write_batch,
            version,
            transaction,
            property_definition_changes,
            property_value_changes,
            next_property_value_record_version,
            encoding,
            sequences,
        })
    }

    pub(super) fn commit(self) -> Result<PersistedBatch<'a>, String> {
        self.commit_with(|batch| {
            batch
                .commit()
                .map_err(|error| format!("failed to commit Fjall write batch: {error}"))
        })
    }

    fn commit_with(
        self,
        store: impl FnOnce(fjall::OwnedWriteBatch) -> Result<(), String>,
    ) -> Result<PersistedBatch<'a>, String> {
        let Self {
            db,
            state,
            write_batch,
            version,
            transaction,
            property_definition_changes,
            property_value_changes,
            next_property_value_record_version,
            encoding,
            sequences,
        } = self;
        let op_count = write_batch.len();
        let outstanding_flushes_before = db.outstanding_flushes();
        let active_compactions_before = db.active_compactions();
        let commit_start = Instant::now();
        store(write_batch)?;
        let commit_elapsed = commit_start.elapsed();
        db_counters()
            .timers_rare
            .record_elapsed(WorldStateTimerOp::BatchWriterCommit, commit_elapsed);
        for update in sequences {
            let current = state
                .confirmed_sequences
                .get_mut(update.slot)
                .expect("validated sequence slot");
            *current = (*current).max(update.value);
        }
        state.property_names.apply(property_definition_changes);
        for change in property_value_changes {
            match change {
                PropertyValueChainChange::Reset {
                    property,
                    full_version,
                } => {
                    state
                        .property_value_chains
                        .insert(property, PropertyValueChain::full(full_version));
                }
                PropertyValueChainChange::Append {
                    property,
                    record_version,
                    payload_bytes,
                } => {
                    state
                        .property_value_chains
                        .get_mut(&property)
                        .expect("validated property-value append chain")
                        .push_append(record_version, payload_bytes);
                }
                PropertyValueChainChange::Delete(property) => {
                    state.property_value_chains.remove(&property);
                }
            }
        }
        if let Some(next_property_value_record_version) = next_property_value_record_version {
            state.next_property_value_record_version = next_property_value_record_version;
        }

        if encoding.elapsed > ENCODE_WARNING_DURATION
            && let Some((slowest_target, slowest_encode_elapsed, slowest_encoded_bytes)) =
                encoding.slowest
        {
            let slowest_target = state.property_names.display(&slowest_target);
            warn!(
                op_count,
                encoded_bytes = encoding.encoded_bytes,
                version,
                transaction,
                encode_elapsed = ?encoding.elapsed,
                ?commit_elapsed,
                slowest_target = %slowest_target,
                ?slowest_encode_elapsed,
                slowest_encoded_bytes,
                outstanding_flushes_before,
                outstanding_flushes_after = db.outstanding_flushes(),
                active_compactions_before,
                active_compactions_after = db.active_compactions(),
                "Slow batch encoding. This value used the most encoding time. Split large property values across properties."
            );
        } else if commit_elapsed > WRITE_WARNING_DURATION {
            warn!(
                op_count,
                encoded_bytes = encoding.encoded_bytes,
                version,
                transaction,
                encode_elapsed = ?encoding.elapsed,
                ?commit_elapsed,
                outstanding_flushes_before,
                outstanding_flushes_after = db.outstanding_flushes(),
                active_compactions_before,
                active_compactions_after = db.active_compactions(),
                "Slow Fjall batch commit"
            );
        }

        Ok(PersistedBatch { db, state, version })
    }
}

impl PersistedBatch<'_> {
    pub(super) fn complete(
        self,
        completed_version: &AtomicU64,
        admission: Option<CommitAdmission>,
    ) {
        completed_version.store(self.version, Ordering::Release);
        drop(admission);
        self.state.next_version += 1;
        BatchWriter::reply_ready_barriers(self.state, self.version);
        BatchWriter::reply_ready_snapshots(self.db, self.state, self.version);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fjall::{KeyspaceCreateOptions, Readable};
    use moor_var::v_int;

    #[test]
    fn storage_failure_preserves_metadata_and_success_completes_waiters() {
        let directory = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(directory.path()).open().unwrap();
        let partition = db
            .keyspace("values", KeyspaceCreateOptions::default)
            .unwrap();
        let property = ObjAndUUIDHolder::new(&Obj::mk_id(42), Uuid::from_u128(7));
        let mut names = AHashMap::new();
        names.insert(property.uuid(), Symbol::mk("before"));
        let mut state = WriterState::new(
            names,
            AHashMap::new(),
            PROPERTY_VALUE_CHAIN_LIMITS,
            db.keyspace("sequences", KeyspaceCreateOptions::default)
                .unwrap(),
            Vec::new(),
        );
        let completed = AtomicU64::new(0);
        let (sender, _receiver) = flume::bounded(1);
        let rollup = RollupEncoder { sender };
        let record =
            encode_full_record(&mut planus::Builder::new(), &v_int(42), Timestamp(1)).unwrap();
        let batch = || EncodedCommitBatch {
            version: 1,
            timestamp: Timestamp(1),
            operations: vec![EncodedBatchOp {
                partition: partition.clone(),
                op_type: EncodedBatchOpType::PropertyValue(EncodedPropertyValueOp {
                    property: property.clone(),
                    mutation: EncodedPropertyValueMutation::Replace {
                        record: record.clone().into(),
                    },
                }),
            }],
            sequences: Vec::new(),
            admission: None,
            property_definition_changes: vec![PropertyDefinitionChange::Remove(property.uuid())]
                .into_boxed_slice(),
            encoding: EncodingStats {
                elapsed: Duration::ZERO,
                encoded_bytes: record.len(),
                slowest: None,
            },
        };
        let (reply, barrier) = oneshot::channel();
        state.barrier_waiters.push((1, reply));
        let (reply, snapshot) = oneshot::channel();
        state.snapshot_waiters.push((1, reply));
        let result = PreparedPersistence::prepare(&db, batch(), &mut state, &rollup)
            .unwrap()
            .commit_with(|_| Err("injected storage error".to_string()));
        assert!(matches!(result, Err(ref e) if e == "injected storage error"));
        drop(result);
        assert_eq!(state.next_version, 1);
        assert_eq!(state.next_property_value_record_version, 1);
        assert!(state.property_names.by_uuid.contains_key(&property.uuid()));
        assert!(state.property_value_chains.is_empty());
        assert_eq!(completed.load(Ordering::Acquire), 0);
        assert!(barrier.try_recv().is_err());
        assert!(snapshot.try_recv().is_err());
        let key = encode_property_value_record_key(&property, 1);
        assert!(partition.get(key).unwrap().is_none());

        let persisted = PreparedPersistence::prepare(&db, batch(), &mut state, &rollup)
            .unwrap()
            .commit()
            .unwrap();
        // Acceptance and metadata updates precede publication to waiters.
        assert_eq!(completed.load(Ordering::Acquire), 0);
        assert!(barrier.try_recv().is_err());
        persisted.complete(&completed, None);
        assert_eq!(completed.load(Ordering::Acquire), 1);
        assert_eq!(state.next_version, 2);
        assert_eq!(state.next_property_value_record_version, 2);
        assert!(!state.property_names.by_uuid.contains_key(&property.uuid()));
        assert_eq!(
            state.property_value_chains[&property],
            PropertyValueChain::full(1)
        );
        barrier.recv().unwrap().unwrap();
        let snapshot = snapshot.recv().unwrap().unwrap();
        assert_eq!(
            snapshot.get(&partition, key).unwrap().unwrap().as_ref(),
            record.as_slice()
        );
    }
}
