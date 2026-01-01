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

//! Execution tools: eval, command, invoke_verb, function_help, test_compile, command parsing

use crate::mcp_types::{Tool, ToolCallResult};
use crate::moor_client::{MoorClient, MoorResult};
use eyre::Result;
use moor_compiler::{CompileOptions, DiagnosticRenderOptions, compile, format_compile_error};
use moor_var::Var;
use serde_json::{Value, json};

use super::helpers::{
    format_task_result, format_var, format_var_as_literal, json_to_var, parse_object_ref,
};

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

pub fn tool_moo_function_help() -> Tool {
    Tool {
        name: "moo_function_help".to_string(),
        description: "Get documentation for a MOO builtin function. Returns usage information, \
            argument types, and description for functions like 'notify', 'move', 'create', etc."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "function_name": {
                    "type": "string",
                    "description": "Name of the builtin function (e.g., 'notify', 'move', 'create', 'eval')"
                }
            },
            "required": ["function_name"]
        }),
    }
}

pub fn tool_moo_test_compile() -> Tool {
    Tool {
        name: "moo_test_compile".to_string(),
        description: "Compile MOO code without executing it. Returns syntax/compile errors for \
            a program (such as a verb body)."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "MOO source code to compile (e.g., a verb body)"
                },
                "legacy_type_constants": {
                    "type": "boolean",
                    "description": "Allow legacy type constants (INT, OBJ, STR) in code. Defaults to false.",
                    "default": false
                }
            },
            "required": ["code"]
        }),
    }
}

pub fn tool_moo_parse_command() -> Tool {
    Tool {
        name: "moo_parse_command".to_string(),
        description: "Parse a command string using the built-in command parser. Returns a map \
            of parsed components (verb, args, dobj, prep, iobj, etc)."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command string to parse (e.g., 'give lamp to bob')"
                },
                "environment": {
                    "type": "array",
                    "description": "List of objects and/or {obj, names[]} entries for name matching",
                    "items": {}
                },
                "complex": {
                    "type": "boolean",
                    "description": "Enable complex matching (fuzzy + ordinals). Defaults to false.",
                    "default": false
                }
            },
            "required": ["command", "environment"]
        }),
    }
}

pub fn tool_moo_parse_command_for_player() -> Tool {
    Tool {
        name: "moo_parse_command_for_player".to_string(),
        description: "Parse a command using the player's match environment. Uses \
            player:match_environment when available, with a fallback environment built from the \
            player's inventory and location."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command string to parse (e.g., 'give lamp to bob')"
                },
                "player": {
                    "type": "string",
                    "description": "Player object reference (defaults to current player)"
                },
                "complex": {
                    "type": "boolean",
                    "description": "Enable complex matching (fuzzy + ordinals). Defaults to false.",
                    "default": false
                }
            },
            "required": ["command"]
        }),
    }
}

pub fn tool_moo_find_command_verb() -> Tool {
    Tool {
        name: "moo_find_command_verb".to_string(),
        description: "Find command verbs matching a parsed command spec on a given \
            command environment. Useful for testing verb argspecs and matches."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "parsed_spec": {
                    "type": "object",
                    "description": "Parsed command spec map (e.g., from moo_parse_command)"
                },
                "parsed_spec_moo": {
                    "type": "string",
                    "description": "Parsed command spec as a MOO literal (overrides parsed_spec)"
                },
                "command_environment": {
                    "type": "array",
                    "description": "List of objects and/or {obj, names[]} entries for verb search",
                    "items": {}
                }
            },
            "required": ["command_environment"]
        }),
    }
}

pub fn tool_moo_dispatch_command_verb() -> Tool {
    Tool {
        name: "moo_dispatch_command_verb".to_string(),
        description: "Dispatch a command verb using a parsed command spec. This is wizard-only \
            and bypasses the exec bit requirement."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "Target object to dispatch against"
                },
                "verb": {
                    "type": "string",
                    "description": "Verb name to dispatch"
                },
                "parsed_spec": {
                    "type": "object",
                    "description": "Parsed command spec map (e.g., from moo_parse_command)"
                },
                "parsed_spec_moo": {
                    "type": "string",
                    "description": "Parsed command spec as a MOO literal (overrides parsed_spec)"
                }
            },
            "required": ["target", "verb"]
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

pub async fn execute_moo_function_help(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let function_name = args
        .get("function_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'function_name' parameter"))?;

    let expression = format!("return function_help(\"{}\");", function_name);

    match client.eval(&expression).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format_var(&var))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_test_compile(
    _client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let code = args
        .get("code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'code' parameter"))?;

    let legacy_type_constants = args
        .get("legacy_type_constants")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let options = CompileOptions {
        legacy_type_constants,
        ..Default::default()
    };

    match compile(code, options) {
        Ok(program) => Ok(ToolCallResult::text(format!(
            "Compilation succeeded ({} lines, {} ops)",
            code.lines().count(),
            program.0.main_vector.len()
        ))),
        Err(err) => {
            let rendered =
                format_compile_error(&err, Some(code), DiagnosticRenderOptions::default())
                    .join("\n");
            Ok(ToolCallResult::error(rendered))
        }
    }
}

pub async fn execute_moo_parse_command(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'command' parameter"))?;

    let environment = args
        .get("environment")
        .ok_or_else(|| eyre::eyre!("Missing 'environment' parameter"))?;

    let complex = args
        .get("complex")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let env_literal = build_environment_literal(environment)?;
    let escaped_command = escape_moo_string(command);
    let complex_flag = if complex { "1" } else { "0" };

    let expr = format!(
        "return parse_command(\"{}\", {}, {});",
        escaped_command, env_literal, complex_flag
    );

    match client.eval(&expr).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format_var(&var))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_parse_command_for_player(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'command' parameter"))?;

    let player = args.get("player").and_then(|v| v.as_str());

    let complex = args
        .get("complex")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let escaped_command = escape_moo_string(command);
    let player_expr = player.unwrap_or("player");
    let complex_flag = if complex { "1" } else { "0" };

    let expr = format!(
        r#"who = {player_expr};
 cmd = "{command}";
 complex = {complex_flag};
 env = `who:match_environment(cmd, ["complex" -> complex]) ! E_VERBNF => 0';

if (env == 0)
    env = {{who}};
    loc = `who.location ! E_PROPNF => #-1';
    if (valid(loc))
        env = {{@env, loc}};
        loc_contents = `loc.contents ! E_PROPNF => {{}}';
        env = {{@env, @loc_contents}};
    endif
    inv = `who.contents ! E_PROPNF => {{}}';
    env = {{@env, @inv}};
endif
return parse_command(cmd, env, complex);"#,
        player_expr = player_expr,
        command = escaped_command,
        complex_flag = complex_flag
    );

    match client.eval(&expr).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format_var(&var))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_find_command_verb(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let command_environment = args
        .get("command_environment")
        .ok_or_else(|| eyre::eyre!("Missing 'command_environment' parameter"))?;

    let spec_expr = if let Some(spec) = args.get("parsed_spec_moo").and_then(|v| v.as_str()) {
        spec.to_string()
    } else if let Some(spec) = args.get("parsed_spec") {
        build_parsed_spec_literal(spec)?
    } else {
        return Err(eyre::eyre!(
            "Missing 'parsed_spec' or 'parsed_spec_moo' parameter"
        ));
    };

    let env_literal = build_environment_literal(command_environment)?;

    let expr = format!(
        "spec = {}; spec[\"verb\"] = tosym(spec[\"verb\"]); spec['verb] = spec[\"verb\"]; return find_command_verb(spec, {});",
        spec_expr, env_literal
    );

    match client.eval(&expr).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format_var(&var))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_dispatch_command_verb(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let target = args
        .get("target")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'target' parameter"))?;

    let verb = args
        .get("verb")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'verb' parameter"))?;

    let spec_expr = if let Some(spec) = args.get("parsed_spec_moo").and_then(|v| v.as_str()) {
        spec.to_string()
    } else if let Some(spec) = args.get("parsed_spec") {
        build_parsed_spec_literal(spec)?
    } else {
        return Err(eyre::eyre!(
            "Missing 'parsed_spec' or 'parsed_spec_moo' parameter"
        ));
    };

    let escaped_verb = escape_moo_string(verb);

    let expr = format!(
        "spec = {}; spec[\"verb\"] = tosym(spec[\"verb\"]); spec['verb] = spec[\"verb\"]; return dispatch_command_verb({}, \"{}\", spec);",
        spec_expr, target, escaped_verb
    );

    match client.eval(&expr).await? {
        MoorResult::Success(var) => Ok(ToolCallResult::text(format_var(&var))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

fn escape_moo_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn object_ref_literal(value: &Value) -> Result<String> {
    match value {
        Value::String(s) => {
            if s.starts_with('#') || s.starts_with('$') {
                Ok(s.to_string())
            } else if s.parse::<i64>().is_ok() || s.contains('-') {
                Ok(format!("#{}", s))
            } else {
                Err(eyre::eyre!("Invalid object reference: {}", s))
            }
        }
        Value::Number(n) => n
            .as_i64()
            .map(|id| format!("#{}", id))
            .ok_or_else(|| eyre::eyre!("Invalid object reference")),
        _ => Err(eyre::eyre!("Invalid object reference")),
    }
}

fn build_environment_literal(value: &Value) -> Result<String> {
    let Some(entries) = value.as_array() else {
        return Err(eyre::eyre!("'environment' must be a list"));
    };

    let mut items = Vec::new();
    for entry in entries {
        match entry {
            Value::String(_) | Value::Number(_) => {
                items.push(object_ref_literal(entry)?);
            }
            Value::Object(map) => {
                let obj_value = map
                    .get("obj")
                    .ok_or_else(|| eyre::eyre!("Environment entry missing 'obj'"))?;
                let obj_literal = object_ref_literal(obj_value)?;

                let names_value = map.get("names");
                if let Some(names_value) = names_value {
                    let Some(names) = names_value.as_array() else {
                        return Err(eyre::eyre!("'names' must be a list of strings"));
                    };

                    let mut name_literals = Vec::new();
                    for name in names {
                        let Some(name_str) = name.as_str() else {
                            return Err(eyre::eyre!("'names' entries must be strings"));
                        };
                        name_literals.push(format!("\"{}\"", escape_moo_string(name_str)));
                    }

                    items.push(format!(
                        "{{{}, {{{}}}}}",
                        obj_literal,
                        name_literals.join(", ")
                    ));
                } else {
                    items.push(obj_literal);
                }
            }
            _ => return Err(eyre::eyre!("Invalid environment entry")),
        }
    }

    Ok(format!("{{{}}}", items.join(", ")))
}

fn build_parsed_spec_literal(value: &Value) -> Result<String> {
    let Some(map) = value.as_object() else {
        return Err(eyre::eyre!("'parsed_spec' must be a map"));
    };

    let mut pairs = Vec::new();
    for (key, val) in map {
        let value_literal = parsed_spec_value_literal(key, val)?;
        let key_literal = format!("\"{}\"", escape_moo_string(key));
        pairs.push(format!("{} -> {}", key_literal, value_literal));
        if should_include_symbol_key(key) {
            pairs.push(format!("'{} -> {}", key, value_literal));
        }
    }

    Ok(format!("[{}]", pairs.join(", ")))
}

fn parsed_spec_value_literal(key: &str, value: &Value) -> Result<String> {
    match key {
        "verb" => {
            let verb = value
                .as_str()
                .ok_or_else(|| eyre::eyre!("'verb' must be a string"))?;
            Ok(format!("tosym(\"{}\")", escape_moo_string(verb)))
        }
        "dobj" | "iobj" => {
            if value.is_null() {
                Ok("#-1".to_string())
            } else {
                object_ref_literal(value)
            }
        }
        "ambiguous_dobj" | "ambiguous_iobj" => build_object_list_literal(value),
        "prep" => {
            if let Some(num) = value.as_i64() {
                Ok(num.to_string())
            } else {
                Err(eyre::eyre!("'prep' must be an integer"))
            }
        }
        "argstr" | "dobjstr" | "prepstr" | "iobjstr" => {
            let s = value
                .as_str()
                .ok_or_else(|| eyre::eyre!("'{}' must be a string", key))?;
            Ok(format!("\"{}\"", escape_moo_string(s)))
        }
        "args" => {
            let Some(args) = value.as_array() else {
                return Err(eyre::eyre!("'args' must be a list"));
            };
            let mut arg_literals = Vec::new();
            for arg in args {
                if let Some(s) = arg.as_str() {
                    arg_literals.push(format!("\"{}\"", escape_moo_string(s)));
                } else {
                    arg_literals.push(format_var_as_literal(&json_to_var(arg)));
                }
            }
            Ok(format!("{{{}}}", arg_literals.join(", ")))
        }
        _ => Ok(format_var_as_literal(&json_to_var(value))),
    }
}

fn build_object_list_literal(value: &Value) -> Result<String> {
    if value.is_null() {
        return Ok("{}".to_string());
    }
    let Some(list) = value.as_array() else {
        return Err(eyre::eyre!("Expected list of object references"));
    };
    let mut entries = Vec::new();
    for entry in list {
        entries.push(object_ref_literal(entry)?);
    }
    Ok(format!("{{{}}}", entries.join(", ")))
}

fn should_include_symbol_key(key: &str) -> bool {
    matches!(
        key,
        "verb"
            | "argstr"
            | "args"
            | "dobj"
            | "dobjstr"
            | "ambiguous_dobj"
            | "prep"
            | "prepstr"
            | "iobj"
            | "iobjstr"
            | "ambiguous_iobj"
    )
}
