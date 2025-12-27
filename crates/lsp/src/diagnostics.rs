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
    match error {
        CompileError::ParseError {
            error_position,
            end_line_col,
            ..
        } => {
            let (line, col) = error_position.line_col;
            let start = line_col_to_position(line, col);
            let end = if let Some((end_line, end_col)) = end_line_col {
                line_col_to_position(*end_line, *end_col)
            } else {
                // Single character range
                Position {
                    line: start.line,
                    character: start.character + 1,
                }
            };
            Range { start, end }
        }
        // For other error types, use the context() method
        _ => {
            let ctx = error.context();
            let (line, col) = ctx.line_col;
            let start = line_col_to_position(line, col);
            let end = Position {
                line: start.line,
                character: start.character + 1,
            };
            Range { start, end }
        }
    }
}

/// Convert CompileError to LSP Diagnostic.
fn compile_error_to_diagnostic(error: &CompileError) -> Diagnostic {
    let range = compile_error_to_range(error);
    let message = error.to_string();

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("moo".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Convert ObjDefParseError to LSP Diagnostic.
fn parse_error_to_diagnostic(error: &ObjDefParseError) -> Option<Diagnostic> {
    match error {
        ObjDefParseError::ParseError(compile_error)
        | ObjDefParseError::VerbCompileError(compile_error, _) => {
            Some(compile_error_to_diagnostic(compile_error))
        }
        // Simple string errors without position info
        ObjDefParseError::BadVerbFlags(msg)
        | ObjDefParseError::BadVerbArgspec(msg)
        | ObjDefParseError::BadPropFlags(msg)
        | ObjDefParseError::ConstantNotFound(msg)
        | ObjDefParseError::InvalidObjectId(msg) => Some(Diagnostic {
            range: Range::default(),
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("moo".to_string()),
            message: msg.clone(),
            related_information: None,
            tags: None,
            data: None,
        }),
        ObjDefParseError::BadAttributeType(var_type) => Some(Diagnostic {
            range: Range::default(),
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("moo".to_string()),
            message: format!("Bad attribute type: {:?}", var_type),
            related_information: None,
            tags: None,
            data: None,
        }),
    }
}

/// Parse source and return diagnostics for any errors.
pub fn get_diagnostics(source: &str) -> Vec<Diagnostic> {
    let options = CompileOptions::default();
    let mut context = ObjFileContext::default();

    match compile_object_definitions(source, &options, &mut context) {
        Ok(_) => Vec::new(),
        Err(error) => parse_error_to_diagnostic(&error).into_iter().collect(),
    }
}
