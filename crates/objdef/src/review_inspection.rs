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

//! Read-only differences outside the existing-program writer's scope.

use super::{MAX_ROWS, MAX_SOURCE_BYTES, ReviewError, program_text, record};
use crate::{ObjDefSet, ProposedObjectGraph, collect_object_definitions, fingerprint::digest};
use moor_common::model::{ObjFlag, PropFlag, PropPerms, TaskPermissions, WorldState};
use moor_compiler::{ObjectDefinition, to_literal};
use moor_var::{
    Associative, ByteSized, Obj, Symbol, Var, v_bool, v_int, v_list, v_map, v_obj, v_str,
};
use std::collections::{BTreeMap, BTreeSet};

fn metadata(values: &[(Symbol, Var)]) -> Var {
    v_map(
        &values
            .iter()
            .filter(|(key, _)| key.as_string() != super::BASE_KEY)
            .map(|(key, value)| (v_str(&key.to_folded_case()), value.clone()))
            .collect::<Vec<_>>(),
    )
}

fn attributes(object: &ObjectDefinition) -> BTreeMap<String, Var> {
    let mut values = BTreeMap::from([
        ("name".into(), v_str(&object.name)),
        ("parent".into(), v_obj(object.parent)),
        ("owner".into(), v_obj(object.owner)),
        ("location".into(), v_obj(object.location)),
    ]);
    for (name, flag) in [
        ("player", ObjFlag::User),
        ("wizard", ObjFlag::Wizard),
        ("programmer", ObjFlag::Programmer),
        ("readable", ObjFlag::Read),
        ("writable", ObjFlag::Write),
        ("fertile", ObjFlag::Fertile),
    ] {
        values.insert(name.into(), v_bool(object.flags.contains(flag)));
    }
    values
}

struct Property {
    value: Var,
    permissions_known: bool,
}

// Setting an inherited value materializes the defining property's permissions, with the
// receiving object's owner for +c. Intermediate overrides do not supply these permissions.
fn inherited_permissions(
    object: &ObjectDefinition,
    name: Symbol,
    graph: &ProposedObjectGraph,
) -> Option<PropPerms> {
    let mut parent = object.parent;
    let mut visited = BTreeSet::from([object.oid]);
    while visited.insert(parent) {
        let (_, ancestor) = graph.object_definitions().get(&parent)?;
        if let Some(property) = ancestor
            .property_definitions
            .iter()
            .find(|p| p.name == name)
        {
            return Some(if property.perms.flags().contains(PropFlag::Chown) {
                property.perms.clone().with_owner(object.owner)
            } else {
                property.perms.clone()
            });
        }
        parent = ancestor.parent;
    }
    None
}

pub(super) fn property_value(property: &moor_compiler::ObjPropDef) -> Var {
    record(&[
        ("kind", v_str("definition")),
        ("owner", v_obj(property.perms.owner())),
        (
            "flags",
            v_str(&moor_common::model::prop_flags_string(
                property.perms.flags(),
            )),
        ),
        (
            "state",
            v_str(if property.value.is_some() {
                "value"
            } else {
                "clear"
            }),
        ),
        (
            "value",
            property.value.clone().unwrap_or_else(|| v_list(&[])),
        ),
        ("metadata", metadata(&property.metadata)),
    ])
}

fn properties(
    object: &ObjectDefinition,
    graph: Option<&ProposedObjectGraph>,
) -> BTreeMap<String, Property> {
    let mut result = BTreeMap::new();
    for property in &object.property_definitions {
        result.insert(
            property.name.to_folded_case(),
            Property {
                permissions_known: true,
                value: property_value(property),
            },
        );
    }
    for property in &object.property_overrides {
        let mut fields = vec![
            ("kind", v_str("override")),
            (
                "state",
                v_str(if property.value.is_some() {
                    "value"
                } else {
                    "clear"
                }),
            ),
            (
                "value",
                property.value.clone().unwrap_or_else(|| v_list(&[])),
            ),
            ("metadata", metadata(&property.metadata)),
        ];
        let perms = property.perms_update.clone().or_else(|| {
            property.value.as_ref()?;
            inherited_permissions(object, property.name, graph?)
        });
        if let Some(perms) = &perms {
            fields.push(("owner", v_obj(perms.owner())));
            fields.push((
                "flags",
                v_str(&moor_common::model::prop_flags_string(perms.flags())),
            ));
        }
        result.insert(
            property.name.to_folded_case(),
            Property {
                value: record(&fields),
                permissions_known: property.value.is_none() || perms.is_some(),
            },
        );
    }
    result
}

// This metadata records hashes, not copies of the imported object graph.
pub(super) const FIELD_SCHEMA: &str = "objdef-v1:fields:sha256";

pub(crate) fn baseline(
    object: &ObjectDefinition,
    graph: &ProposedObjectGraph,
    source: Option<&Var>,
) -> Var {
    field_baseline(object, Some(graph), source)
}

/// Baselines from an installed object's local rows include resolved override permissions.
pub(super) fn snapshot_baseline(object: &ObjectDefinition, source: Option<&Var>) -> Var {
    field_baseline(object, None, source)
}

fn field_baseline(
    object: &ObjectDefinition,
    graph: Option<&ProposedObjectGraph>,
    source: Option<&Var>,
) -> Var {
    let hashes = |values: BTreeMap<String, Var>| {
        v_map(
            &values
                .into_iter()
                .filter_map(|(name, value)| {
                    digest(&value).ok().map(|hash| (v_str(&name), v_str(&hash)))
                })
                .collect::<Vec<_>>(),
        )
    };
    let mut fields = vec![
        ("schema", v_str(FIELD_SCHEMA)),
        ("attribute", hashes(attributes(object))),
        (
            "property",
            hashes(
                properties(object, graph)
                    .into_iter()
                    .filter(|(_, property)| property.permissions_known)
                    .map(|(name, property)| (name, property.value))
                    .collect(),
            ),
        ),
    ];
    if let Some(source) = source {
        fields.push(("source", source.clone()));
    }
    record(&fields)
}

pub(super) fn baseline_hash(
    object: Option<&ObjectDefinition>,
    field: &str,
    name: &str,
) -> Option<String> {
    let (_, baseline) = object?
        .metadata
        .iter()
        .find(|(key, _)| *key == Symbol::mk(super::BASE_KEY))?;
    let baseline = baseline.as_map()?;
    if baseline.get(&v_str("schema")).ok()?.as_string()? != FIELD_SCHEMA {
        return None;
    }
    let hash = baseline
        .get(&v_str(field))
        .ok()?
        .as_map()?
        .get(&v_str(name))
        .ok()?;
    let hash = hash.as_string()?;
    (hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hash.to_owned())
}

pub(super) fn render(value: &Var, field: &str) -> String {
    if field == "program" {
        return value.as_string().unwrap().to_owned();
    }
    if let Some(map) = value.as_map() {
        return map
            .iter()
            .map(|(key, value)| format!("{}: {}", key.as_string().unwrap(), to_literal(&value)))
            .collect::<Vec<_>>()
            .join("\n");
    }
    to_literal(value)
}

struct Inspection<'a> {
    selected: &'a BTreeSet<String>,
    fields: &'a BTreeSet<String>,
    rows: Vec<Var>,
    revisions: Vec<Var>,
    detail_bytes: usize,
}
impl Inspection<'_> {
    fn add(
        &mut self,
        object: Obj,
        field: &str,
        name: &str,
        local: Option<Var>,
        incoming: Option<Var>,
        base: Option<String>,
    ) -> Result<(), ReviewError> {
        if !self.fields.contains(field) {
            return Ok(());
        }
        let side = if incoming.is_none() {
            "local"
        } else if local.is_none() {
            "incoming"
        } else {
            "both"
        };
        let id = format!("inspect/{object}/{field}/{side}/{name}");
        let mut error = None;
        let mut hash = |value: &Option<Var>| {
            let Some(value) = value else {
                return String::new();
            };
            if value.size_bytes() > MAX_SOURCE_BYTES {
                error = Some("Value exceeds the inspection size limit".to_owned());
                return String::new();
            }
            digest(value).unwrap_or_else(|reason| {
                error = Some(reason);
                String::new()
            })
        };
        let live_hash = hash(&local);
        let incoming_hash = hash(&incoming);
        if local.is_some()
            && incoming.is_some()
            && error.is_none()
            && live_hash == incoming_hash
            && base.as_ref().is_none_or(|base| *base == live_hash)
        {
            return Ok(());
        }
        if self.rows.len() >= MAX_ROWS {
            return Err("inspection exceeds 8192 rows".to_owned().into());
        }
        let classification = if incoming.is_none() {
            "local_only"
        } else if local.is_none() {
            "incoming_only"
        } else if error.is_some() {
            "unbased"
        } else {
            super::classify(base.as_deref(), &live_hash, &incoming_hash)
        };
        let mut row = vec![
            ("id", v_str(&id)),
            ("object", v_obj(object)),
            ("field", v_str(field)),
            ("name", v_str(name)),
            ("classification", v_str(classification)),
            ("read_only", v_bool(true)),
            ("eligible", v_bool(false)),
            ("choices", v_list(&[])),
            ("default", v_str("defer")),
            ("blockers", v_list(&[])),
            ("live", v_str(&live_hash)),
            ("incoming", v_str(&incoming_hash)),
            ("live_present", v_bool(local.is_some())),
            ("incoming_present", v_bool(incoming.is_some())),
        ];
        if let Some(base) = &base {
            row.push(("base", v_str(base)));
        }
        self.revisions.push(v_list(&[
            v_str(&id),
            v_str(&live_hash),
            v_str(&incoming_hash),
            base.as_deref().map(v_str).unwrap_or_else(|| v_list(&[])),
        ]));
        if self.selected.contains(&id) {
            for (key, value) in [("live_text", &local), ("incoming_text", &incoming)] {
                let text = match value {
                    Some(value) if value.size_bytes() <= 262144 => render(value, field),
                    Some(_) => {
                        error = Some("Value is too large to display in this review".to_owned());
                        String::new()
                    }
                    None => String::new(),
                };
                self.detail_bytes += text.len();
                if self.detail_bytes > MAX_SOURCE_BYTES {
                    return Err("selected details exceed 16 MiB".to_owned().into());
                }
                row.push((key, v_str(&text)));
            }
        }
        if let Some(error) = error {
            row.push(("inspection_error", v_str(&error)));
        }
        self.rows.push(record(&row));
        Ok(())
    }
}

pub(super) fn inspect(
    world: &dyn WorldState,
    permissions: &TaskPermissions,
    set: &ObjDefSet,
    scope: &BTreeSet<Obj>,
    selected: &BTreeSet<String>,
    fields: &BTreeSet<String>,
) -> Result<Var, ReviewError> {
    let mut inspection = Inspection {
        selected,
        fields,
        rows: Vec::new(),
        revisions: Vec::new(),
        detail_bytes: 0,
    };
    for object in scope {
        let live = if world.valid(object)? {
            collect_object_definitions(world, permissions, &[*object])
                .map_err(|e| e.to_string())?
                .pop()
        } else {
            None
        };
        let incoming = set
            .graph()
            .object_definitions()
            .get(object)
            .map(|(_, object)| object);
        if live.is_none() && incoming.is_none() {
            continue;
        }
        if let (Some(local), Some(incoming)) = (live.as_ref(), incoming) {
            let live_attrs = attributes(local);
            for (key, value) in attributes(incoming) {
                inspection.add(
                    *object,
                    "attribute",
                    &key,
                    live_attrs.get(&key).cloned(),
                    Some(value),
                    baseline_hash(Some(local), "attribute", &key),
                )?;
            }
        } else {
            let value = |object: &ObjectDefinition| {
                record(
                    &attributes(object)
                        .iter()
                        .map(|(key, value)| (key.as_str(), value.clone()))
                        .collect::<Vec<_>>(),
                )
            };
            inspection.add(
                *object,
                "object",
                "",
                live.as_ref().map(value),
                incoming.map(value),
                None,
            )?;
        }
        let local_props = live
            .as_ref()
            .map(|object| properties(object, None))
            .unwrap_or_default();
        let incoming_props = incoming
            .map(|object| properties(object, Some(set.graph())))
            .unwrap_or_default();
        for name in local_props
            .keys()
            .chain(incoming_props.keys())
            .collect::<BTreeSet<_>>()
        {
            inspection.add(
                *object,
                "property",
                name,
                local_props.get(name).map(|property| property.value.clone()),
                incoming_props
                    .get(name)
                    .map(|property| property.value.clone()),
                // An external ancestor's permissions cannot be reconstructed from this
                // source. Do not mistake missing information for a local/upstream edit.
                incoming_props
                    .get(name)
                    .filter(|property| property.permissions_known)
                    .and_then(|_| baseline_hash(live.as_ref(), "property", name)),
            )?;
        }
        let local_verbs = live
            .as_ref()
            .map(|object| object.verbs.as_slice())
            .unwrap_or_default();
        let incoming_verbs = incoming
            .map(|object| object.verbs.as_slice())
            .unwrap_or_default();
        for (verbs, other, is_local) in [
            (local_verbs, incoming_verbs, true),
            (incoming_verbs, local_verbs, false),
        ] {
            for (index, verb) in verbs.iter().enumerate() {
                if other
                    .iter()
                    .any(|other| other.names == verb.names && other.argspec == verb.argspec)
                {
                    continue;
                }
                let name = verb
                    .names
                    .iter()
                    .map(|name| name.as_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                let text = program_text(&verb.program)?;
                // Index disambiguates overloads and duplicate aliases in the displayed side.
                inspection.add(
                    *object,
                    "program",
                    &format!("{index}:{name}"),
                    is_local.then(|| text.clone()),
                    (!is_local).then_some(text),
                    None,
                )?;
            }
        }
    }
    if selected.iter().any(|id| {
        !inspection
            .rows
            .iter()
            .any(|row| row.as_map().unwrap().get(&v_str("id")).unwrap().as_string() == Some(id))
    }) {
        return Err(
            "inspection item changed or no longer differs; reload the review"
                .to_owned()
                .into(),
        );
    }
    let mut counts = BTreeMap::<String, i64>::new();
    for row in &inspection.rows {
        let classification = row.as_map().unwrap().get(&v_str("classification")).unwrap();
        *counts
            .entry(classification.as_string().unwrap().to_owned())
            .or_default() += 1;
    }
    Ok(record(&[
        ("schema", v_int(1)),
        ("operation", v_str("inspect")),
        ("revision", v_str(&digest(&v_list(&inspection.revisions))?)),
        ("rows", v_list(&inspection.rows)),
        (
            "counts",
            record(
                &counts
                    .iter()
                    .map(|(key, value)| (key.as_str(), v_int(*value)))
                    .collect::<Vec<_>>(),
            ),
        ),
    ]))
}
