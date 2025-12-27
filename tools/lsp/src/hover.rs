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

use moor_common::model::{ObjFlag, PropFlag, VerbFlag};
use moor_common::util::BitEnum;
use moor_compiler::{
    CompileOptions, ObjFileContext, ObjPropDef, ObjVerbDef, ObjectDefinition,
    compile_object_definitions,
};
use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

/// Format object flags as a human-readable string.
fn format_obj_flags(flags: BitEnum<ObjFlag>) -> String {
    let mut parts = Vec::new();
    if flags.contains(ObjFlag::Wizard) {
        parts.push("wizard");
    }
    if flags.contains(ObjFlag::Programmer) {
        parts.push("programmer");
    }
    if flags.contains(ObjFlag::User) {
        parts.push("player");
    }
    if flags.contains(ObjFlag::Fertile) {
        parts.push("fertile");
    }
    if flags.contains(ObjFlag::Read) {
        parts.push("readable");
    }
    if flags.contains(ObjFlag::Write) {
        parts.push("writable");
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

/// Format verb flags as a human-readable string.
fn format_verb_flags(flags: BitEnum<VerbFlag>) -> String {
    let mut parts = Vec::new();
    if flags.contains(VerbFlag::Read) {
        parts.push("r");
    }
    if flags.contains(VerbFlag::Write) {
        parts.push("w");
    }
    if flags.contains(VerbFlag::Exec) {
        parts.push("x");
    }
    if flags.contains(VerbFlag::Debug) {
        parts.push("d");
    }
    if parts.is_empty() {
        "\"\"".to_string()
    } else {
        format!("\"{}\"", parts.join(""))
    }
}

/// Format property flags as a human-readable string.
fn format_prop_flags(flags: BitEnum<PropFlag>) -> String {
    let mut parts = Vec::new();
    if flags.contains(PropFlag::Read) {
        parts.push("r");
    }
    if flags.contains(PropFlag::Write) {
        parts.push("w");
    }
    if flags.contains(PropFlag::Chown) {
        parts.push("c");
    }
    if parts.is_empty() {
        "\"\"".to_string()
    } else {
        format!("\"{}\"", parts.join(""))
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
        obj.name, obj.oid, obj.parent, obj.owner, obj.location, format_obj_flags(obj.flags)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_line() {
        assert_eq!(classify_line("object #1"), LineType::Object);
        assert_eq!(classify_line("  verb \"look\" (this none this)"), LineType::Verb);
        assert_eq!(classify_line("    property description"), LineType::Property);
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
        let hover = get_hover(source, Position { line: 1, character: 0 });
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
        let hover = get_hover(source, Position { line: 11, character: 5 });
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
        let hover = get_hover(source, Position { line: 11, character: 5 });
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
        let hover = get_hover(source, Position { line: 12, character: 10 });
        assert!(hover.is_none());
    }
}
