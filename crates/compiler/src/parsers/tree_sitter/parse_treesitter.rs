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

//! Clean tree-sitter to CST conversion using recursive semantic traversal

use tree_sitter::{Node, Parser};
use tree_sitter_moo;

use crate::cst::{CSTNode, CSTNodeKind, CSTSpan};
use crate::errors::enhanced_errors::ParseContext;
use crate::errors::tree_error_recovery::{
    ErrorFix, ErrorPosition, ErrorSpan, TreeErrorInfo, TreeErrorType,
};
use crate::parsers::parse::moo::Rule;
use moor_common::model::{CompileContext, CompileError};

/// Categories of tree-sitter nodes for cleaner dispatch
#[derive(Debug, Clone, Copy, PartialEq)]
enum NodeCategory {
    Program,
    Statement,
    Expression,
    Terminal,
    Collection,
    System,
    Error,
    Unknown,
}

/// Clean tree-sitter to CST converter using recursive semantic traversal
pub struct TreeSitterConverter<'a> {
    source: &'a str,
}

impl<'a> TreeSitterConverter<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    /// Parse source and convert to CST
    pub fn parse(&mut self) -> Result<CSTNode, CompileError> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_moo::language())
            .map_err(|e| self.error_at_position(0, 0, format!("Failed to set language: {}", e)))?;

        let tree = parser
            .parse(self.source, None)
            .ok_or_else(|| self.error_at_position(0, 0, "Failed to parse source".to_string()))?;

        let root = tree.root_node();

        // Check for parse errors
        if root.has_error() {
            return Err(self.find_error(&root));
        }

        // Convert the root node using recursive semantic traversal
        self.convert_node(&root)
    }

    /// Parse using semantic walker approach for improved robustness
    pub fn parse_with_semantic_walker(&mut self) -> Result<CSTNode, CompileError> {
        use super::parse_treesitter_semantic::parse_with_semantic_walker;
        parse_with_semantic_walker(self.source)
    }

    /// Main recursive conversion function - dispatches based on semantic node type
    fn convert_node(&mut self, node: &Node) -> Result<CSTNode, CompileError> {
        let span = self.node_span(node);
        let node_kind = node.kind();

        // First check for structural tokens we skip
        if self.is_punctuation(node_kind) {
            return self.convert_punctuation(node, span);
        }

        // Then dispatch based on node category
        match self.categorize_node(node_kind) {
            NodeCategory::Program => self.convert_program_node(node_kind, node, span),
            NodeCategory::Statement => self.convert_statement_node(node_kind, node, span),
            NodeCategory::Expression => self.convert_expression_node(node_kind, node, span),
            NodeCategory::Terminal => self.convert_terminal_node(node_kind, node, span),
            NodeCategory::Collection => self.convert_collection_node(node_kind, node, span),
            NodeCategory::System => self.convert_system_node(node_kind, node, span),
            NodeCategory::Error => self.convert_error_node(node, span),
            NodeCategory::Unknown => self.convert_unknown_node(node, span),
        }
    }

    /// Categorize a node type for dispatch
    fn categorize_node(&self, node_kind: &str) -> NodeCategory {
        match node_kind {
            "program" | "source_file" => NodeCategory::Program,

            "statement"
            | "expression_statement"
            | "assignment_operation"
            | "assignment_expr"
            | "try_statement"
            | "if_statement"
            | "while_statement"
            | "for_statement"
            | "for_in_statement"
            | "return_statement"
            | "return_expression"
            | "break_statement"
            | "continue_statement" => NodeCategory::Statement,

            "expression"
            | "binary_operation"
            | "unary_operation"
            | "conditional_operation"
            | "method_call"
            | "function_call"
            | "call"
            | "property"
            | "index"
            | "parenthesized_expression" => NodeCategory::Expression,

            "identifier" | "integer" | "INTEGER" | "float" | "string" | "boolean" | "<" | ">"
            | "<=" | ">=" | "==" | "!=" | "+" | "-" | "*" | "/" | "%" | "^" | "&&" | "||" | "!"
            | "in" | "=" | "error_code" => NodeCategory::Terminal,

            "list" | "map" | "binding_pattern" => NodeCategory::Collection,

            "system_property" => NodeCategory::System,

            "ERROR" => NodeCategory::Error,

            _ => NodeCategory::Unknown,
        }
    }

    /// Check if a node is punctuation
    fn is_punctuation(&self, node_kind: &str) -> bool {
        matches!(
            node_kind,
            "(" | ")" | "{" | "}" | "[" | "]" | ";" | "," | "." | ":" | "?" | "|" | "=" | "$"
        )
    }

    /// Convert punctuation tokens
    fn convert_punctuation(&self, node: &Node, span: CSTSpan) -> Result<CSTNode, CompileError> {
        Ok(CSTNode {
            rule: Rule::expr,
            span,
            kind: CSTNodeKind::Terminal {
                text: self.node_text(node),
            },
        })
    }

    /// Convert program-level nodes
    fn convert_program_node(
        &mut self,
        node_kind: &str,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        match node_kind {
            "program" | "source_file" => self.convert_program(node, span),
            _ => unreachable!("Invalid program node type: {}", node_kind),
        }
    }

    /// Convert statement nodes
    fn convert_statement_node(
        &mut self,
        node_kind: &str,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        match node_kind {
            "statement" => self.convert_statement(node, span),
            "expression_statement" => self.convert_expression_statement(node, span),
            "assignment_operation" | "assignment_expr" => self.convert_assignment(node, span),
            "try_statement" => self.convert_try_statement(node, span),
            "if_statement" => self.convert_if_statement(node, span),
            "while_statement" => self.convert_while_statement(node, span),
            "for_statement" | "for_in_statement" => self.convert_for_statement(node, span),
            "return_statement" | "return_expression" => self.convert_return_statement(node, span),
            "break_statement" => self.convert_break_statement(node, span),
            "continue_statement" => self.convert_continue_statement(node, span),
            _ => unreachable!("Invalid statement node type: {}", node_kind),
        }
    }

    /// Convert expression nodes
    fn convert_expression_node(
        &mut self,
        node_kind: &str,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        match node_kind {
            "expression" => self.convert_expression(node, span),
            "binary_operation" => self.convert_binary_operation(node, span),
            "unary_operation" => self.convert_unary_operation(node, span),
            "conditional_operation" => self.convert_conditional_operation(node, span),
            "method_call" => self.convert_method_call(node, span),
            "function_call" | "call" => self.convert_function_call(node, span),
            "property" => self.convert_property_access(node, span),
            "index" => self.convert_index(node, span),
            "parenthesized_expression" => self.convert_parenthesized_expression(node, span),
            _ => unreachable!("Invalid expression node type: {}", node_kind),
        }
    }

    /// Convert terminal nodes
    fn convert_terminal_node(
        &mut self,
        node_kind: &str,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let rule = match node_kind {
            "identifier" => Rule::ident,
            "integer" | "INTEGER" => Rule::integer,
            "float" => Rule::float,
            "string" => Rule::string,
            "boolean" => Rule::boolean,
            "<" => Rule::lt,
            ">" => Rule::gt,
            "<=" => Rule::lte,
            ">=" => Rule::gte,
            "==" => Rule::eq,
            "!=" => Rule::neq,
            "+" => Rule::add,
            "-" => Rule::sub,
            "*" => Rule::mul,
            "/" => Rule::div,
            "%" => Rule::modulus,
            "^" => Rule::pow,
            "&&" => Rule::land,
            "||" => Rule::lor,
            "in" => Rule::in_range,
            "!" => Rule::not,
            "=" => Rule::assign,
            "error_code" => {
                // For error_code nodes, get the text from the first child
                if let Some(child) = node.child(0) {
                    return Ok(CSTNode {
                        rule: Rule::errcode,
                        span,
                        kind: CSTNodeKind::Terminal {
                            text: self.node_text(&child),
                        },
                    });
                } else {
                    return Err(CompileError::ParseError {
                        error_position: CompileContext {
                            line_col: (
                                node.start_position().row + 1,
                                node.start_position().column + 1,
                            ),
                        },
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Error code node has no text".to_string(),
                    });
                }
            }
            _ => unreachable!("Invalid terminal node type: {}", node_kind),
        };
        self.convert_terminal(node, span, rule)
    }

    /// Convert collection nodes
    fn convert_collection_node(
        &mut self,
        node_kind: &str,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        match node_kind {
            "list" => self.convert_list(node, span),
            "map" => self.convert_map(node, span),
            "binding_pattern" => self.convert_scatter(node, span),
            _ => unreachable!("Invalid collection node type: {}", node_kind),
        }
    }

    /// Convert system nodes
    fn convert_system_node(
        &mut self,
        node_kind: &str,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        match node_kind {
            "system_property" => self.convert_system_property(node, span),
            _ => unreachable!("Invalid system node type: {}", node_kind),
        }
    }

    /// Convert error nodes
    fn convert_error_node(&mut self, node: &Node, _span: CSTSpan) -> Result<CSTNode, CompileError> {
        use crate::errors::tree_error_recovery::TreeErrorInfo;

        // Check for specific error patterns first by examining the source code
        let error_msg = {
            // Get a broader context around the error to understand the pattern
            let start_byte = node.start_byte().saturating_sub(20);
            let end_byte = (node.end_byte() + 10).min(self.source.len());
            let context_text = &self.source[start_byte..end_byte];
            let error_offset = node.start_byte() - start_byte;

            // Check for property access pattern: "obj." at error boundary
            if error_offset > 0 && context_text[..error_offset].ends_with('.') {
                "Incomplete property access - missing property name after '.'".to_string()
            }
            // Check for method call pattern: "obj:" at error boundary
            else if error_offset > 0 && context_text[..error_offset].ends_with(':') {
                "Incomplete method call - missing method name after ':'".to_string()
            }
            // Look for patterns in the whole context
            else if context_text.contains("obj.") && node.start_byte() > 0 {
                // Find the position of obj. relative to our error
                if let Some(dot_pos) = context_text.rfind('.') {
                    if dot_pos < error_offset && error_offset - dot_pos < 3 {
                        "Incomplete property access - add property name after '.'".to_string()
                    } else {
                        TreeErrorInfo::error_message(node)
                    }
                } else {
                    TreeErrorInfo::error_message(node)
                }
            } else if context_text.contains("obj:") && node.start_byte() > 0 {
                // Find the position of obj: relative to our error
                if let Some(colon_pos) = context_text.rfind(':') {
                    if colon_pos < error_offset && error_offset - colon_pos < 3 {
                        "Incomplete method call - add method name and parentheses after ':'"
                            .to_string()
                    } else {
                        TreeErrorInfo::error_message(node)
                    }
                } else {
                    TreeErrorInfo::error_message(node)
                }
            } else {
                // Use the enhanced error info as fallback
                TreeErrorInfo::error_message(node)
            }
        };

        let context = TreeErrorInfo::parse_context(node);
        let fixes = TreeErrorInfo::suggested_fixes(node);
        let missing_fields = TreeErrorInfo::missing_fields(node);

        // Build comprehensive error message
        let mut message = error_msg;

        // Add missing fields info if any
        if !missing_fields.is_empty() {
            message.push_str(&format!(
                "\nMissing required fields: {}",
                missing_fields.join(", ")
            ));
        }

        // Add context information
        message.push_str(&format!("\nContext: {:?}", context));

        // Add expected tokens
        let expected = context.expected_tokens();
        if !expected.is_empty() {
            message.push_str(&format!("\nExpected: {}", expected.join(", ")));
        }

        // Add fix suggestions if available
        if !fixes.is_empty() {
            message.push_str("\nSuggested fixes:");
            for fix in fixes.iter().take(3) {
                // Limit to 3 suggestions
                message.push_str(&format!("\n  • {}", fix.description));
            }
        }

        Err(self.error_at_position(
            node.start_position().row,
            node.start_position().column,
            message,
        ))
    }

    /// Convert program node using field access
    fn convert_program(&mut self, node: &Node, span: CSTSpan) -> Result<CSTNode, CompileError> {
        let mut statements = Vec::new();

        // Use field access if available, otherwise iterate children
        if let Some(body) = node.child_by_field_name("body") {
            // Program has a body field containing statements
            for child in body.children(&mut body.walk()) {
                if child.kind() != "ERROR" && child.kind() != "comment" {
                    statements.push(self.convert_node(&child)?);
                }
            }
        } else {
            // Fallback to child iteration for direct statement children
            for child in node.children(&mut node.walk()) {
                if child.kind() != "ERROR" && child.kind() != "comment" && child.kind() != ";" {
                    statements.push(self.convert_node(&child)?);
                }
            }
        }

        // Wrap in statements node if we have any
        let mut children = Vec::new();
        if !statements.is_empty() {
            let statements_span = CSTSpan {
                start: statements.first().unwrap().span.start,
                end: statements.last().unwrap().span.end,
                line_col: statements.first().unwrap().span.line_col,
            };

            children.push(CSTNode {
                rule: Rule::statements,
                span: statements_span,
                kind: CSTNodeKind::NonTerminal {
                    children: statements,
                },
            });
        }

        // Add EOI
        children.push(CSTNode {
            rule: Rule::EOI,
            span: CSTSpan {
                start: span.end,
                end: span.end,
                line_col: (span.line_col.0, span.line_col.1 + 1),
            },
            kind: CSTNodeKind::Terminal {
                text: String::new(),
            },
        });

        Ok(CSTNode {
            rule: Rule::program,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Helper to wrap converted nodes as a statement
    #[allow(dead_code)]
    fn wrap_as_statement(
        &mut self,
        parts: Vec<CSTNode>,
        _end_pos: usize,
    ) -> Result<CSTNode, CompileError> {
        if parts.is_empty() {
            return Err(CompileError::ParseError {
                error_position: moor_common::model::CompileContext::new((1, 1)),
                context: "tree-sitter parsing".to_string(),
                end_line_col: None,
                message: "Empty statement".to_string(),
            });
        }

        let start_span = parts[0].span.clone();
        let end_span = parts.last().unwrap().span.clone();
        let stmt_span = CSTSpan {
            start: start_span.start,
            end: end_span.end,
            line_col: start_span.line_col,
        };

        // If we have a single part that's already a statement-like structure, use it
        let content = if parts.len() == 1 {
            let part = parts.into_iter().next().unwrap();
            match part.rule {
                Rule::assign => {
                    // Assignment should be wrapped in expr_statement
                    CSTNode {
                        rule: Rule::expr_statement,
                        span: part.span.clone(),
                        kind: CSTNodeKind::NonTerminal {
                            children: vec![part],
                        },
                    }
                }
                _ => part,
            }
        } else {
            // Multiple parts - treat as expression list in expr_statement
            let exprlist = CSTNode {
                rule: Rule::exprlist,
                span: stmt_span.clone(),
                kind: CSTNodeKind::NonTerminal { children: parts },
            };
            CSTNode {
                rule: Rule::expr_statement,
                span: stmt_span.clone(),
                kind: CSTNodeKind::NonTerminal {
                    children: vec![exprlist],
                },
            }
        };

        Ok(CSTNode {
            rule: Rule::statement,
            span: stmt_span,
            kind: CSTNodeKind::NonTerminal {
                children: vec![content],
            },
        })
    }

    /// Convert statement node using field access
    fn convert_statement(&mut self, node: &Node, span: CSTSpan) -> Result<CSTNode, CompileError> {
        // Try field access first, then fallback to positional access
        let content_node = if let Some(body) = node.child_by_field_name("body") {
            body
        } else if let Some(expression) = node.child_by_field_name("expression") {
            expression
        } else if let Some(child) = node.child(0) {
            child
        } else {
            return Err(self.error_at_position(
                node.start_position().row,
                node.start_position().column,
                "Statement has no content".to_string(),
            ));
        };

        let content = self.convert_node(&content_node)?;

        // Wrap appropriately based on content type
        let wrapped_content = match content_node.kind() {
            // These are already statement types
            "return_statement" | "return_expression" | "break_statement" | "continue_statement"
            | "if_statement" | "while_statement" | "for_statement" | "for_in_statement"
            | "try_statement" => content,
            // These need to be wrapped as expression statements
            "expression"
            | "assignment_operation"
            | "assignment_expr"
            | "method_call"
            | "function_call"
            | "call"
            | "binary_operation"
            | "unary_operation" => CSTNode {
                rule: Rule::expr_statement,
                span: content.span.clone(),
                kind: CSTNodeKind::NonTerminal {
                    children: vec![content],
                },
            },
            // Everything else passes through
            _ => content,
        };

        Ok(CSTNode {
            rule: Rule::statement,
            span,
            kind: CSTNodeKind::NonTerminal {
                children: vec![wrapped_content],
            },
        })
    }

    /// Convert expression statement using field access
    fn convert_expression_statement(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        // Use field access to get the expression
        let expr_node = if let Some(expr) = node.child_by_field_name("expression") {
            expr
        } else {
            // Fallback to finding expression child by type, skipping punctuation
            node.children(&mut node.walk())
                .find(|child| {
                    matches!(
                        child.kind(),
                        "expression"
                            | "assignment_operation"
                            | "assignment_expr"
                            | "method_call"
                            | "function_call"
                            | "call"
                    )
                })
                .ok_or_else(|| {
                    self.error_at_position(
                        node.start_position().row,
                        node.start_position().column,
                        "Expression statement has no expression".to_string(),
                    )
                })?
        };

        let expr = self.convert_node(&expr_node)?;

        Ok(CSTNode {
            rule: Rule::expr_statement,
            span,
            kind: CSTNodeKind::NonTerminal {
                children: vec![expr],
            },
        })
    }

    /// Convert assignment operation
    fn convert_assignment(&mut self, node: &Node, span: CSTSpan) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        // Get left side using field name
        if let Some(left) = node.child_by_field_name("left") {
            children.push(self.convert_node(&left)?);
        }

        // Add assignment operator terminal
        children.push(CSTNode {
            rule: Rule::assign,
            span: span.clone(),
            kind: CSTNodeKind::Terminal {
                text: "=".to_string(),
            },
        });

        // Get right side using field name
        if let Some(right) = node.child_by_field_name("right") {
            children.push(self.convert_node(&right)?);
        }

        Ok(CSTNode {
            rule: Rule::assign,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert method call using field access
    fn convert_method_call(&mut self, node: &Node, span: CSTSpan) -> Result<CSTNode, CompileError> {
        // Extract method call components
        let (object, method, args) = self.extract_method_call_parts(node)?;

        // Check if this is a system property call
        if let Some(ref obj) = object {
            if obj.rule == Rule::sysprop {
                return self.build_sysprop_call(obj.clone(), method, args, span);
            }
        }

        // Build regular verb call
        self.build_verb_call(object, method, args, span)
    }

    /// Extract object, method, and arguments from a method call node
    fn extract_method_call_parts(
        &mut self,
        node: &Node,
    ) -> Result<(Option<CSTNode>, Option<CSTNode>, Vec<CSTNode>), CompileError> {
        let object = node
            .child_by_field_name("object")
            .map(|obj| self.convert_node(&obj))
            .transpose()?;

        let method = node
            .child_by_field_name("method")
            .map(|method| self.convert_node(&method))
            .transpose()?;

        let args = self.extract_arguments(node)?;

        Ok((object, method, args))
    }

    /// Extract arguments from a method call or function call node
    fn extract_arguments(&mut self, node: &Node) -> Result<Vec<CSTNode>, CompileError> {
        let mut args = Vec::new();

        if let Some(arguments) = node.child_by_field_name("arguments") {
            for child in arguments.children(&mut arguments.walk()) {
                if child.kind() == "expression" {
                    args.push(self.convert_node(&child)?);
                }
            }
        }

        Ok(args)
    }

    /// Build a system property call node
    fn build_sysprop_call(
        &mut self,
        object: CSTNode,
        method: Option<CSTNode>,
        args: Vec<CSTNode>,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let mut children = vec![object];

        if let Some(verb) = method {
            children.push(verb);
        }

        let arglist = self.create_arglist(args, span.clone());
        children.push(arglist);

        Ok(CSTNode {
            rule: Rule::sysprop_call,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Build a regular verb call node
    fn build_verb_call(
        &mut self,
        object: Option<CSTNode>,
        method: Option<CSTNode>,
        args: Vec<CSTNode>,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        if let Some(obj) = object {
            children.push(obj);
        }

        if let Some(verb) = method {
            children.push(verb);
        }

        let arglist = self.create_arglist(args, span.clone());
        children.push(arglist);

        Ok(CSTNode {
            rule: Rule::verb_call,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert system property using field access
    fn convert_system_property(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        // Try field access first
        if let Some(name) = node.child_by_field_name("name") {
            let prop_name = self.node_text(&name);
            return Ok(CSTNode {
                rule: Rule::sysprop,
                span,
                kind: CSTNodeKind::Terminal {
                    text: format!("${}", prop_name),
                },
            });
        }

        // Fallback to finding identifier child
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                let prop_name = self.node_text(&child);
                return Ok(CSTNode {
                    rule: Rule::sysprop,
                    span,
                    kind: CSTNodeKind::Terminal {
                        text: format!("${}", prop_name),
                    },
                });
            }
        }

        Err(self.error_at_position(
            node.start_position().row,
            node.start_position().column,
            "System property has no identifier".to_string(),
        ))
    }

    /// Convert scatter (binding pattern) using field-based access
    fn convert_scatter(&mut self, node: &Node, span: CSTSpan) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        // Use TreeWalker style: iterate through children but use field access for structured nodes
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "identifier" => {
                    // Simple required parameter: just convert the identifier
                    children.push(self.convert_node(&child)?);
                }
                "binding_optional" => {
                    // Optional parameter: ?name = default
                    children.push(self.convert_binding_optional(&child)?);
                }
                "binding_rest" => {
                    // Rest parameter: @name
                    children.push(self.convert_binding_rest(&child)?);
                }
                "{" | "}" | "," => {
                    // Skip structural tokens
                }
                _ => {
                    // Skip other structural elements
                }
            }
        }

        Ok(CSTNode {
            rule: Rule::scatter,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert binding_optional using field-based access
    fn convert_binding_optional(&mut self, node: &Node) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        // Use field access for name (required)
        if let Some(name_node) = node.child_by_field_name("name") {
            children.push(self.convert_node(&name_node)?);
        } else {
            return Err(self.error_at_position(
                node.start_position().row,
                node.start_position().column,
                "Optional binding missing name field".to_string(),
            ));
        }

        // Use field access for default value (optional)
        if let Some(default_node) = node.child_by_field_name("default") {
            children.push(self.convert_node(&default_node)?);
        }

        Ok(CSTNode {
            rule: Rule::scatter_optional,
            span: self.node_span(node),
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert binding_rest using field-based access
    fn convert_binding_rest(&mut self, node: &Node) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        // Use field access for name (required)
        if let Some(name_node) = node.child_by_field_name("name") {
            children.push(self.convert_node(&name_node)?);
        } else {
            return Err(self.error_at_position(
                node.start_position().row,
                node.start_position().column,
                "Rest binding missing name field".to_string(),
            ));
        }

        Ok(CSTNode {
            rule: Rule::scatter_rest,
            span: self.node_span(node),
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Create arglist from arguments
    fn create_arglist(&mut self, args: Vec<CSTNode>, span: CSTSpan) -> CSTNode {
        // Wrap each argument in an argument node
        let wrapped_args: Vec<CSTNode> = args
            .into_iter()
            .map(|arg| CSTNode {
                rule: Rule::argument,
                span: arg.span.clone(),
                kind: CSTNodeKind::NonTerminal {
                    children: vec![arg],
                },
            })
            .collect();

        let exprlist = CSTNode {
            rule: Rule::exprlist,
            span: span.clone(),
            kind: CSTNodeKind::NonTerminal {
                children: wrapped_args,
            },
        };

        CSTNode {
            rule: Rule::arglist,
            span,
            kind: CSTNodeKind::NonTerminal {
                children: vec![exprlist],
            },
        }
    }

    /// Convert terminal node
    fn convert_terminal(
        &mut self,
        node: &Node,
        span: CSTSpan,
        rule: Rule,
    ) -> Result<CSTNode, CompileError> {
        Ok(CSTNode {
            rule,
            span,
            kind: CSTNodeKind::Terminal {
                text: self.node_text(node),
            },
        })
    }

    /// Convert expression node - delegate to first child
    fn convert_expression(&mut self, node: &Node, span: CSTSpan) -> Result<CSTNode, CompileError> {
        // Expression nodes typically have a single child that is the actual expression
        // (e.g., binary_operation, unary_operation, identifier, literal, etc.)
        if let Some(child) = node.child(0) {
            // For expression nodes, we should convert the child based on its type
            let child_span = self.node_span(&child);
            match child.kind() {
                "binary_operation" => self.convert_binary_operation(&child, child_span),
                "unary_operation" => self.convert_unary_operation(&child, child_span),
                "assignment_operation" | "assignment_expr" => {
                    self.convert_assignment(&child, child_span)
                }
                "method_call" => self.convert_method_call(&child, child_span),
                "function_call" | "call" => self.convert_function_call(&child, child_span),
                "property" => self.convert_property_access(&child, child_span),
                "index" => self.convert_index(&child, child_span),
                "conditional_operation" => self.convert_conditional_operation(&child, child_span),
                "parenthesized_expression" => {
                    self.convert_parenthesized_expression(&child, child_span)
                }
                "identifier" | "integer" | "float" | "string" | "boolean" => {
                    // Terminal nodes - convert them directly
                    self.convert_node(&child)
                }
                _ => {
                    // For other node types, just convert normally
                    let child_node = self.convert_node(&child)?;

                    // Don't wrap certain node types that should maintain their specific rules
                    match child_node.rule {
                        Rule::assign
                        | Rule::verb_call
                        | Rule::builtin_call
                        | Rule::sysprop_call => {
                            // These should keep their specific rules, not be wrapped as expr
                            Ok(child_node)
                        }
                        Rule::expr => {
                            // Already an expr, no need to wrap
                            Ok(child_node)
                        }
                        _ => {
                            // Wrap other types in expr rule
                            Ok(CSTNode {
                                rule: Rule::expr,
                                span,
                                kind: CSTNodeKind::NonTerminal {
                                    children: vec![child_node],
                                },
                            })
                        }
                    }
                }
            }
        } else {
            Err(self.error_at_position(
                node.start_position().row,
                node.start_position().column,
                "Expression has no content".to_string(),
            ))
        }
    }

    /// Convert return statement using field access
    fn convert_return_statement(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        // Use field access to get the expression if any
        if let Some(expr) = node.child_by_field_name("expression") {
            children.push(self.convert_node(&expr)?);
        } else {
            // Fallback to finding expression child by type
            for child in node.children(&mut node.walk()) {
                if child.kind() == "expression" {
                    children.push(self.convert_node(&child)?);
                    break;
                }
            }
        }

        Ok(CSTNode {
            rule: Rule::return_expr,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    fn convert_break_statement(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        if node.child_count() > 0 {
            return Err(self.error_at_position(
                node.start_position().row,
                node.start_position().column,
                "Break statement cannot have any content".to_string(),
            ));
        }
        Ok(CSTNode {
            rule: Rule::break_statement,
            span,
            kind: CSTNodeKind::Terminal {
                text: "break".to_string(),
            },
        })
    }

    fn convert_continue_statement(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        if node.child_count() > 0 {
            return Err(self.error_at_position(
                node.start_position().row,
                node.start_position().column,
                "Break statement cannot have any content".to_string(),
            ));
        }
        Ok(CSTNode {
            rule: Rule::continue_statement,
            span,
            kind: CSTNodeKind::Terminal {
                text: "continue".to_string(),
            },
        })
    }

    fn convert_if_statement(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        // Extract if statement components
        let condition = self.extract_if_condition(node)?;
        let consequence = self.extract_if_consequence(node)?;
        let alternative = self.extract_if_alternative(node);

        // Build children vector
        let mut children = vec![condition, consequence];
        if let Some(alt) = alternative {
            children.push(alt);
        }

        Ok(CSTNode {
            rule: Rule::if_statement,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Extract condition from if statement node
    fn extract_if_condition(&mut self, node: &Node) -> Result<CSTNode, CompileError> {
        node.child_by_field_name("condition")
            .ok_or_else(|| {
                self.error_at_position(
                    node.start_position().row,
                    node.start_position().column,
                    "If statement missing condition".to_string(),
                )
            })
            .and_then(|cond| self.convert_node(&cond))
    }

    /// Extract consequence (then body) from if statement node
    fn extract_if_consequence(&mut self, node: &Node) -> Result<CSTNode, CompileError> {
        // Try various field names for the consequence body
        const CONSEQUENCE_FIELDS: &[&str] = &["consequence", "body", "then_body", "then"];

        for field_name in CONSEQUENCE_FIELDS {
            if let Some(body) = node.child_by_field_name(field_name) {
                return self.convert_node(&body);
            }
        }

        // Fallback - look for statement child after condition
        self.find_consequence_fallback(node)
    }

    /// Fallback method to find consequence body when field access fails
    fn find_consequence_fallback(&mut self, node: &Node) -> Result<CSTNode, CompileError> {
        let mut statement_count = 0;

        for child in node.children(&mut node.walk()) {
            if child.kind() == "statement" {
                statement_count += 1;
                // The second statement is typically the consequence
                if statement_count == 2 {
                    return self.convert_node(&child);
                }
            }
        }

        Err(self.error_at_position(
            node.start_position().row,
            node.start_position().column,
            "If statement missing consequence body".to_string(),
        ))
    }

    /// Extract alternative (else body) from if statement node
    fn extract_if_alternative(&mut self, node: &Node) -> Option<CSTNode> {
        // Try various field names for the alternative body
        const ALTERNATIVE_FIELDS: &[&str] = &["alternative", "else_body", "else"];

        for field_name in ALTERNATIVE_FIELDS {
            if let Some(else_body) = node.child_by_field_name(field_name) {
                return self.convert_node(&else_body).ok();
            }
        }

        None
    }

    fn convert_while_statement(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        if let Some(condition) = node.child_by_field_name("condition") {
            children.push(self.convert_node(&condition)?);
        }

        if let Some(body) = node.child_by_field_name("body") {
            children.push(self.convert_node(&body)?);
        }

        if children.is_empty() {
            return Err(self.error_at_position(
                node.start_position().row,
                node.start_position().column,
                "While statement missing components".to_string(),
            ));
        }

        Ok(CSTNode {
            rule: Rule::while_statement,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    fn convert_for_statement(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        if let Some(variable) = node.child_by_field_name("variable") {
            children.push(self.convert_node(&variable)?);
        }

        if let Some(iterable) = node.child_by_field_name("iterable") {
            children.push(self.convert_node(&iterable)?);
        }

        if let Some(body) = node.child_by_field_name("body") {
            children.push(self.convert_node(&body)?);
        }

        Ok(CSTNode {
            rule: Rule::for_in_statement,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    fn convert_try_statement(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        if let Some(body) = node.child_by_field_name("body") {
            children.push(self.convert_node(&body)?);
        }

        // Handle except clauses - tree-sitter uses "handlers" field
        if let Some(handlers) = node.child_by_field_name("handlers") {
            children.push(self.convert_except_clause(&handlers)?);
        }

        Ok(CSTNode {
            rule: Rule::try_except_statement,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    fn convert_except_clause(&mut self, node: &Node) -> Result<CSTNode, CompileError> {
        let span = self.node_span(node);
        let mut children = Vec::new();

        // Create unlabelled_except node with codes
        if let Some(codes) = node.child_by_field_name("codes") {
            let codes_span = self.node_span(&codes);
            let mut except_clause_children = Vec::new();

            // Create a codes node containing an exprlist with the error code
            let mut codes_children = Vec::new();
            let mut exprlist_children = Vec::new();

            // Wrap the error code in an argument node
            let codes_cst = self.convert_node(&codes)?;
            let arg_node = CSTNode {
                rule: Rule::argument,
                span: codes_cst.span.clone(),
                kind: CSTNodeKind::NonTerminal {
                    children: vec![CSTNode {
                        rule: Rule::expr,
                        span: codes_cst.span.clone(),
                        kind: CSTNodeKind::NonTerminal {
                            children: vec![CSTNode {
                                rule: Rule::atom,
                                span: codes_cst.span.clone(),
                                kind: CSTNodeKind::NonTerminal {
                                    children: vec![CSTNode {
                                        rule: Rule::err,
                                        span: codes_cst.span.clone(),
                                        kind: CSTNodeKind::NonTerminal {
                                            children: vec![codes_cst],
                                        },
                                    }],
                                },
                            }],
                        },
                    }],
                },
            };
            exprlist_children.push(arg_node);

            let exprlist_node = CSTNode {
                rule: Rule::exprlist,
                span: codes_span.clone(),
                kind: CSTNodeKind::NonTerminal {
                    children: exprlist_children,
                },
            };
            codes_children.push(exprlist_node);

            let codes_node = CSTNode {
                rule: Rule::codes,
                span: codes_span.clone(),
                kind: CSTNodeKind::NonTerminal {
                    children: codes_children,
                },
            };
            except_clause_children.push(codes_node);

            let unlabelled_except = CSTNode {
                rule: Rule::unlabelled_except,
                span: codes_span,
                kind: CSTNodeKind::NonTerminal {
                    children: except_clause_children,
                },
            };
            children.push(unlabelled_except);
        }

        // Convert body to statements node
        if let Some(body) = node.child_by_field_name("body") {
            let body_span = self.node_span(&body);
            let body_cst = self.convert_node(&body)?;

            // Wrap in statements node
            let statements_node = CSTNode {
                rule: Rule::statements,
                span: body_span,
                kind: CSTNodeKind::NonTerminal {
                    children: vec![body_cst],
                },
            };
            children.push(statements_node);
        }

        Ok(CSTNode {
            rule: Rule::except,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    fn convert_binary_operation(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        if let Some(left) = node.child_by_field_name("left") {
            children.push(self.convert_node(&left)?);
        }

        if let Some(operator) = node.child_by_field_name("operator") {
            children.push(self.convert_node(&operator)?);
        }

        if let Some(right) = node.child_by_field_name("right") {
            children.push(self.convert_node(&right)?);
        }

        Ok(CSTNode {
            rule: Rule::expr,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    fn convert_unary_operation(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        if let Some(operator) = node.child_by_field_name("operator") {
            children.push(self.convert_node(&operator)?);
        }

        if let Some(operand) = node.child_by_field_name("operand") {
            children.push(self.convert_node(&operand)?);
        }

        Ok(CSTNode {
            rule: Rule::expr,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    fn convert_conditional_operation(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        if let Some(condition) = node.child_by_field_name("condition") {
            children.push(self.convert_node(&condition)?);
        }

        if let Some(consequence) = node.child_by_field_name("consequence") {
            children.push(self.convert_node(&consequence)?);
        }

        if let Some(alternative) = node.child_by_field_name("alternative") {
            children.push(self.convert_node(&alternative)?);
        }

        Ok(CSTNode {
            rule: Rule::cond_expr,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    fn convert_function_call(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        if let Some(function) = node.child_by_field_name("function") {
            children.push(self.convert_node(&function)?);
        }

        if let Some(arguments) = node.child_by_field_name("arguments") {
            // Arguments field might contain an argument list or individual expressions
            if arguments.kind() == "argument_list" {
                // Convert the argument list structure
                let mut args = Vec::new();
                for child in arguments.children(&mut arguments.walk()) {
                    if child.kind() == "expression" {
                        args.push(self.convert_node(&child)?);
                    }
                }
                let arglist = self.create_arglist(args, span.clone());
                children.push(arglist);
            } else {
                // Single argument or other structure
                let converted_args = self.convert_node(&arguments)?;
                if converted_args.rule == Rule::arglist {
                    children.push(converted_args);
                } else {
                    // Wrap in arglist
                    let arglist = self.create_arglist(vec![converted_args], span.clone());
                    children.push(arglist);
                }
            }
        } else {
            // Create empty arglist
            let arglist = self.create_arglist(vec![], span.clone());
            children.push(arglist);
        }

        Ok(CSTNode {
            rule: Rule::builtin_call,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    fn convert_property_access(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        if let Some(object) = node.child_by_field_name("object") {
            children.push(self.convert_node(&object)?);
        }

        if let Some(property) = node.child_by_field_name("property") {
            children.push(self.convert_node(&property)?);
        }

        Ok(CSTNode {
            rule: Rule::prop,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    fn convert_index(&mut self, node: &Node, span: CSTSpan) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        if let Some(object) = node.child_by_field_name("object") {
            children.push(self.convert_node(&object)?);
        }

        if let Some(index) = node.child_by_field_name("index") {
            children.push(self.convert_node(&index)?);
        }

        Ok(CSTNode {
            rule: Rule::index_single,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    fn convert_parenthesized_expression(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        // Extract the inner expression and wrap in paren_expr
        if let Some(inner) = node.child(0) {
            if inner.kind() == "expression" {
                let inner_expr = self.convert_node(&inner)?;
                return Ok(CSTNode {
                    rule: Rule::paren_expr,
                    span,
                    kind: CSTNodeKind::NonTerminal {
                        children: vec![inner_expr],
                    },
                });
            }
        }

        // Fallback
        self.convert_unknown_node(node, span)
    }

    fn convert_list(&mut self, node: &Node, span: CSTSpan) -> Result<CSTNode, CompileError> {
        let mut arguments = Vec::new();

        // Convert all expression children and wrap them as arguments
        for child in node.children(&mut node.walk()) {
            if child.kind() == "expression" {
                let expr = self.convert_node(&child)?;
                // Wrap the expression in an argument node
                let argument = CSTNode {
                    rule: Rule::argument,
                    span: self.node_span(&child),
                    kind: CSTNodeKind::NonTerminal {
                        children: vec![expr],
                    },
                };
                arguments.push(argument);
            }
        }

        // Wrap in exprlist
        let exprlist = CSTNode {
            rule: Rule::exprlist,
            span: span.clone(),
            kind: CSTNodeKind::NonTerminal {
                children: arguments,
            },
        };

        Ok(CSTNode {
            rule: Rule::list,
            span,
            kind: CSTNodeKind::NonTerminal {
                children: vec![exprlist],
            },
        })
    }

    fn convert_map(&mut self, node: &Node, span: CSTSpan) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        // Iterate through all children looking for map_entry nodes
        for child in node.children(&mut node.walk()) {
            if child.kind() == "map_entry" {
                // Convert the key-value pair and flatten them into the list
                if let Some(key) = child.child_by_field_name("key") {
                    let key_node = self.convert_node(&key)?;
                    // Wrap in expr if it's not already an expr
                    let key_expr = if key_node.rule == Rule::expr {
                        key_node
                    } else {
                        CSTNode {
                            rule: Rule::expr,
                            span: self.node_span(&key),
                            kind: CSTNodeKind::NonTerminal {
                                children: vec![key_node],
                            },
                        }
                    };
                    children.push(key_expr);
                }
                if let Some(value) = child.child_by_field_name("value") {
                    let value_node = self.convert_node(&value)?;
                    // Wrap in expr if it's not already an expr
                    let value_expr = if value_node.rule == Rule::expr {
                        value_node
                    } else {
                        CSTNode {
                            rule: Rule::expr,
                            span: self.node_span(&value),
                            kind: CSTNodeKind::NonTerminal {
                                children: vec![value_node],
                            },
                        }
                    };
                    children.push(value_expr);
                }
            }
        }

        Ok(CSTNode {
            rule: Rule::map,
            span,
            kind: CSTNodeKind::NonTerminal { children },
        })
    }

    /// Convert unknown node types by recursively converting children
    fn convert_unknown_node(
        &mut self,
        node: &Node,
        span: CSTSpan,
    ) -> Result<CSTNode, CompileError> {
        let mut children = Vec::new();

        for child in node.children(&mut node.walk()) {
            // Skip comments
            if child.kind() != "comment" {
                children.push(self.convert_node(&child)?);
            }
        }

        // If no children, treat as terminal
        if children.is_empty() {
            Ok(CSTNode {
                rule: Rule::expr,
                span,
                kind: CSTNodeKind::Terminal {
                    text: self.node_text(node),
                },
            })
        } else {
            Ok(CSTNode {
                rule: Rule::expr,
                span,
                kind: CSTNodeKind::NonTerminal { children },
            })
        }
    }

    /// Utility functions
    fn node_span(&self, node: &Node) -> CSTSpan {
        let start_pos = node.start_position();
        CSTSpan {
            start: node.start_byte(),
            end: node.end_byte(),
            line_col: (start_pos.row + 1, start_pos.column + 1),
        }
    }

    fn node_text(&self, node: &Node) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        self.source[start..end].to_string()
    }

    fn error_at_position(&self, row: usize, col: usize, message: String) -> CompileError {
        CompileError::ParseError {
            error_position: CompileContext::new((row + 1, col + 1)),
            context: "tree-sitter parsing".to_string(),
            end_line_col: None,
            message,
        }
    }

    fn find_error(&self, node: &Node) -> CompileError {
        use crate::errors::tree_error_recovery::TreeErrorInfo;

        if node.kind() == "ERROR" {
            let pos = node.start_position();

            // Try to use semantic analysis from tree walker for better error detection
            let message = self
                .analyze_error_with_semantic_walker(node)
                .unwrap_or_else(|| self.analyze_error_with_source_pattern(node));

            let context = TreeErrorInfo::parse_context(node);
            let fixes = TreeErrorInfo::suggested_fixes(node);

            // Build comprehensive error message
            let mut full_message = message;

            // Add context information
            full_message.push_str(&format!("\nContext: {:?}", context));

            // Add expected tokens
            let expected = context.expected_tokens();
            if !expected.is_empty() {
                full_message.push_str(&format!("\nExpected one of: {}", expected.join(", ")));
            }

            // Add fix suggestions if available
            if !fixes.is_empty() {
                full_message.push_str("\nSuggested fixes:");
                for (i, fix) in fixes.iter().enumerate() {
                    full_message.push_str(&format!("\n  {}. {}", i + 1, fix.description));
                }
            }

            return self.error_at_position(pos.row, pos.column, full_message);
        }

        for child in node.children(&mut node.walk()) {
            if child.has_error() {
                return self.find_error(&child);
            }
        }

        let pos = node.start_position();
        self.error_at_position(pos.row, pos.column, "Parse error".to_string())
    }

    /// Analyze error using semantic tree walker for enhanced context
    fn analyze_error_with_semantic_walker(&self, node: &Node) -> Option<String> {
        use super::tree_walker::SemanticTreeWalker;

        // Create a semantic walker to analyze the tree structure
        let mut walker = SemanticTreeWalker::new(self.source);

        // Get root node by traversing up from the error node
        let mut current = *node;
        while let Some(parent) = current.parent() {
            current = parent;
        }
        let root = current;

        // Attempt three-phase semantic analysis
        if walker.discover_semantics(&root).is_ok() && walker.analyze_semantics().is_ok() {
            // If semantic analysis succeeds, analyze the specific error location
            self.analyze_error_with_semantic_context(&walker, node)
        } else {
            // If semantic analysis fails, use pattern matching
            None
        }
    }

    /// Analyze error with semantic context from walker
    fn analyze_error_with_semantic_context(
        &self,
        _walker: &super::tree_walker::SemanticTreeWalker,
        node: &Node,
    ) -> Option<String> {
        // Check if this is a property access or method call error
        if let Some(parent) = node.parent() {
            match parent.kind() {
                "property_access" | "member_access" => Some(
                    "Incomplete property access - property name required after '.'".to_string(),
                ),
                "method_call" | "verb_call" => {
                    Some("Incomplete method call - method name required after ':'".to_string())
                }
                "expression_statement" => {
                    // Check the source text around the error
                    let start_byte = node.start_byte().saturating_sub(5);
                    let end_byte = (node.end_byte() + 5).min(self.source.len());
                    let context = &self.source[start_byte..end_byte];

                    if context.contains(".") {
                        Some("Incomplete property access - add property name after '.'".to_string())
                    } else if context.contains(":") {
                        Some(
                            "Incomplete method call - add method name and arguments after ':'"
                                .to_string(),
                        )
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    }

    /// Analyze error using source pattern matching as fallback
    fn analyze_error_with_source_pattern(&self, node: &Node) -> String {
        // Get context around the error position
        let start_byte = node.start_byte().saturating_sub(10);
        let end_byte = (node.end_byte() + 10).min(self.source.len());
        let context = &self.source[start_byte..end_byte];
        let error_offset = node.start_byte() - start_byte;

        // Check for specific patterns
        if context[..error_offset].ends_with('.') {
            "Property name required after '.'".to_string()
        } else if context[..error_offset].ends_with(':') {
            "Method name required after ':'".to_string()
        } else if context.contains("\"") && !context.matches("\"").count() % 2 == 0 {
            "Unclosed string literal - add closing quote".to_string()
        } else if context.contains("(")
            && context.matches("(").count() > context.matches(")").count()
        {
            "Unmatched opening parenthesis - add closing ')' ".to_string()
        } else if context.contains("[")
            && context.matches("[").count() > context.matches("]").count()
        {
            "Unmatched opening bracket - add closing ']'".to_string()
        } else if context.contains("{")
            && context.matches("{").count() > context.matches("}").count()
        {
            "Unmatched opening brace - add closing '}'".to_string()
        } else {
            // Generic error message based on node type
            match node.kind() {
                "ERROR" => {
                    if let Some(parent) = node.parent() {
                        match parent.kind() {
                            "program" => "Syntax error".to_string(),
                            "expression_statement" => "Invalid expression".to_string(),
                            "binary_operation" => "Invalid operator or operand".to_string(),
                            _ => format!("Unexpected '{}' in {}", node.kind(), parent.kind()),
                        }
                    } else {
                        "Parse error".to_string()
                    }
                }
                _ => format!("Unexpected '{}'", node.kind()),
            }
        }
    }
}

/// Public API function
pub fn parse_with_tree_sitter(source: &str) -> Result<CSTNode, CompileError> {
    let mut converter = TreeSitterConverter::new(source);
    converter.parse()
}

/// Parse with tree-sitter returning ParseCst (equivalent to parse_program_cst)
pub fn parse_program_with_tree_sitter(
    source: &str,
    options: crate::CompileOptions,
) -> Result<crate::parsers::parse_cst::ParseCst, CompileError> {
    let cst = parse_with_tree_sitter(source)?;

    // Convert CST to AST using the CST transformer
    let transformer = crate::parsers::parse_cst::CSTTreeTransformer::new(options);
    transformer.transform(cst)
}

/// Compile with tree-sitter
pub fn compile_with_tree_sitter(
    source: &str,
    options: crate::CompileOptions,
) -> Result<crate::Program, CompileError> {
    let parse_cst = parse_program_with_tree_sitter(source, options.clone())?;
    crate::codegen::do_compile_cst(parse_cst, options)
}

/// Implementation of TreeNode trait for tree_sitter::Node
///
/// This allows tree-sitter nodes to work with the generic AST building pipeline
impl<'a> crate::parsers::tree_sitter::tree_traits::TreeNode for tree_sitter::Node<'a> {
    fn node_kind(&self) -> &str {
        // Tree-sitter already provides semantic node kinds, but we may need
        // to normalize some names to match our expected semantics
        match self.kind() {
            // Normalize some tree-sitter names to match our semantic names
            "INTEGER" => "integer_literal",
            "identifier" => "identifier",
            "string" => "string_literal",
            "float" => "float_literal",
            "boolean" => "boolean_literal",
            "binary_operation" => {
                // For binary operations, we need to check the operator
                // This is a simplified approach - ideally we'd look at the operator child
                "binary_operation"
            }
            "unary_operation" => "unary_operation",
            "assignment_operation" | "assignment_expr" => "assignment",
            "method_call" => "method_call",
            "function_call" | "call" => "function_call",
            "property" => "property_access",
            "index" => "index_access",
            "conditional_operation" => "conditional_expression",
            "parenthesized_expression" => "parenthesized_expression",
            "list" => "list_literal",
            "map" => "map_literal",
            "error_code" => "error_literal",
            "system_property" => "system_property_access",
            "binding_pattern" => "scatter_pattern",
            "binding_optional" => "optional_parameter",
            "binding_rest" => "rest_parameter",

            // Statements
            "expression_statement" => "expression_statement",
            "if_statement" => "if_statement",
            "while_statement" => "while_statement",
            "for_statement" | "for_in_statement" => "for_statement",
            "try_statement" => "try_statement",
            "return_statement" | "return_expression" => "return_statement",
            "break_statement" => "break_statement",
            "continue_statement" => "continue_statement",

            // Operators - normalize to semantic names
            "+" => "binary_add",
            "-" => "binary_sub",
            "*" => "binary_mul",
            "/" => "binary_div",
            "%" => "binary_mod",
            "^" => "binary_pow",
            "==" => "binary_eq",
            "!=" => "binary_neq",
            ">" => "binary_gt",
            "<" => "binary_lt",
            ">=" => "binary_gte",
            "<=" => "binary_lte",
            "in" => "binary_in",
            "&&" => "logical_and",
            "||" => "logical_or",
            "!" => "unary_not",
            "=" => "assignment_op",

            // Structural
            "program" | "source_file" => "program",
            "statement" => "statement",
            "expression" => "expression",

            // Comments
            "comment" => "line_comment",

            // Pass through everything else as-is
            other => other,
        }
    }

    fn text(&self) -> Option<&str> {
        // For tree-sitter nodes, we don't have direct access to text
        // This would need to be implemented by storing a reference to the source
        // For now, return None and let the caller handle text extraction
        None
    }

    fn children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
        // Tree-sitter's children() returns owned nodes, but we need references
        // For now, return empty iterator - this would need a different approach in practice
        Box::new(std::iter::empty())
    }

    fn child_by_name(&self, _name: &str) -> Option<&Self> {
        // Tree-sitter provides excellent field name support, but returns owned nodes
        // For now, return None - this would need a different approach in practice
        None
    }

    fn span(&self) -> (usize, usize) {
        (self.start_byte(), self.end_byte())
    }

    fn line_col(&self) -> (usize, usize) {
        let pos = self.start_position();
        (pos.row + 1, pos.column + 1) // Convert to 1-based indexing
    }

    fn is_error(&self) -> bool {
        self.kind() == "ERROR" || self.has_error()
    }

    fn is_content(&self) -> bool {
        // Filter out comments and structural tokens
        !matches!(
            self.kind(),
            "comment"
                | "("
                | ")"
                | "{"
                | "}"
                | "["
                | "]"
                | ";"
                | ","
                | "."
                | ":"
                | "?"
                | "|"
                | "WHITESPACE"
        )
    }
}

/// Helper struct to provide text access for tree-sitter nodes
///
/// Since tree-sitter nodes don't directly contain text, we need to pair them
/// with the source code to extract text content.
pub struct TreeSitterNodeWithSource<'a> {
    pub node: tree_sitter::Node<'a>,
    pub source: &'a str,
}

impl<'a> TreeSitterNodeWithSource<'a> {
    pub fn new(node: tree_sitter::Node<'a>, source: &'a str) -> Self {
        Self { node, source }
    }

    /// Extract text content from the source for this node
    pub fn text_content(&self) -> &str {
        let start = self.node.start_byte();
        let end = self.node.end_byte();
        &self.source[start..end]
    }
}

impl<'a> crate::parsers::tree_sitter::tree_traits::TreeNode for TreeSitterNodeWithSource<'a> {
    fn node_kind(&self) -> &str {
        crate::parsers::tree_sitter::tree_traits::TreeNode::node_kind(&self.node)
    }

    fn text(&self) -> Option<&str> {
        Some(self.text_content())
    }

    fn children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
        // This is tricky because we need to create TreeSitterNodeWithSource
        // for each child, but we can't do that without allocating
        // For now, delegate to the inner node
        Box::new(std::iter::empty()) // TODO: Implement properly
    }

    fn child_by_name(&self, _name: &str) -> Option<&Self> {
        // Same issue as children() - would need allocation
        None // TODO: Implement properly
    }

    fn span(&self) -> (usize, usize) {
        crate::parsers::tree_sitter::tree_traits::TreeNode::span(&self.node)
    }

    fn line_col(&self) -> (usize, usize) {
        crate::parsers::tree_sitter::tree_traits::TreeNode::line_col(&self.node)
    }

    fn is_error(&self) -> bool {
        crate::parsers::tree_sitter::tree_traits::TreeNode::is_error(&self.node)
    }

    fn is_content(&self) -> bool {
        crate::parsers::tree_sitter::tree_traits::TreeNode::is_content(&self.node)
    }
}

/// Implementation of TreeErrorInfo for enhanced error recovery  
impl<'a> TreeErrorInfo for tree_sitter::Node<'a> {
    fn node_kind(&self) -> &str {
        self.kind()
    }

    fn is_error(&self) -> bool {
        self.kind() == "ERROR" || self.has_error()
    }

    fn line_col(&self) -> (usize, usize) {
        let pos = self.start_position();
        (pos.row + 1, pos.column + 1)
    }

    fn span(&self) -> (usize, usize) {
        (self.start_byte(), self.end_byte())
    }
    fn is_missing(&self) -> bool {
        self.is_missing()
    }

    fn is_extra(&self) -> bool {
        self.is_extra()
    }

    fn error_type(&self) -> Option<TreeErrorType> {
        if !self.is_error() {
            return None;
        }

        if self.is_missing() {
            // Determine what was expected based on parent
            let expected = if let Some(parent) = self.parent() {
                get_expected_tokens_for_parent(parent.kind())
            } else {
                vec!["valid syntax".to_string()]
            };

            Some(TreeErrorType::Missing { expected })
        } else if self.is_extra() {
            // Try to determine what the extra token is based on context
            let found = match self.kind() {
                "." => ".".to_string(),
                ":" => ":".to_string(),
                ";" => ";".to_string(),
                "," => ",".to_string(),
                "(" => "(".to_string(),
                ")" => ")".to_string(),
                "[" => "[".to_string(),
                "]" => "]".to_string(),
                "{" => "{".to_string(),
                "}" => "}".to_string(),
                _ => format!("'{}'", self.kind()),
            };
            Some(TreeErrorType::Extra { found })
        } else if self.kind() == "ERROR" {
            // Check for specific error patterns based on node structure
            // Since we don't have direct access to source text, we'll infer from context
            if let Some(parent) = self.parent() {
                match parent.kind() {
                    "string" => Some(TreeErrorType::Unclosed {
                        delimiter: "\"".to_string(),
                        opened_at: ErrorPosition::new(
                            self.start_position().row + 1,
                            self.start_position().column + 1,
                            self.start_byte(),
                        ),
                    }),
                    "parenthesized_expression" => Some(TreeErrorType::Unclosed {
                        delimiter: "(".to_string(),
                        opened_at: ErrorPosition::new(
                            parent.start_position().row + 1,
                            parent.start_position().column + 1,
                            parent.start_byte(),
                        ),
                    }),
                    "list" => Some(TreeErrorType::Unclosed {
                        delimiter: "[".to_string(),
                        opened_at: ErrorPosition::new(
                            parent.start_position().row + 1,
                            parent.start_position().column + 1,
                            parent.start_byte(),
                        ),
                    }),
                    "map" => Some(TreeErrorType::Unclosed {
                        delimiter: "{".to_string(),
                        opened_at: ErrorPosition::new(
                            parent.start_position().row + 1,
                            parent.start_position().column + 1,
                            parent.start_byte(),
                        ),
                    }),
                    _ => Some(TreeErrorType::Invalid {
                        reason: "syntax error".to_string(),
                    }),
                }
            } else {
                Some(TreeErrorType::Syntax)
            }
        } else {
            Some(TreeErrorType::Syntax)
        }
    }

    fn missing_fields(&self) -> Vec<&str> {
        let mut missing = Vec::new();

        // Check common required fields based on node kind
        match self.kind() {
            "if_statement" => {
                if self.child_by_field_name("condition").is_none() {
                    missing.push("condition");
                }
                if self.child_by_field_name("consequence").is_none() {
                    missing.push("consequence");
                }
            }
            "while_statement" => {
                if self.child_by_field_name("condition").is_none() {
                    missing.push("condition");
                }
                if self.child_by_field_name("body").is_none() {
                    missing.push("body");
                }
            }
            "for_statement" => {
                if self.child_by_field_name("variable").is_none() {
                    missing.push("variable");
                }
                if self.child_by_field_name("iterable").is_none() {
                    missing.push("iterable");
                }
                if self.child_by_field_name("body").is_none() {
                    missing.push("body");
                }
            }
            "assignment_operation" => {
                if self.child_by_field_name("left").is_none() {
                    missing.push("left");
                }
                if self.child_by_field_name("right").is_none() {
                    missing.push("right");
                }
            }
            _ => {}
        }

        missing
    }

    fn parse_context(&self) -> ParseContext {
        // First check the current node
        match self.kind() {
            "statement" => ParseContext::Statement,
            "expression" => ParseContext::Expression,
            "assignment_operation" => ParseContext::Assignment,
            "if_statement" => ParseContext::IfStatement,
            "for_statement" => ParseContext::ForLoop,
            "while_statement" => ParseContext::WhileLoop,
            "function_call" => ParseContext::FunctionCall,
            "function_definition" => ParseContext::FunctionDefinition,
            "list" => ParseContext::List,
            "map" => ParseContext::Map,
            "binding_pattern" => ParseContext::ScatterAssignment,
            "condition" | "parenthesized_expression" => ParseContext::Condition,
            _ => {
                // Check parent context for more specific information
                if let Some(parent) = self.parent() {
                    match parent.kind() {
                        "property" | "property_access" => {
                            ParseContext::Unknown("PropertyAccess".to_string())
                        }
                        "method_call" | "verb_call" => {
                            ParseContext::Unknown("MethodCall".to_string())
                        }
                        "statement" => ParseContext::Statement,
                        "expression" => ParseContext::Expression,
                        "assignment_operation" => ParseContext::Assignment,
                        "if_statement" => ParseContext::IfStatement,
                        "for_statement" => ParseContext::ForLoop,
                        "while_statement" => ParseContext::WhileLoop,
                        "function_call" => ParseContext::FunctionCall,
                        "function_definition" => ParseContext::FunctionDefinition,
                        "list" => ParseContext::List,
                        "map" => ParseContext::Map,
                        "binding_pattern" => ParseContext::ScatterAssignment,
                        "condition" | "parenthesized_expression" => ParseContext::Condition,
                        _ => {
                            // Check grandparent for even more context
                            if let Some(grandparent) = parent.parent() {
                                match grandparent.kind() {
                                    "property" | "property_access" => {
                                        ParseContext::Unknown("PropertyAccess".to_string())
                                    }
                                    "method_call" | "verb_call" => {
                                        ParseContext::Unknown("MethodCall".to_string())
                                    }
                                    _ => ParseContext::Unknown(parent.kind().to_string()),
                                }
                            } else {
                                ParseContext::Unknown(parent.kind().to_string())
                            }
                        }
                    }
                } else {
                    ParseContext::Statement
                }
            }
        }
    }

    fn suggested_fixes(&self) -> Vec<ErrorFix> {
        let mut fixes = Vec::new();
        let pos = ErrorPosition::new(
            self.start_position().row + 1,
            self.start_position().column + 1,
            self.start_byte(),
        );
        let end_pos = ErrorPosition::new(
            self.end_position().row + 1,
            self.end_position().column + 1,
            self.end_byte(),
        );

        let context = self.parse_context();

        match self.error_type() {
            Some(TreeErrorType::Unclosed { ref delimiter, .. }) => {
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
                        end_pos,
                        closer.to_string(),
                        format!("Add missing {}", closer),
                    ));
                }
            }
            Some(TreeErrorType::Missing { ref expected }) => {
                // Context-specific suggestions
                match context {
                    ParseContext::Unknown(ctx) if ctx == "PropertyAccess" => {
                        fixes.push(ErrorFix::insertion(
                            end_pos,
                            "property_name".to_string(),
                            "Add property name after '.'".to_string(),
                        ));
                    }
                    ParseContext::Unknown(ctx) if ctx == "MethodCall" => {
                        fixes.push(ErrorFix::insertion(
                            end_pos,
                            "method_name()".to_string(),
                            "Add method name and parentheses after ':'".to_string(),
                        ));
                    }
                    _ => {
                        // General suggestions
                        for exp in expected {
                            if exp == ";" {
                                fixes.push(ErrorFix::insertion(
                                    end_pos,
                                    ";".to_string(),
                                    "Add missing semicolon".to_string(),
                                ));
                            } else if exp == "endif" {
                                let (line, _col) = self.line_col();
                                fixes.push(ErrorFix::insertion(
                                    ErrorPosition::new(line + 1, 1, self.end_byte()),
                                    "endif".to_string(),
                                    "Add missing endif".to_string(),
                                ));
                            }
                        }
                    }
                }
            }
            Some(TreeErrorType::Extra { ref found }) => {
                // Context-specific fix suggestions
                match (found.as_str(), &context) {
                    (".", ParseContext::Unknown(ctx)) if ctx.contains("expression") => {
                        fixes.push(ErrorFix::replacement(
                            ErrorSpan::new(pos, end_pos),
                            ".property_name".to_string(),
                            "Complete property access with property name".to_string(),
                        ));
                        fixes.push(ErrorFix::replacement(
                            ErrorSpan::new(pos, end_pos),
                            ":method_name()".to_string(),
                            "Change to method call syntax".to_string(),
                        ));
                    }
                    (":", ParseContext::Unknown(ctx)) if ctx.contains("expression") => {
                        fixes.push(ErrorFix::replacement(
                            ErrorSpan::new(pos, end_pos),
                            ":method_name()".to_string(),
                            "Complete method call with method name".to_string(),
                        ));
                        fixes.push(ErrorFix::replacement(
                            ErrorSpan::new(pos, end_pos),
                            ".property_name".to_string(),
                            "Change to property access syntax".to_string(),
                        ));
                    }
                    _ => {
                        fixes.push(ErrorFix::deletion(
                            ErrorSpan::new(pos, end_pos),
                            "Remove unexpected token".to_string(),
                        ));
                    }
                }
            }
            _ => {
                // Additional context-specific fixes
                match context {
                    ParseContext::Unknown(ctx) if ctx == "PropertyAccess" => {
                        if let Some(parent) = self.parent() {
                            // Check if there's a dot before the error
                            let mut has_dot = false;
                            for i in 0..parent.child_count() {
                                if let Some(child) = parent.child(i) {
                                    if child.kind() == "." {
                                        has_dot = true;
                                        break;
                                    }
                                }
                            }
                            if has_dot {
                                fixes.push(ErrorFix::insertion(
                                    end_pos,
                                    "property_name".to_string(),
                                    "Add property name".to_string(),
                                ));
                            }
                        }
                    }
                    ParseContext::Unknown(ctx) if ctx == "MethodCall" => {
                        if let Some(parent) = self.parent() {
                            // Check if there's a colon before the error
                            let mut has_colon = false;
                            for i in 0..parent.child_count() {
                                if let Some(child) = parent.child(i) {
                                    if child.kind() == ":" {
                                        has_colon = true;
                                        break;
                                    }
                                }
                            }
                            if has_colon {
                                fixes.push(ErrorFix::insertion(
                                    end_pos,
                                    "method_name()".to_string(),
                                    "Add method name with parentheses".to_string(),
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        fixes
    }
}

/// Helper to determine expected tokens based on parent node type
fn get_expected_tokens_for_parent(parent_kind: &str) -> Vec<String> {
    match parent_kind {
        "statement" => vec![
            "identifier".to_string(),
            "if".to_string(),
            "while".to_string(),
            "for".to_string(),
            "return".to_string(),
            "try".to_string(),
            ";".to_string(),
        ],
        "if_statement" => vec![
            "endif".to_string(),
            "else".to_string(),
            "elseif".to_string(),
        ],
        "while_statement" => vec!["endwhile".to_string()],
        "for_statement" => vec!["endfor".to_string()],
        "try_statement" => vec![
            "except".to_string(),
            "finally".to_string(),
            "endtry".to_string(),
        ],
        "expression" => vec![
            "identifier".to_string(),
            "number".to_string(),
            "string".to_string(),
            "[".to_string(),
            "{".to_string(),
            "(".to_string(),
        ],
        "property" | "property_access" => vec!["property name".to_string()],
        "method_call" | "verb_call" => vec!["method name".to_string(), "(".to_string()],
        "arguments" => vec!["expression".to_string(), ",".to_string(), ")".to_string()],
        "list" => vec!["expression".to_string(), ",".to_string(), "]".to_string()],
        "map" => vec![
            "key-value pair".to_string(),
            ",".to_string(),
            "}".to_string(),
        ],
        _ => vec!["valid syntax".to_string()],
    }
}
