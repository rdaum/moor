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

//! Find references to symbols in MOO source code.
//!
//! This module re-parses source code using pest to locate all occurrences
//! of identifiers, properties, verbs, and other symbol references with
//! their exact source locations.

use moor_compiler::parse::moo::{MooParser, Rule};
use pest::Parser;
use pest::iterators::Pair;
use tower_lsp::lsp_types::{Location, Position, Range, Url};

/// A reference to a symbol found in source code.
#[derive(Debug, Clone)]
pub struct SymbolReference {
    /// The symbol name.
    pub name: String,
    /// The kind of reference.
    pub kind: ReferenceKind,
    /// Line number (0-based).
    pub line: u32,
    /// Start column (0-based).
    pub start_col: u32,
    /// End column (0-based).
    pub end_col: u32,
}

/// The kind of symbol reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    /// A variable or identifier reference.
    Variable,
    /// A property access (e.g., `.name`).
    Property,
    /// A verb call (e.g., `:foo()`).
    Verb,
    /// A system property/object (e.g., `$player`).
    SysProp,
    /// A builtin function call.
    Builtin,
}

/// Find all symbol references in the given source code.
///
/// This handles both pure MOO program code and .moo object definition files.
/// For object definition files, it extracts verb bodies and parses them separately.
pub fn find_all_references(source: &str) -> Vec<SymbolReference> {
    let mut references = Vec::new();

    // First, try parsing as a MOO program (verb code)
    if let Ok(pairs) = MooParser::parse(Rule::program, source) {
        for pair in pairs {
            collect_references_from_pair(pair, &mut references);
        }
        if !references.is_empty() {
            return references;
        }
    }

    // If that didn't work, try extracting verb bodies from object definition format
    references = find_references_in_object_file(source);

    references
}

/// Find references in an object definition file by extracting verb bodies.
fn find_references_in_object_file(source: &str) -> Vec<SymbolReference> {
    let mut references = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    let mut in_verb = false;
    let mut verb_start_line = 0;
    let mut verb_lines: Vec<&str> = Vec::new();

    for (line_num, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Detect verb start: "verb name (args) owner: ... flags: ..."
        if trimmed.starts_with("verb ") && !in_verb {
            in_verb = true;
            verb_start_line = line_num + 1; // Verb body starts on next line
            verb_lines.clear();
            continue;
        }

        // Detect verb end
        if trimmed == "endverb" && in_verb {
            in_verb = false;
            // Parse the accumulated verb body
            let verb_source = verb_lines.join("\n");
            if let Ok(pairs) = MooParser::parse(Rule::program, &verb_source) {
                for pair in pairs {
                    let mut verb_refs = Vec::new();
                    collect_references_from_pair(pair, &mut verb_refs);
                    // Adjust line numbers to account for verb body offset
                    for mut reference in verb_refs {
                        reference.line += verb_start_line as u32;
                        references.push(reference);
                    }
                }
            }
            continue;
        }

        if in_verb {
            verb_lines.push(line);
        }
    }

    references
}

/// Find all references to a specific symbol name.
pub fn find_references_to(source: &str, symbol_name: &str) -> Vec<SymbolReference> {
    let all_refs = find_all_references(source);
    all_refs
        .into_iter()
        .filter(|r| r.name.eq_ignore_ascii_case(symbol_name))
        .collect()
}

/// Convert a SymbolReference to an LSP Location.
pub fn reference_to_location(reference: &SymbolReference, uri: &Url) -> Location {
    Location {
        uri: uri.clone(),
        range: Range {
            start: Position {
                line: reference.line,
                character: reference.start_col,
            },
            end: Position {
                line: reference.line,
                character: reference.end_col,
            },
        },
    }
}

/// Get the symbol at a specific position in the source.
pub fn symbol_at_position(source: &str, line: u32, character: u32) -> Option<SymbolReference> {
    let references = find_all_references(source);

    references.into_iter().find(|r| {
        r.line == line && r.start_col <= character && character <= r.end_col
    })
}

/// Extract inner ident from a pair and add it as a reference with the given kind.
fn extract_inner_ident(pair: &Pair<Rule>, kind: ReferenceKind, references: &mut Vec<SymbolReference>) {
    for inner in pair.clone().into_inner() {
        if inner.as_rule() == Rule::ident {
            let inner_span = inner.as_span();
            let (inner_line, inner_col) = inner_span.start_pos().line_col();
            references.push(SymbolReference {
                name: inner.as_str().to_string(),
                kind,
                line: (inner_line - 1) as u32,
                start_col: (inner_col - 1) as u32,
                end_col: (inner_col - 1) as u32 + inner.as_str().len() as u32,
            });
        }
    }
}

fn collect_references_from_pair(
    pair: Pair<Rule>,
    references: &mut Vec<SymbolReference>,
) {
    let rule = pair.as_rule();
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let line = (line - 1) as u32; // Convert to 0-based
    let start_col = (col - 1) as u32;
    let end_col = start_col + span.as_str().len() as u32;

    match rule {
        Rule::ident => {
            // Variable or identifier reference
            let name = pair.as_str().to_string();
            references.push(SymbolReference {
                name,
                kind: ReferenceKind::Variable,
                line,
                start_col,
                end_col,
            });
        }
        Rule::sysprop => {
            // System property like $player
            let text = pair.as_str();
            // Strip the leading $
            let name = text.strip_prefix('$').unwrap_or(text).to_string();
            references.push(SymbolReference {
                name,
                kind: ReferenceKind::SysProp,
                line,
                start_col,
                end_col,
            });
        }
        Rule::prop => extract_inner_ident(&pair, ReferenceKind::Property, references),
        Rule::verb_call => extract_inner_ident(&pair, ReferenceKind::Verb, references),
        Rule::builtin_call => extract_inner_ident(&pair, ReferenceKind::Builtin, references),
        _ => {}
    }

    // Recurse into children
    for inner in pair.into_inner() {
        collect_references_from_pair(inner, references);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_variable_references() {
        let source = r#"
x = 1;
y = x + 2;
return x;
"#;
        let refs = find_references_to(source, "x");
        assert_eq!(refs.len(), 3, "Should find 3 references to x");
        assert!(refs.iter().all(|r| r.kind == ReferenceKind::Variable));
    }

    #[test]
    fn test_find_property_references() {
        let source = r#"
this.name = "test";
return this.name;
"#;
        let refs = find_references_to(source, "name");
        // Filter to only property references
        let prop_refs: Vec<_> = refs.iter().filter(|r| r.kind == ReferenceKind::Property).collect();
        assert_eq!(prop_refs.len(), 2, "Should find 2 property references to name");
    }

    #[test]
    fn test_find_verb_references() {
        let source = r#"
this:initialize();
return this:initialize();
"#;
        let refs = find_references_to(source, "initialize");
        // Filter to only verb references
        let verb_refs: Vec<_> = refs.iter().filter(|r| r.kind == ReferenceKind::Verb).collect();
        assert_eq!(verb_refs.len(), 2, "Should find 2 verb references to initialize");
    }

    #[test]
    fn test_find_sysprop_references() {
        let source = r#"
x = $player;
$player:tell("hello");
"#;
        let refs = find_references_to(source, "player");
        // Filter to only sysprop references
        let sysprop_refs: Vec<_> = refs.iter().filter(|r| r.kind == ReferenceKind::SysProp).collect();
        assert_eq!(sysprop_refs.len(), 2, "Should find 2 sysprop references to $player");
    }

    #[test]
    fn test_symbol_at_position() {
        let source = "x = foo + bar;";
        // foo starts at column 4 (0-based)
        let sym = symbol_at_position(source, 0, 4);
        assert!(sym.is_some());
        assert_eq!(sym.unwrap().name, "foo");
    }

    #[test]
    fn test_case_insensitive_search() {
        let source = r#"
Player = $player;
PLAYER:tell("hi");
"#;
        let refs = find_references_to(source, "player");
        // Should find: Player (var), player (sysprop), PLAYER (var before :tell)
        assert!(refs.len() >= 2, "Should find references case-insensitively");
    }

    #[test]
    fn test_all_reference_kinds() {
        let source = r#"
x = $player.name;
$player:tell("hi");
length(x);
"#;
        let all_refs = find_all_references(source);

        // Check we find various kinds
        let has_var = all_refs.iter().any(|r| r.kind == ReferenceKind::Variable);
        let has_prop = all_refs.iter().any(|r| r.kind == ReferenceKind::Property);
        let has_verb = all_refs.iter().any(|r| r.kind == ReferenceKind::Verb);
        let has_sysprop = all_refs.iter().any(|r| r.kind == ReferenceKind::SysProp);
        let has_builtin = all_refs.iter().any(|r| r.kind == ReferenceKind::Builtin);

        assert!(has_var, "Should find variable references");
        assert!(has_prop, "Should find property references");
        assert!(has_verb, "Should find verb references");
        assert!(has_sysprop, "Should find sysprop references");
        assert!(has_builtin, "Should find builtin references");
    }

    #[test]
    fn test_find_references_in_object_file() {
        // Test parsing a .moo object definition file
        let source = r#"
object PLAYER
  name: "generic player"
  parent: ROOT_CLASS
  owner: #2

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    this.home = $player_start;
    if (caller != this)
      return E_PERM;
    endif
  endverb

  verb confunc (this none this) owner: #2 flags: "rxd"
    player = this;
    notify(player, "Welcome!");
  endverb

endobject
"#;
        let refs = find_all_references(source);

        // Should find references from verb bodies
        assert!(!refs.is_empty(), "Should find references in object file");

        // Check that we find 'this', 'caller', 'player', etc.
        let this_refs: Vec<_> = refs.iter().filter(|r| r.name == "this").collect();
        assert!(!this_refs.is_empty(), "Should find 'this' references");

        let caller_refs: Vec<_> = refs.iter().filter(|r| r.name == "caller").collect();
        assert!(!caller_refs.is_empty(), "Should find 'caller' references");

        let player_refs: Vec<_> = refs.iter().filter(|r| r.name == "player").collect();
        assert!(!player_refs.is_empty(), "Should find 'player' references");

        // Check line numbers are adjusted correctly
        // 'this.home' is on line 7 (0-based)
        let home_ref = refs.iter().find(|r| r.name == "home" && r.kind == ReferenceKind::Property);
        assert!(home_ref.is_some(), "Should find 'home' property");
        assert_eq!(home_ref.unwrap().line, 7, "home should be on line 7");
    }
}
