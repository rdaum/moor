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

//! One repeatable-read startup snapshot streamed directly into resident indexes.
use super::{
    PostgresConnection, PostgresError, PostgresStorageConfig,
    codec::{self, invalid},
    rows::{self, RowKey, RowValue},
    state::{self, Progress},
};
use crate::{
    ObjAndUUIDHolder,
    engine::moor_db::{Caches, Relations, SEQUENCE_COUNT, WorldStateSnapshot},
    provider::{
        backend::SeededWorld,
        property_value_store::{
            PROPERTY_VALUE_CHAIN_LIMITS, PropertyValueChain, PropertyValueReconstructor,
            PropertyValueRecordKind,
        },
    },
    tx::{RelationCodomain, RelationDomain, Timestamp},
};
use moor_var::Var;
use std::{sync::Arc, time::Instant};
use uuid::Uuid;

pub(super) type Chains = ahash::AHashMap<ObjAndUUIDHolder, PropertyValueChain>;
pub(super) struct LoadedWorld {
    pub seed: SeededWorld,
    pub chains: Chains,
    pub progress: Progress,
    pub identity: Uuid,
}

/// Called on the owning writer thread, before its handle is exposed to the engine.
pub(super) fn load(
    connection: &mut PostgresConnection,
    config: &PostgresStorageConfig,
    relations: &Relations,
    identity: Uuid,
    claimed: &Progress,
) -> Result<LoadedWorld, PostgresError> {
    let deadline = Instant::now() + config.query_timeout;
    connection.query(
        "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY",
        &[],
        deadline,
        |_| unreachable!(),
    )?;
    if state::validate_metadata(connection, config, deadline)? != identity {
        return Err(invalid("database_id", "database changed during opening"));
    }
    let progress = state::read_progress(connection, config, deadline)?;
    if &progress != claimed {
        return Err(PostgresError::OwnershipLost);
    }
    let sequences = sequences(connection, config)?;
    let (root, chains) = seed_relations(connection, config, relations, &progress)?;
    connection.query(
        "COMMIT",
        &[],
        Instant::now() + config.query_timeout,
        |_| unreachable!(),
    )?;
    Ok(LoadedWorld {
        seed: SeededWorld { root, sequences },
        chains,
        progress,
        identity,
    })
}

fn sequences(
    connection: &mut PostgresConnection,
    config: &PostgresStorageConfig,
) -> Result<Vec<i64>, PostgresError> {
    let table = config.schema.qualify("sequence_slots")?;
    let mut slots = [None; SEQUENCE_COUNT];
    connection.query(
        &format!("SELECT to_jsonb(t)::text FROM {table} t"),
        &[],
        Instant::now() + config.query_timeout,
        |row| {
            let value = rows::parse_row(row)?;
            let row = value
                .as_object()
                .ok_or_else(|| invalid("sequence_slots", "invalid row"))?;
            let slot: usize = rows::number(row, "slot")?;
            let entry = slots
                .get_mut(slot)
                .ok_or_else(|| invalid("sequence_slots", "unknown sequence slot"))?;
            if entry.is_some() {
                return Err(invalid("sequence_slots", "duplicate slot"));
            }
            *entry = Some(rows::number::<i64>(row, "high_water")?);
            Ok(())
        },
    )?;
    slots
        .into_iter()
        .map(|value| value.ok_or_else(|| invalid("sequence_slots", "missing slot")))
        .collect()
}

fn scan<K: RowKey + RelationDomain, V: RowValue + RelationCodomain>(
    connection: &mut PostgresConnection,
    config: &PostgresStorageConfig,
    relation: &'static str,
    progress: &Progress,
    emit: &mut dyn FnMut(Timestamp, K, V),
) -> Result<(), PostgresError> {
    let table = config.schema.qualify(relation)?;
    let projection = rows::select_row(relation);
    connection.query(
        &format!("SELECT {projection} FROM {table} t"),
        &[],
        Instant::now() + config.query_timeout,
        |row| {
            let (timestamp, key, value) =
                rows::decode(relation, rows::parse_row(row)?, &config.profile)?;
            check_timestamp(timestamp, progress)?;
            emit(timestamp, key, value);
            Ok(())
        },
    )?;
    Ok(())
}

fn check_timestamp(timestamp: Timestamp, progress: &Progress) -> Result<(), PostgresError> {
    if timestamp.0 > progress.max_timestamp {
        return Err(invalid(
            "logical_timestamp",
            "tuple is newer than persistent progress",
        ));
    }
    Ok(())
}

/// Stream one chain at a time. Publication order, not timestamp order, chooses the final value.
fn properties(
    connection: &mut PostgresConnection,
    config: &PostgresStorageConfig,
    progress: &Progress,
    chains: &mut Chains,
    emit: &mut dyn FnMut(Timestamp, ObjAndUUIDHolder, Var),
) -> Result<(), PostgresError> {
    let relation = "object_propvalues";
    let table = config.schema.qualify(relation)?;
    let projection = rows::select_row(relation);
    let mut current = None;
    let finish = |current: &mut Option<(ObjAndUUIDHolder, PropertyValueReconstructor)>,
                  chains: &mut Chains,
                  emit: &mut dyn FnMut(Timestamp, ObjAndUUIDHolder, Var)|
     -> Result<(), PostgresError> {
        let Some((key, reconstructor)) = current.take() else {
            return Ok(());
        };
        let result = reconstructor
            .finish()
            .map_err(|_| invalid("object_propvalues", "invalid property chain"))?;
        emit(result.logical_timestamp, key.clone(), result.value);
        if chains.insert(key, result.chain).is_some() {
            return Err(invalid("object_propvalues", "duplicate property chain"));
        }
        Ok(())
    };
    connection.query(
        &format!(
            "SELECT {projection} FROM {table} t ORDER BY object_ref, property_uuid, record_sequence"
        ),
        &[],
        Instant::now() + config.query_timeout,
        |wire| {
            let value = rows::parse_row(wire)?;
            let row = value
                .as_object()
                .ok_or_else(|| invalid("row", "expected object"))?;
            let key = ObjAndUUIDHolder::decode_key(row, relation)?;
            if current
                .as_ref()
                .is_some_and(|(previous, _)| *previous != key)
            {
                finish(&mut current, chains, emit)?;
            }
            let (_, reconstructor) = current.get_or_insert_with(|| {
                (
                    key,
                    PropertyValueReconstructor::new(PROPERTY_VALUE_CHAIN_LIMITS),
                )
            });
            let sequence: i64 = rows::number(row, "record_sequence")?;
            if sequence <= 0 || sequence > progress.property_sequence {
                return Err(invalid(
                    "record_sequence",
                    "record exceeds persistent property counter",
                ));
            }
            let timestamp = Timestamp(rows::number(row, "logical_timestamp")?);
            check_timestamp(timestamp, progress)?;
            let decoded = Var::decode_value(row, relation, &config.profile)
                .map_err(|error| rows::contextual(relation, row, error))?;
            if rows::text(row, "value_kind")? != codec::value_kind(&decoded) {
                return Err(invalid("value_kind", "kind disagrees with literal"));
            }
            let kind = match rows::text(row, "record_kind")? {
                "full" => PropertyValueRecordKind::Full,
                "list_append" => PropertyValueRecordKind::ListAppend,
                _ => return Err(invalid("record_kind", "unknown property record kind")),
            };
            reconstructor
                .push_decoded(
                    sequence as u64,
                    kind,
                    timestamp,
                    rows::text(row, "value_literal")?.len(),
                    decoded,
                )
                .map_err(|_| invalid("object_propvalues", "invalid property chain"))?;
            Ok(())
        },
    )?;
    finish(&mut current, chains, emit)
}

macro_rules! define_seed {
    ($( $field:ident $category:ident $policy:ident $arrow:tt $domain:ty, $codomain:ty ),* $(,)?) => {
        fn seed_relations(connection: &mut PostgresConnection, config: &PostgresStorageConfig, relations: &Relations, progress: &Progress) -> Result<(WorldStateSnapshot, Chains), PostgresError> {
            let mut chains = Chains::new();
            $(let ($field, _) = relations.$field.seeded_index_with(|emit| define_seed!(@scan $category, connection, config, stringify!($field), progress, &mut chains, emit))?;)*
            Ok((WorldStateSnapshot {
                version: 0,
                committed_ts: Timestamp(progress.max_timestamp),
                caches: Arc::new(Caches::new()),
                $($field: Arc::from($field),)*
                commit_bloom: None,
                bloom_since_version: 0,
            }, chains))
        }
    };
    (@scan Ordinary, $connection:expr, $config:expr, $relation:expr, $progress:expr, $chains:expr, $emit:expr) => { scan($connection, $config, $relation, $progress, $emit) };
    (@scan PropertyValueChain, $connection:expr, $config:expr, $relation:expr, $progress:expr, $chains:expr, $emit:expr) => { properties($connection, $config, $progress, $chains, $emit) };
}
crate::relation_registry::relation_registry!(define_seed);
