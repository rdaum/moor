// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! Diff and sync operations between filesystem and database.
//!
//! The database is the source of truth, but users edit files locally.
//! This module provides:
//! - Comparison of verb code between file and database
//! - Comparison of object definitions between file and database
//! - Sync operations to upload/download content

// Allow dead code for utility functions intended for future use by code action commands
#![allow(dead_code)]

use std::collections::HashSet;

use moor_common::model::ObjectRef;
use moor_compiler::{CompileOptions, ObjFileContext, ObjectDefinition, compile_object_definitions};
use moor_var::Obj;
use moor_var::program::ProgramType;
use tracing::warn;

use crate::client::MoorClient;

/// Result of comparing verb code between file and database.
#[derive(Debug, Clone, Default)]
pub struct VerbDiff {
    /// Lines that only exist in the file version.
    pub file_only_lines: Vec<usize>,
    /// Lines that only exist in the database version.
    pub db_only_lines: Vec<usize>,
    /// Lines that differ: (line number, file content, db content).
    pub modified_lines: Vec<(usize, String, String)>,
    /// Whether the file and database versions are identical.
    pub is_identical: bool,
}

impl VerbDiff {
    /// Create a diff indicating identical content.
    pub fn identical() -> Self {
        Self {
            is_identical: true,
            ..Default::default()
        }
    }

    /// Create a diff indicating the verb only exists in the file.
    pub fn file_only(line_count: usize) -> Self {
        Self {
            file_only_lines: (0..line_count).collect(),
            is_identical: false,
            ..Default::default()
        }
    }

    /// Create a diff indicating the verb only exists in the database.
    pub fn db_only(line_count: usize) -> Self {
        Self {
            db_only_lines: (0..line_count).collect(),
            is_identical: false,
            ..Default::default()
        }
    }
}

/// Result of comparing an object definition between file and database.
#[derive(Debug, Clone, Default)]
pub struct ObjectDiff {
    /// Verbs that only exist in the file.
    pub verbs_only_in_file: Vec<String>,
    /// Verbs that only exist in the database.
    pub verbs_only_in_db: Vec<String>,
    /// Verbs that exist in both but have different code.
    pub verbs_that_differ: Vec<String>,
    /// Properties that only exist in the file.
    pub props_only_in_file: Vec<String>,
    /// Properties that only exist in the database.
    pub props_only_in_db: Vec<String>,
}

impl ObjectDiff {
    /// Check if there are any differences.
    pub fn has_differences(&self) -> bool {
        !self.verbs_only_in_file.is_empty()
            || !self.verbs_only_in_db.is_empty()
            || !self.verbs_that_differ.is_empty()
            || !self.props_only_in_file.is_empty()
            || !self.props_only_in_db.is_empty()
    }

    /// Get a human-readable summary of differences.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if !self.verbs_only_in_file.is_empty() {
            parts.push(format!(
                "{} verb(s) only in file",
                self.verbs_only_in_file.len()
            ));
        }
        if !self.verbs_only_in_db.is_empty() {
            parts.push(format!(
                "{} verb(s) only in database",
                self.verbs_only_in_db.len()
            ));
        }
        if !self.verbs_that_differ.is_empty() {
            parts.push(format!(
                "{} verb(s) with different code",
                self.verbs_that_differ.len()
            ));
        }
        if !self.props_only_in_file.is_empty() {
            parts.push(format!(
                "{} property(ies) only in file",
                self.props_only_in_file.len()
            ));
        }
        if !self.props_only_in_db.is_empty() {
            parts.push(format!(
                "{} property(ies) only in database",
                self.props_only_in_db.len()
            ));
        }

        if parts.is_empty() {
            "No differences".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Compare verb code between file and database versions.
///
/// Uses a simple line-by-line comparison. For more sophisticated diffing,
/// consider using a proper diff algorithm like Myers or patience diff.
pub fn compare_verb(file_code: &[String], db_code: &[String]) -> VerbDiff {
    if file_code == db_code {
        return VerbDiff::identical();
    }

    let mut diff = VerbDiff::default();
    let max_len = file_code.len().max(db_code.len());

    for i in 0..max_len {
        let file_line = file_code.get(i);
        let db_line = db_code.get(i);

        match (file_line, db_line) {
            (Some(f), Some(d)) if f != d => {
                diff.modified_lines.push((i, f.clone(), d.clone()));
            }
            (Some(_), None) => {
                diff.file_only_lines.push(i);
            }
            (None, Some(_)) => {
                diff.db_only_lines.push(i);
            }
            _ => {}
        }
    }

    diff
}

/// Compare an object definition from a file against the database.
///
/// Returns an `ObjectDiff` showing which verbs and properties differ.
pub async fn compare_object(
    file_def: &ObjectDefinition,
    client: &mut MoorClient,
    obj: Obj,
) -> ObjectDiff {
    let mut diff = ObjectDiff::default();
    let obj_ref = ObjectRef::Id(obj);

    // Get verbs from database
    let db_verbs = match client.list_verbs(&obj_ref, false).await {
        Ok(verbs) => verbs,
        Err(e) => {
            warn!("Failed to list verbs from database: {}", e);
            return diff;
        }
    };

    // Build set of verb names from file
    let file_verb_names: HashSet<String> = file_def
        .verbs
        .iter()
        .flat_map(|v| v.names.iter().map(|s| s.to_string()))
        .collect();

    // Build set of verb names from database (first name only for matching)
    let db_verb_names: HashSet<String> = db_verbs
        .iter()
        .filter_map(|v| v.name.split_whitespace().next().map(String::from))
        .collect();

    // Find verbs only in file
    for name in &file_verb_names {
        if !db_verb_names.contains(name) {
            diff.verbs_only_in_file.push(name.clone());
        }
    }

    // Find verbs only in database
    for name in &db_verb_names {
        if !file_verb_names.contains(name) {
            diff.verbs_only_in_db.push(name.clone());
        }
    }

    // Compare verbs that exist in both
    for file_verb in &file_def.verbs {
        let verb_name = match file_verb.names.first() {
            Some(name) => name.to_string(),
            None => continue,
        };

        if !db_verb_names.contains(&verb_name) {
            continue;
        }

        // Get verb code from database
        let db_code = match client.get_verb(&obj_ref, &verb_name).await {
            Ok(code) => code,
            Err(e) => {
                warn!("Failed to get verb {} from database: {}", verb_name, e);
                continue;
            }
        };

        // Get file verb code (decompiled if necessary)
        let file_code = get_verb_code_from_definition(file_verb);

        // Compare
        let verb_diff = compare_verb(&file_code, &db_code.code);
        if !verb_diff.is_identical {
            diff.verbs_that_differ.push(verb_name);
        }
    }

    // Get properties from database
    let db_props = match client.list_properties(&obj_ref, false).await {
        Ok(props) => props,
        Err(e) => {
            warn!("Failed to list properties from database: {}", e);
            return diff;
        }
    };

    // Build set of property names from file (definitions + overrides)
    let file_prop_names: HashSet<String> = file_def
        .property_definitions
        .iter()
        .map(|p| p.name.to_string())
        .chain(
            file_def
                .property_overrides
                .iter()
                .map(|p| p.name.to_string()),
        )
        .collect();

    // Build set of property names from database
    let db_prop_names: HashSet<String> = db_props.iter().map(|p| p.name.clone()).collect();

    // Find properties only in file
    for name in &file_prop_names {
        if !db_prop_names.contains(name) {
            diff.props_only_in_file.push(name.clone());
        }
    }

    // Find properties only in database
    for name in &db_prop_names {
        if !file_prop_names.contains(name) {
            diff.props_only_in_db.push(name.clone());
        }
    }

    diff
}

/// Extract verb code lines from an object verb definition.
fn get_verb_code_from_definition(verb: &moor_compiler::ObjVerbDef) -> Vec<String> {
    use moor_compiler::{program_to_tree, unparse};

    match &verb.program {
        ProgramType::MooR(program) => {
            // First decompile the program to an AST, then unparse to source
            let Ok(tree) = program_to_tree(program) else {
                return Vec::new();
            };
            unparse(&tree, false, true).unwrap_or_default()
        }
    }
}

/// Parse a .moo file and extract object definitions.
/// Uses a fresh context - prefer `parse_object_definitions_with_context` when constants are available.
pub fn parse_object_definitions(source: &str) -> Option<Vec<ObjectDefinition>> {
    parse_object_definitions_with_context(source, &ObjFileContext::default())
}

/// Parse a .moo file using the provided context (with loaded constants).
pub fn parse_object_definitions_with_context(
    source: &str,
    context: &ObjFileContext,
) -> Option<Vec<ObjectDefinition>> {
    let options = CompileOptions::default();
    let mut local_context = context.clone();

    compile_object_definitions(source, &options, &mut local_context).ok()
}

/// Sync direction for upload/download operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    /// Upload file content to database.
    Upload,
    /// Download database content to file.
    Download,
}

/// Result of a sync operation.
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Human-readable message about what happened.
    pub message: String,
}

impl SyncResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
        }
    }
}

/// Upload a verb's code from file to database.
pub async fn upload_verb(
    client: &mut MoorClient,
    obj: Obj,
    verb_name: &str,
    code: Vec<String>,
) -> SyncResult {
    let obj_ref = ObjectRef::Id(obj);

    match client.program_verb(&obj_ref, verb_name, code).await {
        Ok(()) => SyncResult::success(format!("Uploaded verb '{}' to database", verb_name)),
        Err(e) => SyncResult::failure(format!("Failed to upload verb '{}': {}", verb_name, e)),
    }
}

/// Get verb code from the database (for download operation).
pub async fn download_verb(
    client: &mut MoorClient,
    obj: Obj,
    verb_name: &str,
) -> Result<Vec<String>, String> {
    let obj_ref = ObjectRef::Id(obj);

    match client.get_verb(&obj_ref, verb_name).await {
        Ok(verb_code) => Ok(verb_code.code),
        Err(e) => Err(format!("Failed to download verb '{}': {}", verb_name, e)),
    }
}

/// Information about a sync difference for diagnostics.
#[derive(Debug, Clone)]
pub struct SyncDiagnosticInfo {
    /// The object ID.
    pub obj_id: Obj,
    /// The object name.
    pub obj_name: String,
    /// Summary of differences.
    pub summary: String,
    /// Start line of the object in the file (0-based).
    pub start_line: u32,
    /// End line of the object in the file (0-based).
    pub end_line: u32,
}

/// Check a file against the database and return diagnostic info for out-of-sync objects.
/// Uses the provided context (with loaded constants) for parsing.
pub async fn check_sync_status(
    source: &str,
    client: &mut MoorClient,
    context: &ObjFileContext,
) -> Vec<SyncDiagnosticInfo> {
    let Some(object_defs) = parse_object_definitions_with_context(source, context) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();

    // Estimate line numbers for each object (simple approach - count "object" and "endobject" keywords)
    let lines: Vec<&str> = source.lines().collect();
    let mut current_obj_start: Option<u32> = None;
    let mut obj_line_ranges: Vec<(u32, u32)> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim().to_lowercase();
        if trimmed.starts_with("object ") || trimmed.starts_with("object\t") {
            current_obj_start = Some(i as u32);
        } else if (trimmed.starts_with("endobject") || trimmed == "endobject")
            && let Some(start) = current_obj_start.take()
        {
            obj_line_ranges.push((start, i as u32));
        }
    }

    for (idx, obj_def) in object_defs.iter().enumerate() {
        let (start_line, end_line) = obj_line_ranges.get(idx).copied().unwrap_or((0, 0));

        let diff = compare_object(obj_def, client, obj_def.oid).await;

        if diff.has_differences() {
            diagnostics.push(SyncDiagnosticInfo {
                obj_id: obj_def.oid,
                obj_name: obj_def.name.clone(),
                summary: diff.summary(),
                start_line,
                end_line,
            });
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_verb_identical() {
        let code = vec!["return 1;".to_string()];
        let diff = compare_verb(&code, &code);
        assert!(diff.is_identical);
        assert!(diff.file_only_lines.is_empty());
        assert!(diff.db_only_lines.is_empty());
        assert!(diff.modified_lines.is_empty());
    }

    #[test]
    fn test_compare_verb_modified() {
        let file_code = vec!["return 1;".to_string()];
        let db_code = vec!["return 2;".to_string()];
        let diff = compare_verb(&file_code, &db_code);
        assert!(!diff.is_identical);
        assert!(diff.file_only_lines.is_empty());
        assert!(diff.db_only_lines.is_empty());
        assert_eq!(diff.modified_lines.len(), 1);
        assert_eq!(diff.modified_lines[0].0, 0);
    }

    #[test]
    fn test_compare_verb_file_longer() {
        let file_code = vec!["line 1".to_string(), "line 2".to_string()];
        let db_code = vec!["line 1".to_string()];
        let diff = compare_verb(&file_code, &db_code);
        assert!(!diff.is_identical);
        assert_eq!(diff.file_only_lines, vec![1]);
        assert!(diff.db_only_lines.is_empty());
    }

    #[test]
    fn test_compare_verb_db_longer() {
        let file_code = vec!["line 1".to_string()];
        let db_code = vec!["line 1".to_string(), "line 2".to_string()];
        let diff = compare_verb(&file_code, &db_code);
        assert!(!diff.is_identical);
        assert!(diff.file_only_lines.is_empty());
        assert_eq!(diff.db_only_lines, vec![1]);
    }

    #[test]
    fn test_object_diff_has_differences() {
        let diff = ObjectDiff::default();
        assert!(!diff.has_differences());

        let diff = ObjectDiff {
            verbs_only_in_file: vec!["test".to_string()],
            ..Default::default()
        };
        assert!(diff.has_differences());
    }

    #[test]
    fn test_object_diff_summary() {
        let diff = ObjectDiff::default();
        assert_eq!(diff.summary(), "No differences");

        let diff = ObjectDiff {
            verbs_only_in_file: vec!["v1".to_string()],
            verbs_that_differ: vec!["v2".to_string(), "v3".to_string()],
            ..Default::default()
        };
        let summary = diff.summary();
        assert!(summary.contains("1 verb(s) only in file"));
        assert!(summary.contains("2 verb(s) with different code"));
    }
}
