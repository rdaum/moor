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

//! Diagnostics generation from MOO compilation errors.

use moor_common::model::CompileError;
use moor_compiler::{CompileOptions, ObjDefParseError, ObjFileContext, compile_object_definitions};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

/// Convert a line and column (1-based) to LSP Position (0-based).
fn line_col_to_position(line: usize, col: usize) -> Position {
    Position {
        line: line.saturating_sub(1) as u32,
        character: col.saturating_sub(1) as u32,
    }
}

/// Extract range from a CompileError.
fn compile_error_to_range(error: &CompileError) -> Range {
    let CompileError::ParseError {
        error_position,
        end_line_col,
        ..
    } = error
    else {
        // For other error types, use the context() method
        let ctx = error.context();
        let (line, col) = ctx.line_col;
        let start = line_col_to_position(line, col);
        let end = Position {
            line: start.line,
            character: start.character + 1,
        };
        return Range { start, end };
    };

    let (line, col) = error_position.line_col;
    let start = line_col_to_position(line, col);
    let end = end_line_col
        .map(|(end_line, end_col)| line_col_to_position(end_line, end_col))
        .unwrap_or(Position {
            line: start.line,
            character: start.character + 1,
        });
    Range { start, end }
}

/// Create an error diagnostic at the given range.
fn error_diagnostic(range: Range, message: String) -> Diagnostic {
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("moor-compiler".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Convert CompileError to LSP Diagnostic.
fn compile_error_to_diagnostic(error: &CompileError) -> Diagnostic {
    error_diagnostic(compile_error_to_range(error), error.to_string())
}

/// Convert ObjDefParseError to LSP Diagnostic.
fn parse_error_to_diagnostic(error: &ObjDefParseError) -> Option<Diagnostic> {
    let (range, message) = match error {
        ObjDefParseError::ParseError(e) | ObjDefParseError::VerbCompileError(e, _) => {
            return Some(compile_error_to_diagnostic(e));
        }
        ObjDefParseError::BadVerbFlags(msg)
        | ObjDefParseError::BadVerbArgspec(msg)
        | ObjDefParseError::BadPropFlags(msg)
        | ObjDefParseError::ConstantNotFound(msg)
        | ObjDefParseError::InvalidObjectId(msg) => (Range::default(), msg.clone()),
        ObjDefParseError::BadAttributeType(var_type) => (
            Range::default(),
            format!("Bad attribute type: {:?}", var_type),
        ),
    };
    Some(error_diagnostic(range, message))
}

/// Parse source and return diagnostics for any errors.
/// Uses a fresh context - prefer `get_diagnostics_with_context` when constants are available.
#[allow(dead_code)]
pub fn get_diagnostics(source: &str) -> Vec<Diagnostic> {
    get_diagnostics_with_context(source, &ObjFileContext::default())
}

/// Parse source using the provided context and return diagnostics for any errors.
/// The context should contain constants loaded from constants.moo or similar.
pub fn get_diagnostics_with_context(source: &str, context: &ObjFileContext) -> Vec<Diagnostic> {
    let options = CompileOptions::default();
    // Clone the context so we don't mutate the original
    let mut local_context = context.clone();

    match compile_object_definitions(source, &options, &mut local_context) {
        Ok(_) => Vec::new(),
        Err(error) => parse_error_to_diagnostic(&error).into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_object_no_diagnostics() {
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
        let diags = get_diagnostics(source);
        assert!(diags.is_empty(), "Valid source should have no diagnostics");
    }

    #[test]
    fn test_valid_object_with_verb() {
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
        let diags = get_diagnostics(source);
        assert!(
            diags.is_empty(),
            "Valid source with verb should have no diagnostics: {:?}",
            diags
        );
    }

    #[test]
    fn test_syntax_error_produces_diagnostic() {
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

    verb "bad" (this none this) owner: #1 flags: "rxd"
        if (1)
            // Missing endif
        return 1;
    endverb
endobject
"#;
        let diags = get_diagnostics(source);
        assert!(!diags.is_empty(), "Syntax error should produce diagnostics");
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diags[0].source, Some("moor-compiler".to_string()));
    }

    #[test]
    fn test_empty_source_no_diagnostics() {
        let diags = get_diagnostics("");
        assert!(diags.is_empty(), "Empty source should have no diagnostics");
    }

    #[test]
    fn test_line_col_conversion() {
        // 1-based to 0-based conversion
        let pos = line_col_to_position(1, 1);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 0);

        let pos = line_col_to_position(5, 10);
        assert_eq!(pos.line, 4);
        assert_eq!(pos.character, 9);
    }
}
