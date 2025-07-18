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

//! Generic tree interface traits for CST/AST conversion
//! 
//! This module provides a generic interface for working with different tree implementations
//! (Pest CST, Tree-sitter, etc.) while maintaining a unified AST building pipeline.

use std::collections::HashMap;
use moor_common::model::CompileError;
use crate::ast::Expr;
use crate::parsers::parse_cst::ParseCst;

/// Generic interface for any tree node implementation
/// 
/// This trait abstracts over different tree representations (CSTNode, tree_sitter::Node, etc.)
/// providing a uniform interface for AST building logic.
pub trait TreeNode {
    /// Semantic node type (e.g., "identifier", "binary_operation", "if_statement")
    /// 
    /// This should return semantic names rather than parser-specific rule identifiers.
    /// For example: "identifier" instead of Rule::ident, "binary_add" instead of Rule::add
    fn node_kind(&self) -> &str;
    
    /// Raw text content for terminal nodes
    /// 
    /// Returns Some(text) for nodes that contain source text (identifiers, literals, operators)
    /// Returns None for structural nodes that only contain children
    fn text(&self) -> Option<&str>;
    
    /// All child nodes
    /// 
    /// Returns an iterator over all direct children of this node
    fn children(&self) -> Box<dyn Iterator<Item = &Self> + '_>;
    
    /// Child node by semantic field name (e.g., "condition", "left", "right")
    /// 
    /// For structured nodes, this provides access to semantically named children.
    /// Tree-sitter provides this naturally, other parsers may need to simulate it.
    fn child_by_name(&self, name: &str) -> Option<&Self>;
    
    /// Source position information - byte positions
    fn span(&self) -> (usize, usize);
    
    /// Source position information - line and column
    fn line_col(&self) -> (usize, usize);
    
    /// Check if this node represents a parse error
    fn is_error(&self) -> bool;
    
    /// Filter for content vs structural nodes
    /// 
    /// Returns false for whitespace, comments, and other non-semantic content.
    /// Allows parsers to include formatting information while filtering it out during AST building.
    fn is_content(&self) -> bool { 
        true // Default to content unless overridden
    }
    
    /// Get only content children (filters out whitespace, comments, etc.)
    fn content_children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
        Box::new(self.children().filter(|child| child.is_content()))
    }
}

/// Visitor pattern for tree traversal and conversion
/// 
/// Provides a structured way to traverse trees and convert nodes to different representations.
/// This trait is provided for future extensibility but is not currently used.
#[allow(dead_code)]
pub trait NodeVisitor<T: TreeNode> {
    type Output;
    type Error;
    
    /// Visit a single node and convert it
    fn visit_node(&mut self, node: &T) -> Result<Self::Output, Self::Error>;
    
    /// Visit all children of a node
    fn visit_children(&mut self, node: &T) -> Result<Vec<Self::Output>, Self::Error> {
        node.children()
            .map(|child| self.visit_node(child))
            .collect()
    }
    
    /// Visit only content children (excluding whitespace, comments)
    fn visit_content_children(&mut self, node: &T) -> Result<Vec<Self::Output>, Self::Error> {
        node.content_children()
            .map(|child| self.visit_node(child))
            .collect()
    }
}

/// High-level AST builder interface
/// 
/// Provides a generic interface for building ASTs from any tree representation.
/// Implementations handle the conversion from tree nodes to AST structures.
pub trait ASTBuilder<T: TreeNode> {
    /// Build complete AST from tree root
    fn build_ast(&mut self, root: &T) -> Result<ParseCst, CompileError>;
    
    /// Register custom handler for specific node types
    /// 
    /// Allows customization of how specific node types are converted to AST nodes.
    /// The handler function receives the node and a builder context.
    fn register_handler<F>(&mut self, node_kind: &str, handler: F)
    where F: Fn(&T, &mut dyn ASTBuilderContext<T>) -> Result<Expr, CompileError> + Send + Sync + 'static;
    
    /// Build an expression from a tree node
    /// 
    /// This is the core method that converts tree nodes to AST expressions.
    /// It uses registered handlers or falls back to default behavior.
    fn build_expression(&mut self, node: &T) -> Result<Expr, CompileError>;
    
    /// Create a parse error with context from a tree node
    fn parse_error(&self, node: &T, message: &str) -> CompileError {
        let (line, col) = node.line_col();
        CompileError::ParseError {
            error_position: moor_common::model::CompileContext::new((line, col)),
            end_line_col: None,
            context: "Generic tree parsing".to_string(),
            message: message.to_string(),
        }
    }
    
    /// Access to variable scope for identifier resolution
    fn get_var_scope(&mut self) -> &mut crate::var_scope::VarScope;
    
    /// Access to compile options
    fn get_options(&self) -> &crate::parsers::parse::CompileOptions;
}

/// Handler function type for node conversion
/// 
/// A handler takes a tree node and builder context, returning an AST expression.
pub type NodeHandler<T> = std::sync::Arc<dyn Fn(&T, &mut dyn ASTBuilderContext<T>) -> Result<Expr, CompileError> + Send + Sync>;

/// Builder context interface for use in handlers
/// 
/// This trait provides the interface that handlers can use to recursively build expressions
/// and access builder state. It's separate from ASTBuilder to allow dyn compatibility.
pub trait ASTBuilderContext<T: TreeNode> {
    /// Build an expression from a child node
    fn build_expression(&mut self, node: &T) -> Result<Expr, CompileError>;
    
    /// Create a parse error with context
    fn parse_error(&self, node: &T, message: &str) -> CompileError;
    
    /// Access to variable scope for identifier resolution
    fn get_var_scope(&mut self) -> &mut crate::var_scope::VarScope;
    
    /// Access to compile options
    fn get_options(&self) -> &crate::parsers::parse::CompileOptions;
}

/// Registry for node handlers
/// 
/// Manages the mapping from node kinds to their conversion handlers.
pub struct NodeHandlerRegistry<T: TreeNode> {
    pub handlers: HashMap<String, NodeHandler<T>>,
}

impl<T: TreeNode> NodeHandlerRegistry<T> {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
    
    /// Register a handler for a specific node kind
    pub fn register<F>(&mut self, node_kind: &str, handler: F)
    where F: Fn(&T, &mut dyn ASTBuilderContext<T>) -> Result<Expr, CompileError> + Send + Sync + 'static
    {
        self.handlers.insert(node_kind.to_string(), std::sync::Arc::new(handler));
    }
    
    /// Get a handler for a node kind
    pub fn get_handler(&self, node_kind: &str) -> Option<&NodeHandler<T>> {
        self.handlers.get(node_kind)
    }
    
    /// Register all default handlers for common node types
    pub fn register_default_handlers(&mut self) {
        // Register common terminal handlers
        self.register("identifier", |node, builder| {
            let text = node.text()
                .ok_or_else(|| builder.parse_error(node, "Missing identifier text"))?;
            let name = builder.get_var_scope()
                .find_or_add_name_global(text.trim(), moor_var::program::DeclType::Unknown)
                .unwrap_or_else(|| {
                    // For now, create a dummy variable on error - this would need proper error handling
                    moor_var::program::names::Variable {
                        id: 0,
                        nr: moor_var::program::names::VarName::Named(moor_var::Symbol::mk("error")),
                        scope_id: 0,
                    }
                });
            Ok(Expr::Id(name))
        });
        
        self.register("integer_literal", |node, builder| {
            let text = node.text()
                .ok_or_else(|| builder.parse_error(node, "Missing integer text"))?;
            let value = text.parse::<i64>()
                .map_err(|e| builder.parse_error(node, &format!("Invalid integer: {}", e)))?;
            Ok(Expr::Value(moor_var::v_int(value)))
        });
        
        self.register("float_literal", |node, builder| {
            let text = node.text()
                .ok_or_else(|| builder.parse_error(node, "Missing float text"))?;
            let value = text.parse::<f64>()
                .map_err(|e| builder.parse_error(node, &format!("Invalid float: {}", e)))?;
            Ok(Expr::Value(moor_var::v_float(value)))
        });
        
        self.register("string_literal", |node, builder| {
            let text = node.text()
                .ok_or_else(|| builder.parse_error(node, "Missing string text"))?;
            let parsed = crate::parsers::parse::unquote_str(text)
                .map_err(|e| builder.parse_error(node, &format!("Invalid string literal: {}", e)))?;
            Ok(Expr::Value(moor_var::v_str(&parsed)))
        });
        
        // Register binary operation handlers
        self.register("binary_add", |node, builder| {
            Self::build_binary_op(node, builder, crate::ast::BinaryOp::Add)
        });
        
        self.register("binary_sub", |node, builder| {
            Self::build_binary_op(node, builder, crate::ast::BinaryOp::Sub)
        });
        
        self.register("binary_mul", |node, builder| {
            Self::build_binary_op(node, builder, crate::ast::BinaryOp::Mul)
        });
        
        self.register("binary_div", |node, builder| {
            Self::build_binary_op(node, builder, crate::ast::BinaryOp::Div)
        });
        
        self.register("binary_eq", |node, builder| {
            Self::build_binary_op(node, builder, crate::ast::BinaryOp::Eq)
        });
        
        self.register("binary_neq", |node, builder| {
            Self::build_binary_op(node, builder, crate::ast::BinaryOp::NEq)
        });
        
        // Register assignment handler
        self.register("assignment", |node, builder| {
            let left = node.child_by_name("left")
                .ok_or_else(|| builder.parse_error(node, "Missing assignment left side"))?;
            let right = node.child_by_name("right")
                .ok_or_else(|| builder.parse_error(node, "Missing assignment right side"))?;
            
            let left_expr = builder.build_expression(left)?;
            let right_expr = builder.build_expression(right)?;
            
            Ok(Expr::Assign {
                left: Box::new(left_expr),
                right: Box::new(right_expr),
            })
        });
    }
    
    /// Helper method to build binary operations
    fn build_binary_op<U: TreeNode>(
        node: &U, 
        builder: &mut dyn ASTBuilderContext<U>, 
        op: crate::ast::BinaryOp
    ) -> Result<Expr, CompileError> {
        let left = node.child_by_name("left")
            .ok_or_else(|| builder.parse_error(node, "Missing binary operation left operand"))?;
        let right = node.child_by_name("right")
            .ok_or_else(|| builder.parse_error(node, "Missing binary operation right operand"))?;
        
        let left_expr = builder.build_expression(left)?;
        let right_expr = builder.build_expression(right)?;
        
        Ok(Expr::Binary(op, Box::new(left_expr), Box::new(right_expr)))
    }
}

impl<T: TreeNode> Default for NodeHandlerRegistry<T> {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register_default_handlers();
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
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
                span: (0, 0),
                line_col: (1, 1),
            }
        }
        
        fn with_text(mut self, text: &str) -> Self {
            self.text = Some(text.to_string());
            self
        }
        
        fn with_child(mut self, child: MockNode) -> Self {
            self.children.push(child);
            self
        }
    }
    
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
            None // Simplified for testing
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
    fn test_node_handler_registry() {
        let mut registry = NodeHandlerRegistry::<MockNode>::new();
        registry.register_default_handlers();
        
        // Test that default handlers are registered
        assert!(registry.get_handler("identifier").is_some());
        assert!(registry.get_handler("integer_literal").is_some());
        assert!(registry.get_handler("binary_add").is_some());
        assert!(registry.get_handler("nonexistent").is_none());
    }
    
    #[test]
    fn test_mock_tree_node() {
        let node = MockNode::new("identifier")
            .with_text("test_var")
            .with_child(MockNode::new("child"));
        
        assert_eq!(node.node_kind(), "identifier");
        assert_eq!(node.text(), Some("test_var"));
        assert_eq!(node.children().count(), 1);
        assert!(!node.is_error());
    }
}