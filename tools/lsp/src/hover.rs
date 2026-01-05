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

//! Hover provider for MOO source files.

use moor_common::builtins::{ArgCount, ArgType, BUILTINS};
use moor_common::model::{obj_flags_string, prop_flags_string, ObjFlag, PropFlag, VerbFlag};
use moor_common::util::BitEnum;
use moor_compiler::{
    CompileOptions, ObjFileContext, ObjPropDef, ObjVerbDef, ObjectDefinition,
    compile_object_definitions,
};
use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use crate::parsing::{parse_object_member_ref, ObjectMemberRef};

/// Format object flags as a human-readable string for display.
fn format_obj_flags(flags: BitEnum<ObjFlag>) -> String {
    let s = obj_flags_string(flags);
    if s.is_empty() {
        "none".to_string()
    } else {
        // Convert flag chars to readable names
        s.chars()
            .filter_map(|c| match c {
                'w' => Some("wizard"),
                'p' => Some("programmer"),
                'u' => Some("player"),
                'f' => Some("fertile"),
                'r' => Some("readable"),
                'W' => Some("writable"),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Format verb flags for display.
fn format_verb_flags(flags: BitEnum<VerbFlag>) -> String {
    use moor_common::model::verb_perms_string;
    let s = verb_perms_string(flags);
    if s.is_empty() {
        "\"\"".to_string()
    } else {
        format!("\"{}\"", s)
    }
}

/// Format an argument type for display.
fn format_arg_type(arg_type: &ArgType) -> &'static str {
    match arg_type {
        ArgType::Typed(var_type) => match var_type {
            moor_var::VarType::TYPE_INT => "int",
            moor_var::VarType::TYPE_OBJ => "obj",
            moor_var::VarType::TYPE_STR => "str",
            moor_var::VarType::TYPE_ERR => "err",
            moor_var::VarType::TYPE_LIST => "list",
            moor_var::VarType::TYPE_FLOAT => "float",
            moor_var::VarType::TYPE_MAP => "map",
            moor_var::VarType::TYPE_BOOL => "bool",
            moor_var::VarType::TYPE_SYMBOL => "symbol",
            moor_var::VarType::TYPE_BINARY => "binary",
            moor_var::VarType::TYPE_FLYWEIGHT => "flyweight",
            _ => "any",
        },
        ArgType::Any => "any",
        ArgType::AnyNum => "num",
    }
}

/// Generate hover content for a builtin function.
pub fn get_builtin_hover(name: &str) -> Option<String> {
    let builtin_id = BUILTINS.find_builtin(moor_var::Symbol::mk(name))?;
    let builtin = BUILTINS.description_for(builtin_id)?;

    // Build the signature
    let min_args = match builtin.min_args {
        ArgCount::Q(n) => n,
        ArgCount::U => 0,
    };
    let max_args = match builtin.max_args {
        ArgCount::Q(n) => Some(n),
        ArgCount::U => None, // unlimited
    };

    // Build argument list
    let mut args = Vec::new();
    for (i, arg_type) in builtin.types.iter().enumerate() {
        let type_str = format_arg_type(arg_type);
        let arg_name = format!("arg{}", i + 1);
        if i >= min_args {
            if max_args.is_none() && i == builtin.types.len() - 1 {
                args.push(format!("[{} {}, ...]", type_str, arg_name));
            } else {
                args.push(format!("[{} {}]", type_str, arg_name));
            }
        } else {
            args.push(format!("{} {}", type_str, arg_name));
        }
    }

    // If unlimited args and we have types, show the variadic nature
    if max_args.is_none() && args.is_empty() {
        args.push("...".to_string());
    }

    let signature = format!("{}({})", name, args.join(", "));

    let implemented_str = if builtin.implemented {
        "✓ Implemented"
    } else {
        "✗ Not implemented"
    };

    Some(format!(
        "**builtin** `{}`\n\n```moo\n{}\n```\n\n{}",
        name, signature, implemented_str
    ))
}

/// Format property flags for display.
fn format_prop_flags(flags: BitEnum<PropFlag>) -> String {
    let s = prop_flags_string(flags);
    if s.is_empty() {
        "\"\"".to_string()
    } else {
        format!("\"{}\"", s)
    }
}

/// Generate hover content for an object definition.
fn object_hover(obj: &ObjectDefinition) -> String {
    format!(
        "**object** `{}`\n\n\
         | Property | Value |\n\
         |----------|-------|\n\
         | Object ID | `{}` |\n\
         | Parent | `{}` |\n\
         | Owner | `{}` |\n\
         | Location | `{}` |\n\
         | Flags | {} |",
        obj.name,
        obj.oid,
        obj.parent,
        obj.owner,
        obj.location,
        format_obj_flags(obj.flags)
    )
}

/// Generate hover content for a verb definition.
fn verb_hover(verb: &ObjVerbDef) -> String {
    let names: Vec<_> = verb.names.iter().map(|s| format!("`{}`", s)).collect();
    format!(
        "**verb** {}\n\n\
         | Property | Value |\n\
         |----------|-------|\n\
         | Argspec | `{:?}` |\n\
         | Owner | `{}` |\n\
         | Flags | {} |",
        names.join(", "),
        verb.argspec,
        verb.owner,
        format_verb_flags(verb.flags)
    )
}

/// Generate hover content for a property definition.
fn property_hover(prop: &ObjPropDef) -> String {
    format!(
        "**property** `{}`\n\n\
         | Property | Value |\n\
         |----------|-------|\n\
         | Owner | `{}` |\n\
         | Flags | {} |",
        prop.name,
        prop.perms.owner(),
        format_prop_flags(prop.perms.flags())
    )
}

/// Determine what kind of definition the given line contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineType {
    Object,
    Verb,
    Property,
    Other,
}

/// Analyze a line to determine its type.
fn classify_line(line: &str) -> LineType {
    let trimmed = line.trim();
    if trimmed.starts_with("object ") {
        LineType::Object
    } else if trimmed.starts_with("verb ") {
        LineType::Verb
    } else if trimmed.starts_with("property ") {
        LineType::Property
    } else {
        LineType::Other
    }
}

/// Get hover information for the given source at the specified position.
pub fn get_hover(source: &str, position: Position) -> Option<Hover> {
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = position.line as usize;

    if line_idx >= lines.len() {
        return None;
    }

    let line = lines[line_idx];
    let line_type = classify_line(line);

    if line_type == LineType::Other {
        return None;
    }

    // Parse the source to get structured definitions
    let options = CompileOptions::default();
    let mut context = ObjFileContext::default();
    let definitions = compile_object_definitions(source, &options, &mut context).ok()?;

    // Find which object we're in by scanning backwards
    let mut current_object: Option<&ObjectDefinition> = None;
    for obj in &definitions {
        // Check if this object contains our line
        // Since we don't have exact line numbers in the AST, we need to use heuristics
        // For now, we'll find the object whose name appears before our position
        for (idx, l) in lines.iter().enumerate() {
            if idx > line_idx {
                break;
            }
            let trimmed = l.trim();
            if trimmed.starts_with("object ") && trimmed.contains(&format!("{}", obj.oid)) {
                current_object = Some(obj);
            }
        }
    }

    let content = match line_type {
        LineType::Object => {
            // Find the object that matches this line
            let trimmed = line.trim();
            definitions
                .iter()
                .find(|obj| trimmed.contains(&format!("{}", obj.oid)))
                .map(object_hover)
        }
        LineType::Verb => {
            // Find the verb that matches this line
            let obj = current_object?;
            // Extract verb name from the line
            let trimmed = line.trim();
            // verb "name" (argspec) or verb name,alias (argspec)
            obj.verbs
                .iter()
                .find(|verb| {
                    verb.names.iter().any(|name| {
                        trimmed.contains(&format!("\"{}\"", name))
                            || trimmed.contains(&format!(" {} ", name))
                            || trimmed.contains(&format!(" {},", name))
                            || trimmed.contains(&format!(",{} ", name))
                    })
                })
                .map(verb_hover)
        }
        LineType::Property => {
            // Find the property that matches this line
            let obj = current_object?;
            let trimmed = line.trim();
            // property "name" or property name
            // The name can appear as: property "name", property name owner:, property name flags:
            obj.property_definitions
                .iter()
                .find(|prop| {
                    let name_str = prop.name.to_string();
                    trimmed.contains(&format!("\"{}\"", name_str))
                        || trimmed.starts_with(&format!("property {} ", name_str))
                        || trimmed.starts_with(&format!("property \"{}\" ", name_str))
                })
                .map(property_hover)
        }
        LineType::Other => None,
    };

    content.map(|text| Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: text,
        }),
        range: None,
    })
}

/// Get hover information from the mooR server for a verb or property reference.
///
/// When hovering over `$foo:bar_verb`, this resolves `$foo` via the ObjectNameRegistry,
/// then queries the server for verb information.
///
/// Returns None if:
/// - The line doesn't contain a recognized object member reference
/// - The object name can't be resolved
/// - The server query fails
pub async fn get_hover_from_server(
    line: &str,
    position: Position,
    client: &mut crate::client::MoorClient,
    object_names: &crate::objects::ObjectNameRegistry,
) -> Option<Hover> {
    // Use require_member=true since we need a complete reference for hover
    let member_ref = parse_object_member_ref(line, position.character as usize, true)?;

    let object_name = member_ref.object_name();
    let is_verb = member_ref.is_verb();

    // Resolve the object name to an Obj
    let obj = object_names.resolve(object_name)?;
    let object_ref = moor_common::model::ObjectRef::Id(obj);

    let content = if is_verb {
        let verb_name = match &member_ref {
            ObjectMemberRef::Verb { verb_name, .. } => verb_name,
            _ => return None,
        };

        // First, get verb info from the list to show argspec, flags, owner
        let verbs = client.list_verbs(&object_ref, true).await.ok()?;
        let verb_info = verbs.iter().find(|v| {
            // Verb names can be space-separated aliases
            v.name.split_whitespace().any(|n| n == verb_name)
        })?;

        // Try to get the verb code for a preview
        let code_preview = match client.get_verb(&object_ref, verb_name).await {
            Ok(verb_code) => {
                let preview_lines: Vec<&str> =
                    verb_code.code.iter().take(5).map(|s| s.as_str()).collect();
                if verb_code.code.len() > 5 {
                    format!(
                        "```moo\n{}\n... ({} more lines)\n```",
                        preview_lines.join("\n"),
                        verb_code.code.len() - 5
                    )
                } else if !preview_lines.is_empty() {
                    format!("```moo\n{}\n```", preview_lines.join("\n"))
                } else {
                    String::new()
                }
            }
            Err(_) => String::new(),
        };

        let mut result = format!(
            "**verb** `${}:{}`\n\n\
             | Property | Value |\n\
             |----------|-------|\n\
             | Names | `{}` |\n\
             | Argspec | `{}` |\n\
             | Owner | `{}` |\n\
             | Flags | `{}` |",
            object_name,
            verb_name,
            verb_info.name,
            verb_info.args,
            verb_info.owner,
            verb_info.flags,
        );

        if !code_preview.is_empty() {
            result.push_str("\n\n**Code Preview:**\n\n");
            result.push_str(&code_preview);
        }

        result
    } else {
        let prop_name = match &member_ref {
            ObjectMemberRef::Property { prop_name, .. } => prop_name,
            _ => return None,
        };

        // Get property info
        let props = client.list_properties(&object_ref, true).await.ok()?;
        let prop_info = props.iter().find(|p| p.name == *prop_name)?;

        // Try to get the property value
        let value_str = match client.get_property(&object_ref, prop_name).await {
            Ok(value) => format!("`{:?}`", value),
            Err(_) => "(unable to retrieve)".to_string(),
        };

        format!(
            "**property** `${}.{}`\n\n\
             | Property | Value |\n\
             |----------|-------|\n\
             | Owner | `{}` |\n\
             | Flags | `{}` |\n\
             | Value | {} |",
            object_name, prop_name, prop_info.owner, prop_info.flags, value_str,
        )
    };

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content,
        }),
        range: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_hover() {
        // Test a simple builtin
        let hover = get_builtin_hover("abs");
        assert!(hover.is_some());
        let content = hover.unwrap();
        assert!(content.contains("**builtin** `abs`"));
        assert!(content.contains("abs("));
        assert!(content.contains("✓ Implemented"));
    }

    #[test]
    fn test_builtin_hover_not_implemented() {
        // Test a builtin that's not implemented
        let hover = get_builtin_hover("encode_binary");
        assert!(hover.is_some());
        let content = hover.unwrap();
        assert!(content.contains("**builtin** `encode_binary`"));
        assert!(content.contains("✗ Not implemented"));
    }

    #[test]
    fn test_builtin_hover_unknown() {
        // Test an unknown builtin
        let hover = get_builtin_hover("not_a_real_builtin");
        assert!(hover.is_none());
    }

    #[test]
    fn test_classify_line() {
        assert_eq!(classify_line("object #1"), LineType::Object);
        assert_eq!(
            classify_line("  verb \"look\" (this none this)"),
            LineType::Verb
        );
        assert_eq!(
            classify_line("    property description"),
            LineType::Property
        );
        assert_eq!(classify_line("    return 1;"), LineType::Other);
    }

    #[test]
    fn test_hover_on_object() {
        let source = r#"
object #1
    parent: #1
    name: "Test Object"
    location: #1
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true
endobject
"#;
        let hover = get_hover(
            source,
            Position {
                line: 1,
                character: 0,
            },
        );
        assert!(hover.is_some());
        let content = match hover.unwrap().contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("Expected markup content"),
        };
        assert!(content.contains("**object**"));
        assert!(content.contains("#1"));
    }

    #[test]
    fn test_hover_on_verb() {
        let source = r#"
object #1
    parent: #1
    name: "Test Object"
    location: #1
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true

    verb "look" (this none this) owner: #1 flags: "rxd"
        return "You look around.";
    endverb
endobject
"#;
        let hover = get_hover(
            source,
            Position {
                line: 11,
                character: 5,
            },
        );
        assert!(hover.is_some());
        let content = match hover.unwrap().contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("Expected markup content"),
        };
        assert!(content.contains("**verb**"));
        assert!(content.contains("look"));
    }

    #[test]
    fn test_hover_on_property() {
        let source = r#"
object #1
    parent: #1
    name: "Test Object"
    location: #1
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true

    property description (owner: #1, flags: "rc") = "A test object";
endobject
"#;
        // Line 11 is the property line (0-indexed)
        let hover = get_hover(
            source,
            Position {
                line: 11,
                character: 5,
            },
        );
        assert!(hover.is_some());
        let content = match hover.unwrap().contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("Expected markup content"),
        };
        assert!(content.contains("**property**"));
        assert!(content.contains("description"));
    }

    #[test]
    fn test_no_hover_on_code() {
        let source = r#"
object #1
    parent: #1
    name: "Test Object"
    location: #1
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true

    verb "look" (this none this) owner: #1 flags: "rxd"
        return "You look around.";
    endverb
endobject
"#;
        let hover = get_hover(
            source,
            Position {
                line: 12,
                character: 10,
            },
        );
        assert!(hover.is_none());
    }
}
