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

//! Example demonstrating how parsers can use the common AST builder
//!
//! This shows how Phase 2 allows different parsers to share common
//! AST building logic while maintaining their own parsing strategies.

use crate::ast::{Expr, BinaryOp, Arg};
use crate::parsers::ast_builder::ASTBuilder;
use crate::parsers::parse::CompileOptions;
use moor_common::model::{CompileContext, CompileError};

/// Example of how PEST parser could use the common builder
pub struct PestASTAdapter {
    builder: ASTBuilder,
}

impl PestASTAdapter {
    pub fn new(options: CompileOptions) -> Self {
        Self {
            builder: ASTBuilder::new(options),
        }
    }
    
    /// Example: Building an expression from PEST parse tree
    pub fn build_literal(&self, kind: &str, text: &str, line_col: (usize, usize)) -> Result<Expr, CompileError> {
        let context = CompileContext::new(line_col);
        self.builder.build_value(kind, text, context)
    }
    
    /// Example: Building a binary expression
    pub fn build_binary_expr(&self, op: &str, lhs: Expr, rhs: Expr) -> Result<Expr, CompileError> {
        match self.builder.parse_binary_op(op) {
            Some(binary_op) => Ok(self.builder.build_binary(binary_op, lhs, rhs)),
            None => panic!("Unknown binary operator: {}", op),
        }
    }
}

/// Example of how CST parser could use the common builder
pub struct CSTASTAdapter {
    builder: ASTBuilder,
}

impl CSTASTAdapter {
    pub fn new(options: CompileOptions) -> Self {
        Self {
            builder: ASTBuilder::new(options),
        }
    }
    
    /// Example: Building from CST node
    pub fn build_from_cst(&self, node_kind: &str, text: &str, line_col: (usize, usize)) -> Result<Expr, CompileError> {
        let context = CompileContext::new(line_col);
        
        match node_kind {
            "integer" | "float" | "string" | "object" | "error" => {
                self.builder.build_value(node_kind, text, context)
            }
            "identifier" => {
                // In a real implementation, we'd need to look up or create the Variable
                // through VarScope. For this example, we just return an error.
                Err(CompileError::UnknownTypeConstant(context, "identifier requires VarScope".to_string()))
            }
            _ => Err(CompileError::UnknownTypeConstant(context, node_kind.to_string())),
        }
    }
}

/// Example of how tree-sitter parser could use the common builder
pub struct TreeSitterASTAdapter {
    builder: ASTBuilder,
}

impl TreeSitterASTAdapter {
    pub fn new(options: CompileOptions) -> Self {
        Self {
            builder: ASTBuilder::new(options),
        }
    }
    
    /// Example: Building from tree-sitter node
    pub fn build_from_node(&self, node_kind: &str, text: &str, _start_pos: usize) -> Result<Expr, CompileError> {
        let context = CompileContext::new((0, 0)); // Would use real line/col from node
        
        match node_kind {
            "INTEGER" => self.builder.build_integer(text, context),
            "FLOAT" => self.builder.build_float(text, context),
            "STRING" => self.builder.build_string(text, context),
            "OBJECT" => self.builder.build_object(text, context),
            "identifier" => {
                // In a real implementation, we'd need to look up or create the Variable
                // through VarScope. For this example, we just return an error.
                Err(CompileError::UnknownTypeConstant(context, "identifier requires VarScope".to_string()))
            }
            _ => Err(CompileError::UnknownTypeConstant(context, node_kind.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;
    use moor_var::v_int;
    
    #[test]
    fn test_pest_adapter() {
        let adapter = PestASTAdapter::new(CompileOptions::default());
        
        // Test building integer literal
        let expr = adapter.build_literal("integer", "42", (1, 1)).unwrap();
        assert_eq!(expr, Expr::Value(v_int(42)));
        
        // Test building binary expression
        let lhs = adapter.build_literal("integer", "1", (1, 1)).unwrap();
        let rhs = adapter.build_literal("integer", "2", (1, 5)).unwrap();
        let binary = adapter.build_binary_expr("+", lhs, rhs).unwrap();
        
        match binary {
            Expr::Binary(BinaryOp::Add, _, _) => {}
            _ => panic!("Expected binary add expression"),
        }
    }
    
    #[test]
    fn test_cst_adapter() {
        let adapter = CSTASTAdapter::new(CompileOptions::default());
        
        // Test building from CST
        let expr = adapter.build_from_cst("integer", "123", (2, 3)).unwrap();
        assert_eq!(expr, Expr::Value(v_int(123)));
        
        // Test that identifier handling requires VarScope
        let id_result = adapter.build_from_cst("identifier", "foo", (2, 7));
        assert!(id_result.is_err());
    }
    
    #[test]
    fn test_tree_sitter_adapter() {
        let adapter = TreeSitterASTAdapter::new(CompileOptions::default());
        
        // Test building from tree-sitter node
        let expr = adapter.build_from_node("INTEGER", "999", 0).unwrap();
        assert_eq!(expr, Expr::Value(v_int(999)));
    }
}