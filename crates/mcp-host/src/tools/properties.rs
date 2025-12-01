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

//! Property tools: list, get, set, add, delete

use crate::mcp_types::{Tool, ToolCallResult};
use crate::moor_client::{MoorClient, MoorResult};
use eyre::Result;
use serde_json::{Value, json};

use super::helpers::{format_var, format_var_as_literal, json_to_var, parse_object_ref};

// ============================================================================
// Tool Definitions
// ============================================================================

pub fn tool_moo_list_properties() -> Tool {
    Tool {
        name: "moo_list_properties".to_string(),
        description: "List all properties defined on an object. Can optionally include inherited \
            properties from parent objects."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#0', '$player')"
                },
                "inherited": {
                    "type": "boolean",
                    "description": "Include inherited properties from parent objects (default: false)"
                }
            },
            "required": ["object"]
        }),
    }
}

pub fn tool_moo_get_property() -> Tool {
    Tool {
        name: "moo_get_property".to_string(),
        description: "Get the value of a property on an object.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#0', '$player')"
                },
                "property": {
                    "type": "string",
                    "description": "Property name to retrieve"
                }
            },
            "required": ["object", "property"]
        }),
    }
}

pub fn tool_moo_set_property() -> Tool {
    Tool {
        name: "moo_set_property".to_string(),
        description: "Set the value of a property on an object. The value should be a valid MOO \
            value (string, number, object reference, list, or map)."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#0', '$player')"
                },
                "property": {
                    "type": "string",
                    "description": "Property name to set"
                },
                "value": {
                    "description": "Value to set (JSON value that will be converted to MOO value)"
                }
            },
            "required": ["object", "property", "value"]
        }),
    }
}

pub fn tool_moo_add_property() -> Tool {
    Tool {
        name: "moo_add_property".to_string(),
        description: "Add a new property to an object with an initial value. Requires programmer \
            permissions and ownership or wizard status."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference to add the property to (e.g., '#123', '$player')"
                },
                "name": {
                    "type": "string",
                    "description": "Property name to add"
                },
                "value": {
                    "description": "Initial value for the property (JSON value converted to MOO value)",
                    "default": 0
                },
                "permissions": {
                    "type": "string",
                    "description": "Permission flags: r=read, w=write, c=chown (e.g., 'rc')",
                    "default": "rc"
                }
            },
            "required": ["object", "name"]
        }),
    }
}

pub fn tool_moo_delete_property() -> Tool {
    Tool {
        name: "moo_delete_property".to_string(),
        description: "Delete a property from an object. Requires programmer permissions and \
            ownership or wizard status."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#123', '$player')"
                },
                "property": {
                    "type": "string",
                    "description": "Property name to delete"
                }
            },
            "required": ["object", "property"]
        }),
    }
}

// ============================================================================
// Tool Implementations
// ============================================================================

pub async fn execute_moo_list_properties(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let inherited = args
        .get("inherited")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let object = parse_object_ref(object_str)
        .ok_or_else(|| eyre::eyre!("Invalid object reference: {}", object_str))?;

    let props = client.list_properties(&object, inherited).await?;
    let mut output = String::new();
    output.push_str(&format!("Properties on {}:\n\n", object_str));
    for prop in &props {
        output.push_str(&format!(
            "  {}: owner={}, flags={}\n",
            prop.name, prop.owner, prop.flags
        ));
    }
    Ok(ToolCallResult::text(output))
}

pub async fn execute_moo_get_property(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let prop_name = args
        .get("property")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'property' parameter"))?;

    let object = parse_object_ref(object_str)
        .ok_or_else(|| eyre::eyre!("Invalid object reference: {}", object_str))?;

    let value = client.get_property(&object, prop_name).await?;
    Ok(ToolCallResult::text(format!(
        "{}.{} = {}",
        object_str,
        prop_name,
        format_var(&value)
    )))
}

pub async fn execute_moo_set_property(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let prop_name = args
        .get("property")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'property' parameter"))?;

    let value_json = args
        .get("value")
        .ok_or_else(|| eyre::eyre!("Missing 'value' parameter"))?;

    let object = parse_object_ref(object_str)
        .ok_or_else(|| eyre::eyre!("Invalid object reference: {}", object_str))?;

    let moo_value = json_to_var(value_json);

    client.set_property(&object, prop_name, &moo_value).await?;
    Ok(ToolCallResult::text(format!(
        "Set {}.{} = {}",
        object_str,
        prop_name,
        format_var(&moo_value)
    )))
}

pub async fn execute_moo_add_property(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'name' parameter"))?;

    let permissions = args
        .get("permissions")
        .and_then(|v| v.as_str())
        .unwrap_or("rc");

    let value_json = args.get("value").unwrap_or(&Value::Null);
    let moo_value = json_to_var(value_json);

    // Build the MOO expression: add_property(obj, "name", value, {owner, perms})
    // Owner will be player (the caller)
    let value_literal = format_var_as_literal(&moo_value);
    let expr = format!(
        "add_property({}, \"{}\", {}, {{player, \"{}\"}});",
        object_str, name, value_literal, permissions
    );

    match client.eval(&expr).await? {
        MoorResult::Success(_) => Ok(ToolCallResult::text(format!(
            "Successfully added property '{}' to {} with value {}",
            name,
            object_str,
            format_var(&moo_value)
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_delete_property(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let property = args
        .get("property")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'property' parameter"))?;

    // Build the MOO expression: delete_property(obj, "propname")
    let expr = format!("delete_property({}, \"{}\");", object_str, property);

    match client.eval(&expr).await? {
        MoorResult::Success(_) => Ok(ToolCallResult::text(format!(
            "Successfully deleted property '{}' from {}",
            property, object_str
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}
