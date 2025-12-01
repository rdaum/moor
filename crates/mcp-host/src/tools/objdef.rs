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

//! Object definition (objdef) tools: dump, load, reload, file operations, diff

use crate::mcp_types::{Tool, ToolCallResult};
use crate::moor_client::{MoorClient, MoorResult};
use eyre::Result;
use serde_json::{Value, json};

use super::helpers::format_var;

// ============================================================================
// Tool Definitions
// ============================================================================

pub fn tool_moo_dump_object() -> Tool {
    Tool {
        name: "moo_dump_object".to_string(),
        description: "Dump an object to objdef format (a text representation of the object's \
            definition including properties and verbs). Returns the objdef as a string. \
            Requires wizard permissions."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference to dump (e.g., '#123', '$thing')"
                }
            },
            "required": ["object"]
        }),
    }
}

pub fn tool_moo_load_object() -> Tool {
    Tool {
        name: "moo_load_object".to_string(),
        description: "Load an object from objdef format. Creates a new object with the \
            properties and verbs defined in the objdef text. Requires wizard permissions. \
            Use object_spec to control how the object ID is assigned: 0=auto (next available), \
            2=UUID format, or a specific object reference like '#123' to use that exact ID."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "objdef": {
                    "type": "string",
                    "description": "Object definition in objdef format (multi-line text)"
                },
                "object_spec": {
                    "type": "string",
                    "description": "How to assign the object ID: '0' for next available, '2' for UUID, or '#N' for specific ID (default: use ID from objdef)"
                },
                "auto_constants": {
                    "type": "boolean",
                    "description": "Automatically build constants map from objects with import_export_id property. This allows symbolic names like HENRI, ACTOR, etc. in objdef files to be resolved. Defaults to true.",
                    "default": true
                }
            },
            "required": ["objdef"]
        }),
    }
}

pub fn tool_moo_reload_object() -> Tool {
    Tool {
        name: "moo_reload_object".to_string(),
        description: "Reload an existing object from objdef format. Completely replaces the \
            object's properties and verbs with those defined in the objdef, removing any \
            properties/verbs not in the new definition. Requires wizard permissions."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "objdef": {
                    "type": "string",
                    "description": "Object definition in objdef format (multi-line text)"
                },
                "target": {
                    "type": "string",
                    "description": "Target object to reload (e.g., '#123'). If not specified, uses the object ID from the objdef."
                },
                "auto_constants": {
                    "type": "boolean",
                    "description": "Automatically build constants map from objects with import_export_id property. This allows symbolic names like HENRI, ACTOR, etc. in objdef files to be resolved. Defaults to true.",
                    "default": true
                }
            },
            "required": ["objdef"]
        }),
    }
}

pub fn tool_moo_read_objdef_file() -> Tool {
    Tool {
        name: "moo_read_objdef_file".to_string(),
        description:
            "Read an objdef file from the filesystem. Objdef files (typically .moo files) \
            contain object definitions in text format. Common locations include cores/cowbell/src/."
                .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the objdef file (e.g., 'cores/cowbell/src/root.moo')"
                }
            },
            "required": ["path"]
        }),
    }
}

pub fn tool_moo_write_objdef_file() -> Tool {
    Tool {
        name: "moo_write_objdef_file".to_string(),
        description: "Write an objdef file to the filesystem. Creates or overwrites an objdef \
            file with the provided content."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to write the objdef file (e.g., 'cores/cowbell/src/myobject.moo')"
                },
                "content": {
                    "type": "string",
                    "description": "Objdef content to write"
                }
            },
            "required": ["path", "content"]
        }),
    }
}

pub fn tool_moo_load_objdef_file() -> Tool {
    Tool {
        name: "moo_load_objdef_file".to_string(),
        description: "Load an object into the MOO database from an objdef file on the filesystem. \
            This is a convenience that combines reading an objdef file and calling load_object. \
            Use object_spec to control ID assignment."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the objdef file (e.g., 'cores/cowbell/src/myobject.moo')"
                },
                "object_spec": {
                    "type": "string",
                    "description": "How to assign the object ID: '0' for next available, '2' for UUID, or '#N' for specific ID"
                },
                "auto_constants": {
                    "type": "boolean",
                    "description": "Automatically build constants map from objects with import_export_id property. This allows symbolic names like HENRI, ACTOR, etc. in objdef files to be resolved. Defaults to true.",
                    "default": true
                }
            },
            "required": ["path"]
        }),
    }
}

pub fn tool_moo_reload_objdef_file() -> Tool {
    Tool {
        name: "moo_reload_objdef_file".to_string(),
        description: "Reload an existing object from an objdef file on the filesystem. \
            Reads the objdef from the file and reloads the target object with that definition."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the objdef file (e.g., 'cores/cowbell/src/root.moo')"
                },
                "target": {
                    "type": "string",
                    "description": "Target object to reload (e.g., '#123'). If not specified, uses the object ID from the objdef."
                },
                "auto_constants": {
                    "type": "boolean",
                    "description": "Automatically build constants map from objects with import_export_id property. This allows symbolic names like HENRI, ACTOR, etc. in objdef files to be resolved. Defaults to true.",
                    "default": true
                }
            },
            "required": ["path"]
        }),
    }
}

pub fn tool_moo_diff_object() -> Tool {
    Tool {
        name: "moo_diff_object".to_string(),
        description: "Compare an object in the database with an objdef file or text to show differences. \
            Useful for seeing what changes would occur before reloading, or for identifying divergence \
            between the database and source files."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object to compare (e.g., '#123', '$thing')"
                },
                "path": {
                    "type": "string",
                    "description": "Path to objdef file to compare against"
                },
                "objdef": {
                    "type": "string",
                    "description": "Objdef text to compare against (alternative to path)"
                }
            },
            "required": ["object"]
        }),
    }
}

// ============================================================================
// Tool Implementations
// ============================================================================

pub async fn execute_moo_dump_object(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    // Build the MOO expression: dump_object(obj)
    // dump_object returns a list of strings, we need to join them
    let expr = format!("return dump_object({});", object_str);

    match client.eval(&expr).await? {
        MoorResult::Success(var) => {
            // The result is a list of strings, join them with newlines
            if let Some(list) = var.as_list() {
                let lines: Vec<String> = list
                    .iter()
                    .filter_map(|v| v.as_string().map(|s| s.to_string()))
                    .collect();
                let objdef = lines.join("\n");
                Ok(ToolCallResult::text(format!(
                    "Object {} dumped ({} lines):\n\n{}",
                    object_str,
                    lines.len(),
                    objdef
                )))
            } else {
                Ok(ToolCallResult::text(format_var(&var)))
            }
        }
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_load_object(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let objdef = args
        .get("objdef")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'objdef' parameter"))?;

    let object_spec = args.get("object_spec").and_then(|v| v.as_str());

    let auto_constants = args
        .get("auto_constants")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Convert objdef text to a list of strings for the builtin
    let lines: Vec<&str> = objdef.lines().collect();
    let lines_literal = format!(
        "{{{}}}",
        lines
            .iter()
            .map(|l| format!("\"{}\"", l.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Build the MOO expression
    // If auto_constants is enabled, build constants map from import_export_id properties
    let expr = if auto_constants {
        let constants_builder = r#"constants = []; for o in (objects()) id = `o.import_export_id ! E_PROPNF => 0'; if (typeof(id) == STR && id != "") constants[id:uppercase()] = o; endif endfor"#;
        if let Some(spec) = object_spec {
            format!(
                "{} return load_object({}, constants, {});",
                constants_builder, lines_literal, spec
            )
        } else {
            format!(
                "{} return load_object({}, constants);",
                constants_builder, lines_literal
            )
        }
    } else if let Some(spec) = object_spec {
        format!("return load_object({}, [], {});", lines_literal, spec)
    } else {
        format!("return load_object({});", lines_literal)
    };

    match client.eval(&expr).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format!(
            "Successfully loaded object: {}",
            format_var(&var)
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_reload_object(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let objdef = args
        .get("objdef")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'objdef' parameter"))?;

    let target = args.get("target").and_then(|v| v.as_str());

    let auto_constants = args
        .get("auto_constants")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Convert objdef text to a list of strings for the builtin
    let lines: Vec<&str> = objdef.lines().collect();
    let lines_literal = format!(
        "{{{}}}",
        lines
            .iter()
            .map(|l| format!("\"{}\"", l.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Build the MOO expression: reload_object(lines [, constants] [, target])
    // If auto_constants is enabled, build constants map from import_export_id properties
    let expr = if auto_constants {
        let constants_builder = r#"constants = []; for o in (objects()) id = `o.import_export_id ! E_PROPNF => 0'; if (typeof(id) == STR && id != "") constants[id:uppercase()] = o; endif endfor"#;
        if let Some(target_obj) = target {
            format!(
                "{} return reload_object({}, constants, {});",
                constants_builder, lines_literal, target_obj
            )
        } else {
            format!(
                "{} return reload_object({}, constants);",
                constants_builder, lines_literal
            )
        }
    } else if let Some(target_obj) = target {
        format!(
            "return reload_object({}, [], {});",
            lines_literal, target_obj
        )
    } else {
        format!("return reload_object({});", lines_literal)
    };

    match client.eval(&expr).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format!(
            "Successfully reloaded object: {}",
            format_var(&var)
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_read_objdef_file(
    _client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'path' parameter"))?;

    // Read the file from the filesystem
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let line_count = content.lines().count();
            Ok(ToolCallResult::text(format!(
                "Objdef file '{}' ({} lines):\n\n{}",
                path, line_count, content
            )))
        }
        Err(e) => Ok(ToolCallResult::error(format!(
            "Failed to read file '{}': {}",
            path, e
        ))),
    }
}

pub async fn execute_moo_write_objdef_file(
    _client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'path' parameter"))?;

    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'content' parameter"))?;

    // Write the file to the filesystem
    match std::fs::write(path, content) {
        Ok(()) => {
            let line_count = content.lines().count();
            Ok(ToolCallResult::text(format!(
                "Successfully wrote '{}' ({} lines)",
                path, line_count
            )))
        }
        Err(e) => Ok(ToolCallResult::error(format!(
            "Failed to write file '{}': {}",
            path, e
        ))),
    }
}

pub async fn execute_moo_load_objdef_file(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'path' parameter"))?;

    let object_spec = args.get("object_spec").and_then(|v| v.as_str());

    let auto_constants = args
        .get("auto_constants")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Read the file from the filesystem
    let objdef = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            return Ok(ToolCallResult::error(format!(
                "Failed to read file '{}': {}",
                path, e
            )));
        }
    };

    // Convert objdef text to a list of strings for the builtin
    let lines: Vec<&str> = objdef.lines().collect();
    let lines_literal = format!(
        "{{{}}}",
        lines
            .iter()
            .map(|l| format!("\"{}\"", l.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Build the MOO expression
    // If auto_constants is enabled, build constants map from import_export_id properties
    let expr = if auto_constants {
        let constants_builder = r#"constants = []; for o in (objects()) id = `o.import_export_id ! E_PROPNF => 0'; if (typeof(id) == STR && id != "") constants[id:uppercase()] = o; endif endfor"#;
        if let Some(spec) = object_spec {
            format!(
                "{} return load_object({}, constants, {});",
                constants_builder, lines_literal, spec
            )
        } else {
            format!(
                "{} return load_object({}, constants);",
                constants_builder, lines_literal
            )
        }
    } else if let Some(spec) = object_spec {
        format!("return load_object({}, [], {});", lines_literal, spec)
    } else {
        format!("return load_object({});", lines_literal)
    };

    match client.eval(&expr).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format!(
            "Successfully loaded object from '{}': {}",
            path,
            format_var(&var)
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_reload_objdef_file(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'path' parameter"))?;

    let target = args.get("target").and_then(|v| v.as_str());

    let auto_constants = args
        .get("auto_constants")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Read the file from the filesystem
    let objdef = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            return Ok(ToolCallResult::error(format!(
                "Failed to read file '{}': {}",
                path, e
            )));
        }
    };

    // Convert objdef text to a list of strings for the builtin
    let lines: Vec<&str> = objdef.lines().collect();
    let lines_literal = format!(
        "{{{}}}",
        lines
            .iter()
            .map(|l| format!("\"{}\"", l.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Build the MOO expression: reload_object(lines [, constants] [, target])
    // If auto_constants is enabled, build constants map from import_export_id properties
    let expr = if auto_constants {
        let constants_builder = r#"constants = []; for o in (objects()) id = `o.import_export_id ! E_PROPNF => 0'; if (typeof(id) == STR && id != "") constants[id:uppercase()] = o; endif endfor"#;
        if let Some(target_obj) = target {
            format!(
                "{} return reload_object({}, constants, {});",
                constants_builder, lines_literal, target_obj
            )
        } else {
            format!(
                "{} return reload_object({}, constants);",
                constants_builder, lines_literal
            )
        }
    } else if let Some(target_obj) = target {
        format!(
            "return reload_object({}, [], {});",
            lines_literal, target_obj
        )
    } else {
        format!("return reload_object({});", lines_literal)
    };

    match client.eval(&expr).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format!(
            "Successfully reloaded object from '{}': {}",
            path,
            format_var(&var)
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_diff_object(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let path = args.get("path").and_then(|v| v.as_str());
    let objdef_text = args.get("objdef").and_then(|v| v.as_str());

    // Get the comparison objdef - either from file or text
    let compare_objdef = if let Some(p) = path {
        match std::fs::read_to_string(p) {
            Ok(content) => content,
            Err(e) => {
                return Ok(ToolCallResult::error(format!(
                    "Failed to read file '{}': {}",
                    p, e
                )));
            }
        }
    } else if let Some(text) = objdef_text {
        text.to_string()
    } else {
        return Ok(ToolCallResult::error(
            "Must provide either 'path' or 'objdef' parameter",
        ));
    };

    // Get current object definition from database
    let expr = format!("return dump_object({});", object_str);
    let current_objdef = match client.eval(&expr).await? {
        MoorResult::Success(var) => {
            if let Some(list) = var.as_list() {
                let lines: Vec<String> = list
                    .iter()
                    .filter_map(|v| v.as_string().map(|s| s.to_string()))
                    .collect();
                lines.join("\n")
            } else {
                return Ok(ToolCallResult::error("Failed to dump object"));
            }
        }
        MoorResult::Error(msg) => return Ok(ToolCallResult::error(msg)),
    };

    // Simple line-by-line diff
    let current_lines: Vec<&str> = current_objdef.lines().collect();
    let compare_lines: Vec<&str> = compare_objdef.lines().collect();

    let mut output = String::new();
    output.push_str(&format!(
        "Diff for {} vs {}\n\n",
        object_str,
        path.unwrap_or("provided objdef")
    ));

    // Track differences
    let mut differences = Vec::new();
    let max_lines = current_lines.len().max(compare_lines.len());

    for i in 0..max_lines {
        let current = current_lines.get(i).copied();
        let compare = compare_lines.get(i).copied();

        match (current, compare) {
            (Some(c), Some(cmp)) if c != cmp => {
                differences.push(format!(
                    "Line {}: database has:\n  {}\nFile has:\n  {}",
                    i + 1,
                    c,
                    cmp
                ));
            }
            (Some(c), None) => {
                differences.push(format!("Line {}: only in database:\n  {}", i + 1, c));
            }
            (None, Some(cmp)) => {
                differences.push(format!("Line {}: only in file:\n  {}", i + 1, cmp));
            }
            _ => {}
        }
    }

    if differences.is_empty() {
        output.push_str("No differences found - objects are identical.\n");
    } else {
        output.push_str(&format!("Found {} differences:\n\n", differences.len()));
        for diff in differences {
            output.push_str(&diff);
            output.push_str("\n\n");
        }
    }

    Ok(ToolCallResult::text(output))
}
