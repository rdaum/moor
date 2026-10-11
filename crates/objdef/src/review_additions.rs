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

//! Analysis of absent verb definitions. Preview constructs data only; UUID allocation
//! and definition writes happen later in the caller's guarded transaction.

use super::{ProgramRow, ProgramTarget, ReviewError, digest, names, program_text, record};
use moor_common::model::{
    HasUuid, Named, ObjFlag, PrepSpec, TaskPermissions, ValSet, VerbArgsSpec, VerbFlag, WorldState,
    verb_perms_string,
};
use moor_compiler::{ObjVerbDef, to_literal};
use moor_var::{Associative, Obj, Symbol, Var, v_bool, v_int, v_list, v_obj, v_str};
use std::collections::BTreeSet;

/// A semantic membership key survives allocation of a new verb's UUID. The UUID
/// remains an independent apply guard for an installed definition.
pub(super) fn member_key(object: Obj, verb: &ObjVerbDef) -> Result<String, ReviewError> {
    Ok(format!(
        "{object}/verb/{}",
        digest(&v_list(&[
            names(&verb.names),
            v_int(
                VerbArgsSpec::try_write(verb.argspec)
                    .map_err(|e| e.to_string())?
                    .into()
            ),
        ]))?
    ))
}

fn overlap(left: Symbol, right: Symbol) -> bool {
    let left = left.to_folded_case();
    let right = right.to_folded_case();
    if !left.contains('*') && !right.contains('*') {
        return left == right;
    }
    // Reject uncertain wildcard overlaps rather than install a definition that can
    // change dispatch for an existing command. Literal, distinct names remain usable.
    let prefix = |name: String| name.split('*').next().unwrap_or("").to_owned();
    let left = prefix(left);
    let right = prefix(right);
    left.starts_with(&right) || right.starts_with(&left)
}

pub(super) struct VerbContext<'a> {
    pub world: &'a dyn WorldState,
    pub permissions: &'a TaskPermissions,
    pub object: Obj,
    pub trusted: bool,
    pub identity_matches: bool,
    pub trusted_owners: &'a BTreeSet<Obj>,
    pub provenance: Option<&'a Var>,
    pub source_digest: &'a str,
}

impl VerbContext<'_> {
    pub fn analyze(
        &self,
        label: &str,
        verb: &ObjVerbDef,
        siblings: &[ObjVerbDef],
        guards: &mut Vec<Var>,
        details: bool,
    ) -> Result<(ProgramRow, Var), ReviewError> {
        let member = member_key(self.object, verb)?;
        let id = format!("add/{member}");
        let mut blockers = Vec::new();
        let owner_flags = if self.world.valid(&verb.owner)? {
            Some(self.world.flags_of(&verb.owner)?)
        } else {
            None
        };
        if !self.trusted
            || !owner_flags.is_some_and(|flags| {
                flags.contains(ObjFlag::Wizard) || self.trusted_owners.contains(&verb.owner)
            })
            || verb.flags.contains(VerbFlag::Write)
        {
            blockers.push(v_str("untrusted_target_authority"));
        }
        if !self.identity_matches {
            blockers.push(v_str("object_identity_mismatch"));
        }
        let mut collision = false;
        for object in self
            .world
            .ancestors_of(self.permissions, &self.object, true)?
            .iter()
        {
            let definitions = self.world.verbs(self.permissions, &object)?;
            let mut signatures = Vec::new();
            for definition in definitions.iter() {
                collision |= definition
                    .names()
                    .iter()
                    .any(|existing| verb.names.iter().any(|name| overlap(*existing, *name)));
                signatures.push(v_list(&[
                    v_str(&definition.uuid().to_string()),
                    names(definition.names()),
                    v_int(
                        VerbArgsSpec::try_write(definition.args())
                            .map_err(|e| e.to_string())?
                            .into(),
                    ),
                ]));
            }
            guards.push(v_list(&[
                v_obj(object),
                v_obj(self.world.parent_of(self.permissions, &object)?),
                v_list(&signatures),
            ]));
        }
        for other in siblings {
            if std::ptr::eq(other, verb) {
                continue;
            }
            collision |= other
                .names
                .iter()
                .any(|existing| verb.names.iter().any(|name| overlap(*existing, *name)));
        }
        if collision {
            blockers.push(v_str("verb_name_collision"));
        }
        guards.push(v_list(&[
            v_str(&id),
            v_obj(verb.owner),
            owner_flags
                .map(|flags| v_int(flags.to_u16().into()))
                .unwrap_or_else(|| v_list(&[])),
        ]));
        let incoming_hash = crate::fingerprint::program_fingerprint(&verb.program)?;
        let incoming_source = self.provenance.map(|value| {
            value
                .as_map()
                .unwrap()
                .set(&v_str("label"), &v_str(label))
                .unwrap()
                .as_map()
                .unwrap()
                .set(&v_str("digest"), &v_str(self.source_digest))
                .unwrap()
        });
        let eligible = blockers.is_empty();
        let row = ProgramRow {
            id: id.clone(),
            member,
            eligible,
            object: self.object,
            target: ProgramTarget::AddVerb(verb.clone()),
            incoming: verb.program.clone(),
            incoming_hash: incoming_hash.clone(),
            incoming_source,
            live_hash: String::new(),
            default: "defer",
        };
        let mut output = vec![
            ("id", v_str(&id)),
            ("object", v_obj(self.object)),
            ("field", v_str("program")),
            ("operation", v_str("add_verb")),
            ("names", names(&verb.names)),
            ("source", v_str(label)),
            ("classification", v_str("incoming_only")),
            ("base", v_list(&[])),
            ("live", v_str("")),
            ("incoming", v_str(&incoming_hash)),
            ("eligible", v_bool(eligible)),
            ("default", v_str("defer")),
            ("blockers", v_list(&blockers)),
            ("live_present", v_bool(false)),
            ("incoming_present", v_bool(true)),
            (
                "choices",
                v_list(&if eligible {
                    vec![v_str("incoming"), v_str("defer")]
                } else {
                    vec![v_str("defer")]
                }),
            ),
        ];
        if details {
            let prep = match verb.argspec.prep {
                PrepSpec::Any => "any".to_owned(),
                PrepSpec::None => "none".to_owned(),
                PrepSpec::Other(prep) => prep.to_string_single().to_owned(),
            };
            let mut definition = format!(
                "Owner: {}\nFlags: {}\nArguments: {} {} {}",
                verb.owner,
                verb_perms_string(verb.flags),
                verb.argspec.dobj.to_string(),
                prep,
                verb.argspec.iobj.to_string(),
            );
            for (key, value) in &verb.metadata {
                if *key != Symbol::mk(super::BASE_KEY) {
                    definition.push_str(&format!("\nMetadata {}: {}", key, to_literal(value)));
                }
            }
            output.extend([
                ("definition_text", v_str(&definition)),
                ("live_text", v_str("")),
                ("incoming_text", program_text(&verb.program)?),
                ("base_text_available", v_bool(false)),
            ]);
        }
        Ok((row, record(&output)))
    }
}
