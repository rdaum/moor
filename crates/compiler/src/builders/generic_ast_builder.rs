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

//! Generic AST builder implementation
//! 
//! This module provides a concrete implementation of the ASTBuilder trait that works
//! with any tree representation implementing the TreeNode trait.

use std::cell::RefCell;

use moor_common::model::{CompileError, CompileContext};
use moor_var::program::DeclType;

use crate::ast::{Arg, BinaryOp, Expr, Stmt, StmtNode, UnaryOp};
use crate::parsers::parse::{CompileOptions, unquote_str};
use crate::parsers::parse_cst::ParseCst;
use crate::parsers::tree_sitter::tree_traits::{TreeNode, ASTBuilder, ASTBuilderContext, NodeHandlerRegistry};
use crate::var_scope::VarScope;

/// Generic AST builder that works with any TreeNode implementation
/// 
/// This builder uses a handler-based approach where specific node types
/// are converted using registered handler functions. It maintains variable
/// scope and provides error handling consistent with the existing parser.
pub struct GenericASTBuilder<T: TreeNode> {
    /// Registry of node type handlers
    handlers: NodeHandlerRegistry<T>,
    /// Variable scope for identifier resolution
    names: RefCell<VarScope>,
    /// Compilation options
    options: CompileOptions,
}

impl<T: TreeNode> GenericASTBuilder<T> {
    /// Create a new generic AST builder with default handlers
    pub fn new(options: CompileOptions) -> Self {
        let mut builder = Self {
            handlers: NodeHandlerRegistry::default(), // This registers default handlers
            names: RefCell::new(VarScope::new()),
            options,
        };
        
        // Register additional handlers specific to our language
        builder.register_language_specific_handlers();
        builder
    }
    
    /// Register handlers specific to the MOO language
    fn register_language_specific_handlers(&mut self) {
        // Object literal handler
        self.handlers.register("object_literal", |node, builder| {
            let text = node.text()
                .ok_or_else(|| builder.parse_error(node, "Missing object literal text"))?;
            let ostr = &text[1..]; // Remove '#' prefix
            let oid = ostr.parse::<i32>()
                .map_err(|e| builder.parse_error(node, &format!("Invalid object ID: {}", e)))?;
            let objid = moor_var::Obj::mk_id(oid);
            Ok(Expr::Value(moor_var::v_obj(objid)))
        });
        
        // Symbol literal handler
        self.handlers.register("symbol_literal", |node, builder| {
            if !builder.get_options().symbol_type {
                return Err(CompileError::DisabledFeature(
                    CompileContext::new(node.line_col()),
                    "Symbols".to_string(),
                ));
            }
            let text = node.text()
                .ok_or_else(|| builder.parse_error(node, "Missing symbol text"))?;
            let s = moor_var::Symbol::mk(&text[1..]); // Remove ' prefix
            Ok(Expr::Value(moor_var::Var::mk_symbol(s)))
        });
        
        // Boolean literal handler
        self.handlers.register("boolean_literal", |node, builder| {
            if !builder.get_options().bool_type {
                return Err(CompileError::DisabledFeature(
                    CompileContext::new(node.line_col()),
                    "Booleans".to_string(),
                ));
            }
            let text = node.text()
                .ok_or_else(|| builder.parse_error(node, "Missing boolean text"))?;
            let b = text.trim() == "true";
            Ok(Expr::Value(moor_var::Var::mk_bool(b)))
        });
        
        // Error literal handler
        self.handlers.register("error_literal", |node, builder| {
            // Extract error code and optional message
            let content_children: Vec<_> = node.content_children().collect();
            
            if content_children.is_empty() {
                return Err(builder.parse_error(node, "Error literal has no content"));
            }
            
            // First child should be the error code
            let errcode_node = content_children[0];
            let errcode_text = errcode_node.text()
                .ok_or_else(|| builder.parse_error(errcode_node, "Error code has no text"))?;
            
            let error_code = moor_var::ErrorCode::parse_str(errcode_text)
                .ok_or_else(|| builder.parse_error(errcode_node, &format!("Unknown error code: {}", errcode_text)))?;
            
            // Check for optional message expression
            let msg_part = if content_children.len() > 1 {
                let msg_children = &content_children[1..];
                // For now, just build the first child - this would need proper expression parsing
                Some(Box::new(builder.build_expression(msg_children[0])?))
            } else {
                None
            };
            
            Ok(Expr::Error(error_code, msg_part))
        });
        
        // Method call handler
        self.handlers.register("method_call", |node, builder| {
            let object = node.child_by_name("object")
                .map(|obj_node| builder.build_expression(obj_node))
                .transpose()?;
            
            let method = node.child_by_name("method")
                .ok_or_else(|| builder.parse_error(node, "Method call missing method"))?;
            let method_expr = builder.build_expression(method)?;
            
            // For now, return empty args - would need proper argument extraction
            let args = Vec::new();
            
            Ok(Expr::Verb {
                location: Box::new(object.unwrap_or(Expr::Value(moor_var::v_obj(moor_var::SYSTEM_OBJECT)))),
                verb: Box::new(method_expr),
                args,
            })
        });
        
        // Function call handler 
        self.handlers.register("function_call", |node, builder| {
            let function = node.child_by_name("function")
                .ok_or_else(|| builder.parse_error(node, "Function call missing function"))?;
            let function_expr = builder.build_expression(function)?;
            
            // For now, return empty args - would need proper argument extraction
            let args = Vec::new();
            
            // Determine if this is a builtin call
            let call_target = if let Expr::Id(_var) = &function_expr {
                // For now, always treat as expression call - proper builtin lookup would need variable name access
                if false {
                    crate::ast::CallTarget::Builtin(moor_var::Symbol::mk("dummy"))
                } else {
                    crate::ast::CallTarget::Expr(Box::new(function_expr))
                }
            } else {
                crate::ast::CallTarget::Expr(Box::new(function_expr))
            };
            
            Ok(Expr::Call {
                function: call_target,
                args,
            })
        });
        
        // Property access handler
        self.handlers.register("property_access", |node, builder| {
            let object = node.child_by_name("object")
                .ok_or_else(|| builder.parse_error(node, "Property access missing object"))?;
            let property = node.child_by_name("property")
                .ok_or_else(|| builder.parse_error(node, "Property access missing property"))?;
            
            let object_expr = builder.build_expression(object)?;
            let property_expr = builder.build_expression(property)?;
            
            Ok(Expr::Prop {
                location: Box::new(object_expr),
                property: Box::new(property_expr),
            })
        });
        
        // Index access handler
        self.handlers.register("index_access", |node, builder| {
            let object = node.child_by_name("object")
                .ok_or_else(|| builder.parse_error(node, "Index access missing object"))?;
            let index = node.child_by_name("index")
                .ok_or_else(|| builder.parse_error(node, "Index access missing index"))?;
            
            let object_expr = builder.build_expression(object)?;
            let index_expr = builder.build_expression(index)?;
            
            Ok(Expr::Index(Box::new(object_expr), Box::new(index_expr)))
        });
        
        // List literal handler
        self.handlers.register("list_literal", |_node, _builder| {
            // For now, return empty list - would need proper element extraction
            let args = Vec::new();
            Ok(Expr::List(args))
        });
        
        // Map literal handler 
        self.handlers.register("map_literal", |_node, _builder| {
            // For now, return empty map - would need proper pair extraction
            let pairs = Vec::new();
            Ok(Expr::Map(pairs))
        });
        
        // Conditional expression handler
        self.handlers.register("conditional_expression", |node, builder| {
            let condition = node.child_by_name("condition")
                .ok_or_else(|| builder.parse_error(node, "Conditional missing condition"))?;
            let consequence = node.child_by_name("consequence")
                .ok_or_else(|| builder.parse_error(node, "Conditional missing consequence"))?;
            let alternative = node.child_by_name("alternative")
                .ok_or_else(|| builder.parse_error(node, "Conditional missing alternative"))?;
            
            let condition_expr = builder.build_expression(condition)?;
            let consequence_expr = builder.build_expression(consequence)?;
            let alternative_expr = builder.build_expression(alternative)?;
            
            Ok(Expr::Cond {
                condition: Box::new(condition_expr),
                consequence: Box::new(consequence_expr),
                alternative: Box::new(alternative_expr),
            })
        });
        
        // Return statement handler
        self.handlers.register("return_statement", |node, builder| {
            let expr = node.child_by_name("expression")
                .map(|expr_node| builder.build_expression(expr_node))
                .transpose()?;
            
            Ok(Expr::Return(expr.map(Box::new)))
        });
        
        // Unary operation handlers
        self.handlers.register("unary_not", |node, builder| {
            Self::build_unary_op(node, builder, UnaryOp::Not)
        });
        
        self.handlers.register("unary_neg", |node, builder| {
            Self::build_unary_op(node, builder, UnaryOp::Neg)
        });
        
        // Additional binary operations not covered by default registry
        self.handlers.register("binary_mod", |node, builder| {
            Self::build_binary_op(node, builder, BinaryOp::Mod)
        });
        
        self.handlers.register("binary_pow", |node, builder| {
            Self::build_binary_op(node, builder, BinaryOp::Exp)
        });
        
        self.handlers.register("binary_gt", |node, builder| {
            Self::build_binary_op(node, builder, BinaryOp::Gt)
        });
        
        self.handlers.register("binary_lt", |node, builder| {
            Self::build_binary_op(node, builder, BinaryOp::Lt)
        });
        
        self.handlers.register("binary_gte", |node, builder| {
            Self::build_binary_op(node, builder, BinaryOp::GtE)
        });
        
        self.handlers.register("binary_lte", |node, builder| {
            Self::build_binary_op(node, builder, BinaryOp::LtE)
        });
        
        self.handlers.register("binary_in", |node, builder| {
            Self::build_binary_op(node, builder, BinaryOp::In)
        });
        
        // Logical operations
        self.handlers.register("logical_and", |node, builder| {
            let left = node.child_by_name("left")
                .ok_or_else(|| builder.parse_error(node, "Logical AND missing left operand"))?;
            let right = node.child_by_name("right")
                .ok_or_else(|| builder.parse_error(node, "Logical AND missing right operand"))?;
            
            let left_expr = builder.build_expression(left)?;
            let right_expr = builder.build_expression(right)?;
            
            Ok(Expr::And(Box::new(left_expr), Box::new(right_expr)))
        });
        
        self.handlers.register("logical_or", |node, builder| {
            let left = node.child_by_name("left")
                .ok_or_else(|| builder.parse_error(node, "Logical OR missing left operand"))?;
            let right = node.child_by_name("right")
                .ok_or_else(|| builder.parse_error(node, "Logical OR missing right operand"))?;
            
            let left_expr = builder.build_expression(left)?;
            let right_expr = builder.build_expression(right)?;
            
            Ok(Expr::Or(Box::new(left_expr), Box::new(right_expr)))
        });
    }
    
    /// Helper method to build binary operations
    fn build_binary_op<U: TreeNode>(
        node: &U, 
        builder: &mut dyn ASTBuilderContext<U>, 
        op: BinaryOp
    ) -> Result<Expr, CompileError> {
        let left = node.child_by_name("left")
            .ok_or_else(|| builder.parse_error(node, "Binary operation missing left operand"))?;
        let right = node.child_by_name("right")
            .ok_or_else(|| builder.parse_error(node, "Binary operation missing right operand"))?;
        
        let left_expr = builder.build_expression(left)?;
        let right_expr = builder.build_expression(right)?;
        
        Ok(Expr::Binary(op, Box::new(left_expr), Box::new(right_expr)))
    }
    
    /// Helper method to build unary operations
    fn build_unary_op<U: TreeNode>(
        node: &U, 
        builder: &mut dyn ASTBuilderContext<U>, 
        op: UnaryOp
    ) -> Result<Expr, CompileError> {
        let operand = node.child_by_name("operand")
            .ok_or_else(|| builder.parse_error(node, "Unary operation missing operand"))?;
        
        let operand_expr = builder.build_expression(operand)?;
        
        Ok(Expr::Unary(op, Box::new(operand_expr)))
    }
    
    /// Extract arguments from a node (for function/method calls)
    fn extract_arguments(&mut self, node: &T) -> Result<Vec<Arg>, CompileError> {
        let mut args = Vec::new();
        
        if let Some(arguments) = node.child_by_name("arguments") {
            for child in arguments.content_children() {
                if child.node_kind() == "expression" {
                    let expr = ASTBuilder::build_expression(self, child)?;
                    args.push(Arg::Normal(expr));
                }
            }
        }
        
        Ok(args)
    }
    
    /// Extract elements from a list literal
    fn extract_list_elements(&mut self, node: &T) -> Result<Vec<Arg>, CompileError> {
        let mut elements = Vec::new();
        
        for child in node.content_children() {
            if child.node_kind() == "expression" {
                let expr = ASTBuilder::build_expression(self, child)?;
                elements.push(Arg::Normal(expr));
            }
        }
        
        Ok(elements)
    }
    
    /// Extract key-value pairs from a map literal
    fn extract_map_pairs(&mut self, node: &T) -> Result<Vec<(Expr, Expr)>, CompileError> {
        let mut pairs = Vec::new();
        
        for child in node.content_children() {
            if child.node_kind() == "map_entry" {
                let key = child.child_by_name("key")
                    .ok_or_else(|| ASTBuilder::parse_error(self, child, "Map entry missing key"))?;
                let value = child.child_by_name("value")
                    .ok_or_else(|| ASTBuilder::parse_error(self, child, "Map entry missing value"))?;
                
                let key_expr = ASTBuilder::build_expression(self, key)?;
                let value_expr = ASTBuilder::build_expression(self, value)?;
                
                pairs.push((key_expr, value_expr));
            }
        }
        
        Ok(pairs)
    }
    
    /// Build expression from a slice of nodes (for complex expressions)
    fn build_expression_from_nodes(&mut self, nodes: &[&T]) -> Result<Expr, CompileError> {
        if nodes.is_empty() {
            return Err(CompileError::ParseError {
                error_position: CompileContext::new((1, 1)),
                end_line_col: None,
                context: "Generic AST building".to_string(),
                message: "Empty expression".to_string(),
            });
        }
        
        if nodes.len() == 1 {
            return ASTBuilder::build_expression(self, nodes[0]);
        }
        
        // For multiple nodes, we need to implement precedence parsing
        // This is a simplified version - a full implementation would need
        // a proper precedence climbing parser
        ASTBuilder::build_expression(self, nodes[0])
    }
    
    /// Default handler for unknown node types
    fn default_expression_handler(&mut self, node: &T) -> Result<Expr, CompileError> {
        // Try to handle as a wrapper node with single child
        let children: Vec<_> = node.content_children().collect();
        
        if children.len() == 1 {
            return ASTBuilder::build_expression(self, children[0]);
        }
        
        // If it has multiple children, treat as a complex expression
        if !children.is_empty() {
            return self.build_expression_from_nodes(&children);
        }
        
        // If it's a terminal node with text, try to infer its type
        if let Some(text) = node.text() {
            if text.chars().all(|c| c.is_ascii_digit()) {
                // Looks like an integer
                let value = text.parse::<i64>()
                    .map_err(|e| ASTBuilder::parse_error(self, node, &format!("Invalid integer: {}", e)))?;
                return Ok(Expr::Value(moor_var::v_int(value)));
            }
            
            if text.starts_with('"') && text.ends_with('"') {
                // Looks like a string
                let parsed = unquote_str(text)
                    .map_err(|e| ASTBuilder::parse_error(self, node, &format!("Invalid string: {}", e)))?;
                return Ok(Expr::Value(moor_var::v_str(&parsed)));
            }
            
            // Treat as identifier
            let name = self.names.borrow_mut()
                .find_or_add_name_global(text, DeclType::Unknown)
                .unwrap_or_else(|| {
                    // Create a dummy variable on error
                    let mut scope = VarScope::new();
                    scope.find_or_add_name_global("error", DeclType::Unknown).unwrap()
                });
            return Ok(Expr::Id(name));
        }
        
        Err(ASTBuilder::parse_error(self, node, &format!("Unknown node type: {}", node.node_kind())))
    }
}

impl<T: TreeNode> ASTBuilder<T> for GenericASTBuilder<T> {
    fn build_ast(&mut self, root: &T) -> Result<ParseCst, CompileError> {
        // Handle different root node types
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
                return Err(ASTBuilder::parse_error(self, root, &format!("Expected program node, got {}", root.node_kind())));
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
            cst: self.create_preserved_cst(root), // Create a dummy CST for compatibility
        })
    }
    
    fn register_handler<F>(&mut self, node_kind: &str, handler: F)
    where F: Fn(&T, &mut dyn ASTBuilderContext<T>) -> Result<Expr, CompileError> + Send + Sync + 'static
    {
        self.handlers.register(node_kind, handler);
    }
    
    fn build_expression(&mut self, node: &T) -> Result<Expr, CompileError> {
        // Check if we have a handler for this node type
        let node_kind = node.node_kind().to_string();
        if self.handlers.handlers.contains_key(&node_kind) {
            // To avoid borrowing issues, we need to use the handler registry directly
            // This is a workaround for the borrow checker
            if let Some(handler) = self.handlers.handlers.get(&node_kind) {
                // Clone the handler reference to avoid borrowing issues
                let handler_fn = handler.clone();
                return handler_fn(node, self);
            }
        }
        // Fall back to default handler
        self.default_expression_handler(node)
    }
    
    fn parse_error(&self, node: &T, message: &str) -> CompileError {
        let (line, col) = node.line_col();
        CompileError::ParseError {
            error_position: CompileContext::new((line, col)),
            end_line_col: None,
            context: "Generic AST building".to_string(),
            message: message.to_string(),
        }
    }
    
    fn get_var_scope(&mut self) -> &mut VarScope {
        self.names.get_mut()
    }
    
    fn get_options(&self) -> &CompileOptions {
        &self.options
    }
}

impl<T: TreeNode> ASTBuilderContext<T> for GenericASTBuilder<T> {
    fn build_expression(&mut self, node: &T) -> Result<Expr, CompileError> {
        ASTBuilder::build_expression(self, node)
    }
    
    fn parse_error(&self, node: &T, message: &str) -> CompileError {
        ASTBuilder::parse_error(self, node, message)
    }
    
    fn get_var_scope(&mut self) -> &mut VarScope {
        ASTBuilder::get_var_scope(self)
    }
    
    fn get_options(&self) -> &CompileOptions {
        ASTBuilder::get_options(self)
    }
}

impl<T: TreeNode> GenericASTBuilder<T> {
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
                    ASTBuilder::build_expression(self, expr_node)?
                } else {
                    // Find first expression child
                    let children: Vec<_> = node.content_children().collect();
                    if children.is_empty() {
                        return Err(ASTBuilder::parse_error(self, node, "Expression statement has no expression"));
                    }
                    ASTBuilder::build_expression(self, children[0])?
                };
                Ok(StmtNode::Expr(expr))
            }
            "return_statement" => {
                let expr = ASTBuilder::build_expression(self, node)?;
                Ok(StmtNode::Expr(expr))
            }
            "break_statement" => {
                Ok(StmtNode::Break { exit: None })
            }
            "continue_statement" => {
                Ok(StmtNode::Continue { exit: None })
            }
            _ => {
                // For other statement types, wrap the expression
                let expr = ASTBuilder::build_expression(self, node)?;
                Ok(StmtNode::Expr(expr))
            }
        }
    }
    
    /// Create a preserved CST for compatibility (simplified)
    fn create_preserved_cst(&self, root: &T) -> crate::cst::CSTNode {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree_traits::TreeNode;
    
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
    }
    
    impl TreeNode for MockNode {
        fn node_kind(&self) -> &str { &self.kind }
        fn text(&self) -> Option<&str> { self.text.as_deref() }
        fn children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
            Box::new(self.children.iter())
        }
        fn child_by_name(&self, _name: &str) -> Option<&Self> { None }
        fn span(&self) -> (usize, usize) { self.span }
        fn line_col(&self) -> (usize, usize) { self.line_col }
        fn is_error(&self) -> bool { false }
    }
    
    #[test]
    fn test_generic_ast_builder_creation() {
        let options = CompileOptions::default();
        let _builder = GenericASTBuilder::<MockNode>::new(options);
        // If this compiles and runs, the basic structure is working
    }
    
    #[test]
    fn test_build_simple_expression() {
        let options = CompileOptions::default();
        let mut builder = GenericASTBuilder::<MockNode>::new(options);
        
        let node = MockNode::new("integer_literal").with_text("42");
        let result = builder.build_expression(&node);
        
        assert!(result.is_ok());
        if let Ok(Expr::Value(val)) = result {
            assert_eq!(val.variant(), moor_var::VarType::TYPE_INT);
        }
    }
}