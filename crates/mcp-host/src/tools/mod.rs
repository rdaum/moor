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

//! MCP Tools for interacting with mooR
//!
//! This module defines all the MCP tools available for AI assistants to interact
//! with the MOO virtual world. Tools are organized into submodules by category:
//!
//! - `eval`: Code execution tools (eval, command, invoke_verb)
//! - `objects`: Object inspection/manipulation (list, resolve, graph, create, recycle, move, set_parent)
//! - `verbs`: Verb operations (list, get, program, add, delete, find_definition)
//! - `properties`: Property operations (list, get, set, add, delete)
//! - `objdef`: Object definition tools (dump, load, reload, file ops, diff)
//! - `util`: Utility tools (notify)
//! - `helpers`: Shared helper functions

mod eval;
mod helpers;
mod objdef;
mod objects;
mod properties;
mod util;
mod verbs;

use crate::mcp_types::{Tool, ToolCallResult};
use crate::moor_client::MoorClient;
use eyre::Result;
use serde_json::Value;
use tracing::debug;

// Re-export helper functions needed by resources module
pub use helpers::{format_var_for_resource, parse_object_ref};

/// Get all available tools
pub fn get_tools() -> Vec<Tool> {
    vec![
        // Execution tools
        eval::tool_moo_eval(),
        eval::tool_moo_command(),
        eval::tool_moo_invoke_verb(),
        eval::tool_moo_function_help(),
        // Object inspection tools
        objects::tool_moo_list_objects(),
        objects::tool_moo_resolve(),
        objects::tool_moo_object_graph(),
        // Verb tools
        verbs::tool_moo_list_verbs(),
        verbs::tool_moo_get_verb(),
        verbs::tool_moo_program_verb(),
        verbs::tool_moo_add_verb(),
        verbs::tool_moo_delete_verb(),
        // Property tools
        properties::tool_moo_list_properties(),
        properties::tool_moo_get_property(),
        properties::tool_moo_set_property(),
        properties::tool_moo_add_property(),
        properties::tool_moo_delete_property(),
        // Object manipulation tools
        objects::tool_moo_create_object(),
        objects::tool_moo_recycle_object(),
        // Object definition (objdef) tools
        objdef::tool_moo_dump_object(),
        objdef::tool_moo_load_object(),
        objdef::tool_moo_reload_object(),
        // Filesystem objdef tools
        objdef::tool_moo_read_objdef_file(),
        objdef::tool_moo_write_objdef_file(),
        objdef::tool_moo_load_objdef_file(),
        objdef::tool_moo_reload_objdef_file(),
        // Utility tools
        objects::tool_moo_move_object(),
        objects::tool_moo_set_parent(),
        objdef::tool_moo_diff_object(),
        util::tool_moo_notify(),
        verbs::tool_moo_find_verb_definition(),
    ]
}

/// Execute a tool call
pub async fn execute_tool(
    client: &mut MoorClient,
    name: &str,
    arguments: &Value,
) -> Result<ToolCallResult> {
    debug!("Executing tool: {} with args: {}", name, arguments);

    match name {
        // Execution tools
        "moo_eval" => eval::execute_moo_eval(client, arguments).await,
        "moo_command" => eval::execute_moo_command(client, arguments).await,
        "moo_invoke_verb" => eval::execute_moo_invoke_verb(client, arguments).await,
        "moo_function_help" => eval::execute_moo_function_help(client, arguments).await,
        // Object inspection tools
        "moo_list_objects" => objects::execute_moo_list_objects(client, arguments).await,
        "moo_resolve" => objects::execute_moo_resolve(client, arguments).await,
        "moo_object_graph" => objects::execute_moo_object_graph(client, arguments).await,
        // Verb tools
        "moo_list_verbs" => verbs::execute_moo_list_verbs(client, arguments).await,
        "moo_get_verb" => verbs::execute_moo_get_verb(client, arguments).await,
        "moo_program_verb" => verbs::execute_moo_program_verb(client, arguments).await,
        "moo_add_verb" => verbs::execute_moo_add_verb(client, arguments).await,
        "moo_delete_verb" => verbs::execute_moo_delete_verb(client, arguments).await,
        // Property tools
        "moo_list_properties" => properties::execute_moo_list_properties(client, arguments).await,
        "moo_get_property" => properties::execute_moo_get_property(client, arguments).await,
        "moo_set_property" => properties::execute_moo_set_property(client, arguments).await,
        "moo_add_property" => properties::execute_moo_add_property(client, arguments).await,
        "moo_delete_property" => properties::execute_moo_delete_property(client, arguments).await,
        // Object manipulation tools
        "moo_create_object" => objects::execute_moo_create_object(client, arguments).await,
        "moo_recycle_object" => objects::execute_moo_recycle_object(client, arguments).await,
        // Objdef tools
        "moo_dump_object" => objdef::execute_moo_dump_object(client, arguments).await,
        "moo_load_object" => objdef::execute_moo_load_object(client, arguments).await,
        "moo_reload_object" => objdef::execute_moo_reload_object(client, arguments).await,
        // Filesystem objdef tools
        "moo_read_objdef_file" => objdef::execute_moo_read_objdef_file(client, arguments).await,
        "moo_write_objdef_file" => objdef::execute_moo_write_objdef_file(client, arguments).await,
        "moo_load_objdef_file" => objdef::execute_moo_load_objdef_file(client, arguments).await,
        "moo_reload_objdef_file" => objdef::execute_moo_reload_objdef_file(client, arguments).await,
        // Utility tools
        "moo_move_object" => objects::execute_moo_move_object(client, arguments).await,
        "moo_set_parent" => objects::execute_moo_set_parent(client, arguments).await,
        "moo_diff_object" => objdef::execute_moo_diff_object(client, arguments).await,
        "moo_notify" => util::execute_moo_notify(client, arguments).await,
        "moo_find_verb_definition" => {
            verbs::execute_moo_find_verb_definition(client, arguments).await
        }
        _ => Ok(ToolCallResult::error(format!("Unknown tool: {}", name))),
    }
}
