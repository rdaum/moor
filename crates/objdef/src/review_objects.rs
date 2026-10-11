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

//! Explicit object bindings and transactional creation. Source cannot choose vacant IDs.

use super::{BASE_KEY, ChangeRow, ChangeTarget, ReviewError, digest, inspection, record};
use crate::{ObjDefSet, ObjectDumpOptions};
use moor_common::model::{
    HasUuid, ObjFlag, ObjectKind, PropAttrs, TaskPermissions, ValSet, VerbFlag, WorldState,
    WorldStateError,
};
use moor_common::util::BitEnum;
use moor_compiler::ObjectDefinition;
use moor_var::{Associative, NOTHING, Obj, Symbol, Var, v_bool, v_int, v_list, v_obj, v_str};
use std::collections::{BTreeSet, HashMap};

const CREATION_SCHEMA: &str = "objdef-v1:creation-text:sha256";

pub(super) fn member_key(object: Obj) -> String {
    format!("{object}/object")
}

fn text(definition: &ObjectDefinition) -> Result<String, ReviewError> {
    let mut bytes = Vec::new();
    crate::write::write_dump_object(
        &HashMap::new(),
        definition,
        ObjectDumpOptions {
            include_baselines: false,
        },
        &mut bytes,
    )
    .map_err(|error| error.to_string())?;
    String::from_utf8(bytes).map_err(|error| error.to_string().into())
}

pub(super) struct Context<'a> {
    pub world: &'a dyn WorldState,
    pub permissions: &'a TaskPermissions,
    pub set: &'a ObjDefSet,
    pub policy: &'a super::policy::Policy,
    pub trusted_owners: &'a BTreeSet<Obj>,
    pub provenance: Option<&'a Var>,
    pub source_digest: &'a str,
}

impl Context<'_> {
    fn proposed(&self, object: Obj) -> Option<&ObjectDefinition> {
        self.policy
            .creations
            .get(&object)
            .filter(|receipt| receipt.is_none())?;
        self.set
            .graph()
            .object_definitions()
            .get(&object)
            .map(|(_, def)| def)
    }

    pub fn analyze(
        &self,
        label: &str,
        definition: &ObjectDefinition,
        guards: &mut Vec<Var>,
        details: bool,
    ) -> Result<(ChangeRow, Var), ReviewError> {
        let object = definition.oid;
        let member = member_key(object);
        let id = format!("object/{member}");
        let installed = self.policy.creations[&object].as_ref();
        let valid = self.world.valid(&object)?;
        let incoming_text = text(definition)?;
        let incoming_hash = digest(&v_str(&incoming_text))?;
        let mut blockers = Vec::new();
        let mut live_text = String::new();
        let mut live_hash = String::new();
        guards.push(v_list(&[v_obj(object), v_bool(valid)]));
        if valid {
            let local = crate::collect_object_definitions(self.world, self.permissions, &[object])
                .map_err(|error| error.to_string())?
                .remove(0);
            let owner_flags = self.world.flags_of(&local.owner)?;
            if let Some((_, incoming_identity)) = definition
                .metadata
                .iter()
                .find(|(key, _)| *key == Symbol::mk("import_export_id"))
            {
                let live_identity = local
                    .metadata
                    .iter()
                    .find(|(key, _)| *key == Symbol::mk("import_export_id"))
                    .map(|(_, value)| value);
                if live_identity.map(digest).transpose()? != Some(digest(incoming_identity)?) {
                    blockers.push(v_str("object_identity_mismatch"));
                }
            }
            if local.flags.contains(ObjFlag::Write)
                || !(owner_flags.contains(ObjFlag::Wizard)
                    || self.trusted_owners.contains(&local.owner))
            {
                blockers.push(v_str("untrusted_target_authority"));
            }
            guards.push(v_list(&[
                v_obj(local.owner),
                v_int(owner_flags.to_u16().into()),
            ]));
            live_text = text(&local)?;
            live_hash = digest(&v_str(&live_text))?;
            let baseline =
                self.world
                    .get_object_metadata(self.permissions, &object, Symbol::mk(BASE_KEY))?;
            let accepted = baseline
                .as_ref()
                .and_then(Var::as_map)
                .filter(|map| {
                    map.get(&v_str("creation_schema"))
                        .ok()
                        .and_then(|value| value.as_string().map(str::to_owned))
                        .as_deref()
                        == Some(CREATION_SCHEMA)
                })
                .and_then(|map| map.get(&v_str("creation")).ok())
                .and_then(|hash| hash.as_string().map(str::to_owned));
            if installed.is_none() {
                blockers.push(v_str("object_address_occupied"));
            } else if installed != accepted.as_ref() {
                blockers.push(v_str("object_binding_changed"));
            }
            guards.push(v_list(&[
                v_str(&live_hash),
                baseline.unwrap_or_else(|| v_list(&[])),
            ]));
        } else if installed.is_some() {
            blockers.push(v_str("object_binding_changed"));
        }
        if installed.is_none() {
            for (index, verb) in definition.verbs.iter().enumerate() {
                if definition.verbs[..index].iter().any(|other| {
                    other.names.iter().any(|left| {
                        verb.names
                            .iter()
                            .any(|right| super::additions::overlap(*left, *right))
                    })
                }) {
                    blockers.push(v_str("verb_name_collision"));
                }
            }
            if !self
                .world
                .flags_of(&self.permissions.principal())?
                .contains(ObjFlag::Wizard)
            {
                blockers.push(v_str("creation_requires_wizard"));
            }
            if definition.flags.contains(ObjFlag::Write)
                || definition
                    .verbs
                    .iter()
                    .any(|verb| verb.flags.contains(VerbFlag::Write))
            {
                blockers.push(v_str("untrusted_target_authority"));
            }
            let mut owners = BTreeSet::from([definition.owner]);
            owners.extend(definition.verbs.iter().map(|verb| verb.owner));
            owners.extend(
                definition
                    .property_definitions
                    .iter()
                    .map(|prop| prop.perms.owner()),
            );
            owners.extend(
                definition
                    .property_overrides
                    .iter()
                    .filter_map(|prop| prop.perms_update.as_ref().map(|perms| perms.owner())),
            );
            for owner in owners {
                let flags = if self.world.valid(&owner)? {
                    Some(self.world.flags_of(&owner)?)
                } else {
                    self.proposed(owner).map(|source| source.flags)
                };
                if !flags.is_some_and(|flags| {
                    flags.contains(ObjFlag::Wizard) || self.trusted_owners.contains(&owner)
                }) {
                    blockers.push(v_str("untrusted_target_authority"));
                }
                guards.push(v_list(&[
                    v_obj(owner),
                    flags
                        .map(|flags| v_int(flags.to_u16().into()))
                        .unwrap_or_else(|| v_list(&[])),
                ]));
            }
            let mut inherited = BTreeSet::new();
            let mut parent = definition.parent;
            let mut visited = BTreeSet::from([object]);
            while parent != NOTHING {
                if !visited.insert(parent) {
                    blockers.push(v_str("invalid_object_parent"));
                    break;
                }
                if let Some(proposed) = self.proposed(parent) {
                    inherited.extend(proposed.property_definitions.iter().map(|prop| prop.name));
                    parent = proposed.parent;
                } else if self.world.valid(&parent)? {
                    let properties = self.world.properties(self.permissions, &parent)?;
                    let signatures: Vec<_> = properties
                        .iter()
                        .map(|property| {
                            inherited.insert(property.name());
                            v_list(&[
                                v_str(&property.uuid().to_string()),
                                v_str(&property.name().to_folded_case()),
                            ])
                        })
                        .collect();
                    let next = self.world.parent_of(self.permissions, &parent)?;
                    guards.push(v_list(&[v_obj(parent), v_obj(next), v_list(&signatures)]));
                    parent = next;
                } else {
                    blockers.push(v_str("invalid_object_parent"));
                    break;
                }
            }
            for property in &definition.property_definitions {
                if inherited.contains(&property.name) {
                    blockers.push(v_str("property_name_collision"));
                }
                if property.value.is_none() {
                    blockers.push(v_str("initial_value_required"));
                }
            }
            for property in &definition.property_overrides {
                if !inherited.contains(&property.name) {
                    blockers.push(v_str("missing_inherited_property"));
                }
            }
            let mut location = definition.location;
            let mut visited = BTreeSet::from([object]);
            while location != NOTHING {
                if !visited.insert(location) {
                    blockers.push(v_str("invalid_object_location"));
                    break;
                }
                if let Some(proposed) = self.proposed(location) {
                    location = proposed.location;
                } else if self.world.valid(&location)? {
                    let next = self.world.location_of(self.permissions, &location)?;
                    guards.push(v_list(&[v_obj(location), v_obj(next)]));
                    location = next;
                } else {
                    blockers.push(v_str("invalid_object_location"));
                    break;
                }
            }
        }
        let eligible = blockers.is_empty();
        let retain = installed.is_some();
        let mut report = vec![
            ("id", v_str(&id)),
            ("object", v_obj(object)),
            ("field", v_str("object")),
            ("names", v_list(&[v_str("")])),
            (
                "operation",
                v_str(if retain {
                    "retain_object"
                } else {
                    "add_object"
                }),
            ),
            (
                "classification",
                v_str(if !valid {
                    "incoming_only"
                } else {
                    super::classify(installed.map(String::as_str), &live_hash, &incoming_hash)
                }),
            ),
            ("source", v_str(label)),
            (
                "base",
                installed
                    .map(|hash| v_str(hash))
                    .unwrap_or_else(|| v_list(&[])),
            ),
            ("live", v_str(&live_hash)),
            ("incoming", v_str(&incoming_hash)),
            ("eligible", v_bool(eligible)),
            ("blockers", v_list(&blockers)),
            ("default", v_str("defer")),
            ("live_present", v_bool(valid)),
            ("incoming_present", v_bool(true)),
            (
                "choices",
                v_list(&if eligible {
                    vec![
                        v_str(if retain { "local" } else { "incoming" }),
                        v_str("defer"),
                    ]
                } else {
                    vec![v_str("defer")]
                }),
            ),
        ];
        if details {
            report.extend([
                ("live_text", v_str(&live_text)),
                ("incoming_text", v_str(&incoming_text)),
                ("base_text_available", v_bool(false)),
            ]);
        }
        let incoming_source = self.provenance.map(|source| {
            source
                .as_map()
                .unwrap()
                .set(&v_str("label"), &v_str(label))
                .unwrap()
                .as_map()
                .unwrap()
                .set(&v_str("digest"), &v_str(self.source_digest))
                .unwrap()
        });
        Ok((
            ChangeRow {
                id,
                member,
                object,
                eligible,
                target: if retain {
                    ChangeTarget::RetainObject {
                        accepted: installed.unwrap().clone(),
                    }
                } else {
                    ChangeTarget::AddObject(Box::new(definition.clone()))
                },
                incoming: None,
                incoming_hash,
                incoming_source,
                live_hash,
                default: "defer",
            },
            record(&report),
        ))
    }
}

/// Allocate all selected addresses first, then connect them. Nothing invokes MOO verbs.
pub(super) fn allocate(
    world: &mut dyn WorldState,
    permissions: &TaskPermissions,
    objects: &[&ObjectDefinition],
) -> Result<(), WorldStateError> {
    for object in objects {
        if world.valid(&object.oid)? {
            return Err(WorldStateError::DatabaseError(
                "object address is occupied".into(),
            ));
        }
        world.create_object(
            permissions,
            &NOTHING,
            &NOTHING,
            BitEnum::new(),
            ObjectKind::Objid(object.oid),
        )?;
    }
    for object in objects {
        world.update_property(
            permissions,
            &object.oid,
            Symbol::mk("owner"),
            &v_obj(object.owner),
        )?;
        world.update_property(
            permissions,
            &object.oid,
            Symbol::mk("name"),
            &v_str(&object.name),
        )?;
        world.set_flags_of(permissions, &object.oid, object.flags)?;
        world.change_parent(permissions, &object.oid, &object.parent)?;
        world.move_object(permissions, &object.oid, &object.location)?;
        for (key, value) in &object.metadata {
            if *key != Symbol::mk(BASE_KEY) {
                world.set_object_metadata(permissions, &object.oid, *key, value.clone())?;
            }
        }
    }
    Ok(())
}

pub(super) fn initialize(
    world: &mut dyn WorldState,
    permissions: &TaskPermissions,
    objects: &[(Box<ObjectDefinition>, Var)],
) -> Result<(), WorldStateError> {
    // Define all properties before overrides so source order and numeric IDs do not
    // determine whether a new child can refer to a new parent's property.
    for (object, _) in objects {
        for property in &object.property_definitions {
            world.define_property(
                permissions,
                &object.oid,
                &object.oid,
                property.name,
                &property.perms.owner(),
                property.perms.flags(),
                property.value.clone(),
            )?;
            for (key, value) in &property.metadata {
                if *key != Symbol::mk(BASE_KEY) {
                    world.set_property_metadata(
                        permissions,
                        &object.oid,
                        property.name,
                        *key,
                        value.clone(),
                    )?;
                }
            }
        }
    }
    for (object, _) in objects {
        for property in &object.property_overrides {
            if let Some(value) = &property.value {
                world.update_property(permissions, &object.oid, property.name, value)?;
            } else if property.clear_value {
                world.clear_property(permissions, &object.oid, property.name)?;
            }
            if let Some(perms) = &property.perms_update {
                world.set_property_info(
                    permissions,
                    &object.oid,
                    property.name,
                    PropAttrs {
                        owner: Some(perms.owner()),
                        flags: Some(perms.flags()),
                        ..PropAttrs::new()
                    },
                )?;
            }
            for (key, value) in &property.metadata {
                if *key != Symbol::mk(BASE_KEY) {
                    world.set_property_metadata(
                        permissions,
                        &object.oid,
                        property.name,
                        *key,
                        value.clone(),
                    )?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn finish(
    world: &mut dyn WorldState,
    permissions: &TaskPermissions,
    objects: &[(Box<ObjectDefinition>, Var)],
) -> Result<(), WorldStateError> {
    for (object, receipt) in objects {
        let local = crate::collect_object_definitions(world, permissions, &[object.oid])
            .map_err(|error| WorldStateError::DatabaseError(error.to_string()))?
            .remove(0);
        let source = receipt.as_map().unwrap().get(&v_str("source")).ok();
        let baseline = inspection::snapshot_baseline(&local, source.as_ref());
        let creation = receipt.as_map().unwrap().get(&v_str("program")).unwrap();
        let baseline = baseline
            .as_map()
            .unwrap()
            .set(&v_str("creation"), &creation)
            .map_err(|error| WorldStateError::DatabaseError(error.to_string()))?;
        let baseline = baseline
            .as_map()
            .unwrap()
            .set(&v_str("creation_schema"), &v_str(CREATION_SCHEMA))
            .map_err(|error| WorldStateError::DatabaseError(error.to_string()))?;
        world.set_object_metadata(permissions, &object.oid, Symbol::mk(BASE_KEY), baseline)?;
    }
    Ok(())
}
