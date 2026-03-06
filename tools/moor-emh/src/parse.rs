// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use eyre::{Report, bail, eyre};
use moor_compiler::to_literal;
use moor_kernel::SchedulerClient;
use moor_var::{Obj, Symbol};
use tracing::error;

/// Parsed command-line flags for dump/load commands
#[derive(Debug, Default)]
pub(crate) struct ParsedFlags {
    /// Positional arguments (e.g., object references)
    positional: Vec<String>,
    /// Flag values keyed by flag name (without the --)
    flags: std::collections::HashMap<String, Option<String>>,
}

impl ParsedFlags {
    /// Get a boolean flag value (true if present, false if absent)
    pub(crate) fn get_bool(&self, name: &str) -> bool {
        self.flags.contains_key(name)
    }

    /// Get a string flag value
    pub(crate) fn get_string(&self, name: &str) -> Option<&str> {
        self.flags.get(name).and_then(|v| v.as_deref())
    }

    /// Get the first positional argument
    pub(crate) fn first_positional(&self) -> Option<&str> {
        self.positional.first().map(|s| s.as_str())
    }
}

/// Parse command arguments into flags and positional args
/// Supports: --flag, --flag value, --flag=value
pub(crate) fn parse_flags(args: &str) -> ParsedFlags {
    const VALUE_FLAGS: &[&str] = &["file", "constants", "conflict-mode", "as"];

    let mut result = ParsedFlags::default();
    let mut tokens: Vec<String> = Vec::new();

    // Simple tokenization respecting quotes
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape_next = false;

    for ch in args.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '\\' => escape_next = true,
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    // Parse tokens into flags and positional args
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];

        if let Some(flag_part) = token.strip_prefix("--") {
            // Check for --flag=value syntax
            if let Some(eq_pos) = flag_part.find('=') {
                let flag_name = flag_part[..eq_pos].to_string();
                let flag_value = flag_part[eq_pos + 1..].to_string();
                result.flags.insert(flag_name, Some(flag_value));
                i += 1;
            } else {
                // Only known flags consume a separate value token.
                let flag_name = flag_part.to_string();
                let takes_value = VALUE_FLAGS.contains(&flag_name.as_str());
                if takes_value && i + 1 < tokens.len() && !tokens[i + 1].starts_with("--") {
                    result.flags.insert(flag_name, Some(tokens[i + 1].clone()));
                    i += 2;
                } else {
                    // Boolean flag
                    result.flags.insert(flag_name, None);
                    i += 1;
                }
            }
        } else {
            // Positional argument
            result.positional.push(token.clone());
            i += 1;
        }
    }

    result
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplCommand {
    Quit,
    Help,
    EvalExpr(String),
    ExecCode(String),
    Get(String),
    Set(String),
    Props(String),
    Verbs(String),
    Prog(String),
    List(String),
    Dump(String),
    Load(String),
    Reload(String),
    Su(String),
    Unknown,
}

pub(crate) fn parse_repl_command(line: &str) -> ReplCommand {
    if line == "quit" || line == "exit" {
        return ReplCommand::Quit;
    }
    if line == "help" || line == "?" {
        return ReplCommand::Help;
    }
    if let Some(code) = line.strip_prefix(";;") {
        return ReplCommand::ExecCode(code.trim().to_string());
    }
    if let Some(expr) = line.strip_prefix(';') {
        return ReplCommand::EvalExpr(expr.trim().to_string());
    }

    let mut parts = line.splitn(2, char::is_whitespace);
    let command = parts.next().unwrap_or_default();
    let args = parts.next().unwrap_or_default().trim().to_string();

    match command {
        "get" => ReplCommand::Get(args),
        "set" => ReplCommand::Set(args),
        "props" => ReplCommand::Props(args),
        "verbs" => ReplCommand::Verbs(args),
        "prog" => ReplCommand::Prog(args),
        "list" => ReplCommand::List(args),
        "dump" => ReplCommand::Dump(args),
        "load" => ReplCommand::Load(args),
        "reload" => ReplCommand::Reload(args),
        "su" => ReplCommand::Su(args),
        _ => ReplCommand::Unknown,
    }
}

pub(crate) fn ensure_args(args: &str, usage: &str) -> bool {
    if !args.is_empty() {
        return true;
    }
    error!("Usage: {usage}");
    false
}

/// Parse an object reference with optional scheduler client for $property resolution
/// Supports "#123" format and "$player" format (which looks up #0.player)
pub(crate) fn parse_objref_with_scheduler(
    s: &str,
    scheduler_client: Option<&SchedulerClient>,
    wizard: Option<&Obj>,
) -> Result<Obj, Report> {
    let s = s.trim();

    if let Some(prop_name) = s.strip_prefix('$') {
        // $property reference - need scheduler client to resolve
        let Some(client) = scheduler_client else {
            bail!("Cannot resolve $property references without scheduler client");
        };
        let Some(wiz) = wizard else {
            bail!("Cannot resolve $property references without wizard");
        };

        if prop_name.is_empty() {
            bail!("Invalid $property reference: missing property name");
        }

        let system_obj = Obj::mk_id(0);
        let prop_symbol = Symbol::mk(prop_name);

        let value = client
            .request_system_property(
                wiz,
                &moor_common::model::ObjectRef::Id(system_obj),
                prop_symbol,
            )
            .map_err(|e| eyre!("Failed to retrieve property ${}: {:?}", prop_name, e))?;

        let Some(obj) = value.as_object() else {
            bail!(
                "Property ${} is not an object reference (value: {})",
                prop_name,
                to_literal(&value)
            );
        };

        return Ok(obj);
    }

    Obj::try_from(s).map_err(|_| {
        eyre!(
            "Invalid object reference: {} (expected #N, #UUID, UUID, or $property)",
            s
        )
    })
}

/// Parse "#OBJ.PROP" or "$obj.PROP" into (object, property_name)
pub(crate) fn parse_propref(
    s: &str,
    scheduler_client: Option<&SchedulerClient>,
    wizard: Option<&Obj>,
) -> Result<(Obj, Symbol), Report> {
    let parts: Vec<&str> = s.splitn(2, '.').collect();
    if parts.len() != 2 {
        bail!("Property reference must be in format #OBJ.PROP or $obj.PROP");
    }
    let obj = parse_objref_with_scheduler(parts[0], scheduler_client, wizard)?;
    let prop = Symbol::mk(parts[1]);
    Ok((obj, prop))
}

/// Parse "#OBJ:VERB" or "$obj:VERB" into (object, verb_name)
pub(crate) fn parse_verbref(
    s: &str,
    scheduler_client: Option<&SchedulerClient>,
    wizard: Option<&Obj>,
) -> Result<(Obj, Symbol), Report> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        bail!("Verb reference must be in format #OBJ:VERB or $obj:VERB");
    }
    let obj = parse_objref_with_scheduler(parts[0], scheduler_client, wizard)?;
    let verb = Symbol::mk(parts[1]);
    Ok((obj, verb))
}

#[cfg(test)]
mod tests {
    use super::{ReplCommand, parse_flags, parse_objref_with_scheduler, parse_repl_command};

    #[test]
    fn parse_flags_supports_quoted_values() {
        let parsed = parse_flags(r#"--file "path with spaces.moo" --dry-run #42"#);
        assert_eq!(parsed.get_string("file"), Some("path with spaces.moo"));
        assert!(parsed.get_bool("dry-run"));
        assert_eq!(parsed.first_positional(), Some("#42"));
    }

    #[test]
    fn parse_repl_command_handles_eval_forms() {
        assert_eq!(
            parse_repl_command(";; return 1 + 1;"),
            ReplCommand::ExecCode("return 1 + 1;".to_string())
        );
        assert_eq!(
            parse_repl_command("; 1 + 1"),
            ReplCommand::EvalExpr("1 + 1".to_string())
        );
    }

    #[test]
    fn parse_repl_command_handles_verb_commands() {
        assert_eq!(
            parse_repl_command("prog #10:foo"),
            ReplCommand::Prog("#10:foo".to_string())
        );
        assert_eq!(
            parse_repl_command("load --file object.moo"),
            ReplCommand::Load("--file object.moo".to_string())
        );
    }

    #[test]
    fn parse_objref_supports_uuid_with_hash() {
        let obj = parse_objref_with_scheduler("#0001F7-9C5EB40302", None, None).unwrap();
        assert_eq!(obj.to_literal(), "0001F7-9C5EB40302");
    }
}
