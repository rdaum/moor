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

//! Objdef program review and guarded application. Reports are data, never executable mutation plans.
//!
//! Requests use schema 1, an `adopt` or `update` operation, explicit `objects`, a
//! `fields` list containing only `program`, and optional `constants`, `details`, and
//! explicitly trusted non-wizard `trusted_owners`. The read-only `inspect` operation also
//! accepts object, attribute, property, and unmatched-program fields; it produces no apply evidence.
//! Object addresses are installation-local bindings; supplied export identities must
//! agree with live metadata. Evidence binds the exact input and compilation profile
//! to the reviewed definitions, authority, live programs, and accepted baselines.
//! Optional `provenance` supplies bounded retrieval hints for accepted incoming programs.
//! `base_sources` supplies historical objdefs for selected `details` rows only. These
//! read-only sources must match each selected member's accepted program fingerprint.

#[path = "review_additions.rs"]
mod additions;
#[path = "review_inspection.rs"]
pub(crate) mod inspection;
#[path = "review_objects.rs"]
mod objects;
#[path = "review_policy.rs"]
mod policy;
#[path = "review_properties.rs"]
mod properties;

use crate::{
    Constants, ObjDefSet, ObjDefSource, ObjdefLoaderError,
    fingerprint::{PROGRAM_SCHEMA, digest, program_fingerprint},
};
use moor_common::model::{
    HasUuid, Named, ObjFlag, TaskPermissions, ValSet, VerbArgsSpec, WorldState, WorldStateError,
};
use moor_compiler::{CompileOptions, compile, program_to_tree, unparse};
use moor_var::{
    Associative, ByteSized, Symbol, Var, program::ProgramType, v_bool, v_int, v_list, v_map, v_obj,
    v_str,
};
use std::collections::{BTreeMap, BTreeSet};

/// Reserved bookkeeping key; source content cannot establish its authority.
pub const BASE_KEY: &str = "objdef_base";
/// Maximum combined source or selected-detail payload.
pub const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
/// Maximum program rows returned by one native analysis.
pub const MAX_ROWS: usize = 8192;

#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Parse(#[from] ObjdefLoaderError),
    #[error(transparent)]
    World(#[from] WorldStateError),
}

impl ReviewError {
    /// Return a versioned diagnostic without embedding source in an error or receipt.
    pub fn diagnostic(&self) -> Var {
        let mut values = vec![
            ("schema", v_int(1)),
            ("code", v_str("invalid_review")),
            (
                "message",
                v_str(&self.to_string().chars().take(4096).collect::<String>()),
            ),
        ];
        if let Self::Parse(ObjdefLoaderError::ObjectDefParseError(label, error)) = self {
            values[1] = ("code", v_str("compile_failure"));
            values.push(("source", v_str(label)));
            values.push(("pane", v_str("incoming")));
            let error = match error.as_ref() {
                moor_compiler::ObjDefParseError::VerbCompileError(error, _)
                | moor_compiler::ObjDefParseError::ParseError(error) => Some(error),
                _ => None,
            };
            if let Some(error) = error {
                let (line, column) = error.context().line_col;
                values.push(("line", v_int(line as i64)));
                values.push(("column", v_int(column as i64)));
                values.push(("end_line", v_int(line as i64)));
                values.push(("end_column", v_int(column as i64 + 1)));
            }
        }
        record(&values)
    }
}

impl From<String> for ReviewError {
    fn from(value: String) -> Self {
        Self::Invalid(value)
    }
}

/// Construct a report map with string keys for all runtime feature profiles.
pub fn record(entries: &[(&str, Var)]) -> Var {
    v_map(
        &entries
            .iter()
            .map(|(k, v)| (v_str(k), v.clone()))
            .collect::<Vec<_>>(),
    )
}

/// Decode map keys once and reject unknown or duplicate case-folded names.
pub fn fields(value: &Var, allowed: &[&str]) -> Result<BTreeMap<String, Var>, ReviewError> {
    let map = value
        .as_map()
        .ok_or_else(|| ReviewError::Invalid("expected a map".into()))?;
    let mut result = BTreeMap::new();
    for (key, value) in map.iter() {
        let key = key
            .as_symbol()
            .map_err(|_| ReviewError::Invalid("map keys must be strings or symbols".into()))?
            .to_folded_case();
        if !allowed.contains(&key.as_str()) || result.insert(key.clone(), value).is_some() {
            return Err(ReviewError::Invalid(format!(
                "unknown or duplicate key: {key}"
            )));
        }
    }
    Ok(result)
}

fn required<'a>(map: &'a BTreeMap<String, Var>, key: &str) -> Result<&'a Var, ReviewError> {
    map.get(key)
        .ok_or_else(|| ReviewError::Invalid(format!("missing {key}")))
}

/// Decode text without allocating more than the native source limit.
pub fn source_text(value: &Var) -> Result<String, ReviewError> {
    if let Some(s) = value.as_string() {
        if s.len() > MAX_SOURCE_BYTES {
            return Err("source exceeds 16 MiB".to_string().into());
        }
        return Ok(s.into());
    }
    let lines = value
        .as_list()
        .ok_or_else(|| ReviewError::Invalid("source must be text or lines".into()))?;
    let mut output = String::new();
    for (i, line) in lines.iter().enumerate() {
        let text = line
            .as_string()
            .ok_or_else(|| ReviewError::Invalid("source lines must be strings".into()))?;
        if output.len().saturating_add(text.len()).saturating_add(1) > MAX_SOURCE_BYTES {
            return Err("source exceeds 16 MiB".to_string().into());
        }
        if i != 0 {
            output.push('\n');
        }
        output.push_str(text);
    }
    Ok(output)
}

fn decode_sources(value: &Var) -> Result<Vec<ObjDefSource>, ReviewError> {
    let list = value
        .as_list()
        .ok_or_else(|| ReviewError::Invalid("sources must be a list".into()))?;
    if list.is_empty() || list.len() > 4096 {
        return Err("expected 1 to 4096 source units".to_string().into());
    }
    let mut labels = BTreeSet::new();
    let mut total = 0usize;
    let mut result = Vec::new();
    for unit in list.iter() {
        let unit = fields(&unit, &["label", "text"])?;
        let label = required(&unit, "label")?
            .as_string()
            .ok_or_else(|| ReviewError::Invalid("label must be a string".into()))?;
        if label.len() > 1024 || !labels.insert(label.to_owned()) {
            return Err("invalid or duplicate source label".to_string().into());
        }
        let text = source_text(required(&unit, "text")?)?;
        total = total.saturating_add(text.len());
        if total > MAX_SOURCE_BYTES {
            return Err("sources exceed 16 MiB".to_string().into());
        }
        result.push(ObjDefSource::new(label, text));
    }
    Ok(result)
}

/// Classify content equality. An absent baseline never authorizes an update.
pub fn classify(base: Option<&str>, live: &str, incoming: &str) -> &'static str {
    match base {
        None => "unbased",
        Some(b) if b == live && b == incoming => "unchanged",
        Some(b) if b == live => "upstream",
        Some(b) if b == incoming => "local",
        Some(_) if live == incoming => "converged",
        Some(_) => "conflict",
    }
}

pub(crate) enum ChangeTarget {
    Existing(uuid::Uuid),
    AddVerb(moor_compiler::ObjVerbDef),
    AddProperty(moor_compiler::ObjPropDef),
    RetainProperty { accepted: String },
    AddObject(Box<moor_compiler::ObjectDefinition>),
    RetainObject { accepted: String },
}

pub(crate) struct ChangeRow {
    pub id: String,
    pub eligible: bool,
    pub object: moor_var::Obj,
    pub target: ChangeTarget,
    pub member: String,
    pub incoming: Option<ProgramType>,
    pub incoming_hash: String,
    pub incoming_source: Option<Var>,
    pub live_hash: String,
    pub default: &'static str,
}

pub(crate) struct Analysis {
    pub report: Var,
    pub evidence: Var,
    pub rows: Vec<ChangeRow>,
    pub adoption: bool,
    policy: policy::Policy,
}

fn names(names: &[Symbol]) -> Var {
    v_list(
        &names
            .iter()
            .map(|n| v_str(&n.to_folded_case()))
            .collect::<Vec<_>>(),
    )
}

fn compile_context(options: &CompileOptions) -> Var {
    v_list(
        &[
            options.flyweight_type,
            options.bool_type,
            options.symbol_type,
            options.custom_errors,
            options.call_unsupported_builtins,
            options.legacy_type_constants,
        ]
        .map(v_bool),
    )
}

pub(crate) fn read_baseline(value: Option<Var>) -> Result<Option<String>, ReviewError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let data = fields(&value, &["schema", "program", "source"])?;
    if data
        .get("source")
        .is_some_and(|source| source.as_map().is_none())
    {
        return Err("baseline source provenance must be a map"
            .to_string()
            .into());
    }
    if required(&data, "schema")?.as_string() != Some(PROGRAM_SCHEMA) {
        return Err(
            "unsupported baseline schema; explicit re-adoption is required"
                .to_string()
                .into(),
        );
    }
    let hash = required(&data, "program")?
        .as_string()
        .ok_or_else(|| ReviewError::Invalid("invalid baseline hash".into()))?;
    if !hash.starts_with(&format!("{PROGRAM_SCHEMA}:"))
        || hash.len() != PROGRAM_SCHEMA.len() + 65
        || !hash[PROGRAM_SCHEMA.len() + 1..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err("invalid baseline hash".to_string().into());
    }
    Ok(Some(hash.into()))
}

fn program_text(program: &ProgramType) -> Result<Var, ReviewError> {
    let ProgramType::MooR(program) = program;
    let tree = program_to_tree(program).map_err(|e| e.to_string())?;
    Ok(v_str(
        &unparse(&tree, false, true)
            .map_err(|e| e.to_string())?
            .join("\n"),
    ))
}

pub(crate) fn analyze(
    world: &dyn WorldState,
    permissions: &TaskPermissions,
    options: &CompileOptions,
    sources: &Var,
    request: &Var,
) -> Result<Analysis, ReviewError> {
    let req = fields(
        request,
        &[
            "schema",
            "operation",
            "objects",
            "fields",
            "constants",
            "details",
            "trusted_owners",
            "provenance",
            "base_sources",
            "policy",
        ],
    )?;
    if required(&req, "schema")?.as_integer() != Some(1) {
        return Err("unsupported request schema".to_string().into());
    }
    let operation = required(&req, "operation")?
        .as_string()
        .ok_or_else(|| ReviewError::Invalid("operation must be a string".into()))?;
    if !matches!(operation, "adopt" | "update" | "inspect") {
        return Err("unsupported operation".to_string().into());
    }
    let adoption = operation == "adopt";
    let policy = policy::Policy::parse(req.get("policy"))?;
    let selected_fields = required(&req, "fields")?
        .as_list()
        .ok_or_else(|| ReviewError::Invalid("fields must be a list".into()))?;
    if operation == "inspect" {
        if selected_fields.is_empty()
            || selected_fields.iter().any(|field| {
                !matches!(
                    field.as_string(),
                    Some("object" | "attribute" | "property" | "program")
                )
            })
        {
            return Err(
                "inspection fields must be object, attribute, property, or program"
                    .to_owned()
                    .into(),
            );
        }
    } else if selected_fields.is_empty()
        || selected_fields
            .iter()
            .any(|field| !matches!(field.as_string(), Some("program" | "property" | "object")))
    {
        return Err("update fields must be program, property, or object"
            .to_owned()
            .into());
    }
    let actor_flags = world.flags_of(&permissions.principal())?;
    if !actor_flags.contains(ObjFlag::Wizard)
        && !permissions.can_call_builtin(Symbol::mk("preview_objdef_changes"))
        && !permissions.can_call_builtin(Symbol::mk("apply_objdef_changes"))
    {
        return Err(ReviewError::World(WorldStateError::VerbPermissionDenied));
    }
    let objects = required(&req, "objects")?
        .as_list()
        .ok_or_else(|| ReviewError::Invalid("objects must be a list".into()))?;
    if objects.is_empty() || objects.len() > 4096 {
        return Err("expected 1 to 4096 objects".to_string().into());
    }
    let mut scope = BTreeSet::new();
    for object in objects.iter() {
        let object = object
            .as_object()
            .ok_or_else(|| ReviewError::Invalid("scope must contain objects".into()))?;
        if !scope.insert(object) {
            return Err("duplicate object in scope".to_string().into());
        }
    }
    let mut trusted_owners = BTreeSet::new();
    if !policy.complete.is_subset(&scope)
        || policy
            .creations
            .keys()
            .any(|object| !scope.contains(object))
    {
        return Err(
            "object policy refers to an address outside the managed scope"
                .to_owned()
                .into(),
        );
    }
    if let Some(owners) = req.get("trusted_owners") {
        let owners = owners
            .as_list()
            .ok_or_else(|| ReviewError::Invalid("trusted_owners must be a list".into()))?;
        if owners.len() > 128 {
            return Err("too many trusted owners".to_string().into());
        }
        for owner in owners.iter() {
            let owner = owner
                .as_object()
                .ok_or_else(|| ReviewError::Invalid("trusted owners must be objects".into()))?;
            if !world.valid(&owner)? || !trusted_owners.insert(owner) {
                return Err("invalid or duplicate trusted owner".to_string().into());
            }
        }
    }
    let constants = req
        .get("constants")
        .map(|v| {
            v.as_map()
                .cloned()
                .map(Constants::Map)
                .ok_or_else(|| ReviewError::Invalid("constants must be a map".into()))
        })
        .transpose()?;
    let details = req
        .get("details")
        .map(|v| {
            v.as_list()
                .ok_or_else(|| ReviewError::Invalid("details must be a list".into()))
        })
        .transpose()?;
    let selected_details: BTreeSet<String> = details
        .into_iter()
        .flat_map(|l| l.iter())
        .map(|v| {
            v.as_string()
                .map(str::to_owned)
                .ok_or_else(|| ReviewError::Invalid("detail IDs must be strings".into()))
        })
        .collect::<Result<_, _>>()?;
    let provenance = req.get("provenance");
    if provenance.is_some_and(|value| value.as_map().is_none() || value.size_bytes() > 8192) {
        return Err("provenance must be a map of at most 8192 bytes"
            .to_string()
            .into());
    }
    let base_set = req
        .get("base_sources")
        .map(|sources| {
            if selected_details.is_empty() || operation == "inspect" {
                return Err(ReviewError::Invalid(
                    "base_sources requires selected program details".into(),
                ));
            }
            Ok(ObjDefSet::parse_sources(
                options,
                None,
                constants.as_ref(),
                decode_sources(sources)?,
            )?)
        })
        .transpose()?;
    let units = decode_sources(sources)?;
    let set = ObjDefSet::parse_sources(options, None, constants.as_ref(), units)?;
    let work = set
        .graph()
        .object_definitions()
        .values()
        .fold(0usize, |sum, (_, def)| {
            sum.saturating_add(
                1 + def.verbs.len() + def.property_definitions.len() + def.property_overrides.len(),
            )
        });
    if work > MAX_ROWS {
        return Err("review exceeds 8192 declarations".to_string().into());
    }
    if operation == "inspect" {
        return Ok(Analysis {
            report: inspection::inspect(
                world,
                permissions,
                &set,
                &scope,
                &selected_details,
                &selected_fields
                    .iter()
                    .map(|field| field.as_string().unwrap().to_owned())
                    .collect(),
            )?,
            evidence: v_list(&[]),
            rows: Vec::new(),
            adoption: false,
            policy,
        });
    }
    let source_digest = digest(sources)?;
    let request_identity = record(
        &req.iter()
            .filter(|(k, _)| !matches!(k.as_str(), "details" | "base_sources"))
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect::<Vec<_>>(),
    );
    let mut guards = vec![
        v_str(&source_digest),
        request_identity,
        compile_context(options),
        v_obj(permissions.principal()),
        v_int(actor_flags.to_u16().into()),
    ];
    let mut report_rows = Vec::new();
    let mut rows = Vec::new();
    let mut diagnostics = Vec::new();
    for object in set
        .graph()
        .object_definitions()
        .keys()
        .filter(|object| !scope.contains(object))
    {
        diagnostics.push(record(&[
            ("object", v_obj(*object)),
            ("code", v_str("outside_managed_scope")),
        ]));
    }
    let mut detail_bytes = 0usize;
    for object in scope {
        let Some((label, incoming)) = set.graph().object_definitions().get(&object) else {
            diagnostics.push(record(&[
                ("object", v_obj(object)),
                ("code", v_str("missing_source")),
            ]));
            continue;
        };
        if !adoption
            && policy.creations.contains_key(&object)
            && selected_fields
                .iter()
                .any(|field| field.as_string() == Some("object"))
        {
            let context = objects::Context {
                world,
                permissions,
                set: &set,
                policy: &policy,
                trusted_owners: &trusted_owners,
                provenance,
                source_digest: &source_digest,
            };
            let (row, report) = context.analyze(
                label,
                incoming,
                &mut guards,
                selected_details.contains(&format!("object/{}", objects::member_key(object))),
            )?;
            for field in ["live_text", "incoming_text"] {
                if let Ok(text) = report.as_map().unwrap().get(&v_str(field)) {
                    detail_bytes += text.as_string().unwrap().len();
                }
            }
            if detail_bytes > MAX_SOURCE_BYTES {
                return Err("selected details exceed 16 MiB".to_owned().into());
            }
            let proposed = matches!(row.target, ChangeTarget::AddObject(_));
            let blocked = !row.eligible;
            rows.push(row);
            report_rows.push(report);
            if proposed || blocked {
                continue;
            }
        }
        if !world.valid(&object)? {
            diagnostics.push(record(&[
                ("object", v_obj(object)),
                ("code", v_str("unsupported_creation")),
            ]));
            continue;
        }
        let owner = world.owner_of(&object)?;
        let object_flags = world.flags_of(&object)?;
        let owner_flags = world.flags_of(&owner)?;
        let live_identity =
            world.get_object_metadata(permissions, &object, Symbol::mk("import_export_id"))?;
        let incoming_identity = incoming
            .metadata
            .iter()
            .find(|(key, _)| *key == Symbol::mk("import_export_id"))
            .map(|(_, v)| v);
        let identity_matches = match (incoming_identity, live_identity.as_ref()) {
            (Some(a), Some(b)) => digest(a)? == digest(b)?,
            (Some(_), None) => false,
            _ => true,
        };
        let trusted = (owner_flags.contains(ObjFlag::Wizard) || trusted_owners.contains(&owner))
            && !object_flags.contains(ObjFlag::Write);
        guards.push(v_list(&[
            v_obj(object),
            v_obj(owner),
            v_int(object_flags.to_u16().into()),
            v_int(owner_flags.to_u16().into()),
            live_identity.unwrap_or_else(|| v_list(&[])),
            v_obj(world.parent_of(permissions, &object)?),
        ]));
        let verbs = world.verbs(permissions, &object)?;
        let definitions = verbs.iter().collect::<Vec<_>>();
        let mut matched = BTreeSet::new();
        for verb in incoming.verbs.iter().filter(|_| {
            selected_fields
                .iter()
                .any(|field| field.as_string() == Some("program"))
        }) {
            if report_rows.len() >= MAX_ROWS {
                return Err("review exceeds 8192 rows".to_string().into());
            }
            let matches = definitions
                .iter()
                .filter(|d| d.names() == verb.names && d.args() == verb.argspec)
                .collect::<Vec<_>>();
            if matches.is_empty() && !adoption {
                let member = additions::member_key(object, verb)?;
                let context = additions::VerbContext {
                    world,
                    permissions,
                    object,
                    trusted,
                    identity_matches,
                    trusted_owners: &trusted_owners,
                    provenance,
                    source_digest: &source_digest,
                };
                let (row, report) = context.analyze(
                    label,
                    verb,
                    &incoming.verbs,
                    &mut guards,
                    selected_details.contains(&format!("add/{member}")),
                )?;
                for field in ["incoming_text", "definition_text"] {
                    if let Ok(text) = report.as_map().unwrap().get(&v_str(field)) {
                        detail_bytes += text.as_string().unwrap().len();
                        if detail_bytes > MAX_SOURCE_BYTES {
                            return Err("selected details exceed 16 MiB".to_string().into());
                        }
                    }
                }
                report_rows.push(report);
                rows.push(row);
                continue;
            }
            if matches.len() != 1 || !matched.insert(matches[0].uuid()) {
                diagnostics.push(record(&[
                    ("object", v_obj(object)),
                    ("names", names(&verb.names)),
                    ("code", v_str("unsupported_or_ambiguous_definition")),
                ]));
                continue;
            }
            let definition = matches[0];
            let (live, _) = world.retrieve_verb(permissions, &object, definition.uuid())?;
            let verb_owner_flags = world.flags_of(&definition.owner())?;
            let raw_base = world.get_verb_metadata(
                permissions,
                &object,
                definition.uuid(),
                Symbol::mk(BASE_KEY),
            )?;
            let parsed_base = read_baseline(raw_base.clone());
            let mut blockers = Vec::new();
            let base = match parsed_base {
                Ok(base) => base,
                Err(error) if adoption => {
                    let _ = error;
                    None
                }
                Err(error) => {
                    blockers.push(v_str(&error.to_string()));
                    None
                }
            };
            if !trusted
                || !(verb_owner_flags.contains(ObjFlag::Wizard)
                    || trusted_owners.contains(&definition.owner()))
                || definition
                    .flags()
                    .contains(moor_common::model::VerbFlag::Write)
            {
                blockers.push(v_str("untrusted_target_authority"));
            }
            if !identity_matches {
                blockers.push(v_str("object_identity_mismatch"));
            }
            let live_hash = program_fingerprint(&live)?;
            let incoming_hash = program_fingerprint(&verb.program)?;
            let classification = classify(base.as_deref(), &live_hash, &incoming_hash);
            if !adoption && base.is_none() {
                blockers.push(v_str("adoption_required"));
            }
            if definition.owner() != verb.owner
                || definition.flags() != verb.flags
                || !verb.metadata.is_empty()
            {
                diagnostics.push(record(&[
                    ("object", v_obj(object)),
                    ("names", names(&verb.names)),
                    ("code", v_str("definition_fields_unmanaged")),
                ]));
            }
            let id = format!("{object}/{}/program", definition.uuid());
            let eligible = blockers.is_empty();
            guards.push(v_list(&[
                v_str(&id),
                v_int(
                    definitions
                        .iter()
                        .position(|d| d.uuid() == definition.uuid())
                        .unwrap() as i64,
                ),
                names(definition.names()),
                v_obj(definition.owner()),
                v_int(definition.flags().to_u16().into()),
                v_int(verb_owner_flags.to_u16().into()),
                v_int(
                    VerbArgsSpec::try_write(definition.args())
                        .map_err(|e| e.to_string())?
                        .into(),
                ),
                v_str(&live_hash),
                raw_base.clone().unwrap_or_else(|| v_list(&[])),
            ]));
            let default = if !eligible {
                "defer"
            } else if adoption {
                "incoming"
            } else {
                match classification {
                    "upstream" | "converged" => "incoming",
                    "conflict" => "unresolved",
                    _ => "defer",
                }
            };
            let mut output = vec![
                ("id", v_str(&id)),
                ("object", v_obj(object)),
                ("field", v_str("program")),
                ("names", names(&verb.names)),
                ("source", v_str(label)),
                ("classification", v_str(classification)),
                (
                    "base",
                    base.as_deref().map(v_str).unwrap_or_else(|| v_list(&[])),
                ),
                ("live", v_str(&live_hash)),
                ("incoming", v_str(&incoming_hash)),
                ("eligible", v_bool(eligible)),
                ("default", v_str(default)),
                ("blockers", v_list(&blockers)),
                (
                    "choices",
                    v_list(&if !eligible {
                        vec![v_str("defer")]
                    } else if adoption {
                        vec![v_str("incoming"), v_str("defer")]
                    } else {
                        vec![
                            v_str("incoming"),
                            v_str("local"),
                            v_str("edited"),
                            v_str("defer"),
                        ]
                    }),
                ),
            ];
            if selected_details.contains(&id) {
                if let Some(source) = raw_base
                    .as_ref()
                    .and_then(Var::as_map)
                    .and_then(|value| value.get(&v_str("source")).ok())
                {
                    // Retrieval hints are never evidence that historical source is correct.
                    output.push(("base_source", source));
                }
                let live_text = program_text(&live)?;
                let incoming_text = program_text(&verb.program)?;
                detail_bytes +=
                    live_text.as_string().unwrap().len() + incoming_text.as_string().unwrap().len();
                if detail_bytes > MAX_SOURCE_BYTES {
                    return Err("selected details exceed 16 MiB".to_string().into());
                }
                output.push(("live_text", live_text));
                output.push(("incoming_text", incoming_text));
                // Both comparison panes are decompiled. File coordinates refer only to source_text.
                output.push((
                    "text_coordinates",
                    record(&[
                        ("kind", v_str("decompiled")),
                        ("line", v_int(1)),
                        ("column", v_int(1)),
                    ]),
                ));
                let base_text = base_set
                    .as_ref()
                    .map(|set| {
                        let expected = base.as_deref().ok_or_else(|| {
                            ReviewError::Invalid("selected member has no accepted baseline".into())
                        })?;
                        let (_, historical) = set
                            .graph()
                            .object_definitions()
                            .get(&object)
                            .ok_or_else(|| {
                                ReviewError::Invalid("historical object is missing".into())
                            })?;
                        if let Some((_, identity)) = historical
                            .metadata
                            .iter()
                            .find(|(key, _)| *key == Symbol::mk("import_export_id"))
                        {
                            let live_identity = world.get_object_metadata(
                                permissions,
                                &object,
                                Symbol::mk("import_export_id"),
                            )?;
                            if live_identity.as_ref().map(digest).transpose()?.as_deref()
                                != Some(digest(identity)?.as_str())
                            {
                                return Err(ReviewError::Invalid(
                                    "historical object identity mismatch".into(),
                                ));
                            }
                        }
                        let matching = historical
                            .verbs
                            .iter()
                            .filter(|candidate| {
                                candidate.names == verb.names && candidate.argspec == verb.argspec
                            })
                            .collect::<Vec<_>>();
                        if matching.len() != 1
                            || program_fingerprint(&matching[0].program)? != expected
                        {
                            return Err(ReviewError::Invalid(
                                "historical member does not match the accepted baseline".into(),
                            ));
                        }
                        program_text(&matching[0].program)
                    })
                    .transpose()?;
                output.push(("base_text_available", v_bool(base_text.is_some())));
                if let Some(text) = base_text {
                    detail_bytes += text.as_string().unwrap().len();
                    if detail_bytes > MAX_SOURCE_BYTES {
                        return Err("selected details exceed 16 MiB".to_string().into());
                    }
                    output.push(("base_text", text));
                }
                if let Some(source) = &verb.source {
                    detail_bytes += source.text.len();
                    if detail_bytes > MAX_SOURCE_BYTES {
                        return Err("selected details exceed 16 MiB".to_string().into());
                    }
                    output.push(("source_text", v_str(&source.text)));
                    output.push((
                        "source_location",
                        record(&[
                            ("label", v_str(label)),
                            ("line", v_int(source.line as i64)),
                            ("column", v_int(source.column as i64)),
                        ]),
                    ));
                }
            }
            report_rows.push(record(&output));
            rows.push(ChangeRow {
                id,
                eligible,
                object,
                target: ChangeTarget::Existing(definition.uuid()),
                member: additions::member_key(object, verb)?,
                incoming: Some(verb.program.clone()),
                incoming_hash,
                incoming_source: provenance.map(|value| {
                    value
                        .as_map()
                        .unwrap()
                        .set(&v_str("label"), &v_str(label))
                        .unwrap()
                        .as_map()
                        .unwrap()
                        .set(&v_str("digest"), &v_str(&source_digest))
                        .unwrap()
                }),
                live_hash,
                default,
            });
        }
        // A program-only update cannot manage unmatched live definitions, but omitting them
        // would make local additions disappear from the read-only comparison.
        for definition in &definitions {
            if !matched.contains(&definition.uuid()) {
                diagnostics.push(record(&[
                    ("object", v_obj(object)),
                    ("names", names(definition.names())),
                    (
                        "code",
                        v_str(if policy.complete.contains(&object) {
                            "unsupported_removal"
                        } else {
                            "live_definition_unmatched"
                        }),
                    ),
                ]));
            }
        }
        if !adoption
            && !incoming.property_definitions.is_empty()
            && selected_fields
                .iter()
                .any(|field| field.as_string() == Some("property"))
        {
            let local = crate::collect_object_definitions(world, permissions, &[object])
                .map_err(|error| error.to_string())?
                .remove(0);
            let context = properties::Context {
                world,
                permissions,
                object,
                trusted,
                identity_matches,
                trusted_owners: &trusted_owners,
                policy: &policy,
                local: &local,
            };
            for property in &incoming.property_definitions {
                let member = properties::member_key(object, property.name);
                // Existing data stays in inspection unless the administrator explicitly
                // includes it in an initialization group.
                let absent = matches!(
                    world.get_property_info(permissions, &object, property.name),
                    Err(WorldStateError::PropertyNotFound(_, _))
                );
                if !absent
                    && local
                        .property_definitions
                        .iter()
                        .any(|p| p.name == property.name)
                    && !policy.initialization.contains_key(&member)
                {
                    continue;
                }
                let (row, report) = context.analyze(
                    label,
                    property,
                    &mut guards,
                    selected_details.contains(&format!("property/{member}")),
                )?;
                for field in ["live_text", "incoming_text"] {
                    if let Ok(text) = report.as_map().unwrap().get(&v_str(field)) {
                        detail_bytes += text.as_string().unwrap().len();
                    }
                }
                if detail_bytes > MAX_SOURCE_BYTES {
                    return Err("selected details exceed 16 MiB".to_owned().into());
                }
                rows.push(row);
                report_rows.push(report);
            }
        }
        // Existing fields and overrides remain read-only.
        if policy.complete.contains(&object) {
            for property in world.properties(permissions, &object)?.iter() {
                if !incoming
                    .property_definitions
                    .iter()
                    .any(|incoming| incoming.name == property.name())
                {
                    diagnostics.push(record(&[
                        ("object", v_obj(object)),
                        ("names", names(&[property.name()])),
                        ("code", v_str("unsupported_removal")),
                    ]));
                }
            }
        }
        if !incoming.property_definitions.is_empty() || !incoming.property_overrides.is_empty() {
            diagnostics.push(record(&[
                ("object", v_obj(object)),
                ("code", v_str("property_fields_unmanaged")),
            ]));
        }
    }
    if selected_details
        .iter()
        .any(|id| !rows.iter().any(|row| &row.id == id))
    {
        return Err("unknown detail row".to_string().into());
    }
    let has_additions = rows.iter().any(|row| {
        matches!(
            row.target,
            ChangeTarget::AddVerb(_) | ChangeTarget::AddProperty(_) | ChangeTarget::AddObject(_)
        )
    });
    policy.annotate(&mut rows, &mut report_rows, has_additions)?;
    let evidence = record(&[
        ("schema", v_int(1)),
        ("guard", v_str(&digest(&v_list(&guards))?)),
    ]);
    let mut counts = BTreeMap::<String, i64>::new();
    for row in &report_rows {
        let classification = row.as_map().unwrap().get(&v_str("classification")).unwrap();
        *counts
            .entry(classification.as_string().unwrap().to_owned())
            .or_default() += 1;
    }
    let report = record(&[
        (
            "counts",
            v_map(
                &counts
                    .into_iter()
                    .map(|(key, count)| (v_str(&key), v_int(count)))
                    .collect::<Vec<_>>(),
            ),
        ),
        ("schema", v_int(1)),
        ("operation", v_str(operation)),
        ("source_digest", v_str(&source_digest)),
        ("evidence", evidence.clone()),
        ("rows", v_list(&report_rows)),
        ("groups", policy.report(&rows)),
        ("diagnostics", v_list(&diagnostics)),
    ]);
    Ok(Analysis {
        report,
        evidence,
        rows,
        adoption,
        policy,
    })
}

/// Inspect programs and validate pending choices without changing world state.
pub fn preview(
    world: &dyn WorldState,
    permissions: &TaskPermissions,
    options: &CompileOptions,
    sources: &Var,
    request: &Var,
    choices: Option<&Var>,
) -> Result<Var, ReviewError> {
    let analysis = analyze(world, permissions, options, sources, request)?;
    let validation = validate_choices(&analysis, options, choices)?;
    let mut output = analysis.report.as_map().unwrap().iter().collect::<Vec<_>>();
    output.push((v_str("validation"), v_list(&validation)));
    output.push((
        v_str("group_validation"),
        v_list(&analysis.policy.selection_issues(&analysis, choices)),
    ));
    Ok(v_map(&output))
}

fn validate_choices(
    analysis: &Analysis,
    options: &CompileOptions,
    choices: Option<&Var>,
) -> Result<Vec<Var>, ReviewError> {
    let mut validation = Vec::new();
    let mut draft_bytes = 0usize;
    if let Some(choices) = choices {
        let map = choices
            .as_map()
            .ok_or_else(|| ReviewError::Invalid("choices must be a map keyed by row ID".into()))?;
        if map.len() > MAX_ROWS {
            return Err("too many choices".to_string().into());
        }
        for (id, choice) in map.iter() {
            let id = id
                .as_string()
                .ok_or_else(|| ReviewError::Invalid("row IDs must be strings".into()))?;
            let row = analysis
                .rows
                .iter()
                .find(|r| r.id == id)
                .ok_or_else(|| ReviewError::Invalid("unknown choice row".into()))?;
            let choice = fields(&choice, &["choice", "program", "validation"])?;
            let kind = required(&choice, "choice")?
                .as_string()
                .ok_or_else(|| ReviewError::Invalid("choice must be a string".into()))?;
            if !matches!(kind, "incoming" | "local" | "edited" | "defer")
                || (!row.eligible && kind != "defer")
                || (analysis.adoption && !matches!(kind, "incoming" | "defer"))
                || (matches!(
                    row.target,
                    ChangeTarget::AddVerb(_)
                        | ChangeTarget::AddProperty(_)
                        | ChangeTarget::AddObject(_)
                ) && !matches!(kind, "incoming" | "defer"))
                || (matches!(
                    row.target,
                    ChangeTarget::RetainProperty { .. } | ChangeTarget::RetainObject { .. }
                ) && !matches!(kind, "local" | "defer"))
            {
                return Err("choice is not allowed for this row".to_string().into());
            }
            if kind == "edited" {
                let text = source_text(required(&choice, "program")?)?;
                draft_bytes = draft_bytes.saturating_add(text.len());
                if draft_bytes > MAX_SOURCE_BYTES {
                    return Err("drafts exceed 16 MiB".to_string().into());
                }
                match compile(&text, options.clone()) {
                    Ok(program) => {
                        let hash = program_fingerprint(&ProgramType::MooR(program))?;
                        validation.push(record(&[
                            ("id", v_str(id)),
                            ("valid", v_bool(true)),
                            ("fingerprint", v_str(&hash)),
                            (
                                "validation",
                                v_str(&digest(&v_list(&[
                                    analysis.evidence.clone(),
                                    v_str(id),
                                    v_str(&text),
                                ]))?),
                            ),
                        ]));
                    }
                    Err(error) => {
                        let (line, column) = error.context().line_col;
                        validation.push(record(&[
                            ("id", v_str(id)),
                            ("valid", v_bool(false)),
                            ("pane", v_str("result")),
                            ("line", v_int(line as i64)),
                            ("column", v_int(column as i64)),
                            ("end_line", v_int(line as i64)),
                            ("end_column", v_int(column as i64 + 1)),
                            ("message", v_str(&error.to_string())),
                        ]));
                    }
                }
            } else if choice.contains_key("program") || choice.contains_key("validation") {
                return Err("program and validation require an edited choice"
                    .to_string()
                    .into());
            }
        }
    }
    Ok(validation)
}

/// Apply failures distinguish read-only rejection from a transaction that must be discarded.
#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error(transparent)]
    Review(#[from] ReviewError),
    /// A write may have occurred. Retry conflicts normally; abort other failures without committing.
    #[error("objdef write failed: {0}")]
    Mutation(WorldStateError),
}

struct ChangeAction {
    object: moor_var::Obj,
    target: ChangeTarget,
    program: Option<ProgramType>,
    baseline: Var,
}

/// Apply reviewed programs and accepted hashes in the caller's current transaction.
///
/// Reparse input and revalidate the original evidence on every invocation, including retries.
/// No source acquisition or task suspension occurs here. All choices and drafts are validated
/// before writes start. The caller must discard the transaction on `ApplyError::Mutation`.
/// Returned receipts contain hashes and decisions, never source or historical program bodies.
pub fn apply(
    world: &mut dyn WorldState,
    permissions: &TaskPermissions,
    options: &CompileOptions,
    sources: &Var,
    request: &Var,
    evidence: &Var,
    choices: &Var,
) -> Result<Var, ApplyError> {
    if !world
        .flags_of(&permissions.principal())
        .map_err(ReviewError::from)?
        .contains(ObjFlag::Wizard)
        && !permissions.can_call_builtin(Symbol::mk("apply_objdef_changes"))
    {
        return Err(ReviewError::World(WorldStateError::VerbPermissionDenied).into());
    }
    let analysis = analyze(world, permissions, options, sources, request)?;
    if analysis
        .report
        .as_map()
        .unwrap()
        .get(&v_str("operation"))
        .unwrap()
        .as_string()
        == Some("inspect")
    {
        return Err(ReviewError::Invalid("inspection cannot be applied".into()).into());
    }
    let provided = fields(evidence, &["schema", "guard"])?;
    if required(&provided, "schema")?.as_integer() != Some(1)
        || digest(evidence).map_err(ReviewError::from)?
            != digest(&analysis.evidence).map_err(ReviewError::from)?
    {
        return Err(ReviewError::Invalid(
            "stale review evidence; inspect current state before applying".into(),
        )
        .into());
    }
    // Preview's choice decoder supplies the same strict eligibility and draft checks.
    let validation = validate_choices(&analysis, options, Some(choices))?;
    let group_issues = analysis.policy.selection_issues(&analysis, Some(choices));
    if let Some(issue) = group_issues.first() {
        return Err(ReviewError::Invalid(
            issue
                .as_map()
                .unwrap()
                .get(&v_str("message"))
                .unwrap()
                .as_string()
                .unwrap()
                .to_owned(),
        )
        .into());
    }
    let choice_map = choices.as_map().unwrap();

    let mut actions = Vec::new();
    let mut decisions = Vec::new();
    let mut created = Vec::new();
    for row in analysis.rows {
        let choice = choice_map
            .iter()
            .find(|(id, _)| id.as_string() == Some(row.id.as_str()))
            .map(|(_, v)| v);
        let fields = choice
            .as_ref()
            .map(|c| fields(c, &["choice", "program", "validation"]))
            .transpose()?;
        let kind = fields
            .as_ref()
            .and_then(|c| c.get("choice"))
            .and_then(Var::as_string)
            .unwrap_or(row.default);
        if kind == "unresolved" {
            return Err(
                ReviewError::Invalid(format!("missing conflict choice: {}", row.id)).into(),
            );
        }
        if kind == "defer" {
            continue;
        }
        let program = if kind == "edited" {
            let selected = fields.as_ref().unwrap();
            let check = validation
                .iter()
                .find(|v| {
                    v.as_map().unwrap().iter().any(|(k, v)| {
                        k.as_string() == Some("id") && v.as_string() == Some(row.id.as_str())
                    })
                })
                .unwrap();
            let check = self::fields(
                check,
                &[
                    "id",
                    "valid",
                    "fingerprint",
                    "validation",
                    "pane",
                    "line",
                    "column",
                    "message",
                ],
            )?;
            if !required(&check, "valid")?.is_true()
                || selected.get("validation").and_then(Var::as_string)
                    != check.get("validation").and_then(Var::as_string)
            {
                return Err(ReviewError::Invalid(
                    "edited program needs successful validation for this exact review and draft"
                        .into(),
                )
                .into());
            }
            let text = source_text(required(selected, "program")?)?;
            Some(ProgramType::MooR(
                compile(&text, options.clone()).map_err(|e| ReviewError::Invalid(e.to_string()))?,
            ))
        } else if kind == "incoming"
            && !analysis.adoption
            && matches!(row.target, ChangeTarget::Existing(_))
            && row.live_hash != row.incoming_hash
        {
            row.incoming
        } else {
            None
        };
        let mut baseline = vec![
            ("schema", v_str(PROGRAM_SCHEMA)),
            ("program", v_str(&row.incoming_hash)),
        ];
        if let Some(source) = row.incoming_source {
            baseline.push(("source", source));
        }
        if let ChangeTarget::AddVerb(verb) = &row.target {
            created.push(record(&[
                ("member", v_str(&row.member)),
                ("object", v_obj(row.object)),
                ("field", v_str("program")),
                ("names", names(&verb.names)),
            ]));
        }
        if let ChangeTarget::AddProperty(property) = &row.target {
            created.push(record(&[
                ("member", v_str(&row.member)),
                ("object", v_obj(row.object)),
                ("field", v_str("property")),
                ("names", names(&[property.name])),
            ]));
        }
        if let ChangeTarget::AddObject(object) = &row.target {
            created.push(record(&[
                ("member", v_str(&row.member)),
                ("object", v_obj(row.object)),
                ("field", v_str("object")),
                ("names", v_list(&[v_str("")])),
                ("creation", v_str(&row.incoming_hash)),
            ]));
            for verb in &object.verbs {
                created.push(record(&[
                    ("member", v_str(&additions::member_key(row.object, verb)?)),
                    ("object", v_obj(row.object)),
                    ("field", v_str("program")),
                    ("names", names(&verb.names)),
                ]));
            }
            for property in &object.property_definitions {
                created.push(record(&[
                    (
                        "member",
                        v_str(&properties::member_key(row.object, property.name)),
                    ),
                    ("object", v_obj(row.object)),
                    ("field", v_str("property")),
                    ("names", names(&[property.name])),
                ]));
            }
        }
        let accepted = match &row.target {
            ChangeTarget::RetainProperty { accepted } | ChangeTarget::RetainObject { accepted } => {
                accepted
            }
            _ => &row.incoming_hash,
        }
        .clone();
        actions.push(ChangeAction {
            object: row.object,
            target: row.target,
            program,
            baseline: record(&baseline),
        });
        decisions.push(record(&[
            ("id", v_str(&row.id)),
            ("choice", v_str(kind)),
            ("accepted", v_str(&accepted)),
        ]));
    }
    let receipt = record(&[
        ("schema", v_int(1)),
        ("evidence", analysis.evidence),
        ("decisions", v_list(&decisions)),
        ("created", v_list(&created)),
    ]);
    write_actions(world, permissions, actions)?;
    Ok(receipt)
}

fn write_actions(
    world: &mut dyn WorldState,
    permissions: &TaskPermissions,
    actions: Vec<ChangeAction>,
) -> Result<(), ApplyError> {
    let mut objects = Vec::new();
    let mut members = Vec::new();
    for action in actions {
        if let ChangeTarget::AddObject(object) = action.target {
            objects.push((object, action.baseline));
        } else {
            members.push(action);
        }
    }
    for (object, baseline) in &objects {
        for verb in &object.verbs {
            let mut verb_baseline = vec![
                ("schema", v_str(PROGRAM_SCHEMA)),
                (
                    "program",
                    v_str(&program_fingerprint(&verb.program).map_err(ReviewError::from)?),
                ),
            ];
            if let Ok(source) = baseline.as_map().unwrap().get(&v_str("source")) {
                verb_baseline.push(("source", source));
            }
            members.push(ChangeAction {
                object: object.oid,
                target: ChangeTarget::AddVerb(verb.clone()),
                program: None,
                baseline: record(&verb_baseline),
            });
        }
    }
    objects::allocate(
        world,
        permissions,
        &objects
            .iter()
            .map(|(object, _)| object.as_ref())
            .collect::<Vec<_>>(),
    )
    .map_err(ApplyError::Mutation)?;
    objects::initialize(world, permissions, &objects).map_err(ApplyError::Mutation)?;
    for action in members {
        let uuid = match action.target {
            ChangeTarget::RetainProperty { .. } | ChangeTarget::RetainObject { .. } => continue,
            ChangeTarget::AddObject(_) => {
                unreachable!("object actions were partitioned before member writes")
            }
            ChangeTarget::AddProperty(property) => {
                world
                    .define_property(
                        permissions,
                        &action.object,
                        &action.object,
                        property.name,
                        &property.perms.owner(),
                        property.perms.flags(),
                        property.value.clone(),
                    )
                    .map_err(ApplyError::Mutation)?;
                for (key, value) in &property.metadata {
                    if *key != Symbol::mk(BASE_KEY) {
                        world
                            .set_property_metadata(
                                permissions,
                                &action.object,
                                property.name,
                                *key,
                                value.clone(),
                            )
                            .map_err(ApplyError::Mutation)?;
                    }
                }
                properties::stamp(world, permissions, action.object, &property)
                    .map_err(ApplyError::Mutation)?;
                continue;
            }
            ChangeTarget::Existing(uuid) => uuid,
            ChangeTarget::AddVerb(verb) => {
                world
                    .add_verb(
                        permissions,
                        &action.object,
                        verb.names.clone(),
                        &verb.owner,
                        verb.flags,
                        verb.argspec,
                        verb.program,
                    )
                    .map_err(ApplyError::Mutation)?;
                let definitions = world
                    .verbs(permissions, &action.object)
                    .map_err(ApplyError::Mutation)?;
                let definition = definitions
                    .iter()
                    .find(|definition| {
                        definition.names() == verb.names && definition.args() == verb.argspec
                    })
                    .ok_or_else(|| {
                        ApplyError::Mutation(WorldStateError::DatabaseError(
                            "new verb is not visible in its transaction".into(),
                        ))
                    })?;
                for (key, value) in verb.metadata {
                    if key != Symbol::mk(BASE_KEY) {
                        world
                            .set_verb_metadata(
                                permissions,
                                &action.object,
                                definition.uuid(),
                                key,
                                value,
                            )
                            .map_err(ApplyError::Mutation)?;
                    }
                }
                definition.uuid()
            }
        };
        if let Some(program) = action.program {
            world
                .update_verb_with_id(
                    permissions,
                    &action.object,
                    uuid,
                    moor_common::model::VerbAttrs {
                        program: Some(program),
                        definer: None,
                        owner: None,
                        names: None,
                        flags: None,
                        args_spec: None,
                    },
                )
                .map_err(ApplyError::Mutation)?;
        }
        world
            .set_verb_metadata(
                permissions,
                &action.object,
                uuid,
                Symbol::mk(BASE_KEY),
                action.baseline,
            )
            .map_err(ApplyError::Mutation)?;
    }
    objects::finish(world, permissions, &objects).map_err(ApplyError::Mutation)?;
    Ok(())
}

#[cfg(test)]
#[path = "review_tests.rs"]
mod tests;
