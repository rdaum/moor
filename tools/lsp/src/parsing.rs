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

//! Shared parsing utilities for MOO source code.
//!
//! This module provides common parsing functions used across the LSP,
//! such as extracting object member references from source text.

/// Represents a reference to an object member (verb or property).
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectMemberRef {
    /// A verb reference like `$foo:bar_verb`
    Verb {
        object_name: String,
        verb_name: String,
    },
    /// A property reference like `$foo.some_prop`
    Property {
        object_name: String,
        prop_name: String,
    },
    /// Partial reference - object name found with separator but no member yet
    /// Used for completion context (e.g., `$foo:` or `$foo.`)
    Partial {
        object_name: String,
        is_verb: bool,
        partial_name: String,
    },
}

impl ObjectMemberRef {
    /// Get the object name from this reference.
    pub fn object_name(&self) -> &str {
        match self {
            ObjectMemberRef::Verb { object_name, .. } => object_name,
            ObjectMemberRef::Property { object_name, .. } => object_name,
            ObjectMemberRef::Partial { object_name, .. } => object_name,
        }
    }

    /// Returns true if this is a verb reference.
    pub fn is_verb(&self) -> bool {
        matches!(
            self,
            ObjectMemberRef::Verb { .. } | ObjectMemberRef::Partial { is_verb: true, .. }
        )
    }
}

/// Parse an object member reference from a line at a given character position.
///
/// Looks for patterns like:
/// - `$foo:bar_verb` (verb reference)
/// - `$foo.some_prop` (property reference)
/// - `$foo:` or `$foo.` (partial, for completion)
///
/// # Arguments
/// * `line` - The source line to parse
/// * `char_pos` - Character position (0-indexed) to search before
/// * `require_member` - If true, requires a non-empty member name (for hover).
///   If false, allows partial references (for completion).
///
/// Returns Some if a valid reference pattern is found before the cursor position.
pub fn parse_object_member_ref(
    line: &str,
    char_pos: usize,
    require_member: bool,
) -> Option<ObjectMemberRef> {
    // Get the portion of the line up to cursor
    let before_cursor = if char_pos <= line.len() {
        &line[..char_pos]
    } else {
        line
    };

    // Find the last $ before cursor
    let dollar_pos = before_cursor.rfind('$')?;

    // For hover, we want to include text after cursor too
    // For completion, we only care about what's before cursor
    let rest = if require_member {
        &line[dollar_pos..]
    } else {
        &before_cursor[dollar_pos..]
    };

    // Parse the reference
    let mut chars = rest.chars().peekable();

    // Skip the $
    chars.next()?;

    // Collect object name (letters, digits, underscores)
    let mut object_name = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_alphanumeric() || c == '_' {
            object_name.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if object_name.is_empty() {
        return None;
    }

    // Check for : or .
    let separator = chars.next()?;
    let is_verb = separator == ':';
    let is_property = separator == '.';

    if !is_verb && !is_property {
        return None;
    }

    // Collect member name
    let mut member_name = String::new();
    for c in chars {
        if c.is_alphanumeric() || c == '_' {
            member_name.push(c);
        } else {
            break;
        }
    }

    // If we require a member name (hover mode) and don't have one, return None
    if require_member && member_name.is_empty() {
        return None;
    }

    // Return appropriate variant
    if member_name.is_empty() {
        // Partial reference for completion
        Some(ObjectMemberRef::Partial {
            object_name,
            is_verb,
            partial_name: String::new(),
        })
    } else if !require_member {
        // Completion mode with partial name
        Some(ObjectMemberRef::Partial {
            object_name,
            is_verb,
            partial_name: member_name,
        })
    } else if is_verb {
        Some(ObjectMemberRef::Verb {
            object_name,
            verb_name: member_name,
        })
    } else {
        Some(ObjectMemberRef::Property {
            object_name,
            prop_name: member_name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_verb_reference() {
        let line = "$player:tell";
        let result = parse_object_member_ref(line, line.len(), true);
        assert_eq!(
            result,
            Some(ObjectMemberRef::Verb {
                object_name: "player".to_string(),
                verb_name: "tell".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_property_reference() {
        let line = "$room.description";
        let result = parse_object_member_ref(line, line.len(), true);
        assert_eq!(
            result,
            Some(ObjectMemberRef::Property {
                object_name: "room".to_string(),
                prop_name: "description".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_partial_verb_completion() {
        let line = "$player:te";
        let result = parse_object_member_ref(line, line.len(), false);
        assert_eq!(
            result,
            Some(ObjectMemberRef::Partial {
                object_name: "player".to_string(),
                is_verb: true,
                partial_name: "te".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_empty_completion() {
        let line = "$player:";
        let result = parse_object_member_ref(line, line.len(), false);
        assert_eq!(
            result,
            Some(ObjectMemberRef::Partial {
                object_name: "player".to_string(),
                is_verb: true,
                partial_name: String::new(),
            })
        );
    }

    #[test]
    fn test_require_member_rejects_empty() {
        let line = "$player:";
        let result = parse_object_member_ref(line, line.len(), true);
        assert_eq!(result, None);
    }

    #[test]
    fn test_mid_line_reference() {
        let line = "player:tell($room.name);";
        // Position at end of $room.name (before semicolon)
        let result = parse_object_member_ref(line, 22, true);
        assert_eq!(
            result,
            Some(ObjectMemberRef::Property {
                object_name: "room".to_string(),
                prop_name: "name".to_string(),
            })
        );
    }
}
