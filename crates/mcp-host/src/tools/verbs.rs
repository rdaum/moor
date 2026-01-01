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

//! Verb tools: list, get, program, apply_patch, add, delete, find_definition, set_verb_info, set_verb_args

use crate::mcp_types::{Tool, ToolCallResult};
use crate::moor_client::{MoorClient, MoorResult};
use diffy::{Patch, apply};
use eyre::Result;
use moor_var::Sequence;
use serde_json::{Value, json};

use super::helpers::{format_var, parse_object_ref, var_key_eq};

// ============================================================================
// Tool Definitions
// ============================================================================

pub fn tool_moo_list_verbs() -> Tool {
    Tool {
        name: "moo_list_verbs".to_string(),
        description: "List all verbs defined on an object. Can optionally include inherited verbs \
            from parent objects."
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
                    "description": "Include inherited verbs from parent objects (default: false)"
                }
            },
            "required": ["object"]
        }),
    }
}

pub fn tool_moo_get_verb() -> Tool {
    Tool {
        name: "moo_get_verb".to_string(),
        description: "Get a verb's source code and metadata (owner, flags, argument spec). \
            Returns the MOO code that defines the verb's behavior along with its configuration."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#0', '$player')"
                },
                "verb": {
                    "type": "string",
                    "description": "Verb name to retrieve"
                }
            },
            "required": ["object", "verb"]
        }),
    }
}

pub fn tool_moo_program_verb() -> Tool {
    Tool {
        name: "moo_program_verb".to_string(),
        description: "Program (compile and save) a verb with new MOO code. The code will be \
            compiled and if successful, the verb will be updated. Requires programmer permissions."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#0', '$player')"
                },
                "verb": {
                    "type": "string",
                    "description": "Verb name to program"
                },
                "code": {
                    "type": "string",
                    "description": "MOO source code for the verb (newline-separated lines)"
                }
            },
            "required": ["object", "verb", "code"]
        }),
    }
}

pub fn tool_moo_apply_patch_verb() -> Tool {
    Tool {
        name: "moo_apply_patch_verb".to_string(),
        description: "Apply a unified diff patch to a verb's source code. The server fetches the \
            current verb, applies the patch, and reprograms it. Requires programmer permissions."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#0', '$player')"
                },
                "verb": {
                    "type": "string",
                    "description": "Verb name to patch"
                },
                "patch": {
                    "type": "string",
                    "description": "Unified diff patch to apply"
                }
            },
            "required": ["object", "verb", "patch"]
        }),
    }
}

pub fn tool_moo_add_verb() -> Tool {
    Tool {
        name: "moo_add_verb".to_string(),
        description: "Add a new verb to an object. Creates a verb with the specified name, \
            permissions, and argument specification. Requires programmer permissions."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference to add the verb to (e.g., '#123', '$player')"
                },
                "name": {
                    "type": "string",
                    "description": "Verb name(s), space-separated for aliases (e.g., 'look l' or 'get take')"
                },
                "permissions": {
                    "type": "string",
                    "description": "Permission flags: r=read, w=write, x=execute, d=debug (e.g., 'rxd')",
                    "default": "rxd"
                },
                "dobj": {
                    "type": "string",
                    "description": "Direct object spec: 'this', 'any', or 'none'",
                    "default": "none"
                },
                "prep": {
                    "type": "string",
                    "description": "Preposition: 'any', 'none', or specific like 'with/using', 'in/inside', etc.",
                    "default": "none"
                },
                "iobj": {
                    "type": "string",
                    "description": "Indirect object spec: 'this', 'any', or 'none'",
                    "default": "none"
                }
            },
            "required": ["object", "name"]
        }),
    }
}

pub fn tool_moo_delete_verb() -> Tool {
    Tool {
        name: "moo_delete_verb".to_string(),
        description: "Delete a verb from an object. Requires programmer permissions and \
            ownership or wizard status."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#123', '$player')"
                },
                "verb": {
                    "type": "string",
                    "description": "Verb name to delete"
                }
            },
            "required": ["object", "verb"]
        }),
    }
}

pub fn tool_moo_find_verb_definition() -> Tool {
    Tool {
        name: "moo_find_verb_definition".to_string(),
        description: "Find where a verb is actually defined in the inheritance hierarchy. \
            Given an object and verb name, walks up the parent chain to find the object \
            that defines the verb. Useful for understanding inherited behavior."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object to start searching from (e.g., '#123', '$player')"
                },
                "verb": {
                    "type": "string",
                    "description": "Verb name to find"
                }
            },
            "required": ["object", "verb"]
        }),
    }
}

pub fn tool_moo_set_verb_info() -> Tool {
    Tool {
        name: "moo_set_verb_info".to_string(),
        description: "Set a verb's metadata (owner, permissions, names) without reprogramming it. \
            This allows changing verb permissions or adding aliases without modifying the code."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#123', '$player')"
                },
                "verb": {
                    "type": "string",
                    "description": "Current verb name"
                },
                "owner": {
                    "type": "string",
                    "description": "New owner object reference (optional, keeps current if not specified)"
                },
                "permissions": {
                    "type": "string",
                    "description": "New permission flags: r=read, w=write, x=execute, d=debug (optional)"
                },
                "names": {
                    "type": "string",
                    "description": "New verb name(s), space-separated for aliases (optional)"
                }
            },
            "required": ["object", "verb"]
        }),
    }
}

pub fn tool_moo_set_verb_args() -> Tool {
    Tool {
        name: "moo_set_verb_args".to_string(),
        description:
            "Set a verb's argument specification (dobj, prep, iobj) without reprogramming it. \
            This controls how the verb matches commands."
                .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "object": {
                    "type": "string",
                    "description": "Object reference (e.g., '#123', '$player')"
                },
                "verb": {
                    "type": "string",
                    "description": "Verb name"
                },
                "dobj": {
                    "type": "string",
                    "description": "Direct object spec: 'this', 'any', or 'none'"
                },
                "prep": {
                    "type": "string",
                    "description": "Preposition: 'any', 'none', or specific like 'with/using', 'in/inside', etc."
                },
                "iobj": {
                    "type": "string",
                    "description": "Indirect object spec: 'this', 'any', or 'none'"
                }
            },
            "required": ["object", "verb", "dobj", "prep", "iobj"]
        }),
    }
}

// ============================================================================
// Tool Implementations
// ============================================================================

pub async fn execute_moo_list_verbs(
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

    let verbs = client.list_verbs(&object, inherited).await?;
    let mut output = String::new();
    output.push_str(&format!("Verbs on {}:\n\n", object_str));
    for verb in &verbs {
        output.push_str(&format!(
            "  {}: owner={}, flags={}, args={}\n",
            verb.name, verb.owner, verb.flags, verb.args
        ));
    }
    Ok(ToolCallResult::text(output))
}

pub async fn execute_moo_get_verb(client: &mut MoorClient, args: &Value) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let verb_name = args
        .get("verb")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'verb' parameter"))?;

    let object = parse_object_ref(object_str)
        .ok_or_else(|| eyre::eyre!("Invalid object reference: {}", object_str))?;

    // Get the verb code via RPC
    let verb = client.get_verb(&object, verb_name).await?;

    // Get verb metadata via eval
    let escaped_name = verb_name.replace('\\', "\\\\").replace('"', "\\\"");
    let expr = format!(
        r#"info = verb_info({}, "{}"); return ["owner" -> info[1], "flags" -> info[2], "names" -> info[3]];"#,
        object_str, escaped_name
    );

    let mut output = String::new();
    output.push_str(&format!("## Verb `{}:{}`\n\n", object_str, verb.name));

    // Try to get metadata, but don't fail if we can't
    if let Ok(MoorResult::Success(var)) = client.eval(&expr).await
        && let Some(map) = var.as_map()
    {
        let owner = map
            .iter()
            .find(|(k, _)| var_key_eq(k, "owner"))
            .map(|(_, v)| format_var(&v))
            .unwrap_or_default();
        let flags = map
            .iter()
            .find(|(k, _)| var_key_eq(k, "flags"))
            .map(|(_, v)| format_var(&v))
            .unwrap_or_default();
        let names = map
            .iter()
            .find(|(k, _)| var_key_eq(k, "names"))
            .map(|(_, v)| format_var(&v))
            .unwrap_or_default();
        output.push_str(&format!(
            "**Owner:** {} | **Flags:** {} | **Names:** {}\n\n",
            owner, flags, names
        ));
    }

    output.push_str("```moo\n");
    for line in &verb.code {
        output.push_str(line);
        output.push('\n');
    }
    output.push_str("```\n");
    Ok(ToolCallResult::markdown(output))
}

pub async fn execute_moo_program_verb(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let verb_name = args
        .get("verb")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'verb' parameter"))?;

    let code_str = args
        .get("code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'code' parameter"))?;

    let object = parse_object_ref(object_str)
        .ok_or_else(|| eyre::eyre!("Invalid object reference: {}", object_str))?;

    let code: Vec<String> = code_str.lines().map(|s| s.to_string()).collect();
    let line_count = code.len();

    client
        .program_verb(&object, verb_name, code.clone())
        .await?;

    // Format output with markdown
    let mut output = format!(
        "✓ Successfully programmed `{}:{}` ({} lines)\n\n```moo\n",
        object_str, verb_name, line_count
    );
    for line in &code {
        output.push_str(line);
        output.push('\n');
    }
    output.push_str("```\n");
    Ok(ToolCallResult::markdown(output))
}

pub async fn execute_moo_apply_patch_verb(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let verb_name = args
        .get("verb")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'verb' parameter"))?;

    let patch_str = args
        .get("patch")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'patch' parameter"))?;

    let object = parse_object_ref(object_str)
        .ok_or_else(|| eyre::eyre!("Invalid object reference: {}", object_str))?;

    let verb = client.get_verb(&object, verb_name).await?;
    let mut source = verb.code.join("\n");
    if !source.is_empty() {
        source.push('\n');
    }

    let patch =
        Patch::from_str(patch_str).map_err(|err| eyre::eyre!("Invalid patch format: {}", err))?;
    let patched =
        apply(&source, &patch).map_err(|err| eyre::eyre!("Patch failed to apply: {}", err))?;

    let code: Vec<String> = patched.lines().map(|line| line.to_string()).collect();
    let line_count = code.len();

    client.program_verb(&object, verb_name, code).await?;

    Ok(ToolCallResult::text(format!(
        "Successfully applied patch to {}:{} ({} lines)",
        object_str, verb_name, line_count
    )))
}

pub async fn execute_moo_add_verb(client: &mut MoorClient, args: &Value) -> Result<ToolCallResult> {
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
        .unwrap_or("rxd");

    let dobj = args.get("dobj").and_then(|v| v.as_str()).unwrap_or("none");

    let prep = args.get("prep").and_then(|v| v.as_str()).unwrap_or("none");

    let iobj = args.get("iobj").and_then(|v| v.as_str()).unwrap_or("none");

    // Build the MOO expression: add_verb(obj, {owner, perms, names}, {dobj, prep, iobj})
    // Owner will be player (the caller)
    let expr = format!(
        "add_verb({}, {{player, \"{}\", \"{}\"}}, {{\"{}\", \"{}\", \"{}\"}});",
        object_str, permissions, name, dobj, prep, iobj
    );

    match client.eval(&expr).await? {
        MoorResult::Success(_) => Ok(ToolCallResult::text(format!(
            "Successfully added verb '{}' to {}",
            name, object_str
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_delete_verb(
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

    // Build the MOO expression: delete_verb(obj, "verbname")
    let expr = format!("delete_verb({}, \"{}\");", object_str, verb);

    match client.eval(&expr).await? {
        MoorResult::Success(_) => Ok(ToolCallResult::text(format!(
            "Successfully deleted verb '{}' from {}",
            verb, object_str
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_find_verb_definition(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let verb_name = args
        .get("verb")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'verb' parameter"))?;

    // Escape verb name for MOO string
    let escaped_verb = verb_name.replace('\\', "\\\\").replace('"', "\\\"");

    // Walk up the parent chain to find where the verb is defined
    // Note: {} is empty list in MOO, not []. Can't use labeled break, use flag instead.
    let expr = format!(
        r#"
        target = {};
        verb = "{}";
        if (!valid(target))
            return E_INVARG;
        endif
        p = target;
        found = #-1;
        chain = {{}};
        while (valid(p) && !valid(found))
            chain = {{@chain, p}};
            for v in (verbs(p))
                if (index(v, verb) == 1)
                    found = p;
                    break;
                endif
            endfor
            if (!valid(found))
                p = parent(p);
            endif
        endwhile
        if (!valid(found))
            return ["found" -> 0, "chain" -> chain, "verb" -> verb, "target" -> target];
        endif
        info = verb_info(found, verb);
        return ["found" -> 1, "definer" -> found, "definer_name" -> found.name, "chain" -> chain, "verb" -> verb, "target" -> target, "owner" -> info[1], "flags" -> info[2], "names" -> info[3]];
        "#,
        object_str, escaped_verb
    );

    match client.eval(&expr).await? {
        MoorResult::Success(var) => {
            if let Some(map) = var.as_map() {
                let mut output = String::new();

                let found = map
                    .iter()
                    .find(|(k, _)| var_key_eq(k, "found"))
                    .and_then(|(_, v)| v.as_integer())
                    .unwrap_or(0)
                    != 0;

                let target = map
                    .iter()
                    .find(|(k, _)| var_key_eq(k, "target"))
                    .map(|(_, v)| format_var(&v))
                    .unwrap_or_default();
                let verb = map
                    .iter()
                    .find(|(k, _)| var_key_eq(k, "verb"))
                    .map(|(_, v)| format_var(&v))
                    .unwrap_or_default();
                let chain_var = map
                    .iter()
                    .find(|(k, _)| var_key_eq(k, "chain"))
                    .map(|(_, v)| v.clone());

                if found {
                    let definer = map
                        .iter()
                        .find(|(k, _)| var_key_eq(k, "definer"))
                        .map(|(_, v)| format_var(&v))
                        .unwrap_or_default();
                    let definer_name = map
                        .iter()
                        .find(|(k, _)| var_key_eq(k, "definer_name"))
                        .map(|(_, v)| format_var(&v))
                        .unwrap_or_default();
                    let names = map
                        .iter()
                        .find(|(k, _)| var_key_eq(k, "names"))
                        .map(|(_, v)| format_var(&v))
                        .unwrap_or_default();
                    let flags = map
                        .iter()
                        .find(|(k, _)| var_key_eq(k, "flags"))
                        .map(|(_, v)| format_var(&v))
                        .unwrap_or_default();
                    let owner = map
                        .iter()
                        .find(|(k, _)| var_key_eq(k, "owner"))
                        .map(|(_, v)| format_var(&v))
                        .unwrap_or_default();

                    output.push_str(&format!("Verb {} found on {}:\n\n", verb, target));
                    output.push_str(&format!("Defined on: {} {}\n", definer, definer_name));
                    output.push_str(&format!("Names: {}\n", names));
                    output.push_str(&format!("Flags: {}\n", flags));
                    output.push_str(&format!("Owner: {}\n\n", owner));

                    // Show inheritance chain
                    output.push_str("Inheritance chain searched:\n");
                    if let Some(chain_var) = chain_var
                        && let Some(chain) = chain_var.as_list()
                    {
                        for (i, obj) in chain.iter().enumerate() {
                            let marker = if format_var(&obj) == definer {
                                " <- DEFINES"
                            } else {
                                ""
                            };
                            output.push_str(&format!(
                                "  {}. {}{}\n",
                                i + 1,
                                format_var(&obj),
                                marker
                            ));
                        }
                    }
                } else {
                    output.push_str(&format!(
                        "Verb {} NOT FOUND on {} or any ancestor.\n\n",
                        verb, target
                    ));
                    output.push_str("Inheritance chain searched:\n");
                    if let Some(chain_var) = chain_var
                        && let Some(chain) = chain_var.as_list()
                    {
                        for (i, obj) in chain.iter().enumerate() {
                            output.push_str(&format!("  {}. {}\n", i + 1, format_var(&obj)));
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

pub async fn execute_moo_set_verb_info(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let verb_name = args
        .get("verb")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'verb' parameter"))?;

    let escaped_verb = verb_name.replace('\\', "\\\\").replace('"', "\\\"");

    // Get current info first
    let get_info_expr = format!(r#"return verb_info({}, "{}");"#, object_str, escaped_verb);

    let current_info = match client.eval(&get_info_expr).await? {
        MoorResult::Success(var) => {
            if let Some(list) = var.as_list() {
                if list.len() >= 3 {
                    (
                        format_var(&list[0]), // owner
                        format_var(&list[1]), // perms
                        format_var(&list[2]), // names
                    )
                } else {
                    return Ok(ToolCallResult::error("Invalid verb_info result"));
                }
            } else {
                return Ok(ToolCallResult::error("verb_info did not return a list"));
            }
        }
        MoorResult::Error(msg) => return Ok(ToolCallResult::error(msg)),
    };

    // Build new values, using provided or keeping current
    let new_owner = args
        .get("owner")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or(current_info.0);

    let new_perms = args
        .get("permissions")
        .and_then(|v| v.as_str())
        .map(|s| format!("\"{}\"", s))
        .unwrap_or(current_info.1);

    let new_names = args
        .get("names")
        .and_then(|v| v.as_str())
        .map(|s| format!("\"{}\"", s))
        .unwrap_or(current_info.2);

    let set_info_expr = format!(
        r#"set_verb_info({}, "{}", {{{}, {}, {}}});"#,
        object_str, escaped_verb, new_owner, new_perms, new_names
    );

    match client.eval(&set_info_expr).await? {
        MoorResult::Success(_) => Ok(ToolCallResult::text(format!(
            "Successfully updated verb info for {}:{}\n  Owner: {}\n  Permissions: {}\n  Names: {}",
            object_str, verb_name, new_owner, new_perms, new_names
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}

pub async fn execute_moo_set_verb_args(
    client: &mut MoorClient,
    args: &Value,
) -> Result<ToolCallResult> {
    let object_str = args
        .get("object")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'object' parameter"))?;

    let verb_name = args
        .get("verb")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'verb' parameter"))?;

    let dobj = args
        .get("dobj")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'dobj' parameter"))?;

    let prep = args
        .get("prep")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'prep' parameter"))?;

    let iobj = args
        .get("iobj")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Missing 'iobj' parameter"))?;

    let escaped_verb = verb_name.replace('\\', "\\\\").replace('"', "\\\"");

    let expr = format!(
        r#"set_verb_args({}, "{}", {{"{}","{}","{}"}});"#,
        object_str, escaped_verb, dobj, prep, iobj
    );

    match client.eval(&expr).await? {
        MoorResult::Success(_) => Ok(ToolCallResult::text(format!(
            "Successfully updated verb args for {}:{}\n  dobj: {}\n  prep: {}\n  iobj: {}",
            object_str, verb_name, dobj, prep, iobj
        ))),
        MoorResult::Error(msg) => Ok(ToolCallResult::error(msg)),
    }
}
