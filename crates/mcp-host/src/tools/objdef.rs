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
use moor_compiler::{CompileOptions, ObjFileContext, compile_object_definitions, to_literal};
use serde_json::{Value, json};

use super::helpers::format_var;

/// Check if a path looks like a URL (http:// or https://)
/// RFC 3986 specifies schemes are case-insensitive
fn is_url(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Maximum content size for URL fetches (10 MB)
const MAX_URL_CONTENT_SIZE: u64 = 10 * 1024 * 1024;

/// Read content from either a local file path or a URL.
/// Returns the content as a string, or an error message.
async fn read_file_or_url(path: &str) -> std::result::Result<String, String> {
    if is_url(path) {
        // Fetch from URL with timeouts
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        match client.get(path).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    return Err(format!(
                        "HTTP error fetching '{}': {}",
                        path,
                        response.status()
                    ));
                }

                // Check content length if provided
                if let Some(content_length) = response.content_length() {
                    if content_length > MAX_URL_CONTENT_SIZE {
                        return Err(format!(
                            "Content too large: {} bytes (max {} bytes)",
                            content_length, MAX_URL_CONTENT_SIZE
                        ));
                    }
                }

                response
                    .text()
                    .await
                    .map_err(|e| format!("Failed to read response from '{}': {}", path, e))
            }
            Err(e) => Err(format!("Failed to fetch '{}': {}", path, e)),
        }
    } else {
        // Read from local file
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read file '{}': {}", path, e))
    }
}

/// Parse a constants file locally and return a MOO map literal string.
/// Uses the objdef compiler to parse `define NAME = value;` statements.
fn parse_constants_file_to_map_literal(content: &str) -> Result<String, String> {
    let mut context = ObjFileContext::new();
    let options = CompileOptions::default();

    // Parse the file - we only care about the constants that get accumulated
    let _ = compile_object_definitions(content, &options, &mut context);

    // Build MOO map literal from parsed constants
    if context.constants().is_empty() {
        return Ok("[]".to_string());
    }

    let entries: Vec<String> = context
        .constants()
        .iter()
        .map(|(name, value)| format!("\"{}\" -> {}", name.as_string(), to_literal(value)))
        .collect();

    Ok(format!("[{}]", entries.join(", ")))
}

// ============================================================================
// Tool Definitions
// ============================================================================

pub fn tool_moo_dump_object() -> Tool {
    Tool {
        name: "moo_dump_object".to_string(),
        description: "Dump an object to objdef format (a text representation of the object's \
            definition including properties and verbs). Requires wizard permissions. \
            RECOMMENDED: Use the 'path' parameter to write directly to a file instead of \
            returning content inline - this saves significant tokens for large objects."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference to dump (e.g., '#123', '$thing')"
                },
                "path": {
                    "type": "string",
                    "description": "Optional file path to write the objdef to. When provided, writes to file and returns only a summary (saves tokens). When omitted, returns full objdef content inline."
                },
                "use_constants": {
                    "type": "boolean",
                    "description": "When true, emits symbolic constant names (e.g., ROOM, PLAYER) instead of raw object numbers (#7, #5). Constants are derived from objects' import_export_id properties. Defaults to true.",
                    "default": true
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
        description: "Load an object into the MOO database from an objdef file or URL. \
            Supports both local file paths and HTTP/HTTPS URLs. \
            This is a convenience that combines reading an objdef and calling load_object. \
            Use object_spec to control ID assignment."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path or URL to the objdef file (e.g., 'src/myobject.moo' or 'https://example.com/object.moo')"
                },
                "object_spec": {
                    "type": "string",
                    "description": "How to assign the object ID: '0' for next available, '2' for UUID, or '#N' for specific ID"
                },
                "auto_constants": {
                    "type": "boolean",
                    "description": "Automatically build constants map from objects with import_export_id property. This allows symbolic names like HENRI, ACTOR, etc. in objdef files to be resolved. Defaults to true.",
                    "default": true
                },
                "constants_file": {
                    "type": "string",
                    "description": "Path or URL to a constants file (e.g., 'src/constants.moo' or 'https://example.com/constants.moo'). The file is read and parsed locally/fetched, and the constants are sent to the remote daemon. This is useful when the constants aren't yet defined as objects in the database."
                }
            },
            "required": ["path"]
        }),
    }
}

pub fn tool_moo_reload_objdef_file() -> Tool {
    Tool {
        name: "moo_reload_objdef_file".to_string(),
        description: "Reload an existing object from an objdef file or URL. \
            Supports both local file paths and HTTP/HTTPS URLs. \
            Reads the objdef and reloads the target object with that definition."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path or URL to the objdef file (e.g., 'src/root.moo' or 'https://example.com/object.moo')"
                },
                "target": {
                    "type": "string",
                    "description": "Target object to reload (e.g., '#123'). If not specified, uses the object ID from the objdef."
                },
                "auto_constants": {
                    "type": "boolean",
                    "description": "Automatically build constants map from objects with import_export_id property. This allows symbolic names like HENRI, ACTOR, etc. in objdef files to be resolved. Defaults to true.",
                    "default": true
                },
                "constants_file": {
                    "type": "string",
                    "description": "Path or URL to a constants file (e.g., 'src/constants.moo' or 'https://example.com/constants.moo'). The file is fetched/read and parsed, and the constants are sent to the remote daemon."
                }
            },
            "required": ["path"]
        }),
    }
}

pub fn tool_moo_diff_object() -> Tool {
    Tool {
        name: "moo_diff_object".to_string(),
        description: "Compare an object in the database with an objdef file or text to show \
            structural differences. Parses both objdefs and reports semantic changes: object \
            attributes, added/removed/changed verbs, and added/removed/changed properties. \
            Much more useful than line-by-line diff for understanding actual changes."
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

    let path = args.get("path").and_then(|v| v.as_str());

    // Default to using constants (emits ROOM instead of #7, etc.)
    let use_constants = args
        .get("use_constants")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Build the MOO expression: dump_object(obj, ['constants -> 1])
    let expr = if use_constants {
        format!("return dump_object({}, ['constants -> 1]);", object_str)
    } else {
        format!("return dump_object({});", object_str)
    };

    match client.eval(&expr).await? {
        MoorResult::Success(var) => {
            // The result is a list of strings, join them with newlines
            let Some(list) = var.as_list() else {
                return Ok(ToolCallResult::text(format_var(&var)));
            };

            let lines: Vec<String> = list
                .iter()
                .filter_map(|v| v.as_string().map(|s| s.to_string()))
                .collect();
            let objdef = lines.join("\n");

            // If path is provided, write to file instead of returning content
            if let Some(file_path) = path {
                match std::fs::write(file_path, &objdef) {
                    Ok(()) => Ok(ToolCallResult::text(format!(
                        "Object {} dumped to '{}' ({} lines, {} bytes)",
                        object_str,
                        file_path,
                        lines.len(),
                        objdef.len()
                    ))),
                    Err(e) => Ok(ToolCallResult::error(format!(
                        "Failed to write to '{}': {}",
                        file_path, e
                    ))),
                }
            } else {
                Ok(ToolCallResult::text(format!(
                    "Object {} dumped ({} lines):\n\n{}",
                    object_str,
                    lines.len(),
                    objdef
                )))
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

    let constants_file = args.get("constants_file").and_then(|v| v.as_str());

    // Read the objdef from file or URL
    let objdef = match read_file_or_url(path).await {
        Ok(content) => content,
        Err(e) => {
            return Ok(ToolCallResult::error(e));
        }
    };

    // If constants_file is provided, read and parse it (supports URLs too)
    let local_constants_map = if let Some(cf_path) = constants_file {
        match read_file_or_url(cf_path).await {
            Ok(content) => match parse_constants_file_to_map_literal(&content) {
                Ok(map_literal) => Some(map_literal),
                Err(e) => {
                    return Ok(ToolCallResult::error(format!(
                        "Failed to parse constants file '{}': {}",
                        cf_path, e
                    )));
                }
            },
            Err(e) => {
                return Ok(ToolCallResult::error(e));
            }
        }
    } else {
        None
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
    // load_object takes: (lines, options_map [, object_spec])
    // where options_map has 'constants key for constant definitions
    // Priority: constants_file > auto_constants > no constants
    let expr = if let Some(constants_map) = local_constants_map {
        // Use locally-parsed constants in options map
        let options = format!("['constants -> {}]", constants_map);
        if let Some(spec) = object_spec {
            format!(
                "return load_object({}, {}, {});",
                lines_literal, options, spec
            )
        } else {
            format!("return load_object({}, {});", lines_literal, options)
        }
    } else if auto_constants {
        // Build constants from objects in the database
        let constants_builder = r#"constants = []; for o in (objects()) id = `o.import_export_id ! E_PROPNF => 0'; if (typeof(id) == STR && id != "") constants[id:uppercase()] = o; endif endfor"#;
        if let Some(spec) = object_spec {
            format!(
                "{} return load_object({}, ['constants -> constants], {});",
                constants_builder, lines_literal, spec
            )
        } else {
            format!(
                "{} return load_object({}, ['constants -> constants]);",
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

    let constants_file = args.get("constants_file").and_then(|v| v.as_str());

    // Read the objdef from file or URL
    let objdef = match read_file_or_url(path).await {
        Ok(content) => content,
        Err(e) => {
            return Ok(ToolCallResult::error(e));
        }
    };

    // If constants_file is provided, read and parse it (supports URLs too)
    let local_constants_map = if let Some(cf_path) = constants_file {
        match read_file_or_url(cf_path).await {
            Ok(content) => match parse_constants_file_to_map_literal(&content) {
                Ok(map_literal) => Some(map_literal),
                Err(e) => {
                    return Ok(ToolCallResult::error(format!(
                        "Failed to parse constants file '{}': {}",
                        cf_path, e
                    )));
                }
            },
            Err(e) => {
                return Ok(ToolCallResult::error(e));
            }
        }
    } else {
        None
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
    // Note: reload_object uses a simpler API - constants is the second arg directly, not in options map
    // Priority: constants_file > auto_constants > no constants
    let expr = if let Some(constants_map) = local_constants_map {
        // Use locally-parsed constants
        if let Some(target_obj) = target {
            format!(
                "return reload_object({}, {}, {});",
                lines_literal, constants_map, target_obj
            )
        } else {
            format!(
                "return reload_object({}, {});",
                lines_literal, constants_map
            )
        }
    } else if auto_constants {
        // Build constants from objects in the database
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
    use std::collections::HashSet;

    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let path = args.get("path").and_then(|v| v.as_str());
    let objdef_text = args.get("objdef").and_then(|v| v.as_str());

    // Get the comparison objdef - either from file or text
    let compare_objdef_str = if let Some(p) = path {
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
    let current_objdef_str = match client.eval(&expr).await? {
        MoorResult::Success(var) => {
            let Some(list) = var.as_list() else {
                return Ok(ToolCallResult::error("Failed to dump object"));
            };
            let lines: Vec<String> = list
                .iter()
                .filter_map(|v| v.as_string().map(|s| s.to_string()))
                .collect();
            lines.join("\n")
        }
        MoorResult::Error(msg) => return Ok(ToolCallResult::error(msg)),
    };

    // Parse both objdefs
    let options = CompileOptions::default();
    let mut db_context = ObjFileContext::new();
    let mut file_context = ObjFileContext::new();

    let db_objs = match compile_object_definitions(&current_objdef_str, &options, &mut db_context) {
        Ok(objs) => objs,
        Err(e) => {
            return Ok(ToolCallResult::error(format!(
                "Failed to parse database objdef: {e}"
            )));
        }
    };

    let file_objs =
        match compile_object_definitions(&compare_objdef_str, &options, &mut file_context) {
            Ok(objs) => objs,
            Err(e) => {
                return Ok(ToolCallResult::error(format!(
                    "Failed to parse file objdef: {e}"
                )));
            }
        };

    let Some(db_obj) = db_objs.first() else {
        return Ok(ToolCallResult::error("No object found in database dump"));
    };

    let Some(file_obj) = file_objs.first() else {
        return Ok(ToolCallResult::error("No object found in file"));
    };

    let mut output = String::new();
    output.push_str(&format!(
        "Structural diff: {} (database) vs {}\n",
        object_str,
        path.unwrap_or("provided objdef")
    ));
    output.push_str("═══════════════════════════════════════════\n\n");

    let mut has_differences = false;

    // Compare object attributes
    let mut attr_diffs = Vec::new();
    if db_obj.name != file_obj.name {
        attr_diffs.push(format!(
            "  name: \"{}\" → \"{}\"",
            db_obj.name, file_obj.name
        ));
    }
    if db_obj.parent != file_obj.parent {
        attr_diffs.push(format!(
            "  parent: {:?} → {:?}",
            db_obj.parent, file_obj.parent
        ));
    }
    if db_obj.owner != file_obj.owner {
        attr_diffs.push(format!(
            "  owner: {:?} → {:?}",
            db_obj.owner, file_obj.owner
        ));
    }
    if db_obj.location != file_obj.location {
        attr_diffs.push(format!(
            "  location: {:?} → {:?}",
            db_obj.location, file_obj.location
        ));
    }
    if db_obj.flags != file_obj.flags {
        attr_diffs.push(format!(
            "  flags: {:?} → {:?}",
            db_obj.flags, file_obj.flags
        ));
    }

    if !attr_diffs.is_empty() {
        has_differences = true;
        output.push_str("## Object Attributes\n");
        for diff in attr_diffs {
            output.push_str(&diff);
            output.push('\n');
        }
        output.push('\n');
    }

    // Compare verbs
    let db_verb_names: HashSet<String> = db_obj
        .verbs
        .iter()
        .map(|v| v.names.first().map(|s| s.as_string()).unwrap_or_default())
        .collect();
    let file_verb_names: HashSet<String> = file_obj
        .verbs
        .iter()
        .map(|v| v.names.first().map(|s| s.as_string()).unwrap_or_default())
        .collect();

    let added_verbs: Vec<_> = file_verb_names.difference(&db_verb_names).collect();
    let removed_verbs: Vec<_> = db_verb_names.difference(&file_verb_names).collect();
    let common_verbs: Vec<_> = db_verb_names.intersection(&file_verb_names).collect();

    let mut verb_diffs = Vec::new();

    for name in &added_verbs {
        verb_diffs.push(format!("  + {} (added)", name));
    }
    for name in &removed_verbs {
        verb_diffs.push(format!("  - {} (removed)", name));
    }

    // Check for changed verbs
    for name in common_verbs {
        let db_verb = db_obj
            .verbs
            .iter()
            .find(|v| v.names.first().map(|s| s.as_string()).as_ref() == Some(name));
        let file_verb = file_obj
            .verbs
            .iter()
            .find(|v| v.names.first().map(|s| s.as_string()).as_ref() == Some(name));

        if let (Some(db_v), Some(file_v)) = (db_verb, file_verb) {
            let mut changes = Vec::new();

            if db_v.names != file_v.names {
                changes.push(format!("names: {:?} → {:?}", db_v.names, file_v.names));
            }
            if db_v.flags != file_v.flags {
                changes.push(format!("flags: {:?} → {:?}", db_v.flags, file_v.flags));
            }
            if db_v.owner != file_v.owner {
                changes.push(format!("owner: {:?} → {:?}", db_v.owner, file_v.owner));
            }
            if db_v.argspec != file_v.argspec {
                changes.push(format!(
                    "argspec: {:?} → {:?}",
                    db_v.argspec, file_v.argspec
                ));
            }
            // Compare program bytecode
            if db_v.program != file_v.program {
                changes.push("code: changed".to_string());
            }

            if !changes.is_empty() {
                verb_diffs.push(format!("  ~ {} (modified: {})", name, changes.join(", ")));
            }
        }
    }

    if !verb_diffs.is_empty() {
        has_differences = true;
        output.push_str("## Verbs\n");
        for diff in &verb_diffs {
            output.push_str(diff);
            output.push('\n');
        }
        output.push('\n');
    }

    // Compare property definitions
    let db_prop_names: HashSet<String> = db_obj
        .property_definitions
        .iter()
        .map(|p| p.name.as_string())
        .collect();
    let file_prop_names: HashSet<String> = file_obj
        .property_definitions
        .iter()
        .map(|p| p.name.as_string())
        .collect();

    let added_props: Vec<_> = file_prop_names.difference(&db_prop_names).collect();
    let removed_props: Vec<_> = db_prop_names.difference(&file_prop_names).collect();
    let common_props: Vec<_> = db_prop_names.intersection(&file_prop_names).collect();

    let mut prop_diffs = Vec::new();

    for name in &added_props {
        prop_diffs.push(format!("  + {} (added)", name));
    }
    for name in &removed_props {
        prop_diffs.push(format!("  - {} (removed)", name));
    }

    for name in common_props {
        let db_prop = db_obj
            .property_definitions
            .iter()
            .find(|p| &p.name.as_string() == name);
        let file_prop = file_obj
            .property_definitions
            .iter()
            .find(|p| &p.name.as_string() == name);

        if let (Some(db_p), Some(file_p)) = (db_prop, file_prop) {
            let mut changes = Vec::new();

            if db_p.perms != file_p.perms {
                changes.push(format!("perms: {:?} → {:?}", db_p.perms, file_p.perms));
            }
            if db_p.value != file_p.value {
                changes.push("value: changed".to_string());
            }

            if !changes.is_empty() {
                prop_diffs.push(format!("  ~ {} (modified: {})", name, changes.join(", ")));
            }
        }
    }

    if !prop_diffs.is_empty() {
        has_differences = true;
        output.push_str("## Property Definitions\n");
        for diff in &prop_diffs {
            output.push_str(diff);
            output.push('\n');
        }
        output.push('\n');
    }

    // Compare property overrides
    let db_override_names: HashSet<String> = db_obj
        .property_overrides
        .iter()
        .map(|p| p.name.as_string())
        .collect();
    let file_override_names: HashSet<String> = file_obj
        .property_overrides
        .iter()
        .map(|p| p.name.as_string())
        .collect();

    let added_overrides: Vec<_> = file_override_names.difference(&db_override_names).collect();
    let removed_overrides: Vec<_> = db_override_names.difference(&file_override_names).collect();
    let common_overrides: Vec<_> = db_override_names
        .intersection(&file_override_names)
        .collect();

    let mut override_diffs = Vec::new();

    for name in &added_overrides {
        override_diffs.push(format!("  + {} (added)", name));
    }
    for name in &removed_overrides {
        override_diffs.push(format!("  - {} (removed)", name));
    }

    for name in common_overrides {
        let db_ov = db_obj
            .property_overrides
            .iter()
            .find(|p| &p.name.as_string() == name);
        let file_ov = file_obj
            .property_overrides
            .iter()
            .find(|p| &p.name.as_string() == name);

        if let (Some(db_o), Some(file_o)) = (db_ov, file_ov) {
            let mut changes = Vec::new();

            if db_o.perms_update != file_o.perms_update {
                changes.push(format!(
                    "perms: {:?} → {:?}",
                    db_o.perms_update, file_o.perms_update
                ));
            }
            if db_o.value != file_o.value {
                changes.push("value: changed".to_string());
            }

            if !changes.is_empty() {
                override_diffs.push(format!("  ~ {} (modified: {})", name, changes.join(", ")));
            }
        }
    }

    if !override_diffs.is_empty() {
        has_differences = true;
        output.push_str("## Property Overrides\n");
        for diff in &override_diffs {
            output.push_str(diff);
            output.push('\n');
        }
        output.push('\n');
    }

    // Summary
    if !has_differences {
        output.push_str("No structural differences found.\n");
    } else {
        let verb_count = added_verbs.len()
            + removed_verbs.len()
            + verb_diffs
                .len()
                .saturating_sub(added_verbs.len() + removed_verbs.len());
        let prop_count = added_props.len()
            + removed_props.len()
            + prop_diffs
                .len()
                .saturating_sub(added_props.len() + removed_props.len());
        let override_count = added_overrides.len()
            + removed_overrides.len()
            + override_diffs
                .len()
                .saturating_sub(added_overrides.len() + removed_overrides.len());

        output.push_str(&format!(
            "Summary: {} verb change(s), {} property def change(s), {} override change(s)\n",
            verb_count, prop_count, override_count
        ));
    }

    Ok(ToolCallResult::text(output))
}
