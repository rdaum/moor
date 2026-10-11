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

//! Explicit dependency groups for guarded changes. Group declarations are caller policy,
//! never inferred from program text or imported as authority from objdef metadata.

use super::{Analysis, MAX_ROWS, ProgramRow, ReviewError, fields, record, required};
use moor_var::{Associative, ByteSized, Var, v_bool, v_list, v_map, v_str};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct Policy {
    groups: BTreeMap<String, Group>,
    membership: BTreeMap<String, String>,
}

struct Group {
    members: BTreeSet<String>,
    requires: BTreeSet<String>,
}

fn strings(value: &Var, limit: usize) -> Result<BTreeSet<String>, ReviewError> {
    let list = value
        .as_list()
        .ok_or_else(|| ReviewError::Invalid("expected a list of names".into()))?;
    if list.len() > limit {
        return Err("too many group entries".to_string().into());
    }
    let mut names = BTreeSet::new();
    for name in list.iter() {
        let name = name.as_string().ok_or_else(|| {
            ReviewError::Invalid("group names and member keys must be strings".into())
        })?;
        if name.is_empty() || name.len() > 256 || !names.insert(name.to_owned()) {
            return Err("empty, duplicate, or oversized group entry"
                .to_string()
                .into());
        }
    }
    Ok(names)
}

impl Policy {
    pub fn parse(input: Option<&Var>) -> Result<Self, ReviewError> {
        let Some(input) = input else {
            return Ok(Self::default());
        };
        if input.size_bytes() > 1024 * 1024 {
            return Err("change policy exceeds 1 MiB".to_string().into());
        }
        let input = fields(input, &["schema", "groups"])?;
        if required(&input, "schema")?.as_integer() != Some(1) {
            return Err("unsupported change policy schema".to_string().into());
        }
        let groups = required(&input, "groups")?
            .as_list()
            .ok_or_else(|| ReviewError::Invalid("groups must be a list".into()))?;
        if groups.len() > 32 {
            return Err("at most 32 change groups are supported".to_string().into());
        }
        let mut policy = Self::default();
        for group in groups.iter() {
            let data = fields(&group, &["name", "members", "requires"])?;
            let name = required(&data, "name")?
                .as_string()
                .ok_or_else(|| ReviewError::Invalid("group name must be a string".into()))?;
            if name.is_empty()
                || name.len() > 64
                || name.chars().any(char::is_control)
                || policy.groups.contains_key(name)
            {
                return Err("invalid or duplicate group name".to_string().into());
            }
            let members = strings(required(&data, "members")?, MAX_ROWS)?;
            if members.is_empty() {
                return Err("a change group must contain at least one member"
                    .to_string()
                    .into());
            }
            for member in &members {
                if policy
                    .membership
                    .insert(member.clone(), name.to_owned())
                    .is_some()
                {
                    return Err("a member cannot belong to two change groups"
                        .to_string()
                        .into());
                }
            }
            if policy.membership.len() > MAX_ROWS {
                return Err("too many managed group members".to_string().into());
            }
            let requires = data
                .get("requires")
                .map(|value| strings(value, 32))
                .transpose()?
                .unwrap_or_default();
            policy
                .groups
                .insert(name.to_owned(), Group { members, requires });
        }
        // A bounded traversal validates every dependency, including disconnected cycles.
        fn visit(
            name: &str,
            policy: &Policy,
            active: &mut BTreeSet<String>,
            done: &mut BTreeSet<String>,
        ) -> Result<(), ReviewError> {
            if done.contains(name) {
                return Ok(());
            }
            let group = policy
                .groups
                .get(name)
                .ok_or_else(|| ReviewError::Invalid(format!("unknown required group: {name}")))?;
            if !active.insert(name.to_owned()) {
                return Err("change groups contain a dependency cycle"
                    .to_string()
                    .into());
            }
            for required in &group.requires {
                visit(required, policy, active, done)?;
            }
            active.remove(name);
            done.insert(name.to_owned());
            Ok(())
        }
        let mut done = BTreeSet::new();
        for name in policy.groups.keys() {
            visit(name, &policy, &mut BTreeSet::new(), &mut done)?;
        }
        Ok(policy)
    }

    pub fn group_for(&self, member: &str) -> Option<&str> {
        self.membership.get(member).map(String::as_str)
    }

    /// Apply policy eligibility after all source rows exist, so missing required members
    /// cannot be hidden by a partial source bundle.
    pub fn annotate(
        &self,
        rows: &mut [ProgramRow],
        reports: &mut [Var],
        additions: bool,
    ) -> Result<(), ReviewError> {
        let present: BTreeSet<_> = rows.iter().map(|row| row.member.clone()).collect();
        for (row, report) in rows.iter_mut().zip(reports) {
            let group = self.group_for(&row.member);
            let blocker = if group.is_none() && additions {
                Some("change_group_required")
            } else if group.is_some_and(|name| !self.groups[name].members.is_subset(&present)) {
                Some("change_group_member_missing")
            } else {
                None
            };
            let map = report.as_map().unwrap();
            let mut values: Vec<_> = map.iter().collect();
            values.push((v_str("member"), v_str(&row.member)));
            if let Some(group) = group {
                values.push((v_str("group"), v_str(group)));
            }
            if let Some(blocker) = blocker {
                row.eligible = false;
                let blockers = map.get(&v_str("blockers")).unwrap();
                let mut blockers: Vec<_> = blockers.as_list().unwrap().iter().collect();
                blockers.push(v_str(blocker));
                values.retain(|(key, _)| {
                    !matches!(key.as_string(), Some("blockers" | "eligible" | "choices"))
                });
                values.extend([
                    (v_str("blockers"), v_list(&blockers)),
                    (v_str("eligible"), v_bool(false)),
                    (v_str("choices"), v_list(&[v_str("defer")])),
                ]);
            }
            if group.is_some() || blocker.is_some() {
                row.default = "defer";
                values.retain(|(key, _)| key.as_string() != Some("default"));
                values.push((v_str("default"), v_str("defer")));
            }
            *report = v_map(&values);
        }
        Ok(())
    }

    pub fn report(&self, rows: &[ProgramRow]) -> Var {
        v_list(
            &self
                .groups
                .iter()
                .map(|(name, group)| {
                    record(&[
                        ("name", v_str(name)),
                        (
                            "requires",
                            v_list(
                                &group
                                    .requires
                                    .iter()
                                    .map(|name| v_str(name))
                                    .collect::<Vec<_>>(),
                            ),
                        ),
                        (
                            "members",
                            v_list(
                                &rows
                                    .iter()
                                    .filter(|row| group.members.contains(&row.member))
                                    .map(|row| v_str(&row.id))
                                    .collect::<Vec<_>>(),
                            ),
                        ),
                        (
                            "complete",
                            v_bool(group.members.iter().all(|member| {
                                rows.iter().any(|row| &row.member == member && row.eligible)
                            })),
                        ),
                    ])
                })
                .collect::<Vec<_>>(),
        )
    }

    /// Report partial selections during review; apply rejects these before its first write.
    pub fn selection_issues(&self, analysis: &Analysis, choices: Option<&Var>) -> Vec<Var> {
        let selected: BTreeSet<_> = analysis
            .rows
            .iter()
            .filter(|row| {
                let kind = choices
                    .and_then(Var::as_map)
                    .and_then(|map| map.get(&v_str(&row.id)).ok())
                    .and_then(|value| {
                        value
                            .as_map()
                            .and_then(|map| map.get(&v_str("choice")).ok())
                    })
                    .and_then(|value| value.as_string().map(str::to_owned));
                matches!(
                    kind.as_deref().unwrap_or(row.default),
                    "incoming" | "local" | "edited"
                )
            })
            .map(|row| row.member.clone())
            .collect();
        let selected_groups: BTreeSet<_> = selected
            .iter()
            .filter_map(|member| self.group_for(member))
            .collect();
        let mut issues = Vec::new();
        for name in &selected_groups {
            let group = &self.groups[*name];
            if !group.members.is_subset(&selected) {
                issues.push(record(&[
                    ("group", v_str(name)),
                    ("code", v_str("partial_group")),
                    (
                        "message",
                        v_str(&format!(
                            "Select every required change in group {name}, or skip the whole group."
                        )),
                    ),
                ]));
            }
            for prerequisite in &group.requires {
                if !selected_groups.contains(prerequisite.as_str()) {
                    issues.push(record(&[("group", v_str(name)), ("code", v_str("missing_prerequisite")), ("message", v_str(&format!("Group {name} requires group {prerequisite}. Select it too, or skip {name}.")))]));
                }
            }
        }
        issues
    }
}
