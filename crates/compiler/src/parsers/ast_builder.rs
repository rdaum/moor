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

//! Common AST building utilities - Phase 2 of unified parsing
//!
//! This module extracts common patterns for building AST nodes that are
//! shared across all three parser implementations.

use moor_var::{v_int, v_float, v_str, v_obj, ErrorCode};
use moor_var::Obj;
use moor_common::model::{CompileError, CompileContext};

use crate::ast::{Expr, BinaryOp, UnaryOp, Arg, CallTarget};
use crate::parsers::parse::CompileOptions;

/// Common AST building utilities shared across parsers
pub struct ASTBuilder {
    options: CompileOptions,
}

impl ASTBuilder {
    pub fn new(options: CompileOptions) -> Self {
        Self { options }
    }

    /// Build a value expression from a string representation
    pub fn build_value(&self, kind: &str, text: &str, context: CompileContext) -> Result<Expr, CompileError> {
        match kind {
            "integer" | "INTEGER" => self.build_integer(text, context),
            "float" | "FLOAT" => self.build_float(text, context),
            "string" | "STRING" => self.build_string(text, context),
            "object" | "OBJECT" => self.build_object(text, context),
            "error" | "ERROR" => self.build_error(text, context),
            _ => Err(CompileError::UnknownTypeConstant(context, text.to_string())),
        }
    }

    /// Build an integer literal
    pub fn build_integer(&self, text: &str, context: CompileContext) -> Result<Expr, CompileError> {
        match text.parse::<i64>() {
            Ok(int) => Ok(Expr::Value(v_int(int))),
            Err(e) => Err(CompileError::StringLexError(
                context,
                format!("invalid integer literal '{}': {}", text, e),
            )),
        }
    }

    /// Build a float literal
    pub fn build_float(&self, text: &str, context: CompileContext) -> Result<Expr, CompileError> {
        match text.parse::<f64>() {
            Ok(float) => Ok(Expr::Value(v_float(float))),
            Err(e) => Err(CompileError::StringLexError(
                context,
                format!("invalid float literal '{}': {}", text, e),
            )),
        }
    }

    /// Build a string literal (handles unquoting)
    pub fn build_string(&self, text: &str, context: CompileContext) -> Result<Expr, CompileError> {
        if text.len() < 2 {
            return Err(CompileError::StringLexError(
                context,
                format!("invalid string literal '{}'", text),
            ));
        }
        
        // Remove quotes and unescape
        let unquoted = crate::parsers::parse::unquote_str(text)
            .map_err(|e| CompileError::StringLexError(context, e))?;
        Ok(Expr::Value(v_str(&unquoted)))
    }

    /// Build an object reference
    pub fn build_object(&self, text: &str, context: CompileContext) -> Result<Expr, CompileError> {
        let number_part = text.trim_start_matches('#');
        match number_part.parse::<i64>() {
            Ok(int) => Ok(Expr::Value(v_obj(Obj::mk_id(int as i32)))),
            Err(e) => Err(CompileError::StringLexError(
                context,
                format!("invalid object literal '{}': {}", text, e),
            )),
        }
    }

    /// Build an error code
    pub fn build_error(&self, text: &str, context: CompileContext) -> Result<Expr, CompileError> {
        let error_code = match text.to_uppercase().as_str() {
            "E_TYPE" => ErrorCode::E_TYPE,
            "E_DIV" => ErrorCode::E_DIV,
            "E_PERM" => ErrorCode::E_PERM,
            "E_PROPNF" => ErrorCode::E_PROPNF,
            "E_VERBNF" => ErrorCode::E_VERBNF,
            "E_VARNF" => ErrorCode::E_VARNF,
            "E_INVIND" => ErrorCode::E_INVIND,
            "E_RECMOVE" => ErrorCode::E_RECMOVE,
            "E_MAXREC" => ErrorCode::E_MAXREC,
            "E_RANGE" => ErrorCode::E_RANGE,
            "E_ARGS" => ErrorCode::E_ARGS,
            "E_NACC" => ErrorCode::E_NACC,
            "E_INVARG" => ErrorCode::E_INVARG,
            "E_QUOTA" => ErrorCode::E_QUOTA,
            "E_FLOAT" => ErrorCode::E_FLOAT,
            // For custom errors when enabled
            _ if self.options.custom_errors => {
                // Custom errors would need to be handled by the parser's VarScope
                // This is a limitation of the current design - we can't create Variables directly
                return Err(CompileError::UnknownTypeConstant(context, text.to_string()));
            }
            _ => return Err(CompileError::UnknownTypeConstant(context, text.to_string())),
        };
        Ok(Expr::Error(error_code, None))
    }

    /// Build a binary expression
    pub fn build_binary(&self, op: BinaryOp, lhs: Expr, rhs: Expr) -> Expr {
        Expr::Binary(op, Box::new(lhs), Box::new(rhs))
    }

    /// Build a unary expression
    pub fn build_unary(&self, op: UnaryOp, expr: Expr) -> Expr {
        Expr::Unary(op, Box::new(expr))
    }

    /// Build a list expression
    pub fn build_list(&self, elements: Vec<Arg>) -> Expr {
        Expr::List(elements)
    }

    /// Build a map expression (if enabled)
    pub fn build_map(&self, pairs: Vec<(Expr, Expr)>, context: CompileContext) -> Result<Expr, CompileError> {
        if !self.options.map_type {
            return Err(CompileError::DisabledFeature(context, "Maps".to_string()));
        }
        Ok(Expr::Map(pairs))
    }

    /// Build an identifier expression
    /// Note: This requires a Variable instance which must be created through VarScope
    /// in the actual parser implementation.
    pub fn build_id(&self, var: moor_var::program::names::Variable) -> Expr {
        Expr::Id(var)
    }

    /// Build a property access expression
    pub fn build_prop(&self, location: Expr, property: Expr) -> Expr {
        Expr::Prop {
            location: Box::new(location),
            property: Box::new(property),
        }
    }

    /// Build a verb call expression
    pub fn build_verb(&self, location: Expr, verb: Expr, args: Vec<Arg>) -> Expr {
        Expr::Verb {
            location: Box::new(location),
            verb: Box::new(verb),
            args,
        }
    }

    /// Build an index expression
    pub fn build_index(&self, base: Expr, index: Expr) -> Expr {
        Expr::Index(Box::new(base), Box::new(index))
    }

    /// Build a range expression
    pub fn build_range(&self, base: Expr, from: Expr, to: Expr) -> Expr {
        Expr::Range {
            base: Box::new(base),
            from: Box::new(from),
            to: Box::new(to),
        }
    }

    /// Build an assignment expression
    pub fn build_assign(&self, lhs: Expr, rhs: Expr) -> Result<Expr, CompileError> {
        Ok(Expr::Assign {
            left: Box::new(lhs),
            right: Box::new(rhs),
        })
    }

    /// Build a return expression
    pub fn build_return(&self, expr: Option<Expr>) -> Expr {
        Expr::Return(expr.map(Box::new))
    }

    /// Build a conditional expression
    pub fn build_cond(&self, condition: Expr, consequence: Expr, alternative: Expr) -> Expr {
        Expr::Cond {
            condition: Box::new(condition),
            consequence: Box::new(consequence),
            alternative: Box::new(alternative),
        }
    }

    /// Build a built-in function call
    pub fn build_builtin_call(&self, name: String, args: Vec<Arg>) -> Expr {
        use moor_var::Symbol;
        Expr::Call {
            function: CallTarget::Builtin(Symbol::mk(&name)),
            args,
        }
    }

    /// Convert binary operator string to enum
    pub fn parse_binary_op(&self, op: &str) -> Option<BinaryOp> {
        match op {
            "+" => Some(BinaryOp::Add),
            "-" => Some(BinaryOp::Sub),
            "*" => Some(BinaryOp::Mul),
            "/" => Some(BinaryOp::Div),
            "%" => Some(BinaryOp::Mod),
            "^" => Some(BinaryOp::Exp),
            "==" => Some(BinaryOp::Eq),
            "!=" => Some(BinaryOp::NEq),
            "<" => Some(BinaryOp::Lt),
            "<=" => Some(BinaryOp::LtE),
            ">" => Some(BinaryOp::Gt),
            ">=" => Some(BinaryOp::GtE),
            "in" => Some(BinaryOp::In),
            // Note: And/Or are separate expression types in the AST, not BinaryOp variants
            _ => None,
        }
    }

    /// Convert unary operator string to enum
    pub fn parse_unary_op(&self, op: &str) -> Option<UnaryOp> {
        match op {
            "-" => Some(UnaryOp::Neg),
            "!" => Some(UnaryOp::Not),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::v_int;

    #[test]
    fn test_build_integer() {
        let builder = ASTBuilder::new(CompileOptions::default());
        let context = CompileContext::new((1, 1));
        
        let result = builder.build_integer("42", context).unwrap();
        assert_eq!(result, Expr::Value(v_int(42)));
        
        let error = builder.build_integer("not_a_number", CompileContext::new((1, 1)));
        assert!(error.is_err());
    }

    #[test]
    fn test_build_string() {
        let builder = ASTBuilder::new(CompileOptions::default());
        let context = CompileContext::new((1, 1));
        
        let result = builder.build_string("\"hello world\"", context).unwrap();
        assert_eq!(result, Expr::Value(v_str("hello world")));
    }

    #[test]
    fn test_parse_operators() {
        let builder = ASTBuilder::new(CompileOptions::default());
        
        assert_eq!(builder.parse_binary_op("+"), Some(BinaryOp::Add));
        assert_eq!(builder.parse_binary_op("=="), Some(BinaryOp::Eq));
        assert_eq!(builder.parse_binary_op("invalid"), None);
        
        assert_eq!(builder.parse_unary_op("-"), Some(UnaryOp::Neg));
        assert_eq!(builder.parse_unary_op("!"), Some(UnaryOp::Not));
        assert_eq!(builder.parse_unary_op("invalid"), None);
    }

    #[test]
    fn test_build_expressions() {
        let builder = ASTBuilder::new(CompileOptions::default());
        
        // Test binary expression
        let lhs = builder.build_integer("1", CompileContext::new((1, 1))).unwrap();
        let rhs = builder.build_integer("2", CompileContext::new((1, 1))).unwrap();
        let binary = builder.build_binary(BinaryOp::Add, lhs, rhs);
        
        match binary {
            Expr::Binary(BinaryOp::Add, _, _) => {}
            _ => panic!("Expected binary add expression"),
        }
        
        // Test list
        let list = builder.build_list(vec![]);
        assert_eq!(list, Expr::List(vec![]));
    }
}