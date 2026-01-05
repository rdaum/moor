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

//! Go-to-definition provider for MOO source files.
//!
//! This module implements jumping to definitions of object references in MOO code.
//! It detects object references like `$foo` or `#42` and uses the workspace index
//! to find where those objects are defined.

use moor_var::Obj;
use tower_lsp::lsp_types::{Location, Position, Range, Url};

use crate::objects::ObjectNameRegistry;
use crate::workspace_index::WorkspaceIndex;

/// Result of detecting an object reference at a position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectReference {
    /// A symbolic reference like `$foo`
    Symbolic(String),
    /// A numeric reference like `#42`
    Numeric(i32),
}

/// Detect what object reference (if any) is at the cursor position.
///
/// Looks for patterns like:
/// - `$name` - symbolic object references
/// - `#number` - numeric object references
pub fn detect_object_reference(source: &str, position: Position) -> Option<ObjectReference> {
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = position.line as usize;

    if line_idx >= lines.len() {
        return None;
    }

    let line = lines[line_idx];
    let char_idx = position.character as usize;

    if char_idx > line.len() {
        return None;
    }

    // Look for $name pattern
    if let Some(reference) = detect_symbolic_reference(line, char_idx) {
        return Some(reference);
    }

    // Look for #number pattern
    if let Some(reference) = detect_numeric_reference(line, char_idx) {
        return Some(reference);
    }

    None
}

/// Detect a symbolic object reference ($name) at the given character position.
fn detect_symbolic_reference(line: &str, char_idx: usize) -> Option<ObjectReference> {
    let bytes = line.as_bytes();

    // Find the start of the token containing our position
    // Walk backwards to find $ or beginning of identifier
    let mut start = char_idx;
    while start > 0 {
        let prev = start - 1;
        let c = bytes[prev] as char;
        if c == '$' {
            // Found the start of a symbolic reference
            start = prev;
            break;
        } else if c.is_alphanumeric() || c == '_' {
            start = prev;
        } else {
            break;
        }
    }

    // Check if we're at a $ character
    if start >= bytes.len() || bytes[start] as char != '$' {
        return None;
    }

    // Now extract the name following the $
    let name_start = start + 1;
    let mut end = name_start;
    while end < bytes.len() {
        let c = bytes[end] as char;
        if c.is_alphanumeric() || c == '_' {
            end += 1;
        } else {
            break;
        }
    }

    if end > name_start {
        let name = &line[name_start..end];
        return Some(ObjectReference::Symbolic(name.to_string()));
    }

    None
}

/// Detect a numeric object reference (#number) at the given character position.
fn detect_numeric_reference(line: &str, char_idx: usize) -> Option<ObjectReference> {
    let bytes = line.as_bytes();

    // Find the start of the token containing our position
    // Walk backwards to find # or digits
    let mut start = char_idx;
    while start > 0 {
        let prev = start - 1;
        let c = bytes[prev] as char;
        if c == '#' {
            // Found the start of a numeric reference
            start = prev;
            break;
        } else if c.is_ascii_digit() || c == '-' {
            start = prev;
        } else {
            break;
        }
    }

    // Check if we're at a # character
    if start >= bytes.len() || bytes[start] as char != '#' {
        return None;
    }

    // Now extract the number following the #
    let num_start = start + 1;
    let mut end = num_start;

    // Handle optional negative sign
    if end < bytes.len() && bytes[end] as char == '-' {
        end += 1;
    }

    while end < bytes.len() {
        let c = bytes[end] as char;
        if c.is_ascii_digit() {
            end += 1;
        } else {
            break;
        }
    }

    if end > num_start {
        let num_str = &line[num_start..end];
        if let Ok(num) = num_str.parse::<i32>() {
            return Some(ObjectReference::Numeric(num));
        }
    }

    None
}

/// Find the definition location for the symbol at the given position.
///
/// Returns a Location pointing to where the object is defined in the workspace,
/// or None if the object cannot be found.
pub fn find_definition(
    source: &str,
    position: Position,
    index: &WorkspaceIndex,
    object_names: &ObjectNameRegistry,
) -> Option<Location> {
    let reference = detect_object_reference(source, position)?;

    // Resolve the reference to an object ID
    let obj: Obj = match reference {
        ObjectReference::Symbolic(name) => object_names.resolve(&name)?,
        ObjectReference::Numeric(id) => Obj::mk_id(id),
    };

    // Find the file where this object is defined
    let file_path = index.file_for_object(&obj)?;

    // Convert path to URI
    let uri = Url::from_file_path(&file_path).ok()?;

    // Return a Location at the start of the file
    // TODO: We could enhance this to find the exact line with `object #N` declaration
    Some(Location {
        uri,
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_symbolic_reference() {
        let source = "return $player:name();";

        // Position at the 'p' in $player
        let result = detect_object_reference(
            source,
            Position {
                line: 0,
                character: 8,
            },
        );
        assert_eq!(
            result,
            Some(ObjectReference::Symbolic("player".to_string()))
        );

        // Position at the '$'
        let result = detect_object_reference(
            source,
            Position {
                line: 0,
                character: 7,
            },
        );
        assert_eq!(
            result,
            Some(ObjectReference::Symbolic("player".to_string()))
        );

        // Position at the end of 'player'
        let result = detect_object_reference(
            source,
            Position {
                line: 0,
                character: 13,
            },
        );
        assert_eq!(
            result,
            Some(ObjectReference::Symbolic("player".to_string()))
        );
    }

    #[test]
    fn test_detect_numeric_reference() {
        let source = "parent: #1";

        // Position at the '1' in #1
        let result = detect_object_reference(
            source,
            Position {
                line: 0,
                character: 9,
            },
        );
        assert_eq!(result, Some(ObjectReference::Numeric(1)));

        // Position at the '#'
        let result = detect_object_reference(
            source,
            Position {
                line: 0,
                character: 8,
            },
        );
        assert_eq!(result, Some(ObjectReference::Numeric(1)));
    }

    #[test]
    fn test_detect_negative_numeric_reference() {
        let source = "return #-1;";

        // Position at the '-' in #-1
        let result = detect_object_reference(
            source,
            Position {
                line: 0,
                character: 8,
            },
        );
        assert_eq!(result, Some(ObjectReference::Numeric(-1)));
    }

    #[test]
    fn test_no_reference() {
        let source = "return 42;";

        let result = detect_object_reference(
            source,
            Position {
                line: 0,
                character: 7,
            },
        );
        assert_eq!(result, None);
    }

    #[test]
    fn test_multiline_source() {
        let source = "object #1\n    parent: #2\n    verb \"look\" (this none this)\n        return $room:description();";

        // $room on line 3 (0-indexed)
        let result = detect_object_reference(
            source,
            Position {
                line: 3,
                character: 16,
            },
        );
        assert_eq!(result, Some(ObjectReference::Symbolic("room".to_string())));

        // #2 on line 1
        let result = detect_object_reference(
            source,
            Position {
                line: 1,
                character: 13,
            },
        );
        assert_eq!(result, Some(ObjectReference::Numeric(2)));
    }

    #[test]
    fn test_symbolic_with_underscores() {
        let source = "$gender_utils:get_conj(args);";

        let result = detect_object_reference(
            source,
            Position {
                line: 0,
                character: 5,
            },
        );
        assert_eq!(
            result,
            Some(ObjectReference::Symbolic("gender_utils".to_string()))
        );
    }
}
