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

//! Completion provider for MOO source files.

use moor_common::builtins::{ArgCount, ArgType, BUILTINS};
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Documentation};

use crate::parsing::{parse_object_member_ref, ObjectMemberRef};

/// Generate documentation string for a builtin function.
fn format_builtin_doc(
    name: &str,
    min_args: &ArgCount,
    max_args: &ArgCount,
    types: &[ArgType],
) -> String {
    let args_str = match (min_args, max_args) {
        (ArgCount::Q(min), ArgCount::Q(max)) if min == max => format!("{} args", min),
        (ArgCount::Q(min), ArgCount::Q(max)) => format!("{}-{} args", min, max),
        (ArgCount::Q(min), ArgCount::U) => format!("{} or more args", min),
        (ArgCount::U, _) => "variadic".to_string(),
    };

    let type_hints: Vec<String> = types
        .iter()
        .map(|t| match t {
            ArgType::Typed(var_type) => format!("{:?}", var_type),
            ArgType::Any => "any".to_string(),
            ArgType::AnyNum => "num".to_string(),
        })
        .collect();

    let types_str = if type_hints.is_empty() {
        String::new()
    } else {
        format!("\n\nArgument types: {}", type_hints.join(", "))
    };

    format!(
        "**{}**\n\nBuiltin function ({}){}",
        name, args_str, types_str
    )
}

/// Get completion items for all MOO builtin functions.
pub fn get_builtin_completions() -> Vec<CompletionItem> {
    BUILTINS
        .descriptions()
        .filter(|b| b.implemented)
        .map(|builtin| {
            let name = builtin.name.to_string();
            let doc =
                format_builtin_doc(&name, &builtin.min_args, &builtin.max_args, &builtin.types);

            CompletionItem {
                label: name.clone(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some("MOO builtin function".to_string()),
                documentation: Some(Documentation::String(doc)),
                insert_text: Some(format!("{}(", name)),
                ..Default::default()
            }
        })
        .collect()
}

/// Represents the context for completion based on what was typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionContext {
    /// User is typing after `$foo:` - suggest verbs
    VerbCompletion {
        object_name: String,
        partial: String,
    },
    /// User is typing after `$foo.` - suggest properties
    PropertyCompletion {
        object_name: String,
        partial: String,
    },
    /// No special context detected
    None,
}

/// Parse the line to determine the completion context.
///
/// Looks for patterns like:
/// - `$foo:` or `$foo:bar` -> VerbCompletion
/// - `$foo.` or `$foo.baz` -> PropertyCompletion
pub fn parse_completion_context(line: &str, character: u32) -> CompletionContext {
    // Use require_member=false to allow partial/empty member names for completion
    let Some(member_ref) = parse_object_member_ref(line, character as usize, false) else {
        return CompletionContext::None;
    };

    match member_ref {
        ObjectMemberRef::Partial {
            object_name,
            is_verb: true,
            partial_name,
        } => CompletionContext::VerbCompletion {
            object_name,
            partial: partial_name,
        },
        ObjectMemberRef::Partial {
            object_name,
            is_verb: false,
            partial_name,
        } => CompletionContext::PropertyCompletion {
            object_name,
            partial: partial_name,
        },
        // Verb/Property variants have complete names - still valid for completion
        ObjectMemberRef::Verb {
            object_name,
            verb_name,
        } => CompletionContext::VerbCompletion {
            object_name,
            partial: verb_name,
        },
        ObjectMemberRef::Property {
            object_name,
            prop_name,
        } => CompletionContext::PropertyCompletion {
            object_name,
            partial: prop_name,
        },
    }
}

/// Get verb completions from the mooR server for a specific object.
///
/// Queries the server for all verbs on the object (including inherited)
/// and returns completion items for each.
pub async fn get_verb_completions(
    client: &mut crate::client::MoorClient,
    obj: moor_var::Obj,
    partial: &str,
) -> Vec<CompletionItem> {
    let object_ref = moor_common::model::ObjectRef::Id(obj);

    let verbs = match client.list_verbs(&object_ref, true).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("Failed to list verbs: {}", e);
            return Vec::new();
        }
    };

    verbs
        .into_iter()
        .filter(|v| {
            if partial.is_empty() {
                true
            } else {
                // Check if any of the verb's names start with the partial
                v.name
                    .split_whitespace()
                    .any(|n| n.to_lowercase().starts_with(&partial.to_lowercase()))
            }
        })
        .flat_map(|verb| {
            // A verb can have multiple names (aliases), create completion for each
            verb.name
                .split_whitespace()
                .filter_map(|name| {
                    if !partial.is_empty()
                        && !name.to_lowercase().starts_with(&partial.to_lowercase())
                    {
                        return None;
                    }

                    let doc = format!(
                        "**Verb** `{}`\n\n\
                     | Property | Value |\n\
                     |----------|-------|\n\
                     | Argspec | `{}` |\n\
                     | Owner | `{}` |\n\
                     | Flags | `{}` |",
                        verb.name, verb.args, verb.owner, verb.flags,
                    );

                    Some(CompletionItem {
                        label: name.to_string(),
                        kind: Some(CompletionItemKind::METHOD),
                        detail: Some(format!("verb ({})", verb.args)),
                        documentation: Some(Documentation::MarkupContent(
                            tower_lsp::lsp_types::MarkupContent {
                                kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                                value: doc,
                            },
                        )),
                        insert_text: Some(format!("{}(", name)),
                        ..Default::default()
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Get property completions from the mooR server for a specific object.
///
/// Queries the server for all properties on the object (including inherited)
/// and returns completion items for each.
pub async fn get_property_completions(
    client: &mut crate::client::MoorClient,
    obj: moor_var::Obj,
    partial: &str,
) -> Vec<CompletionItem> {
    let object_ref = moor_common::model::ObjectRef::Id(obj);

    let props = match client.list_properties(&object_ref, true).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to list properties: {}", e);
            return Vec::new();
        }
    };

    props
        .into_iter()
        .filter(|p| {
            if partial.is_empty() {
                true
            } else {
                p.name.to_lowercase().starts_with(&partial.to_lowercase())
            }
        })
        .map(|prop| {
            let doc = format!(
                "**Property** `{}`\n\n\
                 | Property | Value |\n\
                 |----------|-------|\n\
                 | Owner | `{}` |\n\
                 | Flags | `{}` |",
                prop.name, prop.owner, prop.flags,
            );

            CompletionItem {
                label: prop.name.clone(),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some(format!("property ({})", prop.flags)),
                documentation: Some(Documentation::MarkupContent(
                    tower_lsp::lsp_types::MarkupContent {
                        kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                        value: doc,
                    },
                )),
                insert_text: Some(prop.name),
                ..Default::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_builtin_completions() {
        let completions = get_builtin_completions();

        // Should have many completions
        assert!(
            completions.len() > 50,
            "Should have many builtin completions"
        );

        // Check that common builtins are present
        let names: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();
        assert!(names.contains(&"typeof"), "Should contain typeof");
        assert!(names.contains(&"length"), "Should contain length");
        assert!(names.contains(&"notify"), "Should contain notify");
        assert!(names.contains(&"tostr"), "Should contain tostr");
        assert!(names.contains(&"valid"), "Should contain valid");
    }

    #[test]
    fn test_completion_item_structure() {
        let completions = get_builtin_completions();

        // Find the 'length' builtin
        let length = completions
            .iter()
            .find(|c| c.label == "length")
            .expect("length should exist");

        assert_eq!(length.kind, Some(CompletionItemKind::FUNCTION));
        assert!(length.detail.is_some());
        assert!(length.documentation.is_some());
        assert_eq!(length.insert_text, Some("length(".to_string()));
    }

    #[test]
    fn test_format_builtin_doc() {
        let doc = format_builtin_doc("test", &ArgCount::Q(1), &ArgCount::Q(2), &[ArgType::Any]);
        assert!(doc.contains("**test**"));
        assert!(doc.contains("1-2 args"));
    }
}
