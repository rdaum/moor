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

//! Utility tools: notify, reconnect, server info, players, tasks

use crate::mcp_types::{Tool, ToolCallResult};
use crate::moor_client::{MoorClient, MoorResult};
use eyre::Result;
use moor_common::matching::{all_prepositions, get_preposition_forms};
use moor_var::Sequence;
use serde_json::{Value, json};

use super::helpers::format_var;

// ============================================================================
// Tool Definitions
// ============================================================================

pub fn tool_moo_reconnect() -> Tool {
    Tool {
        name: "moo_reconnect".to_string(),
        description: "Reconnect to the mooR daemon. Use this if the connection has been lost \
            (e.g., after the daemon was restarted). This clears stale connection state, \
            re-establishes the connection, and re-authenticates using stored credentials."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub fn tool_moo_notify() -> Tool {
    Tool {
        name: "moo_notify".to_string(),
        description: "Send a notification message to a connected player. The message appears in \
            their client output. Useful for testing or sending messages programmatically."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "player": {
                    "type": "string",
                    "description": "Player object to notify (e.g., '#2', '$wizard')"
                },
                "message": {
                    "type": "string",
                    "description": "Message text to send"
                },
                "no_flush": {
                    "type": "boolean",
                    "description": "If true, don't flush output buffer immediately",
                    "default": false
                },
                "no_newline": {
                    "type": "boolean",
                    "description": "If true, don't append a newline to the message",
                    "default": false
                }
            },
            "required": ["player", "message"]
        }),
    }
}

pub fn tool_moo_list_prepositions() -> Tool {
    Tool {
        name: "moo_list_prepositions".to_string(),
        description: "List all valid prepositions used in command parsing and verb argspecs."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub fn tool_moo_connected_players() -> Tool {
    Tool {
        name: "moo_connected_players".to_string(),
        description: "List all currently connected players. Returns player objects with their \
            names and connection info."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "include_all": {
                    "type": "boolean",
                    "description": "Include players with negative object numbers (special system connections)",
                    "default": false
                }
            }
        }),
    }
}

pub fn tool_moo_server_info() -> Tool {
    Tool {
        name: "moo_server_info".to_string(),
        description: "Get server information including version, uptime, and memory usage."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub fn tool_moo_queued_tasks() -> Tool {
    Tool {
        name: "moo_queued_tasks".to_string(),
        description: "List all queued (suspended) tasks. Shows task ID, start time, and the \
            verb/object being executed."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub fn tool_moo_kill_task() -> Tool {
    Tool {
        name: "moo_kill_task".to_string(),
        description:
            "Kill a running or suspended task by its ID. Use queued_tasks to find task IDs."
                .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "integer",
                    "description": "The task ID to kill"
                }
            },
            "required": ["task_id"]
        }),
    }
}

// ============================================================================
// Tool Implementations
// ============================================================================

// Note: execute_moo_reconnect is handled as a meta-tool in mcp_server.rs
// to allow it to reconnect ALL connections (both programmer and wizard)

pub async fn execute_moo_notify(client: &mut MoorClient, args: &Value) -> Result<ToolCallResult> {
    let player_str = args
        .get("player")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'player' parameter"))?;

    let message = args
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'message' parameter"))?;

    let no_flush = args
        .get("no_flush")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let no_newline = args
        .get("no_newline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Escape the message for MOO string
    let escaped_message = message.replace('\\', "\\\\").replace('"', "\\\"");

    // Build notify() call with optional flags
    let expr = if no_flush || no_newline {
        format!(
            "notify({}, \"{}\", {}, {});",
            player_str,
            escaped_message,
            if no_flush { "1" } else { "0" },
            if no_newline { "1" } else { "0" }
        )
    } else {
        format!("notify({}, \"{}\");", player_str, escaped_message)
    };

    match client.eval(&expr).await? {
        MoorResult::Success(_) => Ok(ToolCallResult::text(format!(
            "Sent notification to {}: {}",
            player_str, message
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_list_prepositions(
    _client: &mut MoorClient,
    _args: &Value,
) -> Result<ToolCallResult> {
    let mut output = String::new();
    output.push_str("Valid prepositions:\n\n");

    for prep in all_prepositions() {
        let id = prep as u16;
        let canonical = prep.to_string();
        let single = prep.to_string_single();
        let forms = get_preposition_forms(prep).join(", ");
        output.push_str(&format!(
            "  {}: {} (single: {}) forms: {}\n",
            id, canonical, single, forms
        ));
    }

    Ok(ToolCallResult::text(output))
}

pub async fn execute_moo_connected_players(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let include_all = args
        .get("include_all")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let expr = if include_all {
        r#"
        players = connected_players(1);
        result = {};
        for p in (players)
            idle = idle_seconds(p);
            conn = connected_seconds(p);
            result = {@result, ["player" -> p, "name" -> p.name, "idle" -> idle, "connected" -> conn]};
        endfor
        return result;
        "#
    } else {
        r#"
        players = connected_players();
        result = {};
        for p in (players)
            idle = idle_seconds(p);
            conn = connected_seconds(p);
            result = {@result, ["player" -> p, "name" -> p.name, "idle" -> idle, "connected" -> conn]};
        endfor
        return result;
        "#
    };

    match client.eval(expr).await? {
        MoorResult::Success(var) => {
            let mut output = String::new();
            output.push_str("Connected Players:\n\n");

            if let Some(list) = var.as_list() {
                if list.is_empty() {
                    output.push_str("  (no players connected)\n");
                } else {
                    for item in list.iter() {
                        if let Some(map) = item.as_map() {
                            let player = map
                                .iter()
                                .find(|(k, _)| k.as_string().is_some_and(|s| s == "player"))
                                .map(|(_, v)| format_var(&v))
                                .unwrap_or_default();
                            let name = map
                                .iter()
                                .find(|(k, _)| k.as_string().is_some_and(|s| s == "name"))
                                .map(|(_, v)| format_var(&v))
                                .unwrap_or_default();
                            let idle = map
                                .iter()
                                .find(|(k, _)| k.as_string().is_some_and(|s| s == "idle"))
                                .and_then(|(_, v)| v.as_integer())
                                .unwrap_or(0);
                            let conn = map
                                .iter()
                                .find(|(k, _)| k.as_string().is_some_and(|s| s == "connected"))
                                .and_then(|(_, v)| v.as_integer())
                                .unwrap_or(0);

                            output.push_str(&format!(
                                "  {} {} - idle {}s, connected {}s\n",
                                player, name, idle, conn
                            ));
                        }
                    }
                }
            }
            Ok(ToolCallResult::text(output))
        }
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_server_info(
    client: &mut MoorClient,
    _args: &Value,
) -> Result<ToolCallResult> {
    let expr = r#"
    return [
        "version" -> server_version(),
        "memory" -> memory_usage()
    ];
    "#;

    match client.eval(expr).await? {
        MoorResult::Success(var) => {
            let mut output = String::new();
            output.push_str("Server Information:\n\n");

            if let Some(map) = var.as_map() {
                let version = map
                    .iter()
                    .find(|(k, _)| k.as_string().is_some_and(|s| s == "version"))
                    .map(|(_, v)| format_var(&v))
                    .unwrap_or_default();

                output.push_str(&format!("  Version: {}\n", version));

                // memory_usage() returns {block_size, nused, nfree}
                // where block_size is page size, nused is RSS pages, nfree is (vm_size - rss) pages
                if let Some((_, mem_var)) = map
                    .iter()
                    .find(|(k, _)| k.as_string().is_some_and(|s| s == "memory"))
                {
                    if let Some(mem_list) = mem_var.as_list() {
                        let block_size = mem_list
                            .index(0)
                            .ok()
                            .and_then(|v| v.as_integer())
                            .unwrap_or(4096);
                        let nused = mem_list
                            .index(1)
                            .ok()
                            .and_then(|v| v.as_integer())
                            .unwrap_or(0);
                        let nfree = mem_list
                            .index(2)
                            .ok()
                            .and_then(|v| v.as_integer())
                            .unwrap_or(0);

                        let rss_bytes = nused * block_size;
                        let vm_bytes = (nused + nfree) * block_size;

                        output.push_str("  Memory:\n");
                        output.push_str(&format!(
                            "    RSS: {} ({} pages)\n",
                            format_bytes(rss_bytes),
                            nused
                        ));
                        output.push_str(&format!(
                            "    Virtual: {} ({} pages)\n",
                            format_bytes(vm_bytes),
                            nused + nfree
                        ));
                        output.push_str(&format!("    Page size: {} bytes\n", block_size));
                    } else {
                        output.push_str(&format!("  Memory: {}\n", format_var(&mem_var)));
                    }
                }
            }
            Ok(ToolCallResult::text(output))
        }
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

fn format_bytes(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

pub async fn execute_moo_queued_tasks(
    client: &mut MoorClient,
    _args: &Value,
) -> Result<ToolCallResult> {
    let expr = "return queued_tasks();";

    match client.eval(expr).await? {
        MoorResult::Success(var) => {
            let mut output = String::new();
            output.push_str("Queued Tasks:\n\n");

            if let Some(list) = var.as_list() {
                if list.is_empty() {
                    output.push_str("  (no queued tasks)\n");
                } else {
                    // queued_tasks returns list of {task_id, start_time, x, y, programmer, verb_loc, verb_name, line, this}
                    for task in list.iter() {
                        if let Some(task_list) = task.as_list() {
                            let task_id = task_list
                                .index(0)
                                .ok()
                                .map(|v| format_var(&v))
                                .unwrap_or_default();
                            let verb_loc = task_list
                                .index(5)
                                .ok()
                                .map(|v| format_var(&v))
                                .unwrap_or_default();
                            let verb_name = task_list
                                .index(6)
                                .ok()
                                .map(|v| format_var(&v))
                                .unwrap_or_default();
                            let this_obj = task_list
                                .index(8)
                                .ok()
                                .map(|v| format_var(&v))
                                .unwrap_or_default();

                            output.push_str(&format!(
                                "  Task {}: {}:{} (this={})\n",
                                task_id, verb_loc, verb_name, this_obj
                            ));
                        }
                    }
                }
            }
            Ok(ToolCallResult::text(output))
        }
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_kill_task(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let task_id = args
        .get("task_id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| eyre::eyre!("Missing 'task_id' parameter"))?;

    let expr = format!("kill_task({});", task_id);

    match client.eval(&expr).await? {
        MoorResult::Success(_) => Ok(ToolCallResult::text(format!("Task {} killed", task_id))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}
