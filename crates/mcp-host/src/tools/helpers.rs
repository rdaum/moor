// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Shared helper functions for MCP tools

use crate::mcp_types::{Tool, ToolCallResult};
use moor_client::TaskResult;
use moor_common::model::ObjectRef;
use moor_var::Var;
use serde_json::{Value, json};

/// Helper to check if a Var's string value matches a key (case-insensitive per MOO semantics)
pub fn var_key_eq(v: &Var, key: &str) -> bool {
    v.as_string()
        .is_some_and(|s| s.to_string().eq_ignore_ascii_case(key))
}

/// Format a MOO Var for display
pub fn format_var(var: &Var) -> String {
    use moor_var::Variant;
    match var.variant() {
        Variant::None => "none".to_string(),
        Variant::Str(s) => format!("\"{}\"", s),
        Variant::Int(i) => i.to_string(),
        Variant::Float(f) => f.to_string(),
        Variant::Obj(o) => format!("{}", o),
        Variant::List(l) => {
            let items: Vec<String> = l.iter().map(|v| format_var(&v)).collect();
            format!("{{{}}}", items.join(", "))
        }
        Variant::Map(m) => {
            let pairs: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("{}: {}", format_var(&k), format_var(&v)))
                .collect();
            format!("[{}]", pairs.join(", "))
        }
        Variant::Err(e) => format!("{:?}", e),
        Variant::Bool(b) => if b { "true" } else { "false" }.to_string(),
        Variant::Binary(b) => format!("~{}~", base64_encode(b.as_bytes())),
        Variant::Flyweight(f) => format!("<flyweight {:?}>", f),
        Variant::Sym(s) => format!("'{}", s.as_string()),
        Variant::Lambda(l) => format!("<lambda {:?}>", l),
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::new();
    for byte in bytes {
        write!(result, "{:02x}", byte).unwrap();
    }
    result
}

/// Format a MOO Var as a MOO literal for use in eval expressions
pub fn format_var_as_literal(var: &Var) -> String {
    use moor_var::Variant;
    match var.variant() {
        Variant::None => "0".to_string(), // None doesn't have a literal, use 0
        Variant::Str(s) => {
            // Escape quotes and backslashes in strings
            let escaped = s.as_str().replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{}\"", escaped)
        }
        Variant::Int(i) => i.to_string(),
        Variant::Float(f) => format!("{:.6}", f), // Ensure decimal point
        Variant::Obj(o) => format!("{}", o),
        Variant::List(l) => {
            let items: Vec<String> = l.iter().map(|v| format_var_as_literal(&v)).collect();
            format!("{{{}}}", items.join(", "))
        }
        Variant::Map(m) => {
            let pairs: Vec<String> = m
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{} -> {}",
                        format_var_as_literal(&k),
                        format_var_as_literal(&v)
                    )
                })
                .collect();
            format!("[{}]", pairs.join(", "))
        }
        Variant::Err(e) => format!("{}", e), // Error codes like E_PERM
        Variant::Bool(b) => if b { "1" } else { "0" }.to_string(), // MOO uses 1/0 for bools
        Variant::Sym(s) => format!("'{}", s.as_string()),
        // These don't have simple literals, fall back to something reasonable
        Variant::Binary(_) => "\"\"".to_string(),
        Variant::Flyweight(_) => "0".to_string(),
        Variant::Lambda(_) => "0".to_string(),
    }
}

/// Format a MOO Var for resource display (public for resources module)
pub fn format_var_for_resource(var: &Var) -> String {
    format_var(var)
}

/// Parse a MOO-style object reference into an ObjectRef
/// Handles: #123, #FFFFFF-FFFFFFFFFF (UUID), $foo, $foo.bar, and curie formats like oid:123, sysobj:foo.
pub fn parse_object_ref(s: &str) -> Option<ObjectRef> {
    // Try curie format first
    if let Some(obj) = ObjectRef::parse_curie(s) {
        return Some(obj);
    }

    // Handle #... formats (numeric or UUID)
    if let Some(ref_str) = s.strip_prefix('#') {
        // Check if it's a UUID format (contains a dash)
        if ref_str.contains('-') {
            // Try to parse as UuObjid: FFFFFF-FFFFFFFFFF
            if let Ok(uuobjid) = moor_var::UuObjid::from_uuid_string(ref_str) {
                return Some(ObjectRef::Id(moor_var::Obj::mk_uuobjid(uuobjid)));
            }
        }
        // Try as numeric object id
        if let Ok(id) = ref_str.parse::<i32>() {
            return Some(ObjectRef::Id(moor_var::Obj::mk_id(id)));
        }
    }

    // Handle $foo or $foo.bar format
    if let Some(name) = s.strip_prefix('$') {
        let symbols: Vec<moor_var::Symbol> = name.split('.').map(moor_var::Symbol::mk).collect();
        if !symbols.is_empty() {
            return Some(ObjectRef::SysObj(symbols));
        }
    }

    // Try as plain number
    if let Ok(id) = s.parse::<i32>() {
        return Some(ObjectRef::Id(moor_var::Obj::mk_id(id)));
    }

    None
}

/// Convert a JSON value to a MOO Var
pub fn json_to_var(json: &Value) -> Var {
    use moor_var::*;
    match json {
        Value::Null => v_none(),
        Value::Bool(b) => v_bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                v_int(i)
            } else if let Some(f) = n.as_f64() {
                v_float(f)
            } else {
                v_none()
            }
        }
        Value::String(s) => {
            // Check if it's an object reference
            if let Some(num_str) = s.strip_prefix('#')
                && let Ok(num) = num_str.parse::<i32>()
            {
                return v_obj(Obj::mk_id(num));
            }
            v_str(s)
        }
        Value::Array(arr) => {
            let items: Vec<Var> = arr.iter().map(json_to_var).collect();
            v_list(&items)
        }
        Value::Object(obj) => {
            // Convert to MOO map
            let pairs: Vec<(Var, Var)> = obj
                .iter()
                .map(|(k, v)| (v_str(k), json_to_var(v)))
                .collect();
            v_map(&pairs)
        }
    }
}

/// Format a TaskResult for MCP response
pub fn format_task_result(result: &TaskResult) -> ToolCallResult {
    let mut output = String::new();

    // Include narrative output if any
    if !result.narrative.is_empty() {
        for line in &result.narrative {
            output.push_str(line);
            output.push('\n');
        }
    }

    // Add result/error information
    if result.success {
        // Only show return value if it's not None
        use moor_var::Variant;
        if !matches!(result.result.variant(), Variant::None) {
            if !output.is_empty() {
                output.push_str("\n=> ");
            }
            output.push_str(&format_var(&result.result));
        }
        ToolCallResult::text(output)
    } else {
        if !output.is_empty() {
            output.push_str("\nError: ");
        } else {
            output.push_str("Error: ");
        }
        output.push_str(&format_var(&result.result));
        ToolCallResult::error(output)
    }
}

/// Add the wizard parameter to a tool's input schema
///
/// This adds an optional `wizard` boolean parameter that controls which
/// connection is used to execute the tool.
pub fn with_wizard_param(mut tool: Tool) -> Tool {
    // Add wizard parameter to the schema's properties
    if let Some(properties) = tool.input_schema.get_mut("properties")
        && let Some(props_obj) = properties.as_object_mut()
    {
        props_obj.insert(
            "wizard".to_string(),
            json!({
                "type": "boolean",
                "description": "DANGER: Execute with wizard privileges. Only use when \
                    elevated permissions are absolutely required. Wizard mode bypasses \
                    normal permission checks and can modify any object in the database. \
                    Default: false (uses programmer connection).",
                "default": false
            }),
        );
    }

    // Append warning to description
    tool.description = format!(
        "{} [Supports wizard mode for elevated privileges - use with extreme caution]",
        tool.description
    );

    tool
}

/// Mark a tool as requiring wizard privileges
///
/// These tools always execute with wizard privileges and cannot be used
/// with the programmer connection.
pub fn wizard_required(mut tool: Tool) -> Tool {
    // Prepend warning to description
    tool.description = format!(
        "[WIZARD ONLY] {} This operation requires wizard privileges and will always \
        use the wizard connection.",
        tool.description
    );

    tool
}

/// List of tools that always require wizard privileges
pub const WIZARD_ONLY_TOOLS: &[&str] = &[
    "moo_dump_object",
    "moo_load_object",
    "moo_reload_object",
    "moo_apply_patch_objdef",
    "moo_dispatch_command_verb",
    "moo_read_objdef_file",
    "moo_write_objdef_file",
    "moo_load_objdef_file",
    "moo_reload_objdef_file",
    "moo_diff_object",
];
