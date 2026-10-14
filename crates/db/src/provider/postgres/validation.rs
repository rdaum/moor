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

//! Read-only validation of one PostgreSQL snapshot. No writer lock or epoch mutation.
use super::{
    PostgresConnection, PostgresError, PostgresShutdown, PostgresStorageConfig,
    codec::invalid,
    rows::{self, RowKey, RowValue},
    seed, state,
};
use crate::{
    AnonymousObjectMetadata, EntityMetadataKey, ObjAndUUIDHolder, SEQUENCE_MAX_OBJECT,
    StringHolder, provider::read::ReadKey,
};
use moor_common::{
    model::{HasUuid, ObjFlag, PropDefs, PropPerms, ValSet, VerbDefs},
    util::BitEnum,
};
use moor_var::{Obj, Var, program::ProgramType};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::Instant,
};
use uuid::Uuid;

/// Counts describe physical rows, including every full and append property record.
#[derive(Debug, Serialize)]
pub struct PostgresValidationReport {
    pub database_id: Uuid,
    pub writer_epoch: u64,
    pub applied_version: u64,
    pub commit_sequence: u64,
    pub max_timestamp: u64,
    pub property_record_sequence: i64,
    pub durable_fence: u64,
    pub sequence_slots: Vec<i64>,
    pub relation_rows: BTreeMap<&'static str, u64>,
    pub property_values: usize,
    /// Logical value, permission, and metadata entries hidden by deleted definitions or ancestry.
    pub inactive_property_entries: usize,
}

#[derive(Default)]
struct Index {
    objects: HashSet<Obj>,
    names: HashSet<Obj>,
    owners: HashSet<Obj>,
    parents: HashMap<Obj, Obj>,
    locations: HashMap<Obj, Obj>,
    properties: HashMap<Uuid, Obj>,
    verbs: HashSet<ObjAndUUIDHolder>,
    programs: HashSet<ObjAndUUIDHolder>,
    permissions: HashSet<ObjAndUUIDHolder>,
    // Retain keys and graph/definition indexes, never programs or decoded property values.
    keys: Vec<(&'static str, Obj, Option<Uuid>)>,
    metadata: Vec<EntityMetadataKey>,
}
fn at(
    relation: &'static str,
    object: Obj,
    uuid: Option<Uuid>,
    reason: &'static str,
) -> PostgresError {
    PostgresError::Row {
        relation,
        key: format!(
            "object_ref={}{}",
            object.to_literal(),
            uuid.map(|u| format!(", uuid={u}")).unwrap_or_default()
        ),
        source: Box::new(invalid("invariant", reason)),
    }
}
fn scan<K: RowKey + ReadKey, V: RowValue>(
    connection: &mut PostgresConnection,
    config: &PostgresStorageConfig,
    progress: &state::Progress,
    relation: &'static str,
    mut visit: impl FnMut(K, V) -> Result<(), PostgresError>,
) -> Result<u64, PostgresError> {
    let table = config.schema.qualify(relation)?;
    let mut count = 0;
    connection
        .query(
            &format!("SELECT {} FROM {table} t", rows::select_row(relation)),
            &[],
            Instant::now() + config.query_timeout,
            |wire| {
                let row = rows::parse_row(wire)?;
                let (ts, key, value) = rows::decode::<K, V>(relation, row, &config.profile)?;
                seed::check_timestamp(ts, progress).map_err(|error| {
                    rows::contextual(relation, &key.encode_key(relation), error)
                })?;
                visit(key, value)?;
                count += 1;
                Ok(())
            },
        )
        .map_err(|source| PostgresError::Operation {
            relation,
            operation: "validation scan",
            source: Box::new(source),
        })?;
    Ok(count)
}

macro_rules! validate_relations {
    ($( $field:ident $category:ident $policy:ident $arrow:tt $domain:ty, $codomain:ty ),* $(,)?) => {
        fn scan_relations(connection: &mut PostgresConnection, config: &PostgresStorageConfig,
            progress: &state::Progress, index: &mut Index, counts: &mut BTreeMap<&'static str, u64>) -> Result<usize, PostgresError> {
            let property_values;
            $( validate_relations!(@scan $category, $field, $domain, $codomain, connection, config, progress, index, counts, property_values); )*
            Ok(property_values)
        }
    };
    (@scan Ordinary, $field:ident, $domain:ty, $codomain:ty, $c:ident, $cfg:ident, $p:ident, $i:ident, $counts:ident, $values:ident) => {
        #[allow(unused_variables)]
        let count = scan::<$domain,$codomain>($c, $cfg, $p, stringify!($field), |key, value| {
            $i.keys.push((stringify!($field), key.object(), None));
            validate_relations!(@record $field, $i, key, value);
            Ok(())
        })?;
        $counts.insert(stringify!($field), count);
    };
    (@scan PropertyValueChain, $field:ident, $domain:ty, $codomain:ty, $c:ident, $cfg:ident, $p:ident, $i:ident, $counts:ident, $values:ident) => {
        let mut chains = seed::Chains::new();
        seed::properties($c, $cfg, $p, &mut chains, &mut |_, key, _| {
            $i.keys.push(("object_propvalues", key.obj(), Some(key.uuid())));
        })?;
        $values = chains.len();
        let table = $cfg.schema.qualify("object_propvalues")?;
        let row = state::singleton($c, &format!("SELECT jsonb_build_object('count',count(*)::text)::text FROM {table}"), &[], Instant::now() + $cfg.query_timeout)?;
        $counts.insert("object_propvalues", rows::number(&row, "count")?);
    };
    (@record object_flags, $i:ident, $k:ident, $v:ident) => { $i.objects.insert($k); };
    (@record object_name, $i:ident, $k:ident, $v:ident) => { $i.names.insert($k); };
    (@record object_owner, $i:ident, $k:ident, $v:ident) => { $i.owners.insert($k); };
    (@record object_parent, $i:ident, $k:ident, $v:ident) => { $i.parents.insert($k, $v); };
    (@record object_location, $i:ident, $k:ident, $v:ident) => { $i.locations.insert($k, $v); };
    (@record object_propdefs, $i:ident, $k:ident, $v:ident) => {
        let mut names = HashSet::new();
        for definition in $v.iter() {
            if definition.location() != $k || !names.insert(definition.name()) || $i.properties.insert(definition.uuid(), $k).is_some() {
                return Err(at("object_propdefs", $k, Some(definition.uuid()), "duplicate property identity/name or incorrect definition location"));
            }
        }
    };
    (@record object_verbdefs, $i:ident, $k:ident, $v:ident) => {
        for definition in $v.iter() {
            if definition.location() != $k { return Err(at("object_verbdefs", $k, Some(definition.uuid()), "incorrect verb location")); }
            $i.verbs.insert(ObjAndUUIDHolder::new(&$k, definition.uuid()));
        }
    };
    (@record object_verbs, $i:ident, $k:ident, $v:ident) => { $i.programs.insert($k); };
    (@record object_propflags, $i:ident, $k:ident, $v:ident) => {
        $i.keys.push(("object_propflags", $k.obj(), Some($k.uuid())));
        $i.permissions.insert($k);
    };
    (@record entity_metadata, $i:ident, $k:ident, $v:ident) => { $i.metadata.push($k); };
    (@record anonymous_object_metadata, $i:ident, $k:ident, $v:ident) => {
        if !$k.is_anonymous() { return Err(at("anonymous_object_metadata", $k, None, "metadata belongs to a non-anonymous object")); }
    };
    (@record object_last_move, $i:ident, $k:ident, $v:ident) => {};
}
crate::relation_registry::relation_registry!(validate_relations);

impl Index {
    fn graph(
        &self,
        relation: &'static str,
        edges: &HashMap<Obj, Obj>,
    ) -> Result<(), PostgresError> {
        let mut complete = HashSet::new();
        for start in edges.keys() {
            let mut path = HashSet::new();
            let mut object = *start;
            while !object.is_nothing() && !complete.contains(&object) {
                if !self.objects.contains(&object) {
                    return Err(at(
                        relation,
                        *start,
                        None,
                        "relationship points to an absent object",
                    ));
                }
                if !path.insert(object) {
                    return Err(at(relation, object, None, "cycle in object relationships"));
                }
                let Some(next) = edges.get(&object) else {
                    break;
                };
                object = *next;
            }
            complete.extend(path);
        }
        Ok(())
    }
    fn property_visible(&self, object: Obj, uuid: Uuid) -> bool {
        let Some(location) = self.properties.get(&uuid) else {
            return false;
        };
        let mut current = object;
        loop {
            if current == *location {
                return true;
            }
            let Some(parent) = self.parents.get(&current) else {
                break;
            };
            if parent.is_nothing() {
                break;
            }
            current = *parent;
        }
        false
    }
    fn check(&self, sequences: &[i64]) -> Result<usize, PostgresError> {
        let mut inactive = 0;
        self.graph("object_parent", &self.parents)?;
        self.graph("object_location", &self.locations)?;
        for (relation, object, uuid) in &self.keys {
            if !self.objects.contains(object) {
                return Err(at(
                    relation,
                    *object,
                    *uuid,
                    "row belongs to an absent object",
                ));
            }
            if let Some(uuid) = uuid {
                inactive += usize::from(!self.property_visible(*object, *uuid));
            }
        }
        for object in &self.objects {
            if !object.is_positive() {
                return Err(at(
                    "object_flags",
                    *object,
                    None,
                    "negative object identity",
                ));
            }
            for (relation, keys) in [("object_name", &self.names), ("object_owner", &self.owners)] {
                if !keys.contains(object) {
                    return Err(at(relation, *object, None, "required object row is absent"));
                }
            }
            if object.is_oid() && i64::from(object.id().0) > sequences[SEQUENCE_MAX_OBJECT] {
                return Err(at(
                    "sequence_slots",
                    *object,
                    None,
                    "object exceeds allocation high-water mark",
                ));
            }
        }
        for (uuid, location) in &self.properties {
            if !self
                .permissions
                .contains(&ObjAndUUIDHolder::new(location, *uuid))
            {
                return Err(at(
                    "object_propflags",
                    *location,
                    Some(*uuid),
                    "canonical property permissions are absent",
                ));
            }
        }
        if let Some(key) = self.verbs.symmetric_difference(&self.programs).next() {
            return Err(at(
                "object_verbs",
                key.obj(),
                Some(key.uuid()),
                "verb definition and source disagree",
            ));
        }
        for key in &self.metadata {
            if key.is_property() {
                inactive += usize::from(!self.property_visible(key.obj(), key.uuid().unwrap()));
            }
            if key.is_verb()
                && !self
                    .verbs
                    .contains(&ObjAndUUIDHolder::new(&key.obj(), key.uuid().unwrap()))
            {
                return Err(at(
                    "entity_metadata",
                    key.obj(),
                    key.uuid(),
                    "verb definition is absent",
                ));
            }
        }
        Ok(inactive)
    }
}

/// Validate a stable applied SQL prefix using SELECT only. This may run alongside a writer.
/// Connections are created and dropped on the calling administrative thread.
/// Errors stop at the first invalid row and contain redacted relation/key context.
pub fn validate_postgres_storage(
    config: &PostgresStorageConfig,
) -> Result<PostgresValidationReport, PostgresError> {
    config.validate()?;
    let mut options = config.connection.clone();
    options.application_name = "moor-validate".into();
    let mut connection = PostgresConnection::connect(
        &options,
        Instant::now() + config.connect_timeout,
        PostgresShutdown::default(),
    )?;
    let deadline = Instant::now() + config.query_timeout;
    connection.query(
        "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY",
        &[],
        deadline,
        |_| unreachable!(),
    )?;
    let database_id = state::validate_metadata(&mut connection, config, deadline)?;
    let progress = state::read_progress(&mut connection, config, deadline)?;
    let sequence_slots = seed::sequences(&mut connection, config)?;
    if sequence_slots[SEQUENCE_MAX_OBJECT] < -1
        || sequence_slots[SEQUENCE_MAX_OBJECT] > i64::from(i32::MAX)
    {
        return Err(invalid(
            "sequence_slots",
            "object allocation counter is out of range",
        ));
    }
    let mut index = Index::default();
    let mut relation_rows = BTreeMap::from([
        ("world_metadata", 1),
        ("writer_progress", 1),
        ("sequence_slots", sequence_slots.len() as u64),
    ]);
    let property_values = scan_relations(
        &mut connection,
        config,
        &progress,
        &mut index,
        &mut relation_rows,
    )?;
    let inactive_property_entries = index.check(&sequence_slots)?;
    connection.query(
        "COMMIT",
        &[],
        Instant::now() + config.query_timeout,
        |_| unreachable!(),
    )?;
    Ok(PostgresValidationReport {
        database_id,
        writer_epoch: progress.epoch,
        applied_version: progress.applied,
        commit_sequence: progress.commits,
        max_timestamp: progress.max_timestamp,
        property_record_sequence: progress.property_sequence,
        durable_fence: progress.durable_fence,
        sequence_slots,
        relation_rows,
        property_values,
        inactive_property_entries,
    })
}
