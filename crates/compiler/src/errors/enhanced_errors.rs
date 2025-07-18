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

//! Enhanced error reporting system for syntax errors
//! Provides visual indicators, context, and suggestions for parse errors

use moor_common::model::{CompileContext, CompileError};

/// Position information for enhanced error reporting
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ErrorPosition {
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based)
    pub column: usize,
    /// Byte offset in source
    pub byte_offset: usize,
}

impl ErrorPosition {
    pub fn new(line: usize, column: usize, byte_offset: usize) -> Self {
        Self { line, column, byte_offset }
    }

    pub fn to_compile_context(&self) -> CompileContext {
        CompileContext::new((self.line, self.column))
    }
}

/// Span representing a range in source code
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorSpan {
    pub start: ErrorPosition,
    pub end: ErrorPosition,
}

impl ErrorSpan {
    pub fn new(start: ErrorPosition, end: ErrorPosition) -> Self {
        Self { start, end }
    }

    pub fn point(position: ErrorPosition) -> Self {
        Self {
            end: position,
            start: position,
        }
    }
}

/// Context information about what type of construct was being parsed
#[derive(Debug, Clone, PartialEq)]
pub enum ParseContext {
    Statement,
    Expression,
    Assignment,
    IfStatement,
    ForLoop,
    WhileLoop,
    FunctionCall,
    FunctionDefinition,
    List,
    Map,
    ScatterAssignment,
    Condition,
    Unknown(String),
}

impl ParseContext {
    /// Get expected tokens/constructs for this context
    pub fn expected_tokens(&self) -> Vec<String> {
        match self {
            ParseContext::Statement => vec![
                "variable assignment".to_string(),
                "if statement".to_string(),
                "for loop".to_string(),
                "while loop".to_string(),
                "function call".to_string(),
                "return statement".to_string(),
                "function definition".to_string(),
            ],
            ParseContext::Expression | ParseContext::Assignment => vec![
                "identifier".to_string(),
                "number".to_string(),
                "string".to_string(),
                "list literal".to_string(),
                "map literal".to_string(),
                "function call".to_string(),
                "parenthesized expression".to_string(),
            ],
            ParseContext::IfStatement => vec![
                "condition expression".to_string(),
                "'endif'".to_string(),
                "'else'".to_string(),
                "'elseif'".to_string(),
            ],
            ParseContext::ForLoop => vec![
                "variable name".to_string(),
                "'in'".to_string(),
                "iterable expression".to_string(),
                "'endfor'".to_string(),
            ],
            ParseContext::WhileLoop => vec![
                "condition expression".to_string(),
                "'endwhile'".to_string(),
            ],
            ParseContext::FunctionCall => vec![
                "function name".to_string(),
                "argument".to_string(),
                "','".to_string(),
                "')'".to_string(),
            ],
            ParseContext::FunctionDefinition => vec![
                "function name".to_string(),
                "parameter list".to_string(),
                "'endfn'".to_string(),
            ],
            ParseContext::List => vec![
                "expression".to_string(),
                "','".to_string(),
                "']'".to_string(),
            ],
            ParseContext::Map => vec![
                "key-value pair".to_string(),
                "','".to_string(),
                "'}'".to_string(),
            ],
            ParseContext::ScatterAssignment => vec![
                "variable name".to_string(),
                "','".to_string(),
                "'}'".to_string(),
            ],
            ParseContext::Condition => vec![
                "boolean expression".to_string(),
                "comparison operator".to_string(),
                "logical operator".to_string(),
            ],
            ParseContext::Unknown(_) => vec![
                "valid syntax".to_string(),
            ],
        }
    }
}

/// Enhanced error information
#[derive(Debug, Clone)]
pub struct EnhancedError {
    /// Span of the error in source
    pub span: ErrorSpan,
    /// The problematic text
    pub error_text: String,
    /// Context of what was being parsed
    pub context: ParseContext,
    /// Optional additional message
    pub message: Option<String>,
}

impl EnhancedError {
    pub fn new(span: ErrorSpan, error_text: String, context: ParseContext) -> Self {
        Self {
            span,
            error_text,
            context,
            message: None,
        }
    }

    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }
}

/// Trait for parsers to provide enhanced error reporting
pub trait EnhancedErrorReporter {
    /// Create an enhanced error from parser-specific error information
    fn create_enhanced_error(&self, source: &str, enhanced_error: &EnhancedError) -> CompileError;
}

/// Default implementation of enhanced error reporting
pub struct DefaultErrorReporter;

impl EnhancedErrorReporter for DefaultErrorReporter {
    fn create_enhanced_error(&self, source: &str, enhanced_error: &EnhancedError) -> CompileError {
        let message = create_enhanced_error_message(source, enhanced_error);
        
        CompileError::ParseError {
            error_position: enhanced_error.span.start.to_compile_context(),
            context: format!("{:?}", enhanced_error.context),
            end_line_col: Some((enhanced_error.span.end.line, enhanced_error.span.end.column)),
            message,
        }
    }
}

/// Create an enhanced error message with visual indicators and suggestions
pub fn create_enhanced_error_message(source: &str, enhanced_error: &EnhancedError) -> String {
    let mut message = String::new();
    
    // Main error message
    if let Some(custom_message) = &enhanced_error.message {
        message.push_str(custom_message);
    } else if enhanced_error.error_text.trim().is_empty() {
        message.push_str("Unexpected end of input");
    } else {
        // Try to identify the specific problematic token
        let problem_token = find_problem_token(&enhanced_error.error_text);
        message.push_str(&format!("Unexpected token '{problem_token}'"));
    }
    
    // Add source context with improved visual indicators
    add_source_context(&mut message, source, enhanced_error);
    
    // Add context information
    message.push_str(&format!("\n\nContext: {:?}", enhanced_error.context));
    
    // Add suggestions based on context
    let suggestions = enhanced_error.context.expected_tokens();
    if !suggestions.is_empty() {
        message.push_str(&format!("\nExpected one of: {}", suggestions.join(", ")));
    }
    
    message
}

/// Find the most problematic token in error text
fn find_problem_token(error_text: &str) -> &str {
    let text = error_text.trim();
    
    // Look for obvious invalid tokens
    if let Some(at_pos) = text.find('@') {
        // Find the end of the @ token
        let start = at_pos;
        let mut end = start + 1;
        while end < text.len() {
            let ch = text.chars().nth(end).unwrap();
            if ch.is_whitespace() || "(){}[],;".contains(ch) {
                break;
            }
            end += 1;
        }
        return &text[start..end];
    }
    
    // Look for invalid operators
    if text.contains("@@") {
        return "@@";
    }
    
    // Look for unclosed strings
    if text.starts_with('"') && !text[1..].contains('"') {
        return "unclosed string";
    }
    
    // If we can't find a specific problem, return first token
    text.split_whitespace().next().unwrap_or(text)
}

/// Add source context with improved visual indicators
fn add_source_context(message: &mut String, source: &str, enhanced_error: &EnhancedError) {
    let source_lines: Vec<&str> = source.lines().collect();
    let error_line_idx = enhanced_error.span.start.line.saturating_sub(1);
    
    if error_line_idx >= source_lines.len() {
        return;
    }
    
    let error_line = source_lines[error_line_idx];
    
    // Show context around the error (previous and next lines if available)
    let show_context = error_line_idx > 0 || error_line_idx + 1 < source_lines.len();
    
    message.push('\n');
    
    // Show previous line for context
    if error_line_idx > 0 && show_context {
        message.push_str(&format!("\n{:4} | {}", error_line_idx, source_lines[error_line_idx - 1]));
    }
    
    // Show the error line
    message.push_str(&format!("\n{:4} | {}", enhanced_error.span.start.line, error_line));
    
    // Create more precise pointer
    let pointer_info = calculate_pointer_position(source, enhanced_error, error_line);
    message.push_str(&format!("\n{:4} | {}{}", "", pointer_info.spaces, pointer_info.indicators));
    
    // Show next line for context
    if error_line_idx + 1 < source_lines.len() && show_context {
        message.push_str(&format!("\n{:4} | {}", error_line_idx + 2, source_lines[error_line_idx + 1]));
    }
}

/// Information about where to place visual indicators
struct PointerInfo {
    spaces: String,
    indicators: String,
}

/// Calculate precise pointer positioning
fn calculate_pointer_position(_source: &str, enhanced_error: &EnhancedError, error_line: &str) -> PointerInfo {
    let start_col = enhanced_error.span.start.column.saturating_sub(1);
    let end_col = enhanced_error.span.end.column.saturating_sub(1);
    
    // For very wide spans, just point to where the error starts
    let line_len = error_line.len();
    let span_length = end_col.saturating_sub(start_col);
    
    let (pointer_start, pointer_length) = if span_length > 50 || end_col > line_len {
        // For very wide spans or spans that go beyond the line, just point to start
        (start_col, 1)
    } else if span_length > 20 {
        // For moderately wide spans, show start and end
        (start_col, 3.min(span_length))
    } else {
        // For reasonable spans, show the full span
        (start_col, span_length.max(1))
    };
    
    // Find the specific problematic part within the error text
    let problem_token = find_problem_token(&enhanced_error.error_text);
    if let Some(token_pos) = error_line[start_col..].find(problem_token) {
        let actual_start = start_col + token_pos;
        let actual_length = problem_token.len().min(10); // Cap at 10 chars
        
        PointerInfo {
            spaces: " ".repeat(actual_start),
            indicators: format!("{}{}",
                "^".repeat(actual_length),
                if actual_length < problem_token.len() { "..." } else { "" }
            ),
        }
    } else {
        PointerInfo {
            spaces: " ".repeat(pointer_start),
            indicators: "^".repeat(pointer_length),
        }
    }
}

/// Helper function to determine parse context from error location and surrounding code
pub fn infer_parse_context(source: &str, error_position: &ErrorPosition) -> ParseContext {
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = error_position.line.saturating_sub(1);
    
    if line_idx >= lines.len() {
        return ParseContext::Unknown("EOF".to_string());
    }
    
    let current_line = lines[line_idx];
    let before_error = &current_line[..error_position.column.saturating_sub(1).min(current_line.len())];
    
    // Look for keywords and patterns to determine context
    if before_error.contains("if") && !before_error.contains("endif") {
        ParseContext::IfStatement
    } else if before_error.contains("for") && !before_error.contains("endfor") {
        ParseContext::ForLoop
    } else if before_error.contains("while") && !before_error.contains("endwhile") {
        ParseContext::WhileLoop
    } else if before_error.contains("fn") && !before_error.contains("endfn") {
        ParseContext::FunctionDefinition
    } else if before_error.contains('{') && !before_error.contains('}') {
        // Check if this looks like a scatter assignment pattern
        let brace_pos = before_error.rfind('{').unwrap();
        let after_brace = &before_error[brace_pos + 1..];
        let before_brace = &before_error[..brace_pos];
        
        // Look for scatter pattern: variables before brace, then = after
        if before_brace.trim_end().ends_with('=') || after_brace.contains(',') {
            ParseContext::ScatterAssignment
        } else {
            ParseContext::Map
        }
    } else if before_error.contains('[') && !before_error.contains(']') {
        ParseContext::List
    } else if before_error.contains('(') && !before_error.contains(')') {
        ParseContext::FunctionCall
    } else if before_error.contains('=') {
        // Check if this might be a scatter assignment by looking for { before =
        let eq_pos = before_error.rfind('=').unwrap();
        let before_eq = &before_error[..eq_pos].trim();
        if before_eq.ends_with('}') {
            ParseContext::ScatterAssignment
        } else {
            ParseContext::Assignment
        }
    } else if before_error.trim().is_empty() || before_error.ends_with(';') {
        ParseContext::Statement
    } else {
        ParseContext::Expression
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_position() {
        let pos = ErrorPosition::new(5, 10, 42);
        assert_eq!(pos.line, 5);
        assert_eq!(pos.column, 10);
        assert_eq!(pos.byte_offset, 42);
        
        let context = pos.to_compile_context();
        assert_eq!(context.line_col, (5, 10));
    }

    #[test]
    fn test_parse_context_expected_tokens() {
        let statement_context = ParseContext::Statement;
        let tokens = statement_context.expected_tokens();
        assert!(tokens.contains(&"variable assignment".to_string()));
        assert!(tokens.contains(&"if statement".to_string()));
    }

    #[test]
    fn test_enhanced_error_message() {
        let source = "x = 42\ny = @invalid";
        let start_pos = ErrorPosition::new(2, 5, 11);
        let end_pos = ErrorPosition::new(2, 13, 19);
        let span = ErrorSpan::new(start_pos, end_pos);
        
        let error = EnhancedError::new(
            span,
            "@invalid".to_string(),
            ParseContext::Expression,
        );
        
        let message = create_enhanced_error_message(source, &error);
        assert!(message.contains("Unexpected token '@invalid'"));
        assert!(message.contains("   2 | y = @invalid"));
        assert!(message.contains("^^^^^^^"));
        assert!(message.contains("Expected one of:"));
    }

    #[test]
    fn test_infer_parse_context() {
        let source = "if (x > 0) @error";
        let error_pos = ErrorPosition::new(1, 12, 11);
        let context = infer_parse_context(source, &error_pos);
        assert!(matches!(context, ParseContext::IfStatement));
        
        let source2 = "for i in @error";
        let error_pos2 = ErrorPosition::new(1, 10, 9);
        let context2 = infer_parse_context(source2, &error_pos2);
        assert!(matches!(context2, ParseContext::ForLoop));
    }
}