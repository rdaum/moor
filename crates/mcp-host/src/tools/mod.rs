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

//! MCP Tools for interacting with mooR
//!
//! This module defines all the MCP tools available for AI assistants to interact
//! with the MOO virtual world. Tools are organized into submodules by category:
//!
//! - `eval`: Code execution tools (eval, command, invoke_verb, function_help, test_compile, command parsing)
//! - `objects`: Object inspection/manipulation (list, resolve, graph, create, recycle, move, set_parent)
//! - `verbs`: Verb operations (list, get, program, apply_patch, add, delete, find_definition)
//! - `properties`: Property operations (list, get, set, add, delete)
//! - `objdef`: Object definition tools (dump, load, reload, patch, file ops, diff)
//! - `util`: Utility tools (notify)
//! - `dynamic`: MOO-defined tools fetched from #0:external_agent_tools()
//! - `helpers`: Shared helper functions

pub mod dynamic;
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
use helpers::{with_wizard_param, wizard_required};
use serde_json::Value;
use tracing::debug;

// Re-export helper functions needed by other modules
pub use helpers::{WIZARD_ONLY_TOOLS, format_var_for_resource, parse_object_ref};

/// Get all available tools
///
/// All tools support an optional `wizard` parameter that executes the tool
/// with wizard privileges instead of programmer privileges. This should be
/// used sparingly and only when elevated permissions are required.
pub fn get_tools() -> Vec<Tool> {
    vec![
        // Execution tools
        with_wizard_param(eval::tool_moo_eval()),
        with_wizard_param(eval::tool_moo_command()),
        with_wizard_param(eval::tool_moo_invoke_verb()),
        with_wizard_param(eval::tool_moo_function_help()),
        with_wizard_param(eval::tool_moo_test_compile()),
        with_wizard_param(eval::tool_moo_parse_command()),
        with_wizard_param(eval::tool_moo_parse_command_for_player()),
        with_wizard_param(eval::tool_moo_find_command_verb()),
        wizard_required(eval::tool_moo_dispatch_command_verb()),
        // Object inspection tools
        with_wizard_param(objects::tool_moo_list_objects()),
        with_wizard_param(objects::tool_moo_resolve()),
        with_wizard_param(objects::tool_moo_object_graph()),
        with_wizard_param(objects::tool_moo_object_flags()),
        // Verb tools
        with_wizard_param(verbs::tool_moo_list_verbs()),
        with_wizard_param(verbs::tool_moo_get_verb()),
        with_wizard_param(verbs::tool_moo_program_verb()),
        with_wizard_param(verbs::tool_moo_apply_patch_verb()),
        with_wizard_param(verbs::tool_moo_add_verb()),
        with_wizard_param(verbs::tool_moo_delete_verb()),
        with_wizard_param(verbs::tool_moo_set_verb_info()),
        with_wizard_param(verbs::tool_moo_set_verb_args()),
        // Property tools
        with_wizard_param(properties::tool_moo_list_properties()),
        with_wizard_param(properties::tool_moo_get_property()),
        with_wizard_param(properties::tool_moo_set_property()),
        with_wizard_param(properties::tool_moo_add_property()),
        with_wizard_param(properties::tool_moo_delete_property()),
        // Object manipulation tools
        with_wizard_param(objects::tool_moo_create_object()),
        with_wizard_param(objects::tool_moo_recycle_object()),
        with_wizard_param(objects::tool_moo_set_object_flag()),
        // Object definition (objdef) tools - wizard only
        wizard_required(objdef::tool_moo_dump_object()),
        wizard_required(objdef::tool_moo_load_object()),
        wizard_required(objdef::tool_moo_reload_object()),
        wizard_required(objdef::tool_moo_apply_patch_objdef()),
        // Filesystem objdef tools - wizard only
        wizard_required(objdef::tool_moo_read_objdef_file()),
        wizard_required(objdef::tool_moo_write_objdef_file()),
        wizard_required(objdef::tool_moo_load_objdef_file()),
        wizard_required(objdef::tool_moo_reload_objdef_file()),
        // Utility tools
        with_wizard_param(objects::tool_moo_move_object()),
        with_wizard_param(objects::tool_moo_set_parent()),
        wizard_required(objdef::tool_moo_diff_object()),
        with_wizard_param(util::tool_moo_list_prepositions()),
        with_wizard_param(util::tool_moo_notify()),
        with_wizard_param(verbs::tool_moo_find_verb_definition()),
        // Server/session tools
        with_wizard_param(util::tool_moo_connected_players()),
        with_wizard_param(util::tool_moo_server_info()),
        with_wizard_param(util::tool_moo_queued_tasks()),
        with_wizard_param(util::tool_moo_kill_task()),
        // Connection management (no wizard param needed - it reconnects both)
        util::tool_moo_reconnect(),
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
        "moo_test_compile" => eval::execute_moo_test_compile(client, arguments).await,
        "moo_parse_command" => eval::execute_moo_parse_command(client, arguments).await,
        "moo_parse_command_for_player" => {
            eval::execute_moo_parse_command_for_player(client, arguments).await
        }
        "moo_find_command_verb" => eval::execute_moo_find_command_verb(client, arguments).await,
        "moo_dispatch_command_verb" => {
            eval::execute_moo_dispatch_command_verb(client, arguments).await
        }
        // Object inspection tools
        "moo_list_objects" => objects::execute_moo_list_objects(client, arguments).await,
        "moo_resolve" => objects::execute_moo_resolve(client, arguments).await,
        "moo_object_graph" => objects::execute_moo_object_graph(client, arguments).await,
        "moo_object_flags" => objects::execute_moo_object_flags(client, arguments).await,
        // Verb tools
        "moo_list_verbs" => verbs::execute_moo_list_verbs(client, arguments).await,
        "moo_get_verb" => verbs::execute_moo_get_verb(client, arguments).await,
        "moo_program_verb" => verbs::execute_moo_program_verb(client, arguments).await,
        "moo_apply_patch_verb" => verbs::execute_moo_apply_patch_verb(client, arguments).await,
        "moo_add_verb" => verbs::execute_moo_add_verb(client, arguments).await,
        "moo_delete_verb" => verbs::execute_moo_delete_verb(client, arguments).await,
        "moo_set_verb_info" => verbs::execute_moo_set_verb_info(client, arguments).await,
        "moo_set_verb_args" => verbs::execute_moo_set_verb_args(client, arguments).await,
        // Property tools
        "moo_list_properties" => properties::execute_moo_list_properties(client, arguments).await,
        "moo_get_property" => properties::execute_moo_get_property(client, arguments).await,
        "moo_set_property" => properties::execute_moo_set_property(client, arguments).await,
        "moo_add_property" => properties::execute_moo_add_property(client, arguments).await,
        "moo_delete_property" => properties::execute_moo_delete_property(client, arguments).await,
        // Object manipulation tools
        "moo_create_object" => objects::execute_moo_create_object(client, arguments).await,
        "moo_recycle_object" => objects::execute_moo_recycle_object(client, arguments).await,
        "moo_set_object_flag" => objects::execute_moo_set_object_flag(client, arguments).await,
        // Objdef tools
        "moo_dump_object" => objdef::execute_moo_dump_object(client, arguments).await,
        "moo_load_object" => objdef::execute_moo_load_object(client, arguments).await,
        "moo_reload_object" => objdef::execute_moo_reload_object(client, arguments).await,
        "moo_apply_patch_objdef" => objdef::execute_moo_apply_patch_objdef(client, arguments).await,
        // Filesystem objdef tools
        "moo_read_objdef_file" => objdef::execute_moo_read_objdef_file(client, arguments).await,
        "moo_write_objdef_file" => objdef::execute_moo_write_objdef_file(client, arguments).await,
        "moo_load_objdef_file" => objdef::execute_moo_load_objdef_file(client, arguments).await,
        "moo_reload_objdef_file" => objdef::execute_moo_reload_objdef_file(client, arguments).await,
        // Utility tools
        "moo_move_object" => objects::execute_moo_move_object(client, arguments).await,
        "moo_set_parent" => objects::execute_moo_set_parent(client, arguments).await,
        "moo_diff_object" => objdef::execute_moo_diff_object(client, arguments).await,
        "moo_list_prepositions" => util::execute_moo_list_prepositions(client, arguments).await,
        "moo_notify" => util::execute_moo_notify(client, arguments).await,
        "moo_find_verb_definition" => {
            verbs::execute_moo_find_verb_definition(client, arguments).await
        }
        // Server/session tools
        "moo_connected_players" => util::execute_moo_connected_players(client, arguments).await,
        "moo_server_info" => util::execute_moo_server_info(client, arguments).await,
        "moo_queued_tasks" => util::execute_moo_queued_tasks(client, arguments).await,
        "moo_kill_task" => util::execute_moo_kill_task(client, arguments).await,
        // moo_reconnect is handled as a meta-tool in mcp_server.rs
        _ => Ok(ToolCallResult::error(format!("Unknown tool: {}", name))),
    }
}
