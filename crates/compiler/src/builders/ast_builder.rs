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

//! AST Builder implementation
//!
//! This provides a working implementation of the generic tree interface concept
//! for building ASTs from different tree representations.

use std::cell::RefCell;

use moor_common::model::{CompileContext, CompileError};
use moor_var::program::DeclType;

use crate::ast::{BinaryOp, Expr, Stmt, StmtNode};
use crate::parsers::parse::{CompileOptions, unquote_str};
use crate::parsers::parse_cst::ParseCst;
#[cfg(feature = "tree-sitter-parser")]
use crate::parsers::tree_sitter::tree_traits::TreeNode;
use crate::var_scope::VarScope;

#[cfg(feature = "tree-sitter-parser")]
/// AST builder that works with any TreeNode implementation
///
/// This demonstrates the concept of a generic tree interface by providing
/// an AST builder that can work with different tree representations.
#[cfg_attr(not(test), allow(dead_code))]
#[cfg(feature = "tree-sitter-parser")]
pub struct ASTBuilder<T: TreeNode> {
    /// Variable scope for identifier resolution
    names: RefCell<VarScope>,
    /// Compilation options
    options: CompileOptions,
    /// Store the tree type for the compiler
    _phantom: std::marker::PhantomData<T>,
}

#[cfg(feature = "tree-sitter-parser")]
impl<T: TreeNode> ASTBuilder<T> {
    /// Create a new AST builder
    pub fn new(options: CompileOptions) -> Self {
        Self {
            names: RefCell::new(VarScope::new()),
            options,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Build an AST from a tree root
    pub fn build_ast(&mut self, root: &T) -> Result<ParseCst, CompileError> {
        let statements = match root.node_kind() {
            "program" => {
                let mut all_statements = Vec::new();

                for child in root.content_children() {
                    match child.node_kind() {
                        "statement_list" | "statements" => {
                            for stmt_node in child.content_children() {
                                if let Some(stmt) = self.build_statement(stmt_node)? {
                                    all_statements.push(stmt);
                                }
                            }
                        }
                        "statement" => {
                            if let Some(stmt) = self.build_statement(child)? {
                                all_statements.push(stmt);
                            }
                        }
                        _ => {
                            // Skip unknown top-level nodes
                        }
                    }
                }

                all_statements
            }
            _ => {
                return Err(self.parse_error(
                    root,
                    &format!("Expected program node, got {}", root.node_kind()),
                ));
            }
        };

        // Extract final state
        let variables = self.names.replace(VarScope::new());
        let names = variables.bind();

        // Annotate line numbers
        let mut statements = statements;
        crate::unparse::annotate_line_numbers(1, &mut statements);

        Ok(ParseCst {
            stmts: statements,
            variables,
            names,
            cst: self.create_dummy_cst(root),
        })
    }

    /// Build an expression from a tree node
    pub fn build_expression(&mut self, node: &T) -> Result<Expr, CompileError> {
        match node.node_kind() {
            // Literals
            "identifier" => {
                let text = node
                    .text()
                    .ok_or_else(|| self.parse_error(node, "Missing identifier text"))?;
                let name = self
                    .names
                    .borrow_mut()
                    .find_or_add_name_global(text.trim(), DeclType::Unknown)
                    .unwrap_or_else(|| {
                        // Create a dummy variable on error - proper error handling would propagate the error
                        // For now, we'll just use the default variable creation
                        let mut new_scope = VarScope::new();
                        new_scope
                            .find_or_add_name_global("error", DeclType::Unknown)
                            .unwrap()
                    });
                Ok(Expr::Id(name))
            }
            "integer_literal" => {
                let text = node
                    .text()
                    .ok_or_else(|| self.parse_error(node, "Missing integer text"))?;
                let value = text
                    .parse::<i64>()
                    .map_err(|e| self.parse_error(node, &format!("Invalid integer: {}", e)))?;
                Ok(Expr::Value(moor_var::v_int(value)))
            }
            "string_literal" => {
                let text = node
                    .text()
                    .ok_or_else(|| self.parse_error(node, "Missing string text"))?;
                let parsed = unquote_str(text)
                    .map_err(|e| self.parse_error(node, &format!("Invalid string: {}", e)))?;
                Ok(Expr::Value(moor_var::v_str(&parsed)))
            }
            "float_literal" => {
                let text = node
                    .text()
                    .ok_or_else(|| self.parse_error(node, "Missing float text"))?;
                let value = text
                    .parse::<f64>()
                    .map_err(|e| self.parse_error(node, &format!("Invalid float: {}", e)))?;
                Ok(Expr::Value(moor_var::v_float(value)))
            }

            // Binary operations
            "binary_add" => self.build_binary_op(node, BinaryOp::Add),
            "binary_sub" => self.build_binary_op(node, BinaryOp::Sub),
            "binary_mul" => self.build_binary_op(node, BinaryOp::Mul),
            "binary_div" => self.build_binary_op(node, BinaryOp::Div),
            "binary_eq" => self.build_binary_op(node, BinaryOp::Eq),
            "binary_neq" => self.build_binary_op(node, BinaryOp::NEq),

            // Assignment
            "assignment" => {
                let left = node
                    .child_by_name("left")
                    .ok_or_else(|| self.parse_error(node, "Assignment missing left side"))?;
                let right = node
                    .child_by_name("right")
                    .ok_or_else(|| self.parse_error(node, "Assignment missing right side"))?;

                let left_expr = self.build_expression(left)?;
                let right_expr = self.build_expression(right)?;

                Ok(Expr::Assign {
                    left: Box::new(left_expr),
                    right: Box::new(right_expr),
                })
            }

            // Return statement
            "return_statement" => {
                let expr = node
                    .child_by_name("expression")
                    .map(|expr_node| self.build_expression(expr_node))
                    .transpose()?;

                Ok(Expr::Return(expr.map(Box::new)))
            }

            // Wrapper nodes - extract single child
            "expression" | "atom" => {
                let children: Vec<_> = node.content_children().collect();
                if children.len() == 1 {
                    self.build_expression(children[0])
                } else {
                    Err(self.parse_error(
                        node,
                        &format!(
                            "Unexpected expression structure with {} children",
                            children.len()
                        ),
                    ))
                }
            }

            // Unknown node types - try to infer from text
            _ => {
                if let Some(text) = node.text() {
                    if text.chars().all(|c| c.is_ascii_digit()) {
                        // Looks like an integer
                        let value = text.parse::<i64>().map_err(|e| {
                            self.parse_error(node, &format!("Invalid integer: {}", e))
                        })?;
                        return Ok(Expr::Value(moor_var::v_int(value)));
                    }

                    if text.starts_with('"') && text.ends_with('"') {
                        // Looks like a string
                        let parsed = unquote_str(text).map_err(|e| {
                            self.parse_error(node, &format!("Invalid string: {}", e))
                        })?;
                        return Ok(Expr::Value(moor_var::v_str(&parsed)));
                    }

                    // Treat as identifier
                    let name = self
                        .names
                        .borrow_mut()
                        .find_or_add_name_global(text, DeclType::Unknown)
                        .unwrap_or_else(|| {
                            // Create a dummy variable on error - proper error handling would propagate the error
                            let mut new_scope = VarScope::new();
                            new_scope
                                .find_or_add_name_global("error", DeclType::Unknown)
                                .unwrap()
                        });
                    return Ok(Expr::Id(name));
                }

                // Try single child
                let children: Vec<_> = node.content_children().collect();
                if children.len() == 1 {
                    self.build_expression(children[0])
                } else {
                    Err(self.parse_error(node, &format!("Unknown node type: {}", node.node_kind())))
                }
            }
        }
    }

    /// Build a binary operation
    fn build_binary_op(&mut self, node: &T, op: BinaryOp) -> Result<Expr, CompileError> {
        let left = node
            .child_by_name("left")
            .ok_or_else(|| self.parse_error(node, "Binary operation missing left operand"))?;
        let right = node
            .child_by_name("right")
            .ok_or_else(|| self.parse_error(node, "Binary operation missing right operand"))?;

        let left_expr = self.build_expression(left)?;
        let right_expr = self.build_expression(right)?;

        Ok(Expr::Binary(op, Box::new(left_expr), Box::new(right_expr)))
    }

    /// Build a statement from a tree node
    fn build_statement(&mut self, node: &T) -> Result<Option<Stmt>, CompileError> {
        let line_col = node.line_col();

        let stmt_node = match node.node_kind() {
            "statement" => {
                // Statement wrapper - extract the actual statement
                let children: Vec<_> = node.content_children().collect();
                if children.is_empty() {
                    return Ok(None);
                }
                self.build_statement_node(children[0])?
            }
            _ => self.build_statement_node(node)?,
        };

        Ok(Some(Stmt::new(stmt_node, line_col)))
    }

    /// Build a statement node from a tree node
    fn build_statement_node(&mut self, node: &T) -> Result<StmtNode, CompileError> {
        match node.node_kind() {
            "expression_statement" => {
                let expr = if let Some(expr_node) = node.child_by_name("expression") {
                    self.build_expression(expr_node)?
                } else {
                    // Find first expression child
                    let children: Vec<_> = node.content_children().collect();
                    if children.is_empty() {
                        return Err(
                            self.parse_error(node, "Expression statement has no expression")
                        );
                    }
                    self.build_expression(children[0])?
                };
                Ok(StmtNode::Expr(expr))
            }
            "return_statement" => {
                let expr = self.build_expression(node)?;
                Ok(StmtNode::Expr(expr))
            }
            "break_statement" => Ok(StmtNode::Break { exit: None }),
            "continue_statement" => Ok(StmtNode::Continue { exit: None }),
            _ => {
                // For other statement types, wrap the expression
                let expr = self.build_expression(node)?;
                Ok(StmtNode::Expr(expr))
            }
        }
    }

    /// Create a parse error with context from a tree node
    fn parse_error(&self, node: &T, message: &str) -> CompileError {
        let (line, col) = node.line_col();
        CompileError::ParseError {
            error_position: CompileContext::new((line, col)),
            end_line_col: None,
            context: "Simple generic tree parsing".to_string(),
            message: message.to_string(),
        }
    }

    /// Create a dummy CST for compatibility
    fn create_dummy_cst(&self, root: &T) -> crate::cst::CSTNode {
        use crate::cst::{CSTNode, CSTNodeKind, CSTSpan};
        use crate::parsers::parse::moo::Rule;

        let span = root.span();
        CSTNode {
            rule: Rule::program,
            span: CSTSpan {
                start: span.0,
                end: span.1,
                line_col: root.line_col(),
            },
            kind: CSTNodeKind::Terminal {
                text: format!("Generated from {}", root.node_kind()),
            },
        }
    }
}

/// Example usage function to demonstrate the generic interface
#[cfg(feature = "tree-sitter-parser")]
pub fn parse_with_ast_builder<T: TreeNode>(
    root: &T,
    options: CompileOptions,
) -> Result<ParseCst, CompileError> {
    let mut builder = ASTBuilder::new(options);
    builder.build_ast(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "tree-sitter-parser")]
    use crate::parsers::tree_sitter::tree_traits::TreeNode;

    // Mock tree node for testing
    #[derive(Debug)]
    struct MockNode {
        kind: String,
        text: Option<String>,
        children: Vec<MockNode>,
        span: (usize, usize),
        line_col: (usize, usize),
    }

    impl MockNode {
        fn new(kind: &str) -> Self {
            Self {
                kind: kind.to_string(),
                text: None,
                children: Vec::new(),
                span: (0, 10),
                line_col: (1, 1),
            }
        }

        fn with_text(mut self, text: &str) -> Self {
            self.text = Some(text.to_string());
            self
        }
    }

    #[cfg(feature = "tree-sitter-parser")]
    impl TreeNode for MockNode {
        fn node_kind(&self) -> &str {
            &self.kind
        }
        fn text(&self) -> Option<&str> {
            self.text.as_deref()
        }
        fn children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
            Box::new(self.children.iter())
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
            false
        }
    }

    #[test]
    #[cfg(feature = "tree-sitter-parser")]
    fn test_ast_builder() {
        let options = CompileOptions::default();
        let mut builder = super::ASTBuilder::<MockNode>::new(options);

        let node = MockNode::new("integer_literal").with_text("42");
        let result = builder.build_expression(&node);

        assert!(result.is_ok());
        if let Ok(Expr::Value(val)) = result {
            // Check if it's an integer value - use variant type comparison
            assert!(matches!(val.variant(), moor_var::Variant::Int(_)));
        }
    }

    #[test]
    #[cfg(feature = "tree-sitter-parser")]
    fn test_parse_with_generic_builder() {
        let program = MockNode::new("program");
        let options = CompileOptions::default();

        let result = super::parse_with_generic_builder(&program, options);
        assert!(result.is_ok());
    }
}
