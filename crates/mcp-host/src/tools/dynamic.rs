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

//! Dynamic tool support for MOO-defined tools
//!
//! Allows the MOO world to define tools that external agents can use, fetched
//! from #0:external_agent_tools() at connection time.

use crate::mcp_types::{Tool, ToolCallResult};
use moor_client::{MoorClient, MoorResult};
use eyre::{Result, eyre};
use moor_var::{Associative, Var, Variant};
use serde_json::{Value, json};
use tracing::{debug, info, warn};

use super::helpers::{format_task_result, json_to_var, with_wizard_param};

/// A dynamically-defined resource from the MOO world
#[derive(Debug, Clone)]
pub struct DynamicResource {
    /// Resource URI (e.g., "moor://building-guide")
    pub uri: String,
    /// Human-readable name
    pub name: String,
    /// Description of the resource
    pub description: String,
    /// MIME type (e.g., "text/plain")
    pub mime_type: String,
    /// The resource content
    pub content: String,
}

/// A dynamically-defined tool from the MOO world
#[derive(Debug, Clone)]
pub struct DynamicTool {
    /// Tool name (must be unique across all tools)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// JSON Schema for input parameters
    pub input_schema: Value,
    /// Target object to invoke the handler verb on
    pub target_obj: String,
    /// Verb name to invoke on the target object
    pub target_verb: String,
}

impl DynamicTool {
    /// Convert to MCP Tool format with wizard parameter support
    pub fn to_mcp_tool(&self) -> Tool {
        let tool = Tool {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
        };
        with_wizard_param(tool)
    }
}

/// Fetch dynamic tools from the MOO world
///
/// Calls #0:external_agent_tools() and parses the result into DynamicTool structs.
pub async fn fetch_dynamic_tools(client: &mut MoorClient) -> Result<Vec<DynamicTool>> {
    info!("Fetching dynamic tools from #0:external_agent_tools()");

    // Call #0:external_agent_tools() via eval (simpler than invoke_verb for this)
    let expression = "return #0:external_agent_tools();";
    let result = client.eval(expression).await?;

    match result {
        MoorResult::Success(var) => parse_tools_list(&var),
        MoorResult::Error(msg) => {
            warn!("Failed to fetch dynamic tools: {}", msg);
            Ok(vec![])
        }
    }
}

/// Parse a MOO list of tool definitions into DynamicTool structs
fn parse_tools_list(var: &Var) -> Result<Vec<DynamicTool>> {
    let Variant::List(list) = var.variant() else {
        return Err(eyre!(
            "external_agent_tools() should return a list, got {:?}",
            var.variant()
        ));
    };

    let mut tools = Vec::new();
    for item in list.iter() {
        match parse_tool_definition(&item) {
            Ok(tool) => {
                debug!("Loaded dynamic tool: {}", tool.name);
                tools.push(tool);
            }
            Err(e) => {
                warn!("Failed to parse tool definition: {}", e);
            }
        }
    }

    info!("Loaded {} dynamic tools", tools.len());
    Ok(tools)
}

/// Parse a single tool definition map into a DynamicTool
fn parse_tool_definition(var: &Var) -> Result<DynamicTool> {
    let Variant::Map(map) = var.variant() else {
        return Err(eyre!("Tool definition should be a map"));
    };

    let name = get_map_string(map, "name")?;
    let description = get_map_string(map, "description")?;
    let target_verb = get_map_string(map, "target_verb")?;

    // Parse target_obj - could be an object reference
    let target_obj = get_map_obj_ref(map, "target_obj")?;

    // Parse input_schema - convert MOO map to JSON
    let input_schema = get_map_as_json(map, "input_schema")?;

    Ok(DynamicTool {
        name,
        description,
        input_schema,
        target_obj,
        target_verb,
    })
}

/// Get a string value from a MOO map
fn get_map_string(map: &moor_var::Map, key: &str) -> Result<String> {
    let key_var = moor_var::v_str(key);
    let value = map
        .get(&key_var)
        .map_err(|_| eyre!("Missing key: {}", key))?;

    match value.variant() {
        Variant::Str(s) => Ok(s.to_string()),
        _ => Err(eyre!("Key '{}' should be a string", key)),
    }
}

/// Get an object reference from a MOO map (as string like "#123")
fn get_map_obj_ref(map: &moor_var::Map, key: &str) -> Result<String> {
    let key_var = moor_var::v_str(key);
    let value = map
        .get(&key_var)
        .map_err(|_| eyre!("Missing key: {}", key))?;

    match value.variant() {
        Variant::Obj(obj) => Ok(format!("{}", obj)),
        Variant::Str(s) => Ok(s.to_string()),
        _ => Err(eyre!("Key '{}' should be an object or string", key)),
    }
}

/// Convert a MOO map value to JSON
fn get_map_as_json(map: &moor_var::Map, key: &str) -> Result<Value> {
    let key_var = moor_var::v_str(key);
    let value = map
        .get(&key_var)
        .map_err(|_| eyre!("Missing key: {}", key))?;

    var_to_json(&value)
}

/// Convert a MOO Var to JSON Value
fn var_to_json(var: &Var) -> Result<Value> {
    match var.variant() {
        Variant::None => Ok(Value::Null),
        Variant::Bool(b) => Ok(json!(b)),
        Variant::Int(i) => Ok(json!(i)),
        Variant::Float(f) => Ok(json!(f)),
        Variant::Str(s) => Ok(json!(s.as_str())),
        Variant::Obj(o) => Ok(json!(format!("{}", o))),
        Variant::Sym(s) => Ok(json!(s.as_string())),
        Variant::List(list) => {
            let items: Result<Vec<Value>> = list.iter().map(|v| var_to_json(&v)).collect();
            Ok(Value::Array(items?))
        }
        Variant::Map(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map.iter() {
                let key = match k.variant() {
                    Variant::Str(s) => s.to_string(),
                    Variant::Sym(s) => s.as_string().to_string(),
                    _ => format!("{:?}", k),
                };
                obj.insert(key, var_to_json(&v)?);
            }
            Ok(Value::Object(obj))
        }
        Variant::Err(e) => Ok(json!({ "error": format!("{}", e) })),
        _ => Ok(json!(format!("{:?}", var))),
    }
}

/// Execute a dynamic tool
///
/// Invokes the target verb on the target object with the provided arguments.
/// The authenticated player is passed as a second argument so the handler
/// can use it for permission checking.
pub async fn execute_dynamic_tool(
    client: &mut MoorClient,
    tool: &DynamicTool,
    arguments: &Value,
) -> Result<ToolCallResult> {
    debug!(
        "Executing dynamic tool '{}' -> {}:{}",
        tool.name, tool.target_obj, tool.target_verb
    );

    // Parse the target object reference
    let obj_ref = super::helpers::parse_object_ref(&tool.target_obj)
        .ok_or_else(|| eyre!("Invalid target object: {}", tool.target_obj))?;

    // Convert JSON arguments to MOO map
    let moo_args = json_to_var(arguments);

    // Get the authenticated player to pass as actor
    let actor = client
        .player()
        .map(|p| moor_var::v_obj(*p))
        .unwrap_or_else(moor_var::v_none);

    // Invoke the verb with {args_map, actor}
    let result = client
        .invoke_verb(&obj_ref, &tool.target_verb, vec![moo_args, actor])
        .await?;

    Ok(format_task_result(&result))
}

/// Fetch dynamic resources from the MOO world
///
/// Calls #0:external_agent_resources() and parses the result into DynamicResource structs.
pub async fn fetch_dynamic_resources(client: &mut MoorClient) -> Result<Vec<DynamicResource>> {
    info!("Fetching dynamic resources from #0:external_agent_resources()");

    let expression = "return #0:external_agent_resources();";
    let result = client.eval(expression).await?;

    match result {
        MoorResult::Success(var) => parse_resources_list(&var),
        MoorResult::Error(msg) => {
            warn!("Failed to fetch dynamic resources: {}", msg);
            Ok(vec![])
        }
    }
}

/// Parse a MOO list of resource definitions into DynamicResource structs
fn parse_resources_list(var: &Var) -> Result<Vec<DynamicResource>> {
    let Variant::List(list) = var.variant() else {
        return Err(eyre!(
            "external_agent_resources() should return a list, got {:?}",
            var.variant()
        ));
    };

    let mut resources = Vec::new();
    for item in list.iter() {
        match parse_resource_definition(&item) {
            Ok(resource) => {
                debug!("Loaded dynamic resource: {}", resource.uri);
                resources.push(resource);
            }
            Err(e) => {
                warn!("Failed to parse resource definition: {}", e);
            }
        }
    }

    info!("Loaded {} dynamic resources", resources.len());
    Ok(resources)
}

/// Parse a single resource definition map into a DynamicResource
fn parse_resource_definition(var: &Var) -> Result<DynamicResource> {
    let Variant::Map(map) = var.variant() else {
        return Err(eyre!("Resource definition should be a map"));
    };

    let uri = get_map_string(map, "uri")?;
    let name = get_map_string(map, "name")?;
    let description = get_map_string(map, "description")?;
    let mime_type = get_map_string(map, "mimeType").unwrap_or_else(|_| "text/plain".to_string());
    let content = get_map_string(map, "content")?;

    Ok(DynamicResource {
        uri,
        name,
        description,
        mime_type,
        content,
    })
}
