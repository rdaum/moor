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

//! Tree-sitter specific error recovery and enhancement traits
//!
//! This module provides enhanced error recovery capabilities that leverage
//! Tree-sitter's advanced error handling features including missing nodes,
//! extra tokens, and partial parse trees.

use super::enhanced_errors::ParseContext;
use moor_common::model::CompileError;

/// Re-export from enhanced_errors for convenience
pub use super::enhanced_errors::{ErrorPosition, ErrorSpan};

/// Types of errors that can occur in tree parsing
#[derive(Debug, Clone, PartialEq)]
pub enum TreeErrorType {
    /// Expected token/node was missing
    Missing { expected: Vec<String> },
    /// Unexpected token/node was found
    Extra { found: String },
    /// Invalid syntax in a valid position
    Invalid { reason: String },
    /// Unclosed delimiter (parenthesis, bracket, etc.)
    Unclosed {
        delimiter: String,
        opened_at: ErrorPosition,
    },
    /// Unexpected end of input
    UnexpectedEof { expected: Vec<String> },
    /// Generic syntax error
    Syntax,
}

/// Suggested fix for an error
#[derive(Debug, Clone)]
pub struct ErrorFix {
    /// Description of the fix
    pub description: String,
    /// The span where the fix should be applied
    pub span: ErrorSpan,
    /// The replacement text (empty for deletions)
    pub replacement: String,
    /// Confidence level (0.0 to 1.0)
    pub confidence: f32,
}

impl ErrorFix {
    pub fn insertion(position: ErrorPosition, text: String, description: String) -> Self {
        Self {
            description,
            span: ErrorSpan::point(position),
            replacement: text,
            confidence: 0.8,
        }
    }

    pub fn deletion(span: ErrorSpan, description: String) -> Self {
        Self {
            description,
            span,
            replacement: String::new(),
            confidence: 0.9,
        }
    }

    pub fn replacement(span: ErrorSpan, text: String, description: String) -> Self {
        Self {
            description,
            span,
            replacement: text,
            confidence: 0.7,
        }
    }
}

/// Enhanced error information for tree nodes
pub trait TreeErrorInfo {
    /// Get the node kind (equivalent to TreeNode::node_kind)
    fn node_kind(&self) -> &str;

    /// Check if this is an error node
    fn is_error(&self) -> bool;

    /// Get line and column position
    fn line_col(&self) -> (usize, usize);

    /// Get span byte positions
    fn span(&self) -> (usize, usize);

    /// Check if this node represents a missing required element
    fn is_missing(&self) -> bool {
        false
    }

    /// Check if this node is an extra/unexpected element
    fn is_extra(&self) -> bool {
        false
    }

    /// Get the specific error type if this is an error node
    fn error_type(&self) -> Option<TreeErrorType> {
        if !self.is_error() {
            return None;
        }

        // Default implementation based on node kind and context
        Some(TreeErrorType::Syntax)
    }

    /// Get fields that are missing from this node
    fn missing_fields(&self) -> Vec<&str> {
        Vec::new()
    }

    /// Get the parse context where this error occurred
    fn parse_context(&self) -> ParseContext {
        ParseContext::Unknown(self.node_kind().to_string())
    }

    /// Check if recovery is possible from this error
    fn can_recover(&self) -> bool {
        // Most errors can be recovered from in tree-sitter
        true
    }

    /// Get suggested fixes for this error
    fn suggested_fixes(&self) -> Vec<ErrorFix> {
        Vec::new()
    }

    /// Get a descriptive error message
    fn error_message(&self) -> String {
        let context = self.parse_context();

        match self.error_type() {
            Some(TreeErrorType::Missing { ref expected }) => {
                // Provide context-specific messages
                match context {
                    ParseContext::Unknown(ctx) if ctx == "PropertyAccess" => {
                        "Missing property name after '.'".to_string()
                    }
                    ParseContext::Unknown(ctx) if ctx == "MethodCall" => {
                        "Missing method name after ':'".to_string()
                    }
                    _ => format!("Expected one of: {}", expected.join(", ")),
                }
            }
            Some(TreeErrorType::Extra { ref found }) => {
                // Provide context-specific messages for extra tokens
                match (found.as_str(), context) {
                    (".", ParseContext::Unknown(ctx)) if ctx.contains("expression") => {
                        "Incomplete property access - add property name after '.'".to_string()
                    }
                    (":", ParseContext::Unknown(ctx)) if ctx.contains("expression") => {
                        "Incomplete method call - add method name and arguments after ':'"
                            .to_string()
                    }
                    (";", _) => "Unexpected semicolon".to_string(),
                    _ => format!("Unexpected {}", found),
                }
            }
            Some(TreeErrorType::Invalid { ref reason }) => reason.clone(),
            Some(TreeErrorType::Unclosed {
                ref delimiter,
                ref opened_at,
            }) => {
                format!("Unclosed {} opened at line {}", delimiter, opened_at.line)
            }
            Some(TreeErrorType::UnexpectedEof { ref expected }) => {
                format!("Unexpected end of input, expected: {}", expected.join(", "))
            }
            Some(TreeErrorType::Syntax) | None => {
                // Provide context-specific syntax error messages
                match context {
                    ParseContext::Unknown(ctx) if ctx == "PropertyAccess" => {
                        "Invalid property access syntax - use 'object.property'".to_string()
                    }
                    ParseContext::Unknown(ctx) if ctx == "MethodCall" => {
                        "Invalid method call syntax - use 'object:method(args)'".to_string()
                    }
                    _ => "Syntax error".to_string(),
                }
            }
        }
    }
}

/// Error recovery context that helps generate better error messages
pub struct ErrorRecoveryContext<'a> {
    pub source: &'a str,
    pub parent_context: Option<ParseContext>,
    pub sibling_kinds: Vec<&'a str>,
}

impl<'a> ErrorRecoveryContext<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            parent_context: None,
            sibling_kinds: Vec::new(),
        }
    }

    /// Extract text around an error position for context
    pub fn extract_context(&self, position: &ErrorPosition, window: usize) -> String {
        let lines: Vec<&str> = self.source.lines().collect();
        if position.line == 0 || position.line > lines.len() {
            return String::new();
        }

        let line = lines[position.line - 1];
        let start = position.column.saturating_sub(window);
        let end = (position.column + window).min(line.len());

        if start < line.len() {
            line[start..end].to_string()
        } else {
            String::new()
        }
    }

    /// Generate fix suggestions based on context
    pub fn generate_fixes(&self, error_node: &impl TreeErrorInfo) -> Vec<ErrorFix> {
        let mut fixes = Vec::new();

        match error_node.error_type() {
            Some(TreeErrorType::Missing { ref expected }) => {
                // Suggest inserting common missing tokens
                let pos = ErrorPosition::new(
                    error_node.line_col().0,
                    error_node.line_col().1,
                    error_node.span().0,
                );

                for exp in expected {
                    let suggestion = match exp.as_str() {
                        ";" => ErrorFix::insertion(
                            pos.clone(),
                            ";".to_string(),
                            "Add missing semicolon".to_string(),
                        ),
                        ")" => ErrorFix::insertion(
                            pos.clone(),
                            ")".to_string(),
                            "Add missing closing parenthesis".to_string(),
                        ),
                        "]" => ErrorFix::insertion(
                            pos.clone(),
                            "]".to_string(),
                            "Add missing closing bracket".to_string(),
                        ),
                        "}" => ErrorFix::insertion(
                            pos.clone(),
                            "}".to_string(),
                            "Add missing closing brace".to_string(),
                        ),
                        "endif" => ErrorFix::insertion(
                            pos.clone(),
                            "\nendif".to_string(),
                            "Add missing 'endif'".to_string(),
                        ),
                        "endfor" => ErrorFix::insertion(
                            pos.clone(),
                            "\nendfor".to_string(),
                            "Add missing 'endfor'".to_string(),
                        ),
                        "endwhile" => ErrorFix::insertion(
                            pos.clone(),
                            "\nendwhile".to_string(),
                            "Add missing 'endwhile'".to_string(),
                        ),
                        _ => continue,
                    };
                    fixes.push(suggestion);
                }
            }

            Some(TreeErrorType::Extra { ref found }) => {
                // Suggest removing extra tokens
                let span = ErrorSpan::new(
                    ErrorPosition::new(
                        error_node.line_col().0,
                        error_node.line_col().1,
                        error_node.span().0,
                    ),
                    ErrorPosition::new(
                        error_node.line_col().0,
                        error_node.line_col().1,
                        error_node.span().1,
                    ),
                );
                fixes.push(ErrorFix::deletion(
                    span,
                    format!("Remove unexpected '{}'", found),
                ));
            }

            Some(TreeErrorType::Unclosed { ref delimiter, .. }) => {
                // Suggest closing delimiter
                let pos = ErrorPosition::new(
                    error_node.line_col().0,
                    error_node.line_col().1,
                    error_node.span().1,
                );
                let closer = match delimiter.as_str() {
                    "(" => ")",
                    "[" => "]",
                    "{" => "}",
                    "\"" => "\"",
                    "'" => "'",
                    _ => "",
                };
                if !closer.is_empty() {
                    fixes.push(ErrorFix::insertion(
                        pos,
                        closer.to_string(),
                        format!("Close {}", delimiter),
                    ));
                }
            }

            _ => {
                // Return any fixes suggested by the node itself
                fixes.extend(error_node.suggested_fixes());
            }
        }

        fixes
    }
}

/// Pattern matcher for common error scenarios
pub struct ErrorPatternMatcher {
    patterns: Vec<ErrorPattern>,
}

struct ErrorPattern {
    name: String,
    matcher: Box<dyn Fn(&dyn TreeErrorInfo) -> bool>,
    fix_generator: Box<dyn Fn(&dyn TreeErrorInfo) -> Vec<ErrorFix>>,
}

impl ErrorPatternMatcher {
    pub fn new() -> Self {
        let mut matcher = Self {
            patterns: Vec::new(),
        };
        matcher.register_default_patterns();
        matcher
    }

    fn register_default_patterns(&mut self) {
        // Missing semicolon pattern
        self.patterns.push(ErrorPattern {
            name: "missing_semicolon".to_string(),
            matcher: Box::new(|node| {
                matches!(node.error_type(), Some(TreeErrorType::Missing { ref expected }) if expected.contains(&";".to_string()))
            }),
            fix_generator: Box::new(|node| {
                vec![ErrorFix::insertion(
                    ErrorPosition::new(node.line_col().0, node.line_col().1, node.span().1),
                    ";".to_string(),
                    "Add missing semicolon".to_string(),
                )]
            }),
        });

        // Unclosed string pattern
        self.patterns.push(ErrorPattern {
            name: "unclosed_string".to_string(),
            matcher: Box::new(|node| {
                matches!(node.error_type(), Some(TreeErrorType::Unclosed { ref delimiter, .. }) if delimiter == "\"")
            }),
            fix_generator: Box::new(|node| {
                vec![ErrorFix::insertion(
                    ErrorPosition::new(node.line_col().0, node.line_col().1, node.span().1),
                    "\"".to_string(),
                    "Close string literal".to_string(),
                )]
            }),
        });
    }

    pub fn match_error(&self, error_node: &dyn TreeErrorInfo) -> Vec<ErrorFix> {
        let mut all_fixes = Vec::new();

        for pattern in &self.patterns {
            if (pattern.matcher)(error_node) {
                all_fixes.extend((pattern.fix_generator)(error_node));
            }
        }

        all_fixes
    }
}

/// Format error with enhanced visual display
pub fn format_enhanced_error(
    error: &CompileError,
    source: &str,
    error_node: Option<&dyn TreeErrorInfo>,
) -> String {
    let mut output = String::new();

    // Extract error position
    let (line, col) = match error {
        CompileError::ParseError { error_position, .. } => error_position.line_col,
        _ => (1, 1),
    };

    // Get source lines
    let lines: Vec<&str> = source.lines().collect();
    if line > 0 && line <= lines.len() {
        let error_line = lines[line - 1];

        // Format error message with position
        output.push_str(&format!("Error at {}:{}: ", line, col));

        // Add enhanced error message if available
        if let Some(node) = error_node {
            output.push_str(&node.error_message());

            // Add parse context
            let context = node.parse_context();
            output.push_str(&format!("\nContext: {:?}", context));

            // Add expected tokens
            let expected = context.expected_tokens();
            if !expected.is_empty() {
                output.push_str(&format!("\nExpected one of: {}", expected.join(", ")));
            }
        } else {
            output.push_str(&error.to_string());
        }

        output.push_str("\n\n");

        // Show the error line with context
        if line > 1 {
            output.push_str(&format!("{:>4} | {}\n", line - 1, lines[line - 2]));
        }
        output.push_str(&format!("{:>4} | {}\n", line, error_line));

        // Add error indicator
        output.push_str(&format!("{:>4} | ", ""));
        if col > 0 && col <= error_line.len() {
            output.push_str(&" ".repeat(col - 1));
            output.push_str("^");

            // Extend indicator for multi-character errors
            if let Some(node) = error_node {
                let span_len = node.span().1 - node.span().0;
                if span_len > 1 {
                    output.push_str(&"~".repeat(span_len.min(20) - 1));
                }
            }
        }
        output.push('\n');

        if line < lines.len() {
            output.push_str(&format!("{:>4} | {}\n", line + 1, lines[line]));
        }

        // Add fix suggestions
        if let Some(node) = error_node {
            let fixes = node.suggested_fixes();
            if !fixes.is_empty() {
                output.push_str("\nSuggested fixes:\n");
                for (i, fix) in fixes.iter().enumerate() {
                    output.push_str(&format!(
                        "  {}. {} (confidence: {:.0}%)\n",
                        i + 1,
                        fix.description,
                        fix.confidence * 100.0
                    ));
                }
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock implementation for testing
    struct MockErrorNode {
        kind: String,
        error_type: Option<TreeErrorType>,
        span: (usize, usize),
        line_col: (usize, usize),
    }

    #[cfg(feature = "tree-sitter-parser")]
    impl crate::parsers::tree_sitter::tree_traits::TreeNode for MockErrorNode {
        fn node_kind(&self) -> &str {
            &self.kind
        }
        fn text(&self) -> Option<&str> {
            None
        }
        fn children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
            Box::new(std::iter::empty())
        }
        fn child_by_name(&self, _name: &str) -> Option<&Self> {
            None
        }
        fn span(&self) -> (usize, usize) {
            self.span
        }
        fn line_col(&self) -> (usize, usize) {
            self.line_col
        }
        fn is_error(&self) -> bool {
            true
        }
    }

    impl TreeErrorInfo for MockErrorNode {
        fn node_kind(&self) -> &str {
            &self.kind
        }
        
        fn is_error(&self) -> bool {
            true
        }
        
        fn line_col(&self) -> (usize, usize) {
            self.line_col
        }
        
        fn span(&self) -> (usize, usize) {
            self.span
        }
        
        fn error_type(&self) -> Option<TreeErrorType> {
            self.error_type.clone()
        }
    }

    #[test]
    fn test_error_fix_creation() {
        let pos = ErrorPosition::new(1, 10, 10);
        let fix = ErrorFix::insertion(pos, ";".to_string(), "Add semicolon".to_string());

        assert_eq!(fix.replacement, ";");
        assert_eq!(fix.confidence, 0.8);
    }

    #[test]
    fn test_error_pattern_matcher() {
        let matcher = ErrorPatternMatcher::new();
        let error_node = MockErrorNode {
            kind: "ERROR".to_string(),
            error_type: Some(TreeErrorType::Missing {
                expected: vec![";".to_string()],
            }),
            span: (10, 10),
            line_col: (1, 10),
        };

        let fixes = matcher.match_error(&error_node);
        assert!(!fixes.is_empty());
        assert_eq!(fixes[0].replacement, ";");
    }
}
