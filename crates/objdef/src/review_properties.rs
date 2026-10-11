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

//! Absent-only property initialization and read-only retention of installed properties.

use super::{BASE_KEY, ChangeRow, ChangeTarget, ReviewError, classify, digest, inspection, record};
use moor_common::model::{HasUuid, ObjFlag, TaskPermissions, ValSet, WorldState, WorldStateError};
use moor_compiler::{ObjPropDef, ObjectDefinition};
use moor_var::{Associative, Obj, Symbol, Var, v_bool, v_int, v_list, v_map, v_obj, v_str};
use std::collections::BTreeSet;

pub(super) fn member_key(object: Obj, name: Symbol) -> String {
    format!("{object}/property/{}", name.to_folded_case())
}

pub(super) struct Context<'a> {
    pub world: &'a dyn WorldState,
    pub permissions: &'a TaskPermissions,
    pub object: Obj,
    pub trusted: bool,
    pub identity_matches: bool,
    pub trusted_owners: &'a BTreeSet<Obj>,
    pub policy: &'a super::policy::Policy,
    pub local: &'a ObjectDefinition,
}

impl Context<'_> {
    pub fn analyze(
        &self,
        label: &str,
        property: &ObjPropDef,
        guards: &mut Vec<Var>,
        details: bool,
    ) -> Result<(ChangeRow, Var), ReviewError> {
        let member = member_key(self.object, property.name);
        let id = format!("property/{member}");
        let mut blockers = Vec::new();
        if !self.trusted {
            blockers.push(v_str("untrusted_target_authority"));
        }
        if !self.identity_matches {
            blockers.push(v_str("object_identity_mismatch"));
        }
        let local = self
            .local
            .property_definitions
            .iter()
            .find(|p| p.name == property.name);
        let base = inspection::baseline_hash(
            Some(self.local),
            "property",
            &property.name.to_folded_case(),
        );
        let existing =
            match self
                .world
                .get_property_info(self.permissions, &self.object, property.name)
            {
                Ok((definition, permissions)) => Some((definition, permissions)),
                Err(WorldStateError::PropertyNotFound(_, _)) => None,
                Err(error) => return Err(error.into()),
            };
        let retain = local.is_some() && base.is_some();
        if existing.is_some() && !retain {
            blockers.push(v_str("property_name_collision"));
        }
        let mut descendants_count = 0;
        if existing.is_none() {
            let owner = property.perms.owner();
            let owner_flags = if self.world.valid(&owner)? {
                Some(self.world.flags_of(&owner)?)
            } else {
                None
            };
            if !owner_flags.is_some_and(|flags| {
                flags.contains(ObjFlag::Wizard) || self.trusted_owners.contains(&owner)
            }) {
                blockers.push(v_str("untrusted_target_authority"));
            }
            guards.push(v_list(&[
                v_obj(owner),
                owner_flags
                    .map(|flags| v_int(flags.to_u16().into()))
                    .unwrap_or_else(|| v_list(&[])),
            ]));
            if property.value.is_none() {
                blockers.push(v_str("initial_value_required"));
            }
            let descendants = self
                .world
                .descendants_of(self.permissions, &self.object, false)?;
            descendants_count = descendants.len();
            if descendants_count > super::MAX_ROWS {
                return Err("property addition affects more than 8192 descendants"
                    .to_owned()
                    .into());
            }
            match self.policy.initialization.get(&member) {
                None => blockers.push(v_str("initialization_required")),
                Some(false) if descendants_count > 0 => {
                    blockers.push(v_str("inherited_effects_require_approval"))
                }
                _ => {}
            }
            let mut inheritance = Vec::new();
            for object in descendants.iter().chain(
                self.world
                    .ancestors_of(self.permissions, &self.object, true)?
                    .iter(),
            ) {
                let definitions = self.world.properties(self.permissions, &object)?;
                let mut signatures = Vec::new();
                for definition in definitions.iter() {
                    if definition.name() == property.name {
                        blockers.push(v_str("property_name_collision"));
                    }
                    signatures.push(v_list(&[
                        v_str(&definition.uuid().to_string()),
                        v_str(&definition.name().to_folded_case()),
                    ]));
                }
                inheritance.push(v_list(&[
                    v_obj(object),
                    v_obj(self.world.parent_of(self.permissions, &object)?),
                    v_list(&signatures),
                ]));
            }
            inheritance.sort();
            guards.push(v_list(&inheritance));
        }
        let incoming_value = inspection::property_value(property);
        let incoming_hash = digest(&incoming_value)?;
        let live_value = local.map(inspection::property_value);
        let live_hash = live_value
            .as_ref()
            .map(digest)
            .transpose()?
            .unwrap_or_default();
        guards.push(v_list(&[
            v_str(&id),
            v_str(&live_hash),
            base.as_ref()
                .map(|hash| v_str(hash))
                .unwrap_or_else(|| v_list(&[])),
            existing
                .as_ref()
                .map(|(definition, perms)| {
                    v_list(&[
                        v_str(&definition.uuid().to_string()),
                        v_obj(definition.definer()),
                        v_obj(perms.owner()),
                        v_int(perms.flags().to_u16().into()),
                    ])
                })
                .unwrap_or_else(|| v_list(&[])),
        ]));
        let eligible = blockers.is_empty();
        let classification = if existing.is_none() {
            "incoming_only"
        } else {
            classify(base.as_deref(), &live_hash, &incoming_hash)
        };
        let allowed = if eligible {
            vec![
                v_str(if retain { "local" } else { "incoming" }),
                v_str("defer"),
            ]
        } else {
            vec![v_str("defer")]
        };
        let mut output = vec![
            ("id", v_str(&id)),
            ("object", v_obj(self.object)),
            ("field", v_str("property")),
            ("name", v_str(&property.name.to_folded_case())),
            ("names", v_list(&[v_str(&property.name.to_folded_case())])),
            (
                "operation",
                v_str(if retain {
                    "retain_property"
                } else {
                    "add_property"
                }),
            ),
            ("classification", v_str(classification)),
            ("source", v_str(label)),
            (
                "base",
                base.as_ref()
                    .map(|hash| v_str(hash))
                    .unwrap_or_else(|| v_list(&[])),
            ),
            ("live", v_str(&live_hash)),
            ("incoming", v_str(&incoming_hash)),
            ("eligible", v_bool(eligible)),
            ("blockers", v_list(&blockers)),
            ("default", v_str("defer")),
            ("choices", v_list(&allowed)),
            ("live_present", v_bool(existing.is_some())),
            ("incoming_present", v_bool(true)),
            ("descendants", v_int(descendants_count as i64)),
            (
                "initialization_approved",
                v_bool(self.policy.initialization.contains_key(&member)),
            ),
            (
                "descendants_approved",
                v_bool(
                    self.policy
                        .initialization
                        .get(&member)
                        .copied()
                        .unwrap_or(false),
                ),
            ),
        ];
        if details {
            output.push((
                "live_text",
                v_str(
                    &live_value
                        .as_ref()
                        .map(|value| inspection::render(value, "property"))
                        .unwrap_or_default(),
                ),
            ));
            output.push((
                "incoming_text",
                v_str(&inspection::render(&incoming_value, "property")),
            ));
            output.push(("base_text_available", v_bool(false)));
        }
        Ok((
            ChangeRow {
                id,
                member,
                object: self.object,
                eligible,
                target: if retain {
                    ChangeTarget::RetainProperty {
                        accepted: base.unwrap(),
                    }
                } else {
                    ChangeTarget::AddProperty(property.clone())
                },
                incoming: None,
                incoming_hash,
                incoming_source: None,
                live_hash,
                default: "defer",
            },
            record(&output),
        ))
    }
}

/// Extend only this property's field baseline. Other accepted fields and live values survive.
pub(super) fn stamp(
    world: &mut dyn WorldState,
    permissions: &TaskPermissions,
    object: Obj,
    property: &ObjPropDef,
) -> Result<(), WorldStateError> {
    let baseline = world
        .get_object_metadata(permissions, &object, Symbol::mk(BASE_KEY))?
        .unwrap_or_else(|| {
            record(&[
                ("schema", v_str(inspection::FIELD_SCHEMA)),
                ("property", v_map(&[])),
                ("attribute", v_map(&[])),
            ])
        });
    let map = baseline
        .as_map()
        .ok_or_else(|| WorldStateError::DatabaseError("invalid field baseline".into()))?;
    if map
        .get(&v_str("schema"))
        .ok()
        .and_then(|schema| schema.as_string().map(str::to_owned))
        .as_deref()
        != Some(inspection::FIELD_SCHEMA)
    {
        return Err(WorldStateError::DatabaseError(
            "unsupported field baseline schema".into(),
        ));
    }
    let properties = map
        .get(&v_str("property"))
        .map_err(|_| WorldStateError::DatabaseError("missing property baselines".into()))?;
    let hash =
        digest(&inspection::property_value(property)).map_err(WorldStateError::DatabaseError)?;
    let properties = properties
        .as_map()
        .ok_or_else(|| WorldStateError::DatabaseError("invalid property baselines".into()))?
        .set(&v_str(&property.name.to_folded_case()), &v_str(&hash))
        .map_err(|error| WorldStateError::DatabaseError(error.to_string()))?;
    let baseline = map
        .set(&v_str("property"), &properties)
        .map_err(|error| WorldStateError::DatabaseError(error.to_string()))?;
    world.set_object_metadata(permissions, &object, Symbol::mk(BASE_KEY), baseline)
}
