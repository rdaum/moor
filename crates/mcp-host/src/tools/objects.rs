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

//! Object inspection and manipulation tools

use crate::mcp_types::{Tool, ToolCallResult};
use crate::moor_client::{MoorClient, MoorResult};
use eyre::Result;
use moor_var::Sequence;
use serde_json::{Value, json};

use super::helpers::{format_var, var_key_eq};

// ============================================================================
// Tool Definitions
// ============================================================================

pub fn tool_moo_list_objects() -> Tool {
    Tool {
        name: "moo_list_objects".to_string(),
        description: "List objects in the MOO database. Returns basic information about each \
            object including its number, name, and flags. Supports filtering by parent or name pattern."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "parent": {
                    "type": "string",
                    "description": "Filter to only show descendants of this object (e.g., '$thing', '#3')"
                },
                "name_pattern": {
                    "type": "string",
                    "description": "Filter by name pattern (case-insensitive substring match)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of objects to return (default: 100)",
                    "default": 100
                }
            }
        }),
    }
}

pub fn tool_moo_resolve() -> Tool {
    Tool {
        name: "moo_resolve".to_string(),
        description: "Resolve an object reference to get detailed information including name, \
            parent, children, location, contents, owner, flags, and verb/property counts. \
            Accepts object numbers (#123), system properties ($room), or corified references."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference to resolve (e.g., '#0', '$player', '$room')"
                }
            },
            "required": ["object"]
        }),
    }
}

pub fn tool_moo_object_graph() -> Tool {
    Tool {
        name: "moo_object_graph".to_string(),
        description: "Show the inheritance graph for an object, including its ancestor chain \
            (parents up to #0) and descendant tree (children). Useful for understanding \
            object relationships and inheritance structure."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference to show graph for (e.g., '#3', '$thing', '$player')"
                },
                "depth": {
                    "type": "integer",
                    "description": "Maximum depth for descendant tree (default: 3)",
                    "default": 3
                }
            },
            "required": ["object"]
        }),
    }
}

pub fn tool_moo_create_object() -> Tool {
    Tool {
        name: "moo_create_object".to_string(),
        description: "Create a new object with the specified parent. Returns the new object's \
            reference. Requires programmer permissions."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "parent": {
                    "type": "string",
                    "description": "Parent object reference (e.g., '#1', '$thing')"
                },
                "owner": {
                    "type": "string",
                    "description": "Owner object reference (defaults to caller)"
                }
            },
            "required": ["parent"]
        }),
    }
}

pub fn tool_moo_recycle_object() -> Tool {
    Tool {
        name: "moo_recycle_object".to_string(),
        description: "Destroy an object permanently. The object's contents are moved out and \
            its children are reparented. Requires ownership or wizard status."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference to recycle (e.g., '#123')"
                }
            },
            "required": ["object"]
        }),
    }
}

pub fn tool_moo_move_object() -> Tool {
    Tool {
        name: "moo_move_object".to_string(),
        description: "Move an object to a new location. Uses the MOO move() builtin which \
            handles location/contents relationship updates. The object's location property \
            is set and it's added to the new location's contents."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object to move (e.g., '#123', '$player')"
                },
                "destination": {
                    "type": "string",
                    "description": "Destination object (e.g., '#49', '$room'). Use #-1 to move nowhere."
                }
            },
            "required": ["object", "destination"]
        }),
    }
}

pub fn tool_moo_set_parent() -> Tool {
    Tool {
        name: "moo_set_parent".to_string(),
        description: "Change an object's parent, altering its position in the inheritance hierarchy. \
            Uses the MOO chparent() builtin. The object inherits verbs and properties from the new parent."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object to reparent (e.g., '#123')"
                },
                "new_parent": {
                    "type": "string",
                    "description": "New parent object (e.g., '$thing', '#3')"
                }
            },
            "required": ["object", "new_parent"]
        }),
    }
}

pub fn tool_moo_object_flags() -> Tool {
    Tool {
        name: "moo_object_flags".to_string(),
        description: "Get an object's flags (player, programmer, wizard, fertile, readable). \
            These flags control permissions and behavior."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#123', '$player')"
                }
            },
            "required": ["object"]
        }),
    }
}

pub fn tool_moo_set_object_flag() -> Tool {
    Tool {
        name: "moo_set_object_flag".to_string(),
        description: "Set an object flag (player, programmer, wizard, fertile, readable). \
            Requires appropriate permissions."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#123', '$player')"
                },
                "flag": {
                    "type": "string",
                    "description": "Flag to set: 'player', 'programmer', 'wizard', 'fertile', or 'readable'",
                    "enum": ["player", "programmer", "wizard", "fertile", "readable"]
                },
                "value": {
                    "type": "boolean",
                    "description": "True to set the flag, false to clear it"
                }
            },
            "required": ["object", "flag", "value"]
        }),
    }
}

// ============================================================================
// Tool Implementations
// ============================================================================

pub async fn execute_moo_list_objects(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let parent_filter = args.get("parent").and_then(|v| v.as_str());
    let name_pattern = args.get("name_pattern").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(100) as usize;

    // If we have filters, use MOO eval for filtering
    if parent_filter.is_some() || name_pattern.is_some() {
        let mut conditions = Vec::new();

        if let Some(parent) = parent_filter {
            // Check if object is a descendant of parent
            conditions.push(format!(
                "(parent(o) == {} || `isa(o, {}) ! ANY => 0')",
                parent, parent
            ));
        }

        if let Some(pattern) = name_pattern {
            // Case-insensitive substring match
            let escaped = pattern.replace('\\', "\\\\").replace('"', "\\\"");
            conditions.push(format!(
                "(index(o.name:lowercase(), \"{}\":lowercase()) > 0)",
                escaped
            ));
        }

        let condition = if conditions.is_empty() {
            "1".to_string()
        } else {
            conditions.join(" && ")
        };

        let expr = format!(
            r#"result = []; for o in (objects()) if ({}) result = {{@result, {{"obj" -> o, "name" -> o.name}}}}; if (length(result) >= {}) break; endif endif endfor return result;"#,
            condition, limit
        );

        match client.eval(&expr).await? {
            MoorResult::Success(var) => {
                let mut output = String::new();
                if let Some(list) = var.as_list() {
                    output.push_str(&format!("Found {} objects", list.len()));
                    if list.len() >= limit {
                        output.push_str(&format!(" (limit: {})", limit));
                    }
                    output.push_str(":\n\n");
                    for item in list.iter() {
                        if let Some(map) = item.as_map() {
                            let obj = map
                                .iter()
                                .find(|(k, _)| var_key_eq(k, "obj"))
                                .map(|(_, v)| format_var(&v))
                                .unwrap_or_default();
                            let name = map
                                .iter()
                                .find(|(k, _)| var_key_eq(k, "name"))
                                .map(|(_, v)| format_var(&v))
                                .unwrap_or_default();
                            output.push_str(&format!("{}: {}\n", obj, name));
                        }
                    }
                }
                Ok(ToolCallResult::text(output))
            }
            MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
        }
    } else {
        // No filters, use the direct RPC for efficiency
        let objects = client.list_objects().await?;
        let mut output = String::new();
        let total = objects.len();
        let shown = total.min(limit);
        output.push_str(&format!("Found {} objects", total));
        if shown < total {
            output.push_str(&format!(" (showing {})", shown));
        }
        output.push_str(":\n\n");
        for obj in objects.iter().take(limit) {
            output.push_str(&format!(
                "{}: {} (flags: {})\n",
                obj.obj, obj.name, obj.flags
            ));
        }
        if total > limit {
            output.push_str(&format!("\n... and {} more objects", total - limit));
        }
        Ok(ToolCallResult::text(output))
    }
}

pub async fn execute_moo_resolve(client: &mut MoorClient, args: &Value) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    // Build a MOO expression that returns rich object information
    // Return builtin properties as separate booleans, matching MOO's property style
    let expr = format!(
        r#"o = {}; if (!valid(o)) return E_INVARG; endif return ["obj" -> o, "name" -> o.name, "parent" -> parent(o), "children" -> children(o), "location" -> `o.location ! E_PROPNF => #-1', "contents" -> `o.contents ! E_PROPNF => {{}}', "owner" -> o.owner, "player" -> is_player(o), "programmer" -> `o.programmer ! E_PROPNF => 0', "wizard" -> `o.wizard ! E_PROPNF => 0', "r" -> `o.r ! E_PROPNF => 0', "w" -> `o.w ! E_PROPNF => 0', "f" -> `o.f ! E_PROPNF => 0', "verb_count" -> length(verbs(o)), "prop_count" -> length(properties(o))];"#,
        object_str
    );

    match client.eval(&expr).await? {
        MoorResult::Success(var) => {
            // Format the result nicely
            if let Some(map) = var.as_map() {
                let mut output = String::new();

                // Helper to extract a value from the map
                let get_val = |key: &str| -> String {
                    map.iter()
                        .find(|(k, _)| var_key_eq(k, key))
                        .map(|(_, v)| format_var(&v))
                        .unwrap_or_default()
                };

                // Helper to check if a value is truthy (for builtin properties)
                let is_truthy = |key: &str| -> bool {
                    map.iter()
                        .find(|(k, _)| var_key_eq(k, key))
                        .map(|(_, v)| {
                            v.as_integer().map(|i| i != 0).unwrap_or(false)
                                || v.as_bool().unwrap_or(false)
                        })
                        .unwrap_or(false)
                };

                output.push_str(&format!("Object: {} {}\n", get_val("obj"), get_val("name")));
                output.push_str(&format!("  Parent: {}\n", get_val("parent")));
                output.push_str(&format!("  Children: {}\n", get_val("children")));
                output.push_str("  Builtin properties:\n");
                output.push_str(&format!("    name: {}\n", get_val("name")));
                output.push_str(&format!("    owner: {}\n", get_val("owner")));
                output.push_str(&format!("    location: {}\n", get_val("location")));
                output.push_str(&format!("    contents: {}\n", get_val("contents")));
                output.push_str(&format!("    player: {}\n", is_truthy("player")));
                output.push_str(&format!("    programmer: {}\n", is_truthy("programmer")));
                output.push_str(&format!("    wizard: {}\n", is_truthy("wizard")));
                output.push_str(&format!("    r: {}\n", is_truthy("r")));
                output.push_str(&format!("    w: {}\n", is_truthy("w")));
                output.push_str(&format!("    f: {}\n", is_truthy("f")));
                output.push_str(&format!(
                    "  Verbs: {}, Properties: {}",
                    get_val("verb_count"),
                    get_val("prop_count")
                ));

                Ok(ToolCallResult::text(output))
            } else {
                Ok(ToolCallResult::text(format_var(&var)))
            }
        }
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_object_graph(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let depth = args.get("depth").and_then(|v| v.as_i64()).unwrap_or(3);

    // Build MOO expression that returns both ancestors and descendants
    // Note: In MOO, {} is empty list, [] is empty map. Use {{}} in format string to get {}.
    let expr = format!(
        r#"
        target = {};
        if (!valid(target))
            return E_INVARG;
        endif
        ancestors = {{}};
        p = target;
        while (valid(p))
            ancestors = {{@ancestors, ["obj" -> p, "name" -> p.name]}};
            p = parent(p);
        endwhile
        max_depth = {};
        descendants = {{}};
        queue = {{}};
        for c in (children(target))
            queue = {{@queue, ["obj" -> c, "depth" -> 1]}};
        endfor
        while (length(queue) > 0)
            item = queue[1];
            if (length(queue) > 1)
                queue = queue[2..$];
            else
                queue = {{}};
            endif
            o = item["obj"];
            d = item["depth"];
            descendants = {{@descendants, ["obj" -> o, "name" -> o.name, "depth" -> d]}};
            if (d < max_depth)
                for c in (children(o))
                    queue = {{@queue, ["obj" -> c, "depth" -> d + 1]}};
                endfor
            endif
        endwhile
        return ["target" -> target, "target_name" -> target.name, "ancestors" -> ancestors, "descendants" -> descendants];
        "#,
        object_str, depth
    );

    match client.eval(&expr).await? {
        MoorResult::Success(var) => {
            if let Some(map) = var.as_map() {
                let mut output = String::new();

                // Get target info
                let target = map
                    .iter()
                    .find(|(k, _)| var_key_eq(k, "target"))
                    .map(|(_, v)| format_var(&v))
                    .unwrap_or_default();
                let target_name = map
                    .iter()
                    .find(|(k, _)| var_key_eq(k, "target_name"))
                    .map(|(_, v)| format_var(&v))
                    .unwrap_or_default();

                output.push_str(&format!("Object Graph for {} {}\n\n", target, target_name));

                // Show ancestors (reversed so root is first)
                output.push_str("Ancestors (inheritance chain):\n");
                let ancestors_var = map
                    .iter()
                    .find(|(k, _)| var_key_eq(k, "ancestors"))
                    .map(|(_, v)| v.clone());
                if let Some(ancestors_var) = ancestors_var
                    && let Some(ancestors) = ancestors_var.as_list()
                {
                    let ancestors: Vec<_> = ancestors.iter().collect();
                    for (i, a) in ancestors.iter().rev().enumerate() {
                        if let Some(amap) = a.as_map() {
                            let obj = amap
                                .iter()
                                .find(|(k, _)| var_key_eq(k, "obj"))
                                .map(|(_, v)| format_var(&v))
                                .unwrap_or_default();
                            let name = amap
                                .iter()
                                .find(|(k, _)| var_key_eq(k, "name"))
                                .map(|(_, v)| format_var(&v))
                                .unwrap_or_default();
                            let indent = "  ".repeat(i);
                            let marker = if i == ancestors.len() - 1 { "* " } else { "  " };
                            output.push_str(&format!("{}{}{} {}\n", indent, marker, obj, name));
                        }
                    }
                }

                // Show descendants as tree
                output.push_str("\nDescendants (children tree):\n");
                let descendants_var = map
                    .iter()
                    .find(|(k, _)| var_key_eq(k, "descendants"))
                    .map(|(_, v)| v.clone());
                if let Some(descendants_var) = descendants_var
                    && let Some(descendants) = descendants_var.as_list()
                {
                    if descendants.is_empty() {
                        output.push_str("  (no children)\n");
                    } else {
                        for d in descendants.iter() {
                            if let Some(dmap) = d.as_map() {
                                let obj = dmap
                                    .iter()
                                    .find(|(k, _)| var_key_eq(k, "obj"))
                                    .map(|(_, v)| format_var(&v))
                                    .unwrap_or_default();
                                let name = dmap
                                    .iter()
                                    .find(|(k, _)| var_key_eq(k, "name"))
                                    .map(|(_, v)| format_var(&v))
                                    .unwrap_or_default();
                                let depth_val =
                                    dmap.iter()
                                        .find(|(k, _)| var_key_eq(k, "depth"))
                                        .and_then(|(_, v)| v.as_integer())
                                        .unwrap_or(1) as usize;
                                let indent = "  ".repeat(depth_val);
                                output.push_str(&format!("{}├─ {} {}\n", indent, obj, name));
                            }
                        }
                    }
                }

                Ok(ToolCallResult::text(output))
            } else {
                Ok(ToolCallResult::text(format_var(&var)))
            }
        }
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_create_object(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let parent_str = args
        .get("parent")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'parent' parameter"))?;

    let owner_str = args.get("owner").and_then(|v| v.as_str());

    // Build the MOO expression: create(parent [, owner])
    let expr = if let Some(owner) = owner_str {
        format!("return create({}, {});", parent_str, owner)
    } else {
        format!("return create({});", parent_str)
    };

    match client.eval(&expr).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format!(
            "Created new object: {}",
            format_var(&var)
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_recycle_object(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    // Build the MOO expression: recycle(obj)
    let expr = format!("recycle({});", object_str);

    match client.eval(&expr).await? {
        MoorResult::Success(_) => Ok(ToolCallResult::text(format!(
            "Successfully recycled object {}",
            object_str
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_move_object(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let destination_str = args
        .get("destination")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'destination' parameter"))?;

    // Use MOO move() builtin
    let expr = format!(
        "move({}, {}); return {}.location;",
        object_str, destination_str, object_str
    );

    match client.eval(&expr).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format!(
            "Moved {} to {} (new location: {})",
            object_str,
            destination_str,
            format_var(&var)
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_set_parent(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let new_parent_str = args
        .get("new_parent")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'new_parent' parameter"))?;

    // Use MOO chparent() builtin
    let expr = format!(
        "chparent({}, {}); return parent({});",
        object_str, new_parent_str, object_str
    );

    match client.eval(&expr).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format!(
            "Changed parent of {} to {} (new parent: {})",
            object_str,
            new_parent_str,
            format_var(&var)
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_object_flags(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    // Get object flags via properties
    let expr = format!(
        r#"return [
            "player" -> is_player({}),
            "programmer" -> {}.programmer,
            "wizard" -> {}.wizard,
            "fertile" -> {}.f,
            "readable" -> {}.r
        ];"#,
        object_str, object_str, object_str, object_str, object_str
    );

    match client.eval(&expr).await? {
        MoorResult::Success(var) => {
            let mut output = String::new();
            output.push_str(&format!("Flags for {}:\n\n", object_str));

            if let Some(map) = var.as_map() {
                for (k, v) in map.iter() {
                    if let Some(key) = k.as_string() {
                        let value = if v.as_integer().unwrap_or(0) != 0 {
                            "true"
                        } else {
                            "false"
                        };
                        output.push_str(&format!("  {}: {}\n", key, value));
                    }
                }
            }
            Ok(ToolCallResult::text(output))
        }
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_set_object_flag(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let flag = args
        .get("flag")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'flag' parameter"))?;

    let value = args
        .get("value")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| eyre::eyre!("Missing 'value' parameter"))?;

    let value_int = if value { 1 } else { 0 };

    // Map flag name to the appropriate setter
    let expr = match flag {
        "player" => format!("set_player_flag({}, {});", object_str, value_int),
        "programmer" => format!("{}.programmer = {};", object_str, value_int),
        "wizard" => format!("{}.wizard = {};", object_str, value_int),
        "fertile" => format!("{}.f = {};", object_str, value_int),
        "readable" => format!("{}.r = {};", object_str, value_int),
        _ => {
            return Ok(ToolCallResult::error(format!(
                "Unknown flag: {}. Valid flags are: player, programmer, wizard, fertile, readable",
                flag
            )));
        }
    };

    match client.eval(&expr).await? {
        MoorResult::Success(_) => Ok(ToolCallResult::text(format!(
            "Set {} flag on {} to {}",
            flag, object_str, value
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}
