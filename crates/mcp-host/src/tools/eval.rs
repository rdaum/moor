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

//! Execution tools: eval, command, invoke_verb

use crate::mcp_types::{Tool, ToolCallResult};
use crate::moor_client::{MoorClient, MoorResult};
use eyre::Result;
use moor_var::Var;
use serde_json::{Value, json};

use super::helpers::{format_task_result, format_var, json_to_var, parse_object_ref};

// ============================================================================
// Tool Definitions
// ============================================================================

pub fn tool_moo_eval() -> Tool {
    Tool {
        name: "moo_eval".to_string(),
        description: "Evaluate MOO code and return the result. The code is compiled and executed \
            in the context of the authenticated player. IMPORTANT: To return a value, you must use \
            an explicit 'return' statement (e.g., 'return #49.contents;' not just '#49.contents'). \
            Without 'return', the result will be 0."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "MOO code to evaluate. Use 'return X;' to get a value back (e.g., 'return 1 + 2;', 'return player.name;', 'return #49.contents;')"
                }
            },
            "required": ["expression"]
        }),
    }
}

pub fn tool_moo_command() -> Tool {
    Tool {
        name: "moo_command".to_string(),
        description: "Execute a MOO command as the player. This is like typing a command in the \
            game - it goes through the normal command parser and verb dispatch. Use this for \
            game actions like 'look', 'say hello', 'go north', etc."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute (e.g., 'look', 'say Hello!', '@examine me')"
                }
            },
            "required": ["command"]
        }),
    }
}

pub fn tool_moo_invoke_verb() -> Tool {
    Tool {
        name: "moo_invoke_verb".to_string(),
        description: "Directly invoke a verb on an object with specified arguments. This bypasses \
            the command parser and calls the verb directly. Use object references like '#123' \
            (object number), '$room' (system property), or 'name:path' for corified references."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#0', '#123', '$player', '$string_utils')"
                },
                "verb": {
                    "type": "string",
                    "description": "Verb name to invoke"
                },
                "args": {
                    "type": "array",
                    "description": "Arguments to pass to the verb (as JSON values)",
                    "items": {}
                }
            },
            "required": ["object", "verb"]
        }),
    }
}

// ============================================================================
// Tool Implementations
// ============================================================================

pub async fn execute_moo_eval(client: &mut MoorClient, args: &Value) -> Result<ToolCallResult> {
    let expression = args
        .get("expression")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'expression' parameter"))?;

    match client.eval(expression).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format_var(&var))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_command(client: &mut MoorClient, args: &Value) -> Result<ToolCallResult> {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'command' parameter"))?;

    let result = client.command(command).await?;
    Ok(format_task_result(&result))
}

pub async fn execute_moo_invoke_verb(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let verb = args
        .get("verb")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'verb' parameter"))?;

    let args_array = args
        .get("args")
        .and_then(|v| v.as_array())
        .map(|a| a.to_vec())
        .unwrap_or_default();

    let object = parse_object_ref(object_str)
        .ok_or_else(|| eyre::eyre!("Invalid object reference: {}", object_str))?;

    let moo_args: Vec<Var> = args_array.iter().map(json_to_var).collect();

    let result = client.invoke_verb(&object, verb, moo_args).await?;
    Ok(format_task_result(&result))
}
