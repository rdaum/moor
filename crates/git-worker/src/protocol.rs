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

//! Versioned MOO request and response values. No backend types cross this boundary.

use moor_var::{Var, v_bool, v_int, v_list, v_map, v_str};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Operator ceilings. Requests may lower the three result limits.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub max_entries: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
    pub max_fetch_bytes: usize,
    pub max_memory_bytes: usize,
    pub max_seconds: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 4096,
            max_file_bytes: 4 * 1024 * 1024,
            max_total_bytes: 16 * 1024 * 1024,
            max_fetch_bytes: 256 * 1024 * 1024,
            max_memory_bytes: 1024 * 1024 * 1024,
            max_seconds: 30,
        }
    }
}
impl Limits {
    /// Reject unusable ceilings before accepting work.
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.max_entries == 0
            || self.max_entries > 100_000
            || self.max_file_bytes == 0
            || self.max_file_bytes > 64 * 1024 * 1024
            || self.max_total_bytes == 0
            || self.max_total_bytes > 64 * 1024 * 1024
            || self.max_fetch_bytes == 0
            || self.max_fetch_bytes > 4 * 1024 * 1024 * 1024usize
            || self.max_memory_bytes < 256 * 1024 * 1024
            || self.max_memory_bytes > 64 * 1024 * 1024 * 1024usize
            || self.max_seconds == 0
            || self.max_seconds > 86400
        {
            return Err("Invalid worker resource limits".into());
        }
        Ok(())
    }
    pub fn value(&self) -> Var {
        map(&[
            ("max_entries", v_int(self.max_entries as i64)),
            ("max_file_bytes", v_int(self.max_file_bytes as i64)),
            ("max_total_bytes", v_int(self.max_total_bytes as i64)),
            ("max_fetch_bytes", v_int(self.max_fetch_bytes as i64)),
            ("max_memory_bytes", v_int(self.max_memory_bytes as i64)),
            ("max_seconds", v_int(self.max_seconds as i64)),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Operation {
    Capabilities,
    Refs,
    Tree,
    Read,
    Snapshot,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Revision {
    Ref(String),
    Commit(String),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub operation: Operation,
    pub repository: String,
    pub revision: Option<Revision>,
    pub path: String,
    pub recursive: bool,
    pub limits: Limits,
}

/// A stable application error. Backend diagnostics are not exposed to callers.
#[derive(Debug)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
}
impl Error {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn value(&self) -> Var {
        map(&[
            ("schema", v_int(1)),
            ("ok", v_bool(false)),
            (
                "error",
                map(&[
                    ("code", v_str(self.code)),
                    ("message", v_str(&self.message)),
                ]),
            ),
        ])
    }
}
pub type Result<T> = std::result::Result<T, Error>;
pub fn map(fields: &[(&str, Var)]) -> Var {
    v_map(
        &fields
            .iter()
            .map(|(k, v)| (v_str(k), v.clone()))
            .collect::<Vec<_>>(),
    )
}
pub fn success(result: Var) -> Var {
    map(&[
        ("schema", v_int(1)),
        ("ok", v_bool(true)),
        ("result", result),
    ])
}
fn invalid(message: impl Into<String>) -> Error {
    Error::new("invalid_request", message)
}
fn fields(value: &Var, allowed: &[&str]) -> Result<BTreeMap<String, Var>> {
    let values = value.as_map().ok_or_else(|| invalid("Expected a map"))?;
    let mut result = BTreeMap::new();
    for (key, value) in values.iter() {
        let key = key
            .as_symbol()
            .map_err(|_| invalid("Map keys must be strings or symbols"))?
            .to_folded_case();
        if !allowed.contains(&key.as_str()) || result.insert(key, value).is_some() {
            return Err(invalid("Unknown or duplicate field"));
        }
    }
    Ok(result)
}
fn string(value: Option<&Var>, name: &str) -> Result<String> {
    let value = value
        .and_then(Var::as_string)
        .ok_or_else(|| invalid(format!("{name} must be a string")))?;
    if value.len() > 4096 || value.contains('\0') {
        return Err(invalid(format!("Invalid {name}")));
    }
    Ok(value.to_owned())
}
/// Parse and validate before opening a repository or making a network request.
pub fn parse(args: &[Var], limits: &Limits) -> Result<Request> {
    if args.len() != 2 {
        return Err(invalid("Expected operation and request map"));
    }
    let op = args[0]
        .as_symbol()
        .map_err(|_| invalid("Operation must be a string or symbol"))?
        .to_folded_case();
    let operation = match op.as_str() {
        "capabilities" => Operation::Capabilities,
        "refs" => Operation::Refs,
        "tree" => Operation::Tree,
        "read" => Operation::Read,
        "snapshot" => Operation::Snapshot,
        _ => return Err(Error::new("unsupported_operation", "Unknown Git operation")),
    };
    let allowed: &[&str] = match operation {
        Operation::Capabilities => &["schema"],
        Operation::Refs => &["schema", "repository", "limits"],
        Operation::Tree => &[
            "schema",
            "repository",
            "revision",
            "path",
            "recursive",
            "limits",
        ],
        _ => &["schema", "repository", "revision", "path", "limits"],
    };
    let values = fields(&args[1], allowed)?;
    if values.get("schema").and_then(Var::as_integer) != Some(1) {
        return Err(Error::new("unsupported_schema", "Expected schema 1"));
    }
    let mut request = Request {
        operation,
        repository: String::new(),
        revision: None,
        path: String::new(),
        recursive: operation == Operation::Snapshot,
        limits: limits.clone(),
    };
    if operation == Operation::Capabilities {
        return Ok(request);
    }
    request.repository = string(values.get("repository"), "repository")?;
    let url = reqwest::Url::parse(&request.repository)
        .map_err(|_| invalid("Repository must be an absolute HTTP(S) URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::new(
            "unsupported_transport",
            "Only HTTP and HTTPS repositories are supported",
        ));
    }
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid(
            "Repository URL must not contain credentials, query, or fragment",
        ));
    }
    request.repository = url.to_string();
    if !matches!(operation, Operation::Refs) {
        let revision = fields(
            values
                .get("revision")
                .ok_or_else(|| invalid("Missing revision"))?,
            &["ref", "commit"],
        )?;
        if revision.len() != 1 {
            return Err(invalid("Revision must contain exactly one ref or commit"));
        }
        request.revision = if revision.contains_key("ref") {
            let name = string(revision.get("ref"), "ref")?;
            if !name.starts_with("refs/")
                || gix::validate::reference::name(name.as_str().into()).is_err()
            {
                return Err(invalid("Expected a full Git ref name"));
            }
            Some(Revision::Ref(name))
        } else {
            let oid = string(revision.get("commit"), "commit")?;
            let hex = oid
                .strip_prefix("sha1:")
                .ok_or_else(|| invalid("Expected a sha1: commit ID"))?;
            if hex.len() != 40 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(invalid("Expected a full commit ID"));
            }
            Some(Revision::Commit(hex.to_ascii_lowercase()))
        };
        if let Some(path) = values.get("path") {
            request.path = string(Some(path), "path")?;
        }
        if !request.path.is_empty()
            && request
                .path
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == ".." || p.contains('\\'))
        {
            return Err(invalid(
                "Path must contain relative, nonempty components without dot segments",
            ));
        }
        if operation == Operation::Read && request.path.is_empty() {
            return Err(invalid("Read requires a file path"));
        }
    }
    if let Some(recursive) = values.get("recursive") {
        if !matches!(recursive.variant(), moor_var::Variant::Bool(_)) {
            return Err(invalid("recursive must be a boolean"));
        }
        request.recursive = recursive.is_true();
    }
    if let Some(overrides) = values.get("limits") {
        let overrides = fields(
            overrides,
            &["max_entries", "max_file_bytes", "max_total_bytes"],
        )?;
        for (name, target) in [
            ("max_entries", &mut request.limits.max_entries),
            ("max_file_bytes", &mut request.limits.max_file_bytes),
            ("max_total_bytes", &mut request.limits.max_total_bytes),
        ] {
            if let Some(value) = overrides.get(name) {
                let value = value
                    .as_integer()
                    .filter(|v| *v > 0 && *v as u64 <= *target as u64)
                    .ok_or_else(|| {
                        invalid(format!(
                            "{name} must be positive and within the operator limit"
                        ))
                    })?;
                *target = value as usize;
            }
        }
    }
    Ok(request)
}

pub fn capabilities(limits: &Limits, concurrency: u32) -> Var {
    success(map(&[
        (
            "operations",
            v_list(&[
                v_str("capabilities"),
                v_str("refs"),
                v_str("tree"),
                v_str("read"),
                v_str("snapshot"),
            ]),
        ),
        ("transports", v_list(&[v_str("https"), v_str("http")])),
        ("object_formats", v_list(&[v_str("sha1")])),
        ("limits", limits.value()),
        ("max_concurrent_requests", v_int(concurrency as i64)),
        ("content_type", v_str("binary")),
        ("path_encoding", v_str("utf-8")),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::{Symbol, v_sym};
    fn request(extra: &[(&str, Var)]) -> Vec<Var> {
        let mut values = vec![
            ("schema", v_int(1)),
            ("repository", v_str("https://example.org/repo.git")),
            ("revision", map(&[("ref", v_str("refs/heads/main"))])),
        ];
        values.extend_from_slice(extra);
        vec![v_str("snapshot"), map(&values)]
    }
    #[test]
    fn rejects_ambiguous_and_unknown_input_before_network_access() {
        let limits = Limits::default();
        for args in [
            vec![],
            vec![v_str("snapshot")],
            vec![v_str("push"), map(&[("schema", v_int(1))])],
            request(&[("unknown", v_bool(true))]),
            request(&[("path", v_str("../private"))]),
            request(&[("path", v_str("/absolute"))]),
            request(&[("path", v_str("a//b"))]),
            request(&[("limits", map(&[("max_entries", v_int(0))]))]),
            request(&[("limits", map(&[("max_entries", v_int(999999))]))]),
            vec![
                v_str("read"),
                map(&[
                    ("schema", v_int(1)),
                    ("repository", v_str("https://example.org/repo")),
                    ("revision", map(&[("ref", v_str("main"))])),
                ]),
            ],
        ] {
            assert!(parse(&args, &limits).is_err(), "{args:?}");
        }
        let duplicate = v_map(&[
            (v_str("schema"), v_int(1)),
            (v_sym(Symbol::mk("schema")), v_int(1)),
        ]);
        assert!(parse(&[v_str("capabilities"), duplicate], &limits).is_err());
    }
    #[test]
    fn public_urls_only_and_no_credentials_in_requests() {
        for url in [
            "file:///etc",
            "ssh://example.org/repo",
            "git://example.org/repo",
            "ext::command",
            "https://user:secret@example.org/repo",
            "https://example.org/repo?token=x",
            "https://example.org/repo#main",
        ] {
            assert!(
                parse(
                    &[
                        v_str("refs"),
                        map(&[("schema", v_int(1)), ("repository", v_str(url))])
                    ],
                    &Limits::default()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn validates_schema_and_operation_specific_fields() {
        assert!(
            parse(
                &[v_str("capabilities"), map(&[("schema", v_int(2))])],
                &Limits::default()
            )
            .is_err()
        );
        assert!(
            parse(
                &[
                    v_str("capabilities"),
                    map(&[("schema", v_int(1)), ("repository", v_str("unused"))])
                ],
                &Limits::default()
            )
            .is_err()
        );
        let request = parse(
            &request(&[("limits", map(&[("max_total_bytes", v_int(16))]))]),
            &Limits::default(),
        )
        .unwrap();
        assert_eq!(request.limits.max_total_bytes, 16);
        assert!(request.recursive);
    }
}
