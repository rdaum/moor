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

/// CST-based parser that preserves comments and whitespace while converting to AST.
/// This is the next-generation parser for comment preservation.
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use base64::{Engine, engine::general_purpose};
use moor_var::{ErrorCode, SYSTEM_OBJECT, Var, VarType};
use moor_var::{Symbol, v_none};
use pest::Parser as PestParser;

use moor_common::builtins::BUILTINS;
use moor_var::Obj;
use moor_var::{v_binary, v_float, v_int, v_obj, v_str};

use super::parse::moo::{MooParser, Rule};
use super::parse::{CompileOptions, Parse, unquote_str}; // Reuse from original parser
use crate::ast::{
    Arg, BinaryOp, CallTarget, CatchCodes, CondArm, ElseArm, ExceptArm, Expr, ScatterItem,
    ScatterKind, Stmt, StmtNode, UnaryOp,
};
use crate::cst::{CSTExpressionParserBuilder, CSTNode, PestToCSTConverter};
use crate::errors::enhanced_errors::{
    DefaultErrorReporter, EnhancedError, EnhancedErrorReporter, ErrorPosition, ErrorSpan,
    ParseContext, infer_parse_context,
};
use crate::unparse::annotate_line_numbers;
use crate::var_scope::VarScope;
use moor_common::model::CompileError::{DuplicateVariable, UnknownTypeConstant};
use moor_common::model::{CompileContext, CompileError};
use moor_var::program::DeclType;
use moor_var::program::names::Names;

/// The emitted parse tree from the CST-based parse phase of the compiler.
#[derive(Debug)]
pub struct ParseCst {
    pub stmts: Vec<Stmt>,
    pub variables: VarScope,
    pub names: Names,
    pub cst: CSTNode, // Preserve the original CST for comment reconstruction
}

impl From<ParseCst> for Parse {
    /// Convert a CST-based parse result to the standard Parse structure.
    /// This allows CST parsing results to be used with existing code that expects Parse.
    fn from(parse_cst: ParseCst) -> Self {
        Parse {
            stmts: parse_cst.stmts,
            variables: parse_cst.variables,
            names: parse_cst.names,
        }
    }
}

/// CST-based tree transformer that preserves comments and whitespace
pub struct CSTTreeTransformer {
    names: RefCell<VarScope>,
    options: CompileOptions,
    #[allow(dead_code)]
    error_reporter: DefaultErrorReporter,
}

impl CSTTreeTransformer {
    pub fn new(options: CompileOptions) -> Rc<Self> {
        Rc::new(Self {
            names: RefCell::new(VarScope::new()),
            options,
            error_reporter: DefaultErrorReporter,
        })
    }

    fn compile_context(&self, node: &CSTNode) -> CompileContext {
        CompileContext::new(node.line_col())
    }

    /// Create an enhanced error for CST parsing with better error messages
    #[allow(dead_code)]
    fn create_enhanced_parse_error(
        &self,
        source: &str,
        node: &CSTNode,
        message: &str,
        context: ParseContext,
    ) -> CompileError {
        let (line, col) = node.line_col();
        let start_pos = ErrorPosition::new(line, col, node.span.start);
        let end_pos = ErrorPosition::new(line, col, node.span.end);
        let error_span = ErrorSpan::new(start_pos, end_pos);

        let error_text = if let Some(text) = node.text() {
            text.to_string()
        } else {
            "<empty>".to_string()
        };

        let enhanced_error =
            EnhancedError::new(error_span, error_text, context).with_message(message.to_string());

        self.error_reporter
            .create_enhanced_error(source, &enhanced_error)
    }

    /// Convert a CST node representing an atom (terminal expression) to an AST expression
    fn parse_atom(self: Rc<Self>, node: &CSTNode) -> Result<Expr, CompileError> {
        match node.rule {
            Rule::ident => {
                let Some(text) = node.text() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Identifier node has no text".to_string(),
                    });
                };
                let name = {
                    let mut names_guard = self.names.borrow_mut();
                    names_guard
                        .find_or_add_name_global(text.trim(), DeclType::Unknown)
                        .unwrap()
                };
                Ok(Expr::Id(name))
            }
            Rule::type_constant => {
                let Some(type_str) = node.text() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Type constant node has no text".to_string(),
                    });
                };
                let Some(type_id) = VarType::parse(type_str) else {
                    return Err(UnknownTypeConstant(
                        self.compile_context(node),
                        type_str.into(),
                    ));
                };
                Ok(Expr::TypeConstant(type_id))
            }
            Rule::object => {
                let Some(text) = node.text() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Object node has no text".to_string(),
                    });
                };
                let ostr = &text[1..]; // Remove '#' prefix
                let oid = i32::from_str(ostr).unwrap();
                let objid = Obj::mk_id(oid);
                Ok(Expr::Value(v_obj(objid)))
            }
            Rule::integer => {
                let Some(text) = node.text() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Integer node has no text".to_string(),
                    });
                };
                match text.parse::<i64>() {
                    Ok(int) => Ok(Expr::Value(v_int(int))),
                    Err(e) => Err(CompileError::StringLexError(
                        self.compile_context(node),
                        format!("invalid integer literal '{text}': {e}"),
                    )),
                }
            }
            Rule::boolean => {
                if !self.options.bool_type {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(node),
                        "Booleans".to_string(),
                    ));
                }
                let Some(text) = node.text() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Boolean node has no text".to_string(),
                    });
                };
                let b = text.trim() == "true";
                Ok(Expr::Value(Var::mk_bool(b)))
            }
            Rule::symbol => {
                if !self.options.symbol_type {
                    return Err(CompileError::DisabledFeature(
                        self.compile_context(node),
                        "Symbols".to_string(),
                    ));
                }
                let Some(text) = node.text() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Symbol node has no text".to_string(),
                    });
                };
                let s = Symbol::mk(&text[1..]); // Remove ' prefix
                Ok(Expr::Value(Var::mk_symbol(s)))
            }
            Rule::float => {
                // For floats, get the text by reconstructing from CST
                let text = node.to_source();
                if text.is_empty() {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Float node has empty source".to_string(),
                    });
                }
                let float = text.parse::<f64>().map_err(|e| CompileError::ParseError {
                    error_position: self.compile_context(node),
                    end_line_col: None,
                    context: "CST parsing".to_string(),
                    message: format!("Invalid float literal '{text}': {e}"),
                })?;
                Ok(Expr::Value(v_float(float)))
            }
            Rule::string => {
                let Some(text) = node.text() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "String node has no text".to_string(),
                    });
                };
                let parsed = match unquote_str(text) {
                    Ok(str) => str,
                    Err(e) => {
                        return Err(CompileError::StringLexError(
                            self.compile_context(node),
                            format!("invalid string literal '{text}': {e}"),
                        ));
                    }
                };
                Ok(Expr::Value(v_str(&parsed)))
            }
            Rule::literal_binary => {
                let Some(binary_literal) = node.text() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Binary literal node has no text".to_string(),
                    });
                };
                // Remove b" and " from the literal to get just the base64 content
                let base64_content = binary_literal
                    .strip_prefix("b\"")
                    .and_then(|s| s.strip_suffix("\""))
                    .ok_or_else(|| {
                        CompileError::StringLexError(
                            self.compile_context(node),
                            format!(
                                "invalid binary literal '{binary_literal}': missing b\" prefix or \" suffix"
                            ),
                        )
                    })?;

                // Decode the base64 content
                let decoded = match general_purpose::URL_SAFE.decode(base64_content) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        return Err(CompileError::StringLexError(
                            self.compile_context(node),
                            format!("invalid base64 in binary literal '{binary_literal}': {e}"),
                        ));
                    }
                };

                Ok(Expr::Value(v_binary(decoded)))
            }
            // Handle error codes
            Rule::err => {
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Error node has no children".to_string(),
                    });
                };

                // Error nodes can have an error code and optional message expression
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.is_empty() {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Error node has no content children".to_string(),
                    });
                }

                // First child should be the error code
                let errcode_node = content_children[0];
                let Some(errcode_text) = errcode_node.text() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Error code node has no text".to_string(),
                    });
                };

                let error_code = match ErrorCode::parse_str(errcode_text) {
                    Some(ec) => ec,
                    None => {
                        if self.options.custom_errors {
                            // For custom errors, create a new error code
                            // TODO: This needs proper implementation
                            return Err(CompileError::ParseError {
                                error_position: self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Custom error codes not yet implemented: {errcode_text}"
                                ),
                            });
                        } else {
                            return Err(CompileError::ParseError {
                                error_position: self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!("Unknown error code: {errcode_text}"),
                            });
                        }
                    }
                };

                // Check for optional message expression
                let msg_part = if content_children.len() > 1 {
                    // Parse the message expression from remaining children
                    let msg_children: Vec<_> = content_children[1..].to_vec();
                    Some(Box::new(self.clone().parse_expression(&msg_children)?))
                } else {
                    None
                };

                Ok(Expr::Error(error_code, msg_part))
            }
            Rule::lambda => {
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Lambda node has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.len() < 2 {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Lambda missing parameters or body".to_string(),
                    });
                }

                let lambda_params_node = content_children[0];
                let body_part = content_children[1];

                // Parse lambda params by getting the children of lambda_params node
                let params = if lambda_params_node.rule == Rule::lambda_params {
                    self.clone().parse_lambda_params(lambda_params_node)?
                } else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(lambda_params_node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Expected lambda_params node".to_string(),
                    });
                };

                let body = match body_part.rule {
                    Rule::begin_statement => {
                        // Parse begin statement directly
                        let stmt_opt = self.clone().parse_statement(body_part)?;
                        let stmt = stmt_opt.ok_or_else(|| CompileError::ParseError {
                            error_position: self.compile_context(body_part),
                            end_line_col: None,
                            context: "lambda body parsing".to_string(),
                            message: "Expected statement in lambda body".to_string(),
                        })?;
                        Box::new(stmt)
                    }
                    Rule::expr => {
                        // Parse expression and wrap it in a return statement
                        // We need to parse the inner content of the expr node
                        let expr = self.clone().parse_expr_node(body_part)?;
                        let return_stmt = Stmt::new(
                            StmtNode::Expr(Expr::Return(Some(Box::new(expr)))),
                            body_part.line_col(),
                        );
                        Box::new(return_stmt)
                    }
                    _ => {
                        return Err(CompileError::ParseError {
                            error_position: self.compile_context(body_part),
                            end_line_col: None,
                            context: "lambda body parsing".to_string(),
                            message: "Invalid lambda body".to_string(),
                        });
                    }
                };

                Ok(Expr::Lambda {
                    params,
                    body,
                    self_name: None,
                })
            }
            Rule::fn_expr => {
                // fn_expr = { ^"fn" ~ "(" ~ lambda_params ~ ")" ~ statements ~ ^"endfn" }
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Fn expression node has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.len() < 2 {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Fn expression missing parameters or statements".to_string(),
                    });
                }

                let lambda_params_node = content_children[0];
                let statements_part = content_children[1];

                // Parse lambda params
                let params = if lambda_params_node.rule == Rule::lambda_params {
                    self.clone().parse_lambda_params(lambda_params_node)?
                } else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(lambda_params_node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Expected lambda_params node in fn expression".to_string(),
                    });
                };

                // Parse the statements and wrap them in a scope with proper binding tracking
                // Note: we need access to scope management functions, but this is in parse_atom
                // For now, parse statements directly without scope management
                let statements = if statements_part.rule == Rule::statements {
                    let stmt_children = statements_part.children().unwrap_or_default();
                    let content_stmts: Vec<_> =
                        stmt_children.iter().filter(|n| n.is_content()).collect();
                    let mut parsed_statements = vec![];
                    for stmt_node in content_stmts {
                        if let Some(stmt) = self.clone().parse_statement(stmt_node)? {
                            parsed_statements.push(stmt);
                        }
                    }
                    parsed_statements
                } else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(statements_part),
                        end_line_col: None,
                        context: "fn expression parsing".to_string(),
                        message: "Expected statements node in fn expression".to_string(),
                    });
                };

                // Create a scope statement to wrap the function body
                let body = Box::new(Stmt::new(
                    StmtNode::Scope {
                        num_bindings: 0, // TODO: proper scope binding tracking
                        body: statements,
                    },
                    statements_part.line_col(),
                ));

                Ok(Expr::Lambda {
                    params,
                    body,
                    self_name: None,
                })
            }
            _ => Err(CompileError::ParseError {
                error_position: self.compile_context(node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: format!("Unimplemented atom: {:?}", node.rule),
            }),
        }
    }

    /// Parse a single expr CST node that contains the raw Pest expression structure
    fn parse_expr_node(self: Rc<Self>, expr_node: &CSTNode) -> Result<Expr, CompileError> {
        // The expr node contains the raw Pest structure: prefixes, primary, postfixes, infixes
        // We need to apply precedence parsing to this

        let Some(children) = expr_node.children() else {
            // Check if this is a semicolon-only expression node or parentheses
            if let Some(text) = expr_node.text() {
                if text == ";" {
                    // This is just a semicolon, treat it as an empty expression
                    return Ok(Expr::Pass { args: vec![] });
                } else if text == "(" || text == ")" {
                    // These are parentheses - we should skip them in expression parsing
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(expr_node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Parentheses should not be parsed as standalone expressions"
                            .to_string(),
                    });
                }
            }
            return Err(CompileError::ParseError {
                error_position: self.compile_context(expr_node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: "Expression node has no children".to_string(),
            });
        };

        let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

        if content_children.is_empty() {
            return Err(CompileError::ParseError {
                error_position: self.compile_context(expr_node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: "Expression node has no content children".to_string(),
            });
        }

        // Special handling for scatter assignment
        if content_children.len() >= 2 && content_children[0].rule == Rule::scatter_assign {
            // This is a scatter assignment: {a, b, c} = rhs
            let scatter_node = content_children[0];
            let rhs_node = content_children[1];

            let Some(scatter_children) = scatter_node.children() else {
                return Err(CompileError::ParseError {
                    error_position: self.compile_context(scatter_node),
                    end_line_col: None,
                    context: "CST parsing".to_string(),
                    message: "Scatter assignment has no children".to_string(),
                });
            };

            let scatter_content: Vec<_> =
                scatter_children.iter().filter(|n| n.is_content()).collect();
            let mut scatter_items = Vec::new();

            // Parse scatter targets from the scatter_assign node
            for child in scatter_content {
                match child.rule {
                    Rule::scatter_target => {
                        // scatter_target contains an ident
                        if let Some(target_children) = child.children() {
                            let target_content: Vec<_> =
                                target_children.iter().filter(|n| n.is_content()).collect();
                            if !target_content.is_empty() {
                                if let Some(var_name) = target_content[0].text() {
                                    if let Some(var) = self.names.borrow_mut().declare(
                                        var_name,
                                        false,
                                        true,
                                        DeclType::Assign,
                                    ) {
                                        scatter_items.push(ScatterItem {
                                            kind: ScatterKind::Required,
                                            id: var,
                                            expr: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Rule::scatter_optional => {
                        // ?var or ?var=default
                        if let Some(opt_children) = child.children() {
                            let opt_content: Vec<_> =
                                opt_children.iter().filter(|n| n.is_content()).collect();
                            if !opt_content.is_empty() {
                                if let Some(var_name) = opt_content[0].text() {
                                    let var = {
                                        let mut names_guard = self.names.borrow_mut();
                                        names_guard.declare(var_name, false, true, DeclType::Assign)
                                    };
                                    if let Some(var) = var {
                                        // Check for default value
                                        let default_expr = if opt_content.len() > 1 {
                                            Some(self.clone().parse_expression(&opt_content[1..])?)
                                        } else {
                                            None
                                        };

                                        scatter_items.push(ScatterItem {
                                            kind: ScatterKind::Optional,
                                            id: var,
                                            expr: default_expr,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Rule::scatter_rest => {
                        // @var
                        if let Some(rest_children) = child.children() {
                            let rest_content: Vec<_> =
                                rest_children.iter().filter(|n| n.is_content()).collect();
                            if !rest_content.is_empty() {
                                if let Some(var_name) = rest_content[0].text() {
                                    if let Some(var) = self.names.borrow_mut().declare(
                                        var_name,
                                        false,
                                        true,
                                        DeclType::Assign,
                                    ) {
                                        scatter_items.push(ScatterItem {
                                            kind: ScatterKind::Rest,
                                            id: var,
                                            expr: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    _ => {} // Skip whitespace and other non-content nodes
                }
            }

            if scatter_items.is_empty() {
                return Err(CompileError::ParseError {
                    error_position: self.compile_context(scatter_node),
                    end_line_col: None,
                    context: "CST parsing".to_string(),
                    message: "Scatter assignment has no scatter items".to_string(),
                });
            }

            let rhs = self.clone().parse_operand(rhs_node)?;
            return Ok(Expr::Scatter(scatter_items, Box::new(rhs)));
        }

        // Special handling for parenthesized expressions in tree-sitter's flattened structure
        // Look for pattern: expr("("), expr[...], expr(")")
        if content_children.len() >= 3 {
            if let (Some(first_text), Some(last_text)) = (
                content_children[0].text(),
                content_children[content_children.len() - 1].text(),
            ) {
                if first_text == "(" && last_text == ")" {
                    // This is a parenthesized expression, parse the content inside
                    let inner_children = &content_children[1..content_children.len() - 1];
                    return self.parse_expression(inner_children);
                }
            }
        }

        // Always use the full precedence climbing algorithm to handle
        // primary expressions with postfix operators (like x[1])
        self.parse_expression(&content_children)
    }

    /// Parse an atom node (which contains a child that's the actual atom)
    fn parse_atom_node(self: Rc<Self>, atom_node: &CSTNode) -> Result<Expr, CompileError> {
        let Some(children) = atom_node.children() else {
            return Err(CompileError::ParseError {
                error_position: self.compile_context(atom_node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: "Atom node has no children".to_string(),
            });
        };
        let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
        if content_children.is_empty() {
            return Err(CompileError::ParseError {
                error_position: self.compile_context(atom_node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: "Atom node has no content children".to_string(),
            });
        }
        self.parse_atom(content_children[0])
    }
    /// Parse a single operand (number, variable, etc.)
    fn parse_operand(self: Rc<Self>, node: &CSTNode) -> Result<Expr, CompileError> {
        match node.rule {
            Rule::integer
            | Rule::float
            | Rule::string
            | Rule::boolean
            | Rule::symbol
            | Rule::object
            | Rule::literal_binary
            | Rule::type_constant
            | Rule::ident => self.parse_atom(node),
            Rule::list => {
                // Parse list literal: {expr1, expr2, ...} or {}
                let Some(children) = node.children() else {
                    return Ok(Expr::List(vec![])); // Empty list
                };

                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
                if content_children.is_empty() {
                    return Ok(Expr::List(vec![])); // Empty list
                }

                // Should have exactly one exprlist child for non-empty lists
                if content_children.len() != 1 || content_children[0].rule != Rule::exprlist {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: format!(
                            "Expected exactly one exprlist in list, found {} children",
                            content_children.len()
                        ),
                    });
                }

                let exprlist_node = content_children[0];
                let args = self.clone().parse_exprlist(exprlist_node)?;
                Ok(Expr::List(args))
            }
            Rule::atom => self.parse_atom_node(node),
            Rule::expr => self.parse_expr_node(node),
            Rule::builtin_call => {
                // Builtin function calls should be parsed as single-node expressions
                let nodes = &[node];
                self.parse_expression(nodes)
            }
            Rule::sysprop => {
                // System property access should be parsed as single-node expressions
                let nodes = &[node];
                self.parse_expression(nodes)
            }
            Rule::sysprop_call => {
                // System property calls should be parsed as single-node expressions
                let nodes = &[node];
                self.parse_expression(nodes)
            }
            Rule::paren_expr => {
                // Parenthesized expressions: extract the inner expression
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Parenthesized expression has no children".to_string(),
                    });
                };

                // Find the inner expression (skip the parentheses)
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
                if content_children.len() != 1 {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: format!(
                            "Expected 1 inner expression in parentheses, found {}",
                            content_children.len()
                        ),
                    });
                }

                // Parse the inner expression
                self.parse_operand(content_children[0])
            }
            Rule::argument => {
                // Argument node - extract the expression inside
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Argument node has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
                if content_children.is_empty() {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Argument node has no content children".to_string(),
                    });
                }

                // Parse the inner expression
                self.parse_operand(content_children[0])
            }
            Rule::for_in_clause => {
                // For-in clause contains an expression to iterate over
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For-in clause has no children".to_string(),
                    });
                };

                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
                if content_children.len() != 1 {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: format!(
                            "Expected 1 expression in for-in clause, found {}",
                            content_children.len()
                        ),
                    });
                }

                // Parse the expression to iterate over
                self.parse_operand(content_children[0])
            }
            _ => Err(CompileError::ParseError {
                error_position: self.compile_context(node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: format!("Unexpected operand type: {:?}", node.rule),
            }),
        }
    }

    /// Parse an argument list (like function call arguments)
    fn parse_arglist(self: Rc<Self>, arglist_node: &CSTNode) -> Result<Vec<Arg>, CompileError> {
        if arglist_node.rule != Rule::arglist {
            return Err(CompileError::ParseError {
                error_position: self.compile_context(arglist_node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: format!("Expected arglist, found {:?}", arglist_node.rule),
            });
        }

        let Some(children) = arglist_node.children() else {
            // Empty arglist "()"
            return Ok(vec![]);
        };

        let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
        if content_children.is_empty() {
            // Empty arglist "()"
            return Ok(vec![]);
        }

        if content_children.len() != 1 {
            return Err(CompileError::ParseError {
                error_position: self.compile_context(arglist_node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: format!(
                    "Expected 0 or 1 exprlist in arglist, found {}",
                    content_children.len()
                ),
            });
        }

        // Should be an exprlist
        let exprlist_node = content_children[0];
        if exprlist_node.rule != Rule::exprlist {
            return Err(CompileError::ParseError {
                error_position: self.compile_context(exprlist_node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: format!(
                    "Expected exprlist in arglist, found {:?}",
                    exprlist_node.rule
                ),
            });
        }

        self.parse_exprlist(exprlist_node)
    }

    /// Parse an expression list (comma-separated expressions)
    fn parse_exprlist(self: Rc<Self>, exprlist_node: &CSTNode) -> Result<Vec<Arg>, CompileError> {
        let Some(children) = exprlist_node.children() else {
            return Ok(vec![]);
        };

        let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
        let mut args = Vec::new();

        for child in content_children {
            if child.rule == Rule::argument {
                // Parse the argument node
                let arg = self.clone().parse_argument(child)?;
                args.push(arg);
            } else {
                return Err(CompileError::ParseError {
                    error_position: self.compile_context(child),
                    end_line_col: None,
                    context: "CST parsing".to_string(),
                    message: format!("Expected argument in exprlist, found {:?}", child.rule),
                });
            }
        }

        Ok(args)
    }

    /// Parse exception codes (anycode or exprlist)
    fn parse_except_codes(
        self: Rc<Self>,
        codes_node: &CSTNode,
    ) -> Result<CatchCodes, CompileError> {
        match codes_node.rule {
            Rule::anycode => Ok(CatchCodes::Any),
            Rule::exprlist => {
                let args = self.parse_exprlist(codes_node)?;
                Ok(CatchCodes::Codes(args))
            }
            Rule::codes => {
                // codes can contain either anycode or exprlist
                let Some(children) = codes_node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(codes_node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Codes node has no children".to_string(),
                    });
                };

                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
                if content_children.len() != 1 {
                    return Err(CompileError::ParseError {
                        error_position: self.compile_context(codes_node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: format!(
                            "Expected 1 child in codes, found {}",
                            content_children.len()
                        ),
                    });
                }

                // Recursively parse the child (anycode or exprlist)
                self.parse_except_codes(content_children[0])
            }
            _ => Err(CompileError::ParseError {
                error_position: self.compile_context(codes_node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: format!("Unimplemented except codes: {:?}", codes_node.rule),
            }),
        }
    }

    /// Parse a single argument (normal or splice)
    fn parse_argument(self: Rc<Self>, arg_node: &CSTNode) -> Result<Arg, CompileError> {
        let Some(children) = arg_node.children() else {
            return Err(CompileError::ParseError {
                error_position: self.compile_context(arg_node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: "Argument has no children".to_string(),
            });
        };

        // Check if this is a splice argument by looking at the source text
        // Check if this is a system object reference starting with '@'
        let arg_source = arg_node.to_source();
        let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

        if arg_source.starts_with('@') {
            // This is a splice argument: @expr
            // Find the expression node (skip the @ token)
            for child in &content_children {
                if child.text() != Some("@") {
                    let expr = self.parse_operand(child)?;
                    return Ok(Arg::Splice(expr));
                }
            }

            Err(CompileError::ParseError {
                error_position: self.compile_context(arg_node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: "Splice argument missing expression".to_string(),
            })
        } else {
            // Normal argument: expr
            if content_children.len() == 1 {
                let expr = self.parse_operand(content_children[0])?;
                Ok(Arg::Normal(expr))
            } else {
                Err(CompileError::ParseError {
                    error_position: self.compile_context(arg_node),
                    end_line_col: None,
                    context: "CST parsing".to_string(),
                    message: format!(
                        "Normal argument has {} expressions, expected 1",
                        content_children.len()
                    ),
                })
            }
        }
    }

    /// Parse a sequence of CST nodes into an expression using the custom CST expression parser
    fn parse_expression(self: Rc<Self>, nodes: &[&CSTNode]) -> Result<Expr, CompileError> {
        let primary_self = self.clone();
        let infix_self = self.clone();
        let prefix_self = self.clone();
        let postfix_self = self.clone();

        let parser = CSTExpressionParserBuilder::new().build(
            // Primary mapper - handles atoms and complex expressions
            move |node: &CSTNode| -> Result<Expr, CompileError> {
                match node.rule {
                    // Direct atoms
                    Rule::ident
                    | Rule::type_constant
                    | Rule::object
                    | Rule::integer
                    | Rule::boolean
                    | Rule::symbol
                    | Rule::float
                    | Rule::string
                    | Rule::literal_binary
                    | Rule::err
                    | Rule::lambda
                    | Rule::fn_expr => primary_self.clone().parse_atom(node),
                    // Complex expressions that need recursive parsing
                    Rule::atom => {
                        // Atom nodes contain a single child which is the actual atom
                        let Some(children) = node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Atom node has no children".to_string(),
                            });
                        };
                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.is_empty() {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Atom node has no content children".to_string(),
                            });
                        }
                        primary_self.clone().parse_atom(content_children[0])
                    }
                    Rule::expr => {
                        // This is the key: expr nodes contain the raw Pest structure
                        // We need to apply precedence parsing to this structure
                        // For now, delegate back to a simpler approach
                        primary_self.clone().parse_expr_node(node)
                    }
                    Rule::paren_expr => {
                        // Parenthesized expression - parse the inner expression
                        let Some(children) = node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Parenthesized expression node has no children"
                                    .to_string(),
                            });
                        };
                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.is_empty() {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Parenthesized expression has no content children"
                                    .to_string(),
                            });
                        }
                        // The content should be a single expression inside the parens
                        // For tree-sitter's flattened structure, we need to reconstruct the proper parenthesized expression
                        // Structure: expr("("), expr[inner_content], expr(")")
                        if content_children.len() == 3 {
                            // This looks like parentheses with content in the middle
                            if let (Some(left_text), Some(right_text)) =
                                (content_children[0].text(), content_children[2].text())
                            {
                                if left_text == "(" && right_text == ")" {
                                    // Parse the middle expression
                                    return primary_self
                                        .clone()
                                        .parse_expression(&content_children[1..2]);
                                }
                            }
                        }

                        // Fallback: filter out parentheses nodes from the expression
                        let expr_children: Vec<_> = content_children
                            .into_iter()
                            .filter(|n| {
                                if let Some(text) = n.text() {
                                    text != "(" && text != ")"
                                } else {
                                    true
                                }
                            })
                            .collect();

                        if expr_children.is_empty() {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Parenthesized expression has no non-parentheses children"
                                    .to_string(),
                            });
                        }

                        primary_self.clone().parse_expression(&expr_children)
                    }
                    Rule::return_expr => {
                        // Return expression: return [expr]
                        let Some(children) = node.children() else {
                            return Ok(Expr::Return(None)); // Empty return
                        };
                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();

                        // Filter out return_expr children that are just the keyword "return"
                        let expression_children: Vec<_> = content_children
                            .into_iter()
                            .filter(|n| {
                                // Skip return_expr children that are just the keyword
                                if n.rule == Rule::return_expr {
                                    // Check if this is just the keyword "return"
                                    if let Some(text) = n.text() {
                                        return text.trim() != "return";
                                    }
                                }
                                // Skip return_expr nodes but keep other nodes like integer
                                n.rule != Rule::return_expr
                            })
                            .collect();

                        if expression_children.is_empty() {
                            return Ok(Expr::Return(None)); // Empty return
                        }
                        let expr = primary_self
                            .clone()
                            .parse_expression(&expression_children)?;
                        Ok(Expr::Return(Some(Box::new(expr))))
                    }
                    Rule::list => {
                        // List literal: {expr1, expr2, ...} or {}
                        let Some(children) = node.children() else {
                            return Ok(Expr::List(vec![])); // Empty list
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.is_empty() {
                            return Ok(Expr::List(vec![])); // Empty list
                        }

                        // Should have exactly one exprlist child for non-empty lists
                        if content_children.len() != 1 || content_children[0].rule != Rule::exprlist
                        {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected exactly one exprlist in list, found {} children",
                                    content_children.len()
                                ),
                            });
                        }

                        let exprlist_node = content_children[0];
                        let args = primary_self.clone().parse_exprlist(exprlist_node)?;
                        Ok(Expr::List(args))
                    }
                    Rule::assign => {
                        // Assignment expression: lhs = rhs (tree-sitter has 3 children: lhs, "=", rhs)
                        let Some(children) = node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Assignment expression has no children".to_string(),
                            });
                        };
                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();

                        // The new grammar may have a different structure - let's handle both cases
                        if content_children.len() == 3 {
                            // Original structure: lhs, =, rhs
                            // Check if left side is a scatter pattern
                            if content_children[0].rule == Rule::scatter {
                                // This is a scatter assignment: {a, b, c} = rhs
                                let right_expr = primary_self
                                    .clone()
                                    .parse_expression(&content_children[2..3])?;
                                primary_self.clone().parse_scatter_assign(
                                    content_children[0],
                                    right_expr,
                                    false,
                                    false,
                                )
                            } else {
                                let left_expr = primary_self
                                    .clone()
                                    .parse_expression(&content_children[0..1])?;
                                let right_expr = primary_self
                                    .clone()
                                    .parse_expression(&content_children[2..3])?;
                                Ok(Expr::Assign {
                                    left: Box::new(left_expr),
                                    right: Box::new(right_expr),
                                })
                            }
                        } else if content_children.len() >= 3 {
                            // New flattened structure: lhs, =, possibly more tokens for rhs
                            // Find the "=" operator and parse everything after it as the right side
                            let mut equals_pos = None;
                            for (i, child) in content_children.iter().enumerate() {
                                if child.text() == Some("=") {
                                    equals_pos = Some(i);
                                    break;
                                }
                            }

                            if let Some(eq_pos) = equals_pos {
                                if eq_pos + 1 < content_children.len() {
                                    let right_expr = primary_self
                                        .clone()
                                        .parse_expression(&content_children[eq_pos + 1..])?;

                                    // Check if left side is a scatter pattern
                                    if content_children[0].rule == Rule::scatter {
                                        // This is a scatter assignment: {a, b, c} = rhs
                                        primary_self.clone().parse_scatter_assign(
                                            content_children[0],
                                            right_expr,
                                            false,
                                            false,
                                        )
                                    } else {
                                        let left_expr = primary_self
                                            .clone()
                                            .parse_expression(&content_children[0..1])?;
                                        Ok(Expr::Assign {
                                            left: Box::new(left_expr),
                                            right: Box::new(right_expr),
                                        })
                                    }
                                } else {
                                    Err(CompileError::ParseError {
                                        error_position: primary_self.compile_context(node),
                                        end_line_col: None,
                                        context: "CST parsing".to_string(),
                                        message: "Assignment missing right-hand side".to_string(),
                                    })
                                }
                            } else {
                                Err(CompileError::ParseError {
                                    error_position: primary_self.compile_context(node),
                                    end_line_col: None,
                                    context: "CST parsing".to_string(),
                                    message: "Assignment missing = operator".to_string(),
                                })
                            }
                        } else {
                            Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected at least 3 assignment children, found {}",
                                    content_children.len()
                                ),
                            })
                        }
                    }
                    Rule::cond_expr => {
                        // Conditional expression: condition ? true_expr | false_expr
                        let Some(children) = node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Conditional expression has no children".to_string(),
                            });
                        };
                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() != 3 {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected 3 conditional children, found {}",
                                    content_children.len()
                                ),
                            });
                        }
                        let condition = primary_self
                            .clone()
                            .parse_expression(&content_children[0..1])?;
                        let true_expr = primary_self
                            .clone()
                            .parse_expression(&content_children[1..2])?;
                        let false_expr = primary_self
                            .clone()
                            .parse_expression(&content_children[2..3])?;
                        Ok(Expr::Cond {
                            condition: Box::new(condition),
                            consequence: Box::new(true_expr),
                            alternative: Box::new(false_expr),
                        })
                    }
                    Rule::pass_expr => {
                        // Pass expression: pass(exprlist?)
                        let Some(children) = node.children() else {
                            return Ok(Expr::Pass { args: vec![] }); // Empty pass
                        };
                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.is_empty() {
                            return Ok(Expr::Pass { args: vec![] }); // Empty pass
                        }

                        // Parse arguments if present - look for exprlist directly
                        let args = if !content_children.is_empty()
                            && content_children[0].rule == Rule::exprlist
                        {
                            primary_self.clone().parse_exprlist(content_children[0])?
                        } else {
                            vec![]
                        };

                        Ok(Expr::Pass { args })
                    }
                    Rule::try_expr => {
                        // Try-catch expression: `try_expr ! codes => except_expr'
                        let Some(children) = node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Try expression has no children".to_string(),
                            });
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() < 2 {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Try expression missing components".to_string(),
                            });
                        }

                        // Structure: try_expr, codes, [except_expr]
                        let try_expr = primary_self
                            .clone()
                            .parse_expression(&[content_children[0]])?;
                        let codes_node = content_children[1];

                        // Parse codes (anycode or exprlist)
                        let catch_codes = primary_self.clone().parse_except_codes(codes_node)?;

                        // Parse optional except expression
                        let except = if content_children.len() > 2 {
                            Some(Box::new(
                                primary_self
                                    .clone()
                                    .parse_expression(&[content_children[2]])?,
                            ))
                        } else {
                            None
                        };

                        Ok(Expr::TryCatch {
                            trye: Box::new(try_expr),
                            codes: catch_codes,
                            except,
                        })
                    }
                    Rule::builtin_call => {
                        // Builtin function call: func(args)
                        let Some(children) = node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Builtin call has no children".to_string(),
                            });
                        };
                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() < 2 {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Builtin call missing function name or arguments"
                                    .to_string(),
                            });
                        }

                        let Some(func_name) = content_children[0].text() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Builtin function name has no text".to_string(),
                            });
                        };

                        let args = primary_self.clone().parse_arglist(content_children[1])?;

                        // Check if it's a known builtin
                        let symbol = Symbol::mk(func_name);
                        let call_target = if BUILTINS.find_builtin(symbol).is_some() {
                            CallTarget::Builtin(symbol)
                        } else {
                            // Unknown function - could be lambda variable
                            // Create an Id expression to resolve the variable
                            let var_name = primary_self
                                .names
                                .borrow_mut()
                                .find_or_add_name_global(func_name, DeclType::Unknown)
                                .unwrap();
                            CallTarget::Expr(Box::new(Expr::Id(var_name)))
                        };

                        Ok(Expr::Call {
                            function: call_target,
                            args,
                        })
                    }
                    Rule::sysprop => {
                        // System property: $property
                        let text = node.to_source(); // Use to_source() instead of text()
                        // Remove the $ prefix
                        let prop_name = if let Some(stripped) = text.strip_prefix('$') {
                            stripped
                        } else {
                            &text
                        };
                        Ok(Expr::Prop {
                            location: Box::new(Expr::Value(v_obj(SYSTEM_OBJECT))),
                            property: Box::new(Expr::Value(v_str(prop_name))),
                        })
                    }
                    Rule::sysprop_call => {
                        // System property call: $property(args)
                        let Some(children) = node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "System property call has no children".to_string(),
                            });
                        };
                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() < 2 {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "System property call missing property or arguments"
                                    .to_string(),
                            });
                        }

                        let prop_text = content_children[0].to_source();

                        // Remove the $ prefix
                        let prop_name = if let Some(stripped) = prop_text.strip_prefix('$') {
                            stripped
                        } else {
                            &prop_text
                        };

                        let args = primary_self.clone().parse_arglist(content_children[1])?;
                        Ok(Expr::Verb {
                            location: Box::new(Expr::Value(v_obj(SYSTEM_OBJECT))),
                            verb: Box::new(Expr::Value(v_str(prop_name))),
                            args,
                        })
                    }
                    Rule::scatter => {
                        // Scatter pattern: {var1, ?var2, @rest}
                        // Scatter patterns should only appear on the left side of assignments
                        // and are handled specially in the assignment parsing.
                        // If we encounter a scatter pattern here, it's an error.
                        Err(CompileError::ParseError {
                            error_position: primary_self.compile_context(node),
                            end_line_col: None,
                            context: "CST parsing".to_string(),
                            message:
                                "Scatter patterns can only appear on the left side of assignments"
                                    .to_string(),
                        })
                    }
                    Rule::map => {
                        // Map literal: [key -> value, ...]
                        let Some(children) = node.children() else {
                            return Ok(Expr::Map(vec![])); // Empty map
                        };
                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.is_empty() {
                            return Ok(Expr::Map(vec![])); // Empty map
                        }

                        // Get all expressions and chunk into pairs
                        let mut elements = Vec::new();
                        for child in content_children {
                            if child.rule == Rule::expr {
                                let expr = primary_self.clone().parse_operand(child)?;
                                elements.push(expr);
                            }
                        }

                        // Pair up elements: key1, value1, key2, value2, ...
                        let pairs = elements
                            .chunks(2)
                            .map(|pair| {
                                let key = pair[0].clone();
                                let value = pair
                                    .get(1)
                                    .cloned()
                                    .unwrap_or_else(|| Expr::Value(v_none()));
                                (key, value)
                            })
                            .collect();
                        Ok(Expr::Map(pairs))
                    }
                    Rule::range_end => {
                        // Length operator: $
                        Ok(Expr::Length)
                    }
                    Rule::for_in_clause => {
                        // For-in clause contains an expression to iterate over
                        let Some(children) = node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "For-in clause has no children".to_string(),
                            });
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() != 1 {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected 1 expression in for-in clause, found {}",
                                    content_children.len()
                                ),
                            });
                        }

                        // Parse the expression to iterate over
                        primary_self.clone().parse_operand(content_children[0])
                    }
                    Rule::for_iterable => {
                        // For-iterable contains an expression to iterate over
                        let Some(children) = node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "For-iterable has no children".to_string(),
                            });
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() != 1 {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected 1 expression in for-iterable, found {}",
                                    content_children.len()
                                ),
                            });
                        }

                        let child = content_children[0];
                        match child.rule {
                            Rule::for_range_clause => {
                                // Handle range clause - this represents [start..end] syntax
                                let Some(range_children) = child.children() else {
                                    return Err(CompileError::ParseError {
                                        error_position: primary_self.compile_context(child),
                                        end_line_col: None,
                                        context: "CST parsing".to_string(),
                                        message: "For range clause has no children".to_string(),
                                    });
                                };
                                let range_content: Vec<_> =
                                    range_children.iter().filter(|n| n.is_content()).collect();
                                if range_content.len() < 2 {
                                    return Err(CompileError::ParseError {
                                        error_position: primary_self.compile_context(child),
                                        end_line_col: None,
                                        context: "CST parsing".to_string(),
                                        message: "For range clause missing start/end".to_string(),
                                    });
                                }

                                let start =
                                    primary_self.clone().parse_expression(&[range_content[0]])?;
                                let end =
                                    primary_self.clone().parse_expression(&[range_content[1]])?;
                                // For range clauses in for loops, create a list with range
                                // Use a simple integer as base for list ranges
                                let range_arg = Arg::Normal(Expr::Range {
                                    base: Box::new(Expr::Value(v_int(0))), // Use 0 as placeholder for list ranges
                                    from: Box::new(start),
                                    to: Box::new(end),
                                });
                                Ok(Expr::List(vec![range_arg]))
                            }
                            _ => {
                                // Handle other iterables as regular expressions
                                primary_self.clone().parse_operand(child)
                            }
                        }
                    }
                    Rule::exprlist => {
                        // This should not be handled as a primary expression
                        // exprlist is a container for arguments within lists
                        Err(CompileError::ParseError {
                            error_position: primary_self.compile_context(node),
                            end_line_col: None,
                            context: "CST parsing".to_string(),
                            message: "exprlist should not be handled as primary expression"
                                .to_string(),
                        })
                    }
                    Rule::flyweight => {
                        if !primary_self.options.flyweight_type {
                            return Err(CompileError::DisabledFeature(
                                primary_self.compile_context(node),
                                "Flyweight".to_string(),
                            ));
                        }
                        let Some(children) = node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Flyweight has no children".to_string(),
                            });
                        };
                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();

                        if content_children.is_empty() {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Flyweight missing delegate".to_string(),
                            });
                        }

                        // Three components: delegate, optional slots, optional contents
                        let delegate = primary_self.clone().parse_operand(content_children[0])?;
                        let mut slots = vec![];
                        let mut contents = None;

                        // Parse remaining components
                        for child in content_children.iter().skip(1) {
                            match child.rule {
                                Rule::flyweight_slots => {
                                    // Parse slots: [name -> expr, ...]
                                    if let Some(slot_children) = child.children() {
                                        let slot_content: Vec<_> = slot_children
                                            .iter()
                                            .filter(|n| n.is_content())
                                            .collect();
                                        let mut i = 0;
                                        while i + 1 < slot_content.len() {
                                            let slot_name_node = slot_content[i];
                                            let slot_expr_node = slot_content[i + 1];

                                            if let Some(slot_name_text) = slot_name_node.text() {
                                                let slot_name = Symbol::mk(slot_name_text);
                                                if slot_name == Symbol::mk("delegate")
                                                    || slot_name == Symbol::mk("slots")
                                                {
                                                    return Err(CompileError::BadSlotName(
                                                        primary_self
                                                            .compile_context(slot_name_node),
                                                        slot_name.to_string(),
                                                    ));
                                                }
                                                let slot_expr = primary_self
                                                    .clone()
                                                    .parse_operand(slot_expr_node)?;
                                                slots.push((slot_name, slot_expr));
                                            }
                                            i += 2;
                                        }
                                    }
                                }
                                _ => {
                                    // Contents expression
                                    contents =
                                        Some(Box::new(primary_self.clone().parse_operand(child)?));
                                }
                            }
                        }

                        Ok(Expr::Flyweight(Box::new(delegate), slots, contents))
                    }
                    Rule::range_comprehension => {
                        if !primary_self.options.list_comprehensions {
                            return Err(CompileError::DisabledFeature(
                                primary_self.compile_context(node),
                                "ListComprehension".to_string(),
                            ));
                        }
                        let Some(children) = node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Range comprehension has no children".to_string(),
                            });
                        };
                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();

                        if content_children.len() < 3 {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Range comprehension missing components".to_string(),
                            });
                        }

                        // Parse comprehension variable first so producer expression can reference it
                        let Some(varname) = content_children[1].text() else {
                            return Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(content_children[1]),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Comprehension variable has no text".to_string(),
                            });
                        };

                        // Parse the range/list clause
                        let range_clause = content_children[2];
                        match range_clause.rule {
                            Rule::for_range_clause => {
                                // Parse range: [start..end]
                                let Some(range_children) = range_clause.children() else {
                                    return Err(CompileError::ParseError {
                                        error_position: primary_self.compile_context(range_clause),
                                        end_line_col: None,
                                        context: "CST parsing".to_string(),
                                        message: "Range clause has no children".to_string(),
                                    });
                                };
                                let range_content: Vec<_> =
                                    range_children.iter().filter(|n| n.is_content()).collect();
                                if range_content.len() != 2 {
                                    return Err(CompileError::ParseError {
                                        error_position: primary_self.compile_context(range_clause),
                                        end_line_col: None,
                                        context: "CST parsing".to_string(),
                                        message: format!(
                                            "Expected 2 range expressions, found {}",
                                            range_content.len()
                                        ),
                                    });
                                }

                                // Declare variable in current scope
                                let variable = {
                                    let mut names = primary_self.names.borrow_mut();
                                    let Some(name) =
                                        names.declare_name(varname.trim(), DeclType::For)
                                    else {
                                        return Err(DuplicateVariable(
                                            primary_self.compile_context(content_children[1]),
                                            varname.into(),
                                        ));
                                    };
                                    name
                                };

                                // Now parse producer expression after variable is declared
                                let producer_expr =
                                    primary_self.clone().parse_operand(content_children[0])?;
                                // Enter scope for registers only
                                primary_self.enter_scope();
                                let start_expr =
                                    primary_self.clone().parse_operand(range_content[0])?;
                                let end_expr =
                                    primary_self.clone().parse_operand(range_content[1])?;
                                let end_of_range_register =
                                    primary_self.names.borrow_mut().declare_register()?;
                                primary_self.exit_scope();

                                Ok(Expr::ComprehendRange {
                                    variable,
                                    end_of_range_register,
                                    producer_expr: Box::new(producer_expr),
                                    from: Box::new(start_expr),
                                    to: Box::new(end_expr),
                                })
                            }
                            Rule::for_in_clause => {
                                // Parse list/expression: (expr)
                                let Some(in_children) = range_clause.children() else {
                                    return Err(CompileError::ParseError {
                                        error_position: primary_self.compile_context(range_clause),
                                        end_line_col: None,
                                        context: "CST parsing".to_string(),
                                        message: "For-in clause has no children".to_string(),
                                    });
                                };
                                let in_content: Vec<_> =
                                    in_children.iter().filter(|n| n.is_content()).collect();
                                if in_content.len() != 1 {
                                    return Err(CompileError::ParseError {
                                        error_position: primary_self.compile_context(range_clause),
                                        end_line_col: None,
                                        context: "CST parsing".to_string(),
                                        message: format!(
                                            "Expected 1 expression in for-in clause, found {}",
                                            in_content.len()
                                        ),
                                    });
                                }

                                // Declare variable in current scope
                                let variable = {
                                    let mut names = primary_self.names.borrow_mut();
                                    let Some(name) =
                                        names.declare_name(varname.trim(), DeclType::For)
                                    else {
                                        return Err(DuplicateVariable(
                                            primary_self.compile_context(content_children[1]),
                                            varname.into(),
                                        ));
                                    };
                                    name
                                };

                                // Now parse producer expression after variable is declared
                                let producer_expr =
                                    primary_self.clone().parse_operand(content_children[0])?;

                                // Enter scope for registers only
                                primary_self.enter_scope();
                                let list_expr =
                                    primary_self.clone().parse_operand(in_content[0])?;
                                let position_register =
                                    primary_self.names.borrow_mut().declare_register()?;
                                let list_register =
                                    primary_self.names.borrow_mut().declare_register()?;
                                primary_self.exit_scope();

                                Ok(Expr::ComprehendList {
                                    list_register,
                                    variable,
                                    position_register,
                                    producer_expr: Box::new(producer_expr),
                                    list: Box::new(list_expr),
                                })
                            }
                            _ => Err(CompileError::ParseError {
                                error_position: primary_self.compile_context(range_clause),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Unimplemented comprehension clause: {:?}",
                                    range_clause.rule
                                ),
                            }),
                        }
                    }
                    _ => Err(CompileError::ParseError {
                        error_position: primary_self.compile_context(node),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: format!("Unimplemented primary expression: {:?}", node.rule),
                    }),
                }
            },
            // Infix mapper - handles binary operators
            move |left: Expr, op: &CSTNode, right: Expr| -> Result<Expr, CompileError> {
                match op.rule {
                    Rule::add => Ok(Expr::Binary(BinaryOp::Add, Box::new(left), Box::new(right))),
                    Rule::sub => Ok(Expr::Binary(BinaryOp::Sub, Box::new(left), Box::new(right))),
                    Rule::mul => Ok(Expr::Binary(BinaryOp::Mul, Box::new(left), Box::new(right))),
                    Rule::div => Ok(Expr::Binary(BinaryOp::Div, Box::new(left), Box::new(right))),
                    Rule::pow => Ok(Expr::Binary(BinaryOp::Exp, Box::new(left), Box::new(right))),
                    Rule::modulus => {
                        Ok(Expr::Binary(BinaryOp::Mod, Box::new(left), Box::new(right)))
                    }
                    Rule::land => Ok(Expr::And(Box::new(left), Box::new(right))),
                    Rule::lor => Ok(Expr::Or(Box::new(left), Box::new(right))),
                    Rule::eq => Ok(Expr::Binary(BinaryOp::Eq, Box::new(left), Box::new(right))),
                    Rule::neq => Ok(Expr::Binary(BinaryOp::NEq, Box::new(left), Box::new(right))),
                    Rule::lte => Ok(Expr::Binary(BinaryOp::LtE, Box::new(left), Box::new(right))),
                    Rule::gte => Ok(Expr::Binary(BinaryOp::GtE, Box::new(left), Box::new(right))),
                    Rule::lt => Ok(Expr::Binary(BinaryOp::Lt, Box::new(left), Box::new(right))),
                    Rule::gt => Ok(Expr::Binary(BinaryOp::Gt, Box::new(left), Box::new(right))),
                    Rule::in_range => {
                        Ok(Expr::Binary(BinaryOp::In, Box::new(left), Box::new(right)))
                    }
                    _ => Err(CompileError::ParseError {
                        error_position: infix_self.compile_context(op),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: format!("Unimplemented infix operator: {:?}", op.rule),
                    }),
                }
            },
            // Prefix mapper - handles unary prefix operators
            move |op: &CSTNode, rhs: Expr| -> Result<Expr, CompileError> {
                match op.rule {
                    Rule::not => Ok(Expr::Unary(UnaryOp::Not, Box::new(rhs))),
                    Rule::neg => Ok(Expr::Unary(UnaryOp::Neg, Box::new(rhs))),
                    Rule::scatter_assign => {
                        // Scatter assignment: {var1, ?var2, @rest} = expr
                        // This is handled as a prefix operator, but the RHS is in rhs
                        // We need to parse the scatter pattern from op and create a Scatter expression
                        let Some(children) = op.children() else {
                            return Err(CompileError::ParseError {
                                error_position: prefix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Scatter assignment has no children".to_string(),
                            });
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        let mut scatter_items = Vec::new();

                        // Parse scatter items from the pattern
                        for child in content_children {
                            match child.rule {
                                Rule::scatter_target => {
                                    // scatter_target contains an ident terminal
                                    let Some(target_children) = child.children() else {
                                        continue;
                                    };
                                    let target_content: Vec<_> =
                                        target_children.iter().filter(|n| n.is_content()).collect();
                                    if target_content.is_empty() {
                                        continue;
                                    }
                                    let Some(var_name) = target_content[0].text() else {
                                        continue;
                                    };
                                    let Some(var) = prefix_self.names.borrow_mut().declare(
                                        var_name,
                                        false,
                                        true,
                                        DeclType::Assign,
                                    ) else {
                                        return Err(CompileError::ParseError {
                                            error_position: prefix_self.compile_context(op),
                                            end_line_col: None,
                                            context: "CST parsing".to_string(),
                                            message: format!(
                                                "Failed to declare scatter variable: {var_name}"
                                            ),
                                        });
                                    };
                                    eprintln!("DEBUG: Added scatter item for var: {var_name}");
                                    scatter_items.push(ScatterItem {
                                        kind: ScatterKind::Required,
                                        id: var,
                                        expr: None,
                                    });
                                }
                                Rule::scatter_optional => {
                                    // ?var or ?var=default
                                    let Some(opt_children) = child.children() else {
                                        continue;
                                    };
                                    let opt_content: Vec<_> =
                                        opt_children.iter().filter(|n| n.is_content()).collect();
                                    if !opt_content.is_empty() {
                                        let Some(var_name) = opt_content[0].text() else {
                                            continue;
                                        };
                                        let var = {
                                            let mut names_guard = prefix_self.names.borrow_mut();
                                            names_guard.declare(
                                                var_name,
                                                false,
                                                true,
                                                DeclType::Assign,
                                            )
                                        };
                                        let Some(var) = var else {
                                            return Err(CompileError::ParseError {
                                                error_position: prefix_self.compile_context(op),
                                                end_line_col: None,
                                                context: "CST parsing".to_string(),
                                                message: format!(
                                                    "Failed to declare scatter variable: {var_name}"
                                                ),
                                            });
                                        };

                                        // Check for default value
                                        let default_expr = if opt_content.len() > 1 {
                                            Some(
                                                prefix_self
                                                    .clone()
                                                    .parse_expression(&opt_content[1..])?,
                                            )
                                        } else {
                                            None
                                        };

                                        scatter_items.push(ScatterItem {
                                            kind: ScatterKind::Optional,
                                            id: var,
                                            expr: default_expr,
                                        });
                                    }
                                }
                                Rule::scatter_rest => {
                                    // @var
                                    let Some(rest_children) = child.children() else {
                                        continue;
                                    };
                                    let rest_content: Vec<_> =
                                        rest_children.iter().filter(|n| n.is_content()).collect();
                                    if !rest_content.is_empty() {
                                        let Some(var_name) = rest_content[0].text() else {
                                            continue;
                                        };
                                        let Some(var) = prefix_self.names.borrow_mut().declare(
                                            var_name,
                                            false,
                                            true,
                                            DeclType::Assign,
                                        ) else {
                                            return Err(CompileError::ParseError {
                                                error_position: prefix_self.compile_context(op),
                                                end_line_col: None,
                                                context: "CST parsing".to_string(),
                                                message: format!(
                                                    "Failed to declare scatter variable: {var_name}"
                                                ),
                                            });
                                        };
                                        scatter_items.push(ScatterItem {
                                            kind: ScatterKind::Rest,
                                            id: var,
                                            expr: None,
                                        });
                                    }
                                }
                                _ => {} // Skip unknown scatter elements
                            }
                        }

                        Ok(Expr::Scatter(scatter_items, Box::new(rhs)))
                    }
                    _ => Err(CompileError::ParseError {
                        error_position: prefix_self.compile_context(op),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: format!("Unimplemented prefix operator: {:?}", op.rule),
                    }),
                }
            },
            // Postfix mapper - handles postfix operators
            move |lhs: Expr, op: &CSTNode| -> Result<Expr, CompileError> {
                match op.rule {
                    Rule::index_single => {
                        // Single index: expr[index]
                        let Some(children) = op.children() else {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Index operation has no children".to_string(),
                            });
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() != 1 {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected 1 index expression, found {}",
                                    content_children.len()
                                ),
                            });
                        }

                        let index_expr = postfix_self.clone().parse_operand(content_children[0])?;
                        Ok(Expr::Index(Box::new(lhs), Box::new(index_expr)))
                    }
                    Rule::index_range => {
                        // Range index: expr[start..end]
                        let Some(children) = op.children() else {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Range index operation has no children".to_string(),
                            });
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() != 2 {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected 2 range expressions, found {}",
                                    content_children.len()
                                ),
                            });
                        }

                        let start_expr = postfix_self.clone().parse_operand(content_children[0])?;
                        let end_expr = postfix_self.clone().parse_operand(content_children[1])?;
                        Ok(Expr::Range {
                            base: Box::new(lhs),
                            from: Box::new(start_expr),
                            to: Box::new(end_expr),
                        })
                    }
                    Rule::prop => {
                        // Property access: expr.property
                        let Some(children) = op.children() else {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Property access has no children".to_string(),
                            });
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() != 1 {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected 1 property identifier, found {}",
                                    content_children.len()
                                ),
                            });
                        }

                        let prop_node = content_children[0];
                        let Some(prop_name) = prop_node.text() else {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(prop_node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Property identifier has no text".to_string(),
                            });
                        };

                        Ok(Expr::Prop {
                            location: Box::new(lhs),
                            property: Box::new(Expr::Value(v_str(prop_name))),
                        })
                    }
                    Rule::prop_expr => {
                        // Property access with expression: expr.(expr)
                        let Some(children) = op.children() else {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Property expression access has no children".to_string(),
                            });
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() != 1 {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected 1 property expression, found {}",
                                    content_children.len()
                                ),
                            });
                        }

                        let prop_expr = postfix_self.clone().parse_operand(content_children[0])?;
                        Ok(Expr::Prop {
                            location: Box::new(lhs),
                            property: Box::new(prop_expr),
                        })
                    }
                    Rule::verb_call => {
                        // Verb call: expr:verb(args)
                        let Some(children) = op.children() else {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Verb call has no children".to_string(),
                            });
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() != 2 {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected verb name and arglist, found {}",
                                    content_children.len()
                                ),
                            });
                        }

                        let verb_node = content_children[0];
                        let arglist_node = content_children[1];

                        let Some(verb_name) = verb_node.text() else {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(verb_node),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Verb name has no text".to_string(),
                            });
                        };

                        let args = postfix_self.clone().parse_arglist(arglist_node)?;
                        Ok(Expr::Verb {
                            location: Box::new(lhs),
                            verb: Box::new(Expr::Value(v_str(verb_name))),
                            args,
                        })
                    }
                    Rule::verb_expr_call => {
                        // Verb call with expression: expr:(expr)(args)
                        let Some(children) = op.children() else {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Verb expression call has no children".to_string(),
                            });
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() != 2 {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected verb expression and arglist, found {}",
                                    content_children.len()
                                ),
                            });
                        }

                        let verb_expr = postfix_self.clone().parse_operand(content_children[0])?;
                        let args = postfix_self.clone().parse_arglist(content_children[1])?;
                        Ok(Expr::Verb {
                            location: Box::new(lhs),
                            verb: Box::new(verb_expr),
                            args,
                        })
                    }
                    Rule::cond_expr => {
                        // Conditional expression: expr ? true_expr | false_expr
                        let Some(children) = op.children() else {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Conditional expression has no children".to_string(),
                            });
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() != 2 {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected true and false expressions, found {}",
                                    content_children.len()
                                ),
                            });
                        }

                        let true_expr = postfix_self.clone().parse_operand(content_children[0])?;
                        let false_expr = postfix_self.clone().parse_operand(content_children[1])?;
                        Ok(Expr::Cond {
                            condition: Box::new(lhs),
                            consequence: Box::new(true_expr),
                            alternative: Box::new(false_expr),
                        })
                    }
                    Rule::assign => {
                        // Assignment postfix operator: expr = expr
                        let Some(children) = op.children() else {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Assignment has no children".to_string(),
                            });
                        };

                        let content_children: Vec<_> =
                            children.iter().filter(|n| n.is_content()).collect();
                        if content_children.len() != 1 {
                            return Err(CompileError::ParseError {
                                error_position: postfix_self.compile_context(op),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: format!(
                                    "Expected 1 assignment expression, found {}",
                                    content_children.len()
                                ),
                            });
                        }

                        let right_expr = postfix_self.clone().parse_operand(content_children[0])?;
                        Ok(Expr::Assign {
                            left: Box::new(lhs),
                            right: Box::new(right_expr),
                        })
                    }
                    _ => Err(CompileError::ParseError {
                        error_position: postfix_self.compile_context(op),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: format!("Unimplemented postfix operator: {:?}", op.rule),
                    }),
                }
            },
        );

        let node_refs: Vec<CSTNode> = nodes.iter().map(|&n| n.clone()).collect();
        parser.parse(&node_refs)
    }

    /// Parse a sequence of CST nodes representing statements
    fn parse_statements(self: Rc<Self>, nodes: &[&CSTNode]) -> Result<Vec<Stmt>, CompileError> {
        let mut statements = Vec::new();
        let content_nodes: Vec<_> = nodes.iter().filter(|n| n.is_content()).collect();

        for node in content_nodes {
            if let Some(stmt) = self.clone().parse_statement(node)? {
                statements.push(stmt);
            }
        }

        Ok(statements)
    }

    /// Parse a single statement from a CST node
    fn parse_statement(self: Rc<Self>, node: &CSTNode) -> Result<Option<Stmt>, CompileError> {
        let line_col = node.line_col();
        let context = self.compile_context(node);

        match node.rule {
            Rule::statement => {
                // Statement nodes contain a single child which is the actual statement
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Statement node has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
                if content_children.is_empty() {
                    return Ok(None); // Empty statement
                }
                self.parse_statement(content_children[0])
            }
            Rule::expr_statement => {
                let Some(children) = node.children() else {
                    return Ok(None); // Empty expression statement like ";"
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
                if content_children.is_empty() {
                    return Ok(None); // Empty expression statement
                }

                // Check if the child is actually a statement that should be parsed as a statement
                let first_child = content_children[0];
                match first_child.rule {
                    Rule::for_statement
                    | Rule::for_in_statement
                    | Rule::for_range_statement
                    | Rule::while_statement
                    | Rule::labelled_while_statement
                    | Rule::if_statement
                    | Rule::try_except_statement
                    | Rule::try_finally_statement
                    | Rule::break_statement
                    | Rule::continue_statement
                    | Rule::fork_statement
                    | Rule::labelled_fork_statement
                    | Rule::local_assignment
                    | Rule::const_assignment
                    | Rule::global_assignment
                    | Rule::fn_statement
                    | Rule::begin_statement => {
                        // This is actually a statement, not an expression - parse it as a statement
                        return self.parse_statement(first_child);
                    }
                    _ => {
                        // Parse as expression
                    }
                }

                // Parse the expression from the first (and only) child
                let expr = self.clone().parse_expression(&content_children)?;

                // Filter out Pass expressions that represent semicolons
                match expr {
                    Expr::Pass { args } if args.is_empty() => {
                        // This is just a semicolon, skip it
                        Ok(None)
                    }
                    _ => Ok(Some(Stmt::new(StmtNode::Expr(expr), line_col))),
                }
            }
            Rule::empty_return => Ok(Some(Stmt::new(
                StmtNode::Expr(Expr::Return(None)),
                line_col,
            ))),
            Rule::expr => {
                // Handle bare expr nodes that contain statement-like expressions (e.g., return expressions)
                let expr = self.clone().parse_expression(&[node])?;
                Ok(Some(Stmt::new(StmtNode::Expr(expr), line_col)))
            }
            Rule::if_statement => {
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "If statement has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                // Parse: if (condition) statements (elseif)* (else)? endif
                if content_children.len() < 3 {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "If statement missing components".to_string(),
                    });
                }

                let mut idx = 0;
                let mut arms = vec![];
                let mut otherwise = None;

                // Parse main if condition and body
                let condition_node = content_children[idx];
                idx += 1;
                let condition = self.clone().parse_expression(&[condition_node])?;

                let statements_node = content_children[idx];
                idx += 1;
                self.enter_scope();
                let body = self.clone().parse_statements_from_node(statements_node)?;
                let environment_width = self.exit_scope();

                arms.push(CondArm {
                    condition,
                    statements: body,
                    environment_width,
                });

                // Parse elseif and else clauses
                while idx < content_children.len() {
                    let clause = content_children[idx];
                    match clause.rule {
                        Rule::elseif_clause => {
                            let Some(elseif_children) = clause.children() else {
                                return Err(CompileError::ParseError {
                                    error_position: context,
                                    end_line_col: None,
                                    context: "CST parsing".to_string(),
                                    message: "Elseif clause has no children".to_string(),
                                });
                            };
                            let elseif_content: Vec<_> =
                                elseif_children.iter().filter(|n| n.is_content()).collect();

                            if elseif_content.len() < 2 {
                                return Err(CompileError::ParseError {
                                    error_position: context,
                                    end_line_col: None,
                                    context: "CST parsing".to_string(),
                                    message: "Elseif clause missing components".to_string(),
                                });
                            }

                            let condition = self.clone().parse_expression(&[elseif_content[0]])?;
                            self.enter_scope();
                            let body =
                                self.clone().parse_statements_from_node(elseif_content[1])?;
                            let environment_width = self.exit_scope();

                            arms.push(CondArm {
                                condition,
                                statements: body,
                                environment_width,
                            });
                        }
                        Rule::else_clause => {
                            let Some(else_children) = clause.children() else {
                                return Err(CompileError::ParseError {
                                    error_position: context,
                                    end_line_col: None,
                                    context: "CST parsing".to_string(),
                                    message: "Else clause has no children".to_string(),
                                });
                            };
                            let else_content: Vec<_> =
                                else_children.iter().filter(|n| n.is_content()).collect();

                            if else_content.is_empty() {
                                return Err(CompileError::ParseError {
                                    error_position: context,
                                    end_line_col: None,
                                    context: "CST parsing".to_string(),
                                    message: "Else clause missing statements".to_string(),
                                });
                            }

                            self.enter_scope();
                            let statements =
                                self.clone().parse_statements_from_node(else_content[0])?;
                            let environment_width = self.exit_scope();

                            otherwise = Some(ElseArm {
                                statements,
                                environment_width,
                            });
                        }
                        Rule::endif_clause => {
                            // End marker, ignore
                        }
                        _ => {
                            // Skip unrecognized clauses for now
                        }
                    }
                    idx += 1;
                }

                Ok(Some(Stmt::new(
                    StmtNode::Cond { arms, otherwise },
                    line_col,
                )))
            }
            Rule::while_statement => {
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "While statement has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.len() < 2 {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "While statement missing components".to_string(),
                    });
                }

                let condition = self.clone().parse_expression(&content_children[0..1])?;

                self.enter_scope();
                let body = self
                    .clone()
                    .parse_statements_from_node(content_children[1])?;
                let environment_width = self.exit_scope();

                Ok(Some(Stmt::new(
                    StmtNode::While {
                        id: None,
                        condition,
                        body,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::for_statement => {
                // Handle semantic for_statement with fields
                if let Some(fields) = node.fields() {
                    // Get variable field
                    let variable_node =
                        fields
                            .get("variable")
                            .ok_or_else(|| CompileError::ParseError {
                                error_position: context.clone(),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "For statement missing variable field".to_string(),
                            })?;

                    let variable_text =
                        variable_node
                            .text()
                            .ok_or_else(|| CompileError::ParseError {
                                error_position: context.clone(),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "For variable has no text".to_string(),
                            })?;

                    // Get iterable field
                    let iterable_node =
                        fields
                            .get("iterable")
                            .ok_or_else(|| CompileError::ParseError {
                                error_position: context.clone(),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "For statement missing iterable field".to_string(),
                            })?;

                    // Check if iterable is a range (has start/end) or expression
                    if let Some(iterable_fields) = iterable_node.fields() {
                        if iterable_fields.contains_key("start")
                            && iterable_fields.contains_key("end")
                        {
                            // This is a range: for i in [start..end]
                            let start_expr = self
                                .clone()
                                .parse_expression(&[iterable_fields.get("start").unwrap()])?;
                            let end_expr = self
                                .clone()
                                .parse_expression(&[iterable_fields.get("end").unwrap()])?;

                            // Declare loop variable
                            let id = self
                                .names
                                .borrow_mut()
                                .declare_or_use_name(variable_text, DeclType::For);

                            // Parse body
                            let body_node =
                                fields.get("body").ok_or_else(|| CompileError::ParseError {
                                    error_position: context.clone(),
                                    end_line_col: None,
                                    context: "CST parsing".to_string(),
                                    message: "For statement missing body field".to_string(),
                                })?;

                            self.enter_scope();
                            let body = self.clone().parse_statements_from_node(body_node)?;
                            let environment_width = self.exit_scope();

                            return Ok(Some(Stmt::new(
                                StmtNode::ForRange {
                                    id,
                                    from: start_expr,
                                    to: end_expr,
                                    body,
                                    environment_width,
                                },
                                line_col,
                            )));
                        } else if iterable_fields.contains_key("expression") {
                            // This is an expression: for i in (expr)
                            let iter_expr = self
                                .clone()
                                .parse_expression(&[iterable_fields.get("expression").unwrap()])?;

                            // Declare loop variable
                            let value_binding = self
                                .names
                                .borrow_mut()
                                .declare_or_use_name(variable_text, DeclType::For);

                            // Parse body
                            let body_node =
                                fields.get("body").ok_or_else(|| CompileError::ParseError {
                                    error_position: context.clone(),
                                    end_line_col: None,
                                    context: "CST parsing".to_string(),
                                    message: "For statement missing body field".to_string(),
                                })?;

                            self.enter_scope();
                            let body = self.clone().parse_statements_from_node(body_node)?;
                            let environment_width = self.exit_scope();

                            return Ok(Some(Stmt::new(
                                StmtNode::ForList {
                                    value_binding,
                                    key_binding: None,
                                    expr: iter_expr,
                                    body,
                                    environment_width,
                                },
                                line_col,
                            )));
                        }
                    }

                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For statement iterable has unknown structure".to_string(),
                    });
                }

                // Fall back to positional parsing if no fields available
                // This supports the old PEST structure during migration
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For statement has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.len() < 3 {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For statement missing components".to_string(),
                    });
                }

                // Positional parsing: variable, iterable, body
                let variable_node = content_children[0];
                let iterable_node = content_children[1];
                let body_node = content_children[2];

                let variable_text =
                    variable_node
                        .text()
                        .ok_or_else(|| CompileError::ParseError {
                            error_position: context.clone(),
                            end_line_col: None,
                            context: "CST parsing".to_string(),
                            message: "For variable has no text".to_string(),
                        })?;

                // Check iterable type by rule
                match iterable_node.rule {
                    Rule::for_range_clause => {
                        // Handle as range
                        let Some(range_children) = iterable_node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: context,
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "For range clause has no children".to_string(),
                            });
                        };
                        let range_content: Vec<_> =
                            range_children.iter().filter(|n| n.is_content()).collect();

                        if range_content.len() < 2 {
                            return Err(CompileError::ParseError {
                                error_position: context,
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "For range clause missing start/end".to_string(),
                            });
                        }

                        let start_expr = self.clone().parse_expression(&[range_content[0]])?;
                        let end_expr = self.clone().parse_expression(&[range_content[1]])?;

                        let id = self
                            .names
                            .borrow_mut()
                            .declare_or_use_name(variable_text, DeclType::For);

                        self.enter_scope();
                        let body = self.clone().parse_statements_from_node(body_node)?;
                        let environment_width = self.exit_scope();

                        Ok(Some(Stmt::new(
                            StmtNode::ForRange {
                                id,
                                from: start_expr,
                                to: end_expr,
                                body,
                                environment_width,
                            },
                            line_col,
                        )))
                    }
                    Rule::for_in_clause => {
                        // Handle as expression
                        let iter_expr = self.clone().parse_expression(&[iterable_node])?;
                        let value_binding = self
                            .names
                            .borrow_mut()
                            .declare_or_use_name(variable_text, DeclType::For);

                        self.enter_scope();
                        let body = self.clone().parse_statements_from_node(body_node)?;
                        let environment_width = self.exit_scope();

                        Ok(Some(Stmt::new(
                            StmtNode::ForList {
                                value_binding,
                                key_binding: None,
                                expr: iter_expr,
                                body,
                                environment_width,
                            },
                            line_col,
                        )))
                    }
                    Rule::for_iterable => {
                        // Handle for_iterable which contains the actual iterable expression
                        // Check if this contains a range clause
                        let Some(iterable_children) = iterable_node.children() else {
                            return Err(CompileError::ParseError {
                                error_position: context,
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "For iterable has no children".to_string(),
                            });
                        };

                        let iterable_content: Vec<_> = iterable_children
                            .iter()
                            .filter(|n| n.is_content())
                            .collect();
                        if iterable_content.len() != 1 {
                            return Err(CompileError::ParseError {
                                error_position: context,
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "For iterable expected 1 child".to_string(),
                            });
                        }

                        let child = iterable_content[0];
                        match child.rule {
                            Rule::for_range_clause => {
                                // This is a range: for i in [start..end]
                                let Some(range_children) = child.children() else {
                                    return Err(CompileError::ParseError {
                                        error_position: context,
                                        end_line_col: None,
                                        context: "CST parsing".to_string(),
                                        message: "For range clause has no children".to_string(),
                                    });
                                };
                                let range_content: Vec<_> =
                                    range_children.iter().filter(|n| n.is_content()).collect();

                                if range_content.len() < 2 {
                                    return Err(CompileError::ParseError {
                                        error_position: context,
                                        end_line_col: None,
                                        context: "CST parsing".to_string(),
                                        message: "For range clause missing start/end".to_string(),
                                    });
                                }

                                let start_expr =
                                    self.clone().parse_expression(&[range_content[0]])?;
                                let end_expr =
                                    self.clone().parse_expression(&[range_content[1]])?;

                                let id = self
                                    .names
                                    .borrow_mut()
                                    .declare_or_use_name(variable_text, DeclType::For);

                                self.enter_scope();
                                let body = self.clone().parse_statements_from_node(body_node)?;
                                let environment_width = self.exit_scope();

                                Ok(Some(Stmt::new(
                                    StmtNode::ForRange {
                                        id,
                                        from: start_expr,
                                        to: end_expr,
                                        body,
                                        environment_width,
                                    },
                                    line_col,
                                )))
                            }
                            _ => {
                                // Handle other iterables as regular expressions
                                let iter_expr = self.clone().parse_expression(&[iterable_node])?;
                                let value_binding = self
                                    .names
                                    .borrow_mut()
                                    .declare_or_use_name(variable_text, DeclType::For);

                                self.enter_scope();
                                let body = self.clone().parse_statements_from_node(body_node)?;
                                let environment_width = self.exit_scope();

                                Ok(Some(Stmt::new(
                                    StmtNode::ForList {
                                        value_binding,
                                        key_binding: None,
                                        expr: iter_expr,
                                        body,
                                        environment_width,
                                    },
                                    line_col,
                                )))
                            }
                        }
                    }
                    _ => Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: format!("Unknown for iterable type: {:?}", iterable_node.rule),
                    }),
                }
            }
            Rule::for_in_statement => {
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For-in statement has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.len() < 3 {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For-in statement missing components".to_string(),
                    });
                }

                // for for_in_index in for_in_clause statements endfor
                let for_in_index_node = content_children[0];
                let for_in_clause_node = content_children[1];
                let statements_node = content_children[2];

                // Parse for_in_index which can have 1 or 2 variables: ident ~ ("," ~ ident)?
                let Some(index_children) = for_in_index_node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For-in index node has no children".to_string(),
                    });
                };
                let index_content: Vec<_> =
                    index_children.iter().filter(|n| n.is_content()).collect();
                if index_content.is_empty() {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For-in index has no content".to_string(),
                    });
                }

                // Extract the value variable (first identifier)
                let Some(value_var_text) = index_content[0].text() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For value variable text not found".to_string(),
                    });
                };

                // Extract the optional key variable (second identifier if present)
                let key_var = if index_content.len() > 1 {
                    let Some(key_var_text) = index_content[1].text() else {
                        return Err(CompileError::ParseError {
                            error_position: context,
                            end_line_col: None,
                            context: "CST parsing".to_string(),
                            message: "For key variable text not found".to_string(),
                        });
                    };
                    Some(key_var_text)
                } else {
                    None
                };

                // Parse the for_in_clause to get the expression
                let iter_expr = self.clone().parse_expression(&[for_in_clause_node])?;

                // Declare loop variables in current scope
                let value_binding = self
                    .names
                    .borrow_mut()
                    .declare_or_use_name(value_var_text, DeclType::For);
                let key_binding = key_var.map(|key_text| {
                    self.names
                        .borrow_mut()
                        .declare_or_use_name(key_text, DeclType::For)
                });

                self.enter_scope();
                let body = self.clone().parse_statements_from_node(statements_node)?;
                let environment_width = self.exit_scope();

                Ok(Some(Stmt::new(
                    StmtNode::ForList {
                        value_binding,
                        key_binding,
                        expr: iter_expr,
                        body,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::for_range_statement => {
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For-range statement has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.len() < 3 {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For-range statement missing components".to_string(),
                    });
                }

                // for variable in [start..end] statements endfor
                // Expected: ident, for_range_clause, statements
                let var_node = content_children[0];
                let range_clause_node = content_children[1];
                let statements_node = content_children[2];

                // Parse the for_range_clause to extract start and end expressions
                let Some(range_children) = range_clause_node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For-range clause has no children".to_string(),
                    });
                };
                let range_content: Vec<_> =
                    range_children.iter().filter(|n| n.is_content()).collect();
                if range_content.len() < 2 {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "For-range clause missing start/end expressions".to_string(),
                    });
                }

                let start_node = range_content[0];
                let end_node = range_content[1];

                // Extract variable name from non-terminal node
                let var_name = if let Some(var_text) = var_node.text() {
                    var_text
                } else {
                    // Variable is likely in a child node
                    let Some(var_children) = var_node.children() else {
                        return Err(CompileError::ParseError {
                            error_position: context,
                            end_line_col: None,
                            context: "CST parsing".to_string(),
                            message: "For variable node has no children".to_string(),
                        });
                    };
                    let var_content: Vec<_> =
                        var_children.iter().filter(|n| n.is_content()).collect();
                    if var_content.is_empty() {
                        return Err(CompileError::ParseError {
                            error_position: context,
                            end_line_col: None,
                            context: "CST parsing".to_string(),
                            message: "For variable has no content".to_string(),
                        });
                    }
                    let Some(var_text) = var_content[0].text() else {
                        return Err(CompileError::ParseError {
                            error_position: context,
                            end_line_col: None,
                            context: "CST parsing".to_string(),
                            message: "For variable text not found".to_string(),
                        });
                    };
                    var_text
                };

                let start_expr = self.clone().parse_expression(&[start_node])?;
                let end_expr = self.clone().parse_expression(&[end_node])?;

                // Declare loop variable in current scope
                let var_name_obj = self
                    .names
                    .borrow_mut()
                    .declare_or_use_name(var_name, DeclType::For);

                self.enter_scope();
                let body = self.clone().parse_statements_from_node(statements_node)?;
                let environment_width = self.exit_scope();

                Ok(Some(Stmt::new(
                    StmtNode::ForRange {
                        id: var_name_obj,
                        from: start_expr,
                        to: end_expr,
                        body,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::try_except_statement => {
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Try-except statement has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.len() < 2 {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Try-except statement missing components".to_string(),
                    });
                }

                // try statements except handler endtry
                let statements_node = content_children[0];
                let _except_node = content_children[1];

                self.enter_scope();
                let body = self.clone().parse_statements_from_node(statements_node)?;
                let environment_width = self.exit_scope();

                // Parse except clauses
                let mut excepts = vec![];
                // Parse all except clauses from remaining children
                for except_child in content_children.iter().skip(1) {
                    if except_child.rule == Rule::except {
                        // Parse except clause
                        let Some(except_children) = except_child.children() else {
                            continue;
                        };
                        let except_content: Vec<_> =
                            except_children.iter().filter(|n| n.is_content()).collect();
                        if except_content.len() < 2 {
                            continue;
                        }

                        // Parse except type (labelled or unlabelled)
                        let except_clause = except_content[0];
                        let except_statements = except_content[1];

                        let (id, codes) = match except_clause.rule {
                            Rule::labelled_except => {
                                // Extract variable name and codes
                                let Some(clause_children) = except_clause.children() else {
                                    continue;
                                };
                                let clause_content: Vec<_> =
                                    clause_children.iter().filter(|n| n.is_content()).collect();
                                if clause_content.len() < 2 {
                                    continue;
                                }
                                let var_name = clause_content[0].text().unwrap_or("e");
                                let exception_id = Some(
                                    self.names
                                        .borrow_mut()
                                        .declare_or_use_name(var_name, DeclType::Except),
                                );
                                let codes_node = clause_content[1];
                                let codes = self.clone().parse_codes(codes_node)?;
                                (exception_id, codes)
                            }
                            Rule::unlabelled_except => {
                                // Just codes, no variable
                                let Some(clause_children) = except_clause.children() else {
                                    continue;
                                };
                                let clause_content: Vec<_> =
                                    clause_children.iter().filter(|n| n.is_content()).collect();
                                if clause_content.is_empty() {
                                    continue;
                                }
                                let codes_node = clause_content[0];
                                let codes = self.clone().parse_codes(codes_node)?;
                                (None, codes)
                            }
                            _ => continue,
                        };

                        // Parse except statements
                        let statements =
                            self.clone().parse_statements_from_node(except_statements)?;
                        excepts.push(ExceptArm {
                            id,
                            codes,
                            statements,
                        });
                    }
                }

                Ok(Some(Stmt::new(
                    StmtNode::TryExcept {
                        body,
                        excepts,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::try_finally_statement => {
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Try-finally statement has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.len() < 2 {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Try-finally statement missing components".to_string(),
                    });
                }

                // try statements finally handler endtry
                let statements_node = content_children[0];
                let handler_node = content_children[1];

                self.enter_scope();
                let body = self.clone().parse_statements_from_node(statements_node)?;
                let body_env_width = self.exit_scope();

                self.enter_scope();
                let handler = self.clone().parse_statements_from_node(handler_node)?;
                let handler_env_width = self.exit_scope();

                let environment_width = body_env_width.max(handler_env_width);

                Ok(Some(Stmt::new(
                    StmtNode::TryFinally {
                        body,
                        handler,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::break_statement => {
                let Some(children) = node.children() else {
                    // Simple break with no label
                    return Ok(Some(Stmt::new(StmtNode::Break { exit: None }, line_col)));
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.is_empty() {
                    // Simple break with no label
                    return Ok(Some(Stmt::new(StmtNode::Break { exit: None }, line_col)));
                }

                // break with label
                let label_node = content_children[0];
                let Some(label_name) = label_node.text() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Break label has no text".to_string(),
                    });
                };

                let label_var = self
                    .names
                    .borrow_mut()
                    .find_or_add_name_global(label_name, DeclType::Unknown)
                    .unwrap();
                Ok(Some(Stmt::new(
                    StmtNode::Break {
                        exit: Some(label_var),
                    },
                    line_col,
                )))
            }
            Rule::continue_statement => {
                let Some(children) = node.children() else {
                    // Simple continue with no label
                    return Ok(Some(Stmt::new(StmtNode::Continue { exit: None }, line_col)));
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.is_empty() {
                    // Simple continue with no label
                    return Ok(Some(Stmt::new(StmtNode::Continue { exit: None }, line_col)));
                }

                // continue with label
                let label_node = content_children[0];
                let Some(label_name) = label_node.text() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Continue label has no text".to_string(),
                    });
                };

                let label_var = self
                    .names
                    .borrow_mut()
                    .find_or_add_name_global(label_name, DeclType::Unknown)
                    .unwrap();
                Ok(Some(Stmt::new(
                    StmtNode::Continue {
                        exit: Some(label_var),
                    },
                    line_col,
                )))
            }
            Rule::fork_statement => {
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Fork statement has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.len() < 2 {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Fork statement missing components".to_string(),
                    });
                }

                // fork (time_expr) statements endfork or fork label (time_expr) statements endfork
                let mut idx = 0;
                let mut id = None;

                // Check if first child is a label (identifier)
                if content_children[idx].rule == Rule::ident {
                    let Some(label_name) = content_children[idx].text() else {
                        return Err(CompileError::ParseError {
                            error_position: context,
                            end_line_col: None,
                            context: "CST parsing".to_string(),
                            message: "Fork label has no text".to_string(),
                        });
                    };
                    id = Some(
                        self.names
                            .borrow_mut()
                            .find_or_add_name_global(label_name, DeclType::Unknown)
                            .unwrap(),
                    );
                    idx += 1;
                }

                if idx + 1 >= content_children.len() {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Fork statement missing time expression or body".to_string(),
                    });
                }

                let time_expr = self.clone().parse_expression(&[content_children[idx]])?;
                let statements_node = content_children[idx + 1];

                let body = self.clone().parse_statements_from_node(statements_node)?;

                Ok(Some(Stmt::new(
                    StmtNode::Fork {
                        id,
                        time: time_expr,
                        body,
                    },
                    line_col,
                )))
            }
            Rule::labelled_fork_statement => {
                // Labelled fork statements: fork label (time_expr) statements endfork
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Labelled fork statement has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.len() < 3 {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Labelled fork statement missing components".to_string(),
                    });
                }

                // Labelled fork: fork ident (expr) statements endfork
                let label_node = content_children[0]; // ident
                let time_node = content_children[1]; // expr 
                let statements_node = content_children[2]; // statements

                let Some(label_name) = label_node.text() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Fork label has no text".to_string(),
                    });
                };

                let id = Some(
                    self.names
                        .borrow_mut()
                        .find_or_add_name_global(label_name.trim(), DeclType::Unknown)
                        .unwrap(),
                );
                let time_expr = self.clone().parse_expression(&[time_node])?;
                let body = self.clone().parse_statements_from_node(statements_node)?;

                Ok(Some(Stmt::new(
                    StmtNode::Fork {
                        id,
                        time: time_expr,
                        body,
                    },
                    line_col,
                )))
            }
            Rule::local_assignment | Rule::const_assignment => {
                if !self.options.lexical_scopes {
                    return Err(CompileError::DisabledFeature(
                        context,
                        "lexical_scopes".to_string(),
                    ));
                }
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Local assignment has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.is_empty() {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Local assignment missing body".to_string(),
                    });
                }

                // Delegate to the inner assignment type
                let parts = content_children[0];
                match parts.rule {
                    Rule::local_assign_single | Rule::const_assign_single => {
                        let stmt_node = self.clone().parse_decl_assign(parts)?;
                        Ok(Some(Stmt::new(stmt_node, line_col)))
                    }
                    Rule::local_assign_scatter | Rule::const_assign_scatter => {
                        let is_const = parts.rule == Rule::const_assign_scatter;
                        let Some(assign_children) = parts.children() else {
                            return Err(CompileError::ParseError {
                                error_position: context,
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Scatter assignment has no children".to_string(),
                            });
                        };
                        let assign_content: Vec<_> =
                            assign_children.iter().filter(|n| n.is_content()).collect();
                        if assign_content.len() < 2 {
                            return Err(CompileError::ParseError {
                                error_position: context,
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Scatter assignment missing components".to_string(),
                            });
                        }

                        let scatter_op = assign_content[0];
                        let rhs = assign_content[1];
                        let rhs_expr = self.clone().parse_expression(&[rhs])?;
                        let expr =
                            self.parse_scatter_assign(scatter_op, rhs_expr, true, is_const)?;
                        Ok(Some(Stmt::new(StmtNode::Expr(expr), line_col)))
                    }
                    _ => Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: format!("Unimplemented assignment: {:?}", parts.rule),
                    }),
                }
            }
            Rule::labelled_while_statement => {
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Labelled while statement has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.len() < 3 {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Labelled while statement missing components".to_string(),
                    });
                }

                // while label (condition) statements endwhile
                let Some(varname) = content_children[0].text() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "While label has no text".to_string(),
                    });
                };

                let Some(id) = self
                    .names
                    .borrow_mut()
                    .declare_name(varname, DeclType::WhileLabel)
                else {
                    return Err(DuplicateVariable(context, varname.into()));
                };

                let condition = self.clone().parse_expression(&[content_children[1]])?;

                self.enter_scope();
                let body = self
                    .clone()
                    .parse_statements_from_node(content_children[2])?;
                let environment_width = self.exit_scope();

                Ok(Some(Stmt::new(
                    StmtNode::While {
                        id: Some(id),
                        condition,
                        body,
                        environment_width,
                    },
                    line_col,
                )))
            }
            Rule::begin_statement => {
                if !self.options.lexical_scopes {
                    return Err(CompileError::DisabledFeature(
                        context,
                        "lexical_scopes".to_string(),
                    ));
                }
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Begin statement has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.is_empty() {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Begin statement missing body".to_string(),
                    });
                }

                self.enter_scope();
                let body = self
                    .clone()
                    .parse_statements_from_node(content_children[0])?;
                let num_bindings = self.exit_scope();

                Ok(Some(Stmt::new(
                    StmtNode::Scope { num_bindings, body },
                    line_col,
                )))
            }
            Rule::global_assignment => {
                if !self.options.lexical_scopes {
                    return Err(CompileError::DisabledFeature(
                        context,
                        "lexical_scopes".to_string(),
                    ));
                }

                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Global assignment has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                if content_children.is_empty() {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Global assignment missing variable name".to_string(),
                    });
                }

                // global varname (ASSIGN expr)? ;
                let var_node = content_children[0];
                let var_name = var_node.text().ok_or_else(|| CompileError::ParseError {
                    error_position: context.clone(),
                    end_line_col: None,
                    context: "CST parsing".to_string(),
                    message: "Global variable name has no text".to_string(),
                })?;

                // Declare as global variable
                let id = {
                    let mut names = self.names.borrow_mut();
                    let Some(id) = names.find_or_add_name_global(var_name, DeclType::Global) else {
                        return Err(DuplicateVariable(context, var_name.into()));
                    };
                    names.decl_for_mut(&id).decl_type = DeclType::Global;
                    id
                };

                // Parse optional assignment expression
                let expr = if content_children.len() > 1 {
                    self.clone().parse_expression(&content_children[1..])?
                } else {
                    Expr::Value(v_none())
                };

                // Create assignment expression
                Ok(Some(Stmt::new(
                    StmtNode::Expr(Expr::Assign {
                        left: Box::new(Expr::Id(id)),
                        right: Box::new(expr),
                    }),
                    line_col,
                )))
            }
            Rule::fn_statement => {
                let Some(children) = node.children() else {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Fn statement node has no children".to_string(),
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
                if content_children.is_empty() {
                    return Err(CompileError::ParseError {
                        error_position: context,
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: "Fn statement missing content".to_string(),
                    });
                }

                let inner = content_children[0];
                match inner.rule {
                    Rule::fn_named => {
                        // fn name(params) statements endfn
                        // This is like: let name = fn(params) statements endfn;
                        let Some(fn_children) = inner.children() else {
                            return Err(CompileError::ParseError {
                                error_position: self.compile_context(inner),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Fn named statement has no children".to_string(),
                            });
                        };
                        let fn_content: Vec<_> =
                            fn_children.iter().filter(|n| n.is_content()).collect();
                        if fn_content.len() < 3 {
                            return Err(CompileError::ParseError {
                                error_position: self.compile_context(inner),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Fn named statement missing name, params, or body"
                                    .to_string(),
                            });
                        }

                        let func_name =
                            fn_content[0]
                                .text()
                                .ok_or_else(|| CompileError::ParseError {
                                    error_position: self.compile_context(fn_content[0]),
                                    end_line_col: None,
                                    context: "fn named parsing".to_string(),
                                    message: "Function name has no text".to_string(),
                                })?;

                        let params_part = fn_content[1];
                        let statements_part = fn_content[2];

                        // Parse lambda params
                        let params = self.clone().parse_lambda_params(params_part)?;

                        // Parse the function body
                        self.enter_scope();
                        let statements =
                            self.clone().parse_statements_from_node(statements_part)?;
                        let num_total_bindings = self.exit_scope();
                        let body = Box::new(Stmt::new(
                            StmtNode::Scope {
                                num_bindings: num_total_bindings,
                                body: statements,
                            },
                            statements_part.line_col(),
                        ));

                        // Create the variable for the function name
                        let id = {
                            let mut names = self.names.borrow_mut();
                            names.declare_or_use_name(func_name, DeclType::Let)
                        };

                        // Create a lambda expression with self-reference
                        let lambda_expr = Expr::Lambda {
                            params,
                            body,
                            self_name: Some(id),
                        };

                        Ok(Some(Stmt::new(
                            StmtNode::Expr(Expr::Decl {
                                id,
                                expr: Some(Box::new(lambda_expr)),
                                is_const: false,
                            }),
                            line_col,
                        )))
                    }
                    Rule::fn_assignment => {
                        // name = fn(params) statements endfn;
                        // Parse this as: variable = fn_expr
                        let Some(fn_children) = inner.children() else {
                            return Err(CompileError::ParseError {
                                error_position: self.compile_context(inner),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Fn assignment statement has no children".to_string(),
                            });
                        };
                        let fn_content: Vec<_> =
                            fn_children.iter().filter(|n| n.is_content()).collect();
                        if fn_content.len() < 2 {
                            return Err(CompileError::ParseError {
                                error_position: self.compile_context(inner),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Fn assignment statement missing variable or function"
                                    .to_string(),
                            });
                        }

                        let var_name =
                            fn_content[0]
                                .text()
                                .ok_or_else(|| CompileError::ParseError {
                                    error_position: self.compile_context(fn_content[0]),
                                    end_line_col: None,
                                    context: "fn assignment parsing".to_string(),
                                    message: "Variable name has no text".to_string(),
                                })?;

                        let func_expr_part = fn_content[1]; // This contains the fn_expr

                        // Parse the fn expression manually (similar to fn_expr case above)
                        let Some(func_children) = func_expr_part.children() else {
                            return Err(CompileError::ParseError {
                                error_position: self.compile_context(func_expr_part),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Fn expression has no children".to_string(),
                            });
                        };
                        let func_content: Vec<_> =
                            func_children.iter().filter(|n| n.is_content()).collect();
                        if func_content.len() < 2 {
                            return Err(CompileError::ParseError {
                                error_position: self.compile_context(func_expr_part),
                                end_line_col: None,
                                context: "CST parsing".to_string(),
                                message: "Fn expression missing params or body".to_string(),
                            });
                        }

                        let lambda_params = func_content[0];
                        let statements_part = func_content[1];

                        let params = self.clone().parse_lambda_params(lambda_params)?;

                        // Parse the function body with proper scope tracking
                        self.enter_scope();
                        let statements =
                            self.clone().parse_statements_from_node(statements_part)?;
                        let num_total_bindings = self.exit_scope();
                        let body = Box::new(Stmt::new(
                            StmtNode::Scope {
                                num_bindings: num_total_bindings,
                                body: statements,
                            },
                            statements_part.line_col(),
                        ));

                        // Create the lambda expression
                        let lambda_expr = Expr::Lambda {
                            params,
                            body,
                            self_name: None, // No self-reference for assignments
                        };

                        // Create assignment or declaration
                        let maybe_id = self.names.borrow().find_name(var_name);
                        let assign_expr = match maybe_id {
                            Some(id) => {
                                // Variable exists, create assignment
                                Expr::Assign {
                                    left: Box::new(Expr::Id(id)),
                                    right: Box::new(lambda_expr),
                                }
                            }
                            None => {
                                // Variable doesn't exist, declare it
                                let id = {
                                    let mut names = self.names.borrow_mut();
                                    let Some(id) = names.declare_name(var_name, DeclType::Let)
                                    else {
                                        return Err(CompileError::ParseError {
                                            error_position: context,
                                            end_line_col: None,
                                            context: "fn assignment parsing".to_string(),
                                            message: format!(
                                                "Could not declare variable: {var_name}"
                                            ),
                                        });
                                    };
                                    id
                                };
                                Expr::Decl {
                                    id,
                                    expr: Some(Box::new(lambda_expr)),
                                    is_const: false,
                                }
                            }
                        };

                        Ok(Some(Stmt::new(StmtNode::Expr(assign_expr), line_col)))
                    }
                    _ => Err(CompileError::ParseError {
                        error_position: self.compile_context(inner),
                        end_line_col: None,
                        context: "CST parsing".to_string(),
                        message: format!("Unimplemented fn statement type: {:?}", inner.rule),
                    }),
                }
            }
            _ => {
                // For now, return error for unimplemented statement types
                Err(CompileError::ParseError {
                    error_position: context,
                    end_line_col: None,
                    context: "CST parsing".to_string(),
                    message: format!("Unimplemented statement type: {:?}", node.rule),
                })
            }
        }
    }

    /// Parse statements from a statements node (helper function)
    fn parse_statements_from_node(
        self: Rc<Self>,
        node: &CSTNode,
    ) -> Result<Vec<Stmt>, CompileError> {
        match node.rule {
            Rule::statements => {
                let Some(children) = node.children() else {
                    return Ok(vec![]);
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
                self.parse_statements(&content_children)
            }
            _ => {
                // Single statement
                match self.parse_statement(node)? {
                    Some(stmt) => Ok(vec![stmt]),
                    None => Ok(vec![]),
                }
            }
        }
    }

    fn enter_scope(&self) {
        self.names.borrow_mut().enter_new_scope();
    }

    fn exit_scope(&self) -> usize {
        self.names.borrow_mut().exit_scope()
    }

    fn parse_codes(self: Rc<Self>, node: &CSTNode) -> Result<CatchCodes, CompileError> {
        match node.rule {
            Rule::anycode => Ok(CatchCodes::Any),
            Rule::exprlist => {
                let exprs = self.parse_exprlist(node)?;
                Ok(CatchCodes::Codes(exprs))
            }
            Rule::codes => {
                // codes = { anycode | exprlist } - check which child it has
                let Some(children) = node.children() else {
                    return Ok(CatchCodes::Any);
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
                if content_children.is_empty() {
                    return Ok(CatchCodes::Any);
                }
                // Delegate to the actual rule type
                self.parse_codes(content_children[0])
            }
            _ => Err(CompileError::ParseError {
                error_position: self.compile_context(node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: format!("Unimplemented except_codes: {:?}", node.rule),
            }),
        }
    }

    fn parse_decl_assign(self: Rc<Self>, node: &CSTNode) -> Result<StmtNode, CompileError> {
        let context = self.compile_context(node);
        let is_const = node.rule == Rule::const_assign_single;

        // An assignment declaration that introduces a locally lexically scoped variable.
        // May be of form `let x = expr` or just `let x`
        let Some(children) = node.children() else {
            return Err(CompileError::ParseError {
                error_position: context,
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: "Declaration assignment has no children".to_string(),
            });
        };
        let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

        if content_children.is_empty() {
            return Err(CompileError::ParseError {
                error_position: context,
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: "Declaration assignment missing variable name".to_string(),
            });
        }

        let Some(varname) = content_children[0].text() else {
            return Err(CompileError::ParseError {
                error_position: context,
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: "Variable name has no text".to_string(),
            });
        };

        let id = {
            let mut names = self.names.borrow_mut();
            let Some(id) = names.declare(varname, is_const, false, DeclType::Let) else {
                return Err(DuplicateVariable(context, varname.into()));
            };
            id
        };

        let expr = if content_children.len() > 1 {
            Some(Box::new(self.parse_expression(&[content_children[1]])?))
        } else {
            None
        };

        // Create a proper Decl expression for let/const declarations
        Ok(StmtNode::Expr(Expr::Decl { id, is_const, expr }))
    }

    fn parse_scatter_assign(
        self: Rc<Self>,
        scatter_node: &CSTNode,
        rhs: Expr,
        is_local: bool,
        is_const: bool,
    ) -> Result<Expr, CompileError> {
        let Some(children) = scatter_node.children() else {
            return Err(CompileError::ParseError {
                error_position: self.compile_context(scatter_node),
                end_line_col: None,
                context: "CST parsing".to_string(),
                message: "Scatter assignment has no children".to_string(),
            });
        };

        let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
        let mut scatter_items = Vec::new();

        // Parse scatter items from the pattern
        for child in content_children {
            match child.rule {
                Rule::ident => {
                    // Tree-sitter creates direct ident children under scatter
                    let Some(var_name) = child.text() else {
                        continue;
                    };

                    let id = if is_local {
                        let mut names = self.names.borrow_mut();
                        names
                            .declare(var_name, is_const, false, DeclType::Assign)
                            .unwrap_or_else(|| {
                                names
                                    .find_or_add_name_global(var_name, DeclType::Assign)
                                    .unwrap()
                            })
                    } else {
                        self.names
                            .borrow_mut()
                            .find_or_add_name_global(var_name, DeclType::Assign)
                            .unwrap()
                    };

                    scatter_items.push(ScatterItem {
                        kind: ScatterKind::Required,
                        id,
                        expr: None,
                    });
                }
                Rule::scatter_target => {
                    // scatter_target contains an ident terminal
                    let Some(target_children) = child.children() else {
                        continue;
                    };
                    let target_content: Vec<_> =
                        target_children.iter().filter(|n| n.is_content()).collect();
                    if target_content.is_empty() {
                        continue;
                    }
                    let Some(var_name) = target_content[0].text() else {
                        continue;
                    };

                    let id = if is_local {
                        let mut names = self.names.borrow_mut();
                        names
                            .declare(var_name, is_const, false, DeclType::Assign)
                            .unwrap_or_else(|| {
                                names
                                    .find_or_add_name_global(var_name, DeclType::Assign)
                                    .unwrap()
                            })
                    } else {
                        self.names
                            .borrow_mut()
                            .find_or_add_name_global(var_name, DeclType::Assign)
                            .unwrap()
                    };

                    scatter_items.push(ScatterItem {
                        kind: ScatterKind::Required,
                        id,
                        expr: None,
                    });
                }
                Rule::scatter_optional => {
                    // ?var or ?var=default
                    let Some(opt_children) = child.children() else {
                        continue;
                    };
                    let opt_content: Vec<_> =
                        opt_children.iter().filter(|n| n.is_content()).collect();
                    if opt_content.is_empty() {
                        continue;
                    }
                    let Some(var_name) = opt_content[0].text() else {
                        continue;
                    };

                    let id = if is_local {
                        let mut names = self.names.borrow_mut();
                        names
                            .declare(var_name, is_const, false, DeclType::Assign)
                            .unwrap_or_else(|| {
                                names
                                    .find_or_add_name_global(var_name, DeclType::Assign)
                                    .unwrap()
                            })
                    } else {
                        self.names
                            .borrow_mut()
                            .find_or_add_name_global(var_name, DeclType::Assign)
                            .unwrap()
                    };

                    // Check for default expression - if there's a second child that's not an identifier
                    let default_expr =
                        if opt_content.len() >= 2 && opt_content[1].rule != Rule::ident {
                            Some(self.clone().parse_expression(&opt_content[1..2])?)
                        } else {
                            None
                        };

                    scatter_items.push(ScatterItem {
                        kind: ScatterKind::Optional,
                        id,
                        expr: default_expr,
                    });
                }
                Rule::scatter_rest => {
                    // @rest_var
                    let Some(rest_children) = child.children() else {
                        continue;
                    };
                    let rest_content: Vec<_> =
                        rest_children.iter().filter(|n| n.is_content()).collect();
                    if rest_content.is_empty() {
                        continue;
                    }
                    let Some(var_name) = rest_content[0].text() else {
                        continue;
                    };

                    let id = if is_local {
                        let mut names = self.names.borrow_mut();
                        names
                            .declare(var_name, is_const, false, DeclType::Assign)
                            .unwrap_or_else(|| {
                                names
                                    .find_or_add_name_global(var_name, DeclType::Assign)
                                    .unwrap()
                            })
                    } else {
                        self.names
                            .borrow_mut()
                            .find_or_add_name_global(var_name, DeclType::Assign)
                            .unwrap()
                    };

                    scatter_items.push(ScatterItem {
                        kind: ScatterKind::Rest,
                        id,
                        expr: None,
                    });
                }
                _ => {} // Skip unknown scatter elements
            }
        }

        Ok(Expr::Scatter(scatter_items, Box::new(rhs)))
    }

    /// Parse lambda parameters from CST node
    fn parse_lambda_params(
        self: Rc<Self>,
        params_node: &CSTNode,
    ) -> Result<Vec<ScatterItem>, CompileError> {
        let Some(children) = params_node.children() else {
            return Ok(vec![]); // Empty parameter list
        };

        let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();
        let mut items = vec![];

        for child in content_children {
            match child.rule {
                Rule::lambda_param => {
                    let Some(param_children) = child.children() else {
                        continue;
                    };
                    let param_content: Vec<_> =
                        param_children.iter().filter(|n| n.is_content()).collect();
                    if param_content.is_empty() {
                        continue;
                    }

                    let inner_param = param_content[0];
                    match inner_param.rule {
                        Rule::scatter_optional => {
                            let Some(scatter_children) = inner_param.children() else {
                                continue;
                            };
                            let scatter_content: Vec<_> =
                                scatter_children.iter().filter(|n| n.is_content()).collect();
                            if scatter_content.is_empty() {
                                continue;
                            }

                            let Some(id_str) = scatter_content[0].text() else {
                                continue;
                            };
                            let context = self.compile_context(inner_param);
                            let Some(id) = self.names.borrow_mut().declare(
                                id_str,
                                false,
                                false,
                                DeclType::Assign,
                            ) else {
                                return Err(DuplicateVariable(context, id_str.into()));
                            };

                            let expr = if scatter_content.len() > 1 {
                                Some(self.clone().parse_expr_node(scatter_content[1])?)
                            } else {
                                None
                            };

                            items.push(ScatterItem {
                                kind: ScatterKind::Optional,
                                id,
                                expr,
                            });
                        }
                        Rule::scatter_target => {
                            // scatter_target = { ident }, so we need to get the ident child
                            let Some(scatter_children) = inner_param.children() else {
                                continue;
                            };
                            let scatter_content: Vec<_> =
                                scatter_children.iter().filter(|n| n.is_content()).collect();
                            if scatter_content.is_empty() {
                                continue;
                            }

                            // The first child should be the ident
                            let ident_node = scatter_content[0];
                            if ident_node.rule != Rule::ident {
                                continue;
                            }

                            let Some(id_str) = ident_node.text() else {
                                continue;
                            };
                            let context = self.compile_context(inner_param);
                            let Some(id) = self.names.borrow_mut().declare(
                                id_str,
                                false,
                                false,
                                DeclType::Assign,
                            ) else {
                                return Err(DuplicateVariable(context, id_str.into()));
                            };

                            items.push(ScatterItem {
                                kind: ScatterKind::Required,
                                id,
                                expr: None,
                            });
                        }
                        Rule::scatter_rest => {
                            // scatter_rest = { "@" ~ ident }, so we need to get the ident child
                            let Some(scatter_children) = inner_param.children() else {
                                continue;
                            };
                            let scatter_content: Vec<_> =
                                scatter_children.iter().filter(|n| n.is_content()).collect();
                            if scatter_content.is_empty() {
                                continue;
                            }

                            // Find the ident child (should be after the "@")
                            let ident_node = scatter_content.iter().find(|n| n.rule == Rule::ident);
                            let Some(ident_node) = ident_node else {
                                continue;
                            };

                            let Some(id_str) = ident_node.text() else {
                                continue;
                            };
                            let context = self.compile_context(inner_param);
                            let Some(id) = self.names.borrow_mut().declare(
                                id_str,
                                false,
                                false,
                                DeclType::Assign,
                            ) else {
                                return Err(DuplicateVariable(context, id_str.into()));
                            };

                            items.push(ScatterItem {
                                kind: ScatterKind::Rest,
                                id,
                                expr: None,
                            });
                        }
                        _ => {
                            // Skip unrecognized parameter types
                            continue;
                        }
                    }
                }
                _ => {
                    // Skip non-lambda-param children
                    continue;
                }
            }
        }

        Ok(items)
    }

    /// Transform a CST into a ParseCst result
    pub fn transform(self: Rc<Self>, cst: CSTNode) -> Result<ParseCst, CompileError> {
        // Parse the program CST node
        let statements = match cst.rule {
            Rule::program => {
                let Some(children) = cst.children() else {
                    let variables = self.names.replace(VarScope::new());
                    let names = variables.bind();
                    return Ok(ParseCst {
                        stmts: vec![],
                        variables,
                        names,
                        cst,
                    });
                };
                let content_children: Vec<_> = children.iter().filter(|n| n.is_content()).collect();

                let mut all_statements = Vec::new();
                for child in content_children {
                    match child.rule {
                        Rule::statements => {
                            let parsed_statements =
                                self.clone().parse_statements_from_node(child)?;
                            all_statements.extend(parsed_statements);
                        }
                        _ => {
                            // Unexpected rule at program level, skip for now
                        }
                    }
                }
                all_statements
            }
            _ => {
                return Err(CompileError::ParseError {
                    error_position: CompileContext::new((1, 1)),
                    end_line_col: None,
                    context: "CST transformation".to_string(),
                    message: format!("Expected program node, got {:?}", cst.rule),
                });
            }
        };

        // Extract final state
        let variables = self.names.replace(VarScope::new());

        // Create Names from the VarScope
        let names = variables.bind();

        // Annotate the "true" line numbers of the AST nodes (same as original parser)
        let mut statements = statements;
        annotate_line_numbers(1, &mut statements);

        Ok(ParseCst {
            stmts: statements,
            variables,
            names,
            cst,
        })
    }
}

/// Create an enhanced error from a PEST parsing error
fn create_enhanced_pest_error(source: &str, pest_error: &pest::error::Error<Rule>) -> CompileError {
    let error_reporter = DefaultErrorReporter;

    // Extract position information from PEST error
    let (start_line, start_col, end_line, end_col) = match pest_error.line_col {
        pest::error::LineColLocation::Pos((line, col)) => (line, col, line, col),
        pest::error::LineColLocation::Span((l1, c1), (l2, c2)) => (l1, c1, l2, c2),
    };

    // Create error positions
    let start_pos = ErrorPosition::new(start_line, start_col, 0); // PEST doesn't provide byte offset
    let end_pos = ErrorPosition::new(end_line, end_col, 0);
    let error_span = ErrorSpan::new(start_pos.clone(), end_pos);

    // Extract error text from the source around the error location
    let source_lines: Vec<&str> = source.lines().collect();
    let error_line_idx = start_line.saturating_sub(1);
    let error_text = if error_line_idx < source_lines.len() {
        let line = source_lines[error_line_idx];
        let start_char = start_col.saturating_sub(1).min(line.len());
        let end_char = end_col.min(line.len());
        if start_char < end_char {
            line[start_char..end_char].to_string()
        } else {
            // If we can't extract specific text, use the whole line
            line.to_string()
        }
    } else {
        "unknown".to_string()
    };

    // Infer context from the error location
    let context = infer_parse_context(source, &start_pos);

    // Create enhanced error with PEST-specific message
    let pest_message = format!("{}", pest_error.variant);
    let enhanced_error = EnhancedError::new(error_span, error_text, context)
        .with_message(format!("Parse error: {pest_message}"));

    error_reporter.create_enhanced_error(source, &enhanced_error)
}

/// Parse program text into CST then transform to AST, preserving comments
pub fn parse_program_cst(
    program_text: &str,
    options: CompileOptions,
) -> Result<ParseCst, CompileError> {
    // First, parse with Pest to get the parse tree
    let pairs = match MooParser::parse(Rule::program, program_text) {
        Ok(pairs) => pairs,
        Err(e) => {
            // Use enhanced error reporting for PEST errors
            return Err(create_enhanced_pest_error(program_text, &e));
        }
    };

    // Convert Pest parse tree to CST
    let converter = PestToCSTConverter::new(program_text.to_string());
    let program_pair = pairs.into_iter().next().unwrap();
    let cst = converter
        .convert_program(program_pair)
        .map_err(|msg| CompileError::ParseError {
            error_position: CompileContext::new((1, 1)),
            end_line_col: None,
            context: "CST conversion".to_string(),
            message: msg,
        })?;

    // Transform CST to AST
    let transformer = CSTTreeTransformer::new(options);
    transformer.transform(cst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CSTSpan;

    #[test]
    fn test_parse2_basic() {
        let source = "1 + 2;";
        let result = parse_program_cst(source, CompileOptions::default());

        assert!(result.is_ok());
        let parse_result = result.unwrap();

        // Verify we preserved the CST
        assert_eq!(parse_result.cst.to_source(), source);

        // Verify we parsed one statement
        assert_eq!(parse_result.stmts.len(), 1);

        // Verify the statement is an expression statement
        match &parse_result.stmts[0].node {
            StmtNode::Expr(expr) => {
                // Verify it's a binary addition
                match expr {
                    Expr::Binary(BinaryOp::Add, _, _) => {
                        // Success!
                    }
                    _ => panic!("Expected binary addition expression, got: {:?}", expr),
                }
            }
            _ => panic!(
                "Expected expression statement, got: {:?}",
                parse_result.stmts[0].node
            ),
        }
    }

    #[test]
    fn test_parse2_vs_original_simple() {
        use crate::parsers::parse::parse_program;

        let source = "1 + 2;";

        // Parse with original parser
        let original_result =
            parse_program(source, CompileOptions::default()).expect("Original parse failed");

        // Parse with new parser
        let new_result =
            parse_program_cst(source, CompileOptions::default()).expect("New parse failed");

        // Compare statement counts
        assert_eq!(
            new_result.stmts.len(),
            original_result.stmts.len(),
            "Statement count mismatch"
        );

        // Compare first statement structure (both should be expression statements)
        if !original_result.stmts.is_empty() && !new_result.stmts.is_empty() {
            let original_stmt = &original_result.stmts[0];
            let new_stmt = &new_result.stmts[0];

            // Both should be expression statements
            match (&original_stmt.node, &new_stmt.node) {
                (StmtNode::Expr(orig_expr), StmtNode::Expr(new_expr)) => {
                    // Both should be binary addition expressions
                    match (orig_expr, new_expr) {
                        (Expr::Binary(BinaryOp::Add, _, _), Expr::Binary(BinaryOp::Add, _, _)) => {
                            // Success! Both parsers produced equivalent results
                        }
                        _ => {
                            panic!(
                                "Expression mismatch: original={:?}, new={:?}",
                                orig_expr, new_expr
                            );
                        }
                    }
                }
                _ => {
                    panic!(
                        "Statement type mismatch: original={:?}, new={:?}",
                        original_stmt.node, new_stmt.node
                    );
                }
            }
        }
    }

    #[test]
    fn test_parse2_postfix_operators() {
        use crate::parsers::parse::parse_program;

        let test_cases = vec![
            "x[1];",           // index_single
            "x[1..5];",        // index_range
            "obj.prop;",       // prop
            "obj.(\"prop\");", // prop_expr
            "x = 5;",          // assign
                               // TODO: Add verb calls when we implement function parsing
        ];

        for source in test_cases {
            println!("Testing postfix: {}", source);

            // Parse with both parsers
            let original_result = parse_program(source, CompileOptions::default())
                .expect(&format!("Original parse failed for: {}", source));
            let new_result = parse_program_cst(source, CompileOptions::default())
                .expect(&format!("New parse failed for: {}", source));

            // Compare statement counts
            assert_eq!(
                new_result.stmts.len(),
                original_result.stmts.len(),
                "Statement count mismatch for: {}",
                source
            );

            // Both should have at least one statement
            assert!(
                !new_result.stmts.is_empty(),
                "No statements parsed for: {}",
                source
            );
        }
    }

    #[test]
    fn test_debug_postfix_parsing() {
        let source = "x[1];";

        // Parse to CST first
        let pairs = MooParser::parse(Rule::program, source).unwrap();
        let converter = PestToCSTConverter::new(source.to_string());
        let program_pair = pairs.into_iter().next().unwrap();
        let cst = converter.convert_program(program_pair).unwrap();

        println!("CST structure for '{}':", source);
        print_cst_debug(&cst, 0);

        // Now try parsing with our parser
        let result = parse_program_cst(source, CompileOptions::default());
        match result {
            Ok(parse_result) => {
                println!("\nParsed successfully:");
                if !parse_result.stmts.is_empty() {
                    println!("First statement: {:?}", parse_result.stmts[0].node);
                }
            }
            Err(e) => {
                println!("\nParsing failed: {:?}", e);
            }
        }
    }

    fn print_cst_debug(node: &CSTNode, indent: usize) {
        let prefix = "  ".repeat(indent);
        println!("{}Rule::{:?}", prefix, node.rule);
        if let Some(text) = node.text() {
            println!("{}  text: '{}'", prefix, text);
        }
        if let Some(children) = node.children() {
            println!("{}  children:", prefix);
            for child in children {
                print_cst_debug(child, indent + 1);
            }
        }
    }

    #[test]
    fn test_parse2_comprehensive_ast_parity() {
        use crate::ast::assert_trees_match_recursive;
        use crate::parsers::parse::parse_program;

        let test_cases = vec![
            // Basic expressions
            "1;",
            "1 + 2;",
            "1 * 2 + 3;",
            "1 + 2 * 3;",
            "(1 + 2) * 3;",
            "1 == 2;",
            "1 < 2 && 3 > 4;",
            "true || false;",
            // Postfix operators
            "x[1];",
            "x[1..5];",
            "obj.prop;",
            "obj.(\"prop\");",
            "x = 5;",
            "y = x + 1;",
            // Complex expressions
            "x = y[1] + z.prop * 2;",
            "a && b || c;",
            // Atoms
            "42;",
            "3.14;",
            "\"hello\";",
            "true;",
            "false;",
            "#123;",
            "'symbol;",
        ];

        for source in test_cases {
            println!("Testing AST parity: {}", source);

            // Parse with both parsers
            let original_result = parse_program(source, CompileOptions::default())
                .expect(&format!("Original parse failed for: {}", source));
            let new_result = parse_program_cst(source, CompileOptions::default())
                .expect(&format!("New parse failed for: {}", source));

            // Verify complete AST parity using the recursive comparison function
            assert_trees_match_recursive(&new_result.stmts, &original_result.stmts);

            // Also verify source preservation
            assert_eq!(
                new_result.cst.to_source(),
                source,
                "Source preservation failed for: {}",
                source
            );
        }
    }

    #[test]
    fn test_parse2_vs_original_multiple_expressions() {
        use crate::parsers::parse::parse_program;

        let test_cases = vec![
            "1;",
            "1 + 2;",
            "1 * 2 + 3;",
            "1 + 2 * 3;",
            "(1 + 2) * 3;",
            "1 == 2;",
            "1 < 2;",
            "1 <= 2;",
            "true && false;",
            "true || false;",
        ];

        for source in test_cases {
            println!("Testing: {}", source);

            // Parse with both parsers
            let original_result = parse_program(source, CompileOptions::default())
                .expect(&format!("Original parse failed for: {}", source));
            let new_result = parse_program_cst(source, CompileOptions::default())
                .expect(&format!("New parse failed for: {}", source));

            // Compare statement counts
            assert_eq!(
                new_result.stmts.len(),
                original_result.stmts.len(),
                "Statement count mismatch for: {}",
                source
            );

            // Verify CST preserves source
            assert_eq!(
                new_result.cst.to_source(),
                source,
                "CST source preservation failed for: {}",
                source
            );

            // For single expression statements, verify structure similarity
            if original_result.stmts.len() == 1 && new_result.stmts.len() == 1 {
                let original_stmt = &original_result.stmts[0];
                let new_stmt = &new_result.stmts[0];

                match (&original_stmt.node, &new_stmt.node) {
                    (StmtNode::Expr(_), StmtNode::Expr(_)) => {
                        // Both are expression statements - this is the key similarity we need
                        // TODO: We could add deeper expression structure comparison here
                    }
                    _ => {
                        panic!(
                            "Statement type mismatch for {}: original={:?}, new={:?}",
                            source, original_stmt.node, new_stmt.node
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_parse2_vs_original_atoms() {
        use crate::parsers::parse::parse_program;

        let test_cases = vec![
            "42;",
            "3.14;",
            "\"hello\";",
            "true;",
            "false;",
            "#123;",
            "'symbol;",
        ];

        for source in test_cases {
            println!("Testing atom: {}", source);

            let original_result = parse_program(source, CompileOptions::default())
                .expect(&format!("Original parse failed for: {}", source));
            let new_result = parse_program_cst(source, CompileOptions::default())
                .expect(&format!("New parse failed for: {}", source));

            // Both should parse to single expression statements
            assert_eq!(original_result.stmts.len(), 1);
            assert_eq!(new_result.stmts.len(), 1);

            // Verify CST preserves source
            assert_eq!(new_result.cst.to_source(), source);
        }
    }

    #[test]
    fn test_parse2_empty_program() {
        use crate::parsers::parse::parse_program;

        let source = "";

        let original_result =
            parse_program(source, CompileOptions::default()).expect("Original parse failed");
        let new_result =
            parse_program_cst(source, CompileOptions::default()).expect("New parse failed");

        // Both should have no statements
        assert_eq!(original_result.stmts.len(), new_result.stmts.len());
        assert_eq!(new_result.cst.to_source(), source);
    }

    #[test]
    fn debug_scatter_parsing() {
        use crate::codegen::compile;

        let program = "{a, b, c} = args;";

        println!("=== Testing Original Parser ===");
        match compile(program, CompileOptions::default()) {
            Ok(binary) => {
                println!("Original parser succeeded");
                // Try to find our variables
                for var in ["a", "b", "c", "args"] {
                    match std::panic::catch_unwind(|| binary.find_var(var)) {
                        Ok(name) => println!("  {} -> {:?}", var, name),
                        Err(_) => println!("  {} -> ERROR: not found", var),
                    }
                }
            }
            Err(e) => println!("Original parser failed: {:?}", e),
        }

        println!("\n=== Testing CST Parser ===");
        match compile(program, CompileOptions::default()) {
            Ok(binary) => {
                println!("CST parser succeeded");
                // Try to find our variables
                for var in ["a", "b", "c", "args"] {
                    match std::panic::catch_unwind(|| binary.find_var(var)) {
                        Ok(name) => println!("  {} -> {:?}", var, name),
                        Err(_) => println!("  {} -> ERROR: not found", var),
                    }
                }
            }
            Err(e) => println!("CST parser failed: {:?}", e),
        }
    }

    #[test]
    fn test_cst_expression_parsing() {
        use crate::ast::{BinaryOp, Expr};

        let transformer = CSTTreeTransformer::new(CompileOptions::default());

        // Create simple expression: 1 + 2
        let left = CSTNode::terminal(
            Rule::integer,
            "1".to_string(),
            CSTSpan {
                start: 0,
                end: 1,
                line_col: (1, 1),
            },
        );
        let op = CSTNode::terminal(
            Rule::add,
            "+".to_string(),
            CSTSpan {
                start: 2,
                end: 3,
                line_col: (1, 3),
            },
        );
        let right = CSTNode::terminal(
            Rule::integer,
            "2".to_string(),
            CSTSpan {
                start: 4,
                end: 5,
                line_col: (1, 5),
            },
        );

        let nodes = vec![&left, &op, &right];
        let result = transformer.parse_expression(&nodes);

        assert!(result.is_ok());
        let expr = result.unwrap();

        match expr {
            Expr::Binary(BinaryOp::Add, l, r) => {
                assert!(matches!(*l, Expr::Value(_)));
                assert!(matches!(*r, Expr::Value(_)));
            }
            _ => panic!("Expected binary addition expression"),
        }
    }

    #[test]
    fn debug_ast_comparison() {
        use crate::parsers::parse::parse_program;

        let program = r#"begin
            let {things, ?nothingstr = "nothing"} = args;
        end"#;

        println!("=== Original Parser AST ===");
        match parse_program(program, CompileOptions::default()) {
            Ok(original) => {
                println!("{:#?}", original.stmts);
            }
            Err(e) => println!("Original parser error: {:?}", e),
        }

        println!("\n=== CST Parser AST ===");
        match parse_program_cst(program, CompileOptions::default()) {
            Ok(cst) => {
                println!("{:#?}", cst.stmts);
            }
            Err(e) => println!("CST parser error: {:?}", e),
        }

        // This test is just for debugging - don't fail
        assert!(true);
    }

    #[test]
    fn debug_postfix_index_parsing() {
        use crate::parsers::parse::parse_program;

        let test_cases = vec![
            "x[1];",     // index_single
            "x[1..5];",  // index_range
            "obj.prop;", // prop
            "x = 5;",    // assign
        ];

        for source in test_cases {
            println!("Debugging expression: {}", source);

            // Parse with original parser
            println!("\n=== Original Parser ===");
            let original_result = parse_program(source, CompileOptions::default());

            // Parse with CST parser
            println!("=== CST Parser ===");
            let cst_result = parse_program_cst(source, CompileOptions::default());

            match (original_result, cst_result) {
                (Ok(orig), Ok(cst)) => {
                    if let (Some(orig_stmt), Some(cst_stmt)) =
                        (orig.stmts.first(), cst.stmts.first())
                    {
                        if let (StmtNode::Expr(orig_expr), StmtNode::Expr(cst_expr)) =
                            (&orig_stmt.node, &cst_stmt.node)
                        {
                            println!("Original: {:?}", orig_expr);
                            println!("CST:      {:?}", cst_expr);

                            // Check if they're structurally the same
                            let same_type = std::mem::discriminant(orig_expr)
                                == std::mem::discriminant(cst_expr);
                            println!("Match: {}", if same_type { "✅" } else { "❌" });
                        }
                    }
                }
                (Err(orig_err), _) => println!("Original parser error: {:?}", orig_err),
                (_, Err(cst_err)) => println!("CST parser error: {:?}", cst_err),
            }
            println!("────────────────────");
        }
    }

    #[test]
    fn test_debug_conditional_cst() {
        use crate::cst::PestToCSTConverter;
        use crate::parsers::parse::moo::{MooParser, Rule};
        use pest::Parser;

        let source = "a = (1 == 2 ? 3 | 4);";
        let pairs = MooParser::parse(Rule::program, source).expect("Failed to parse");
        let converter = PestToCSTConverter::new(source.to_string());
        let cst = converter
            .convert_program(pairs.into_iter().next().unwrap())
            .expect("Failed to convert to CST");

        println!("CST structure for '{}':", source);
        println!("{}", cst.pretty_print(0));
    }

    #[test]
    fn test_debug_scatter_cst() {
        use crate::cst::PestToCSTConverter;
        use crate::parsers::parse::moo::{MooParser, Rule};
        use pest::Parser;

        let source = "{a, b, c} = args;";
        let pairs = MooParser::parse(Rule::program, source).expect("Failed to parse");
        let converter = PestToCSTConverter::new(source.to_string());
        let cst = converter
            .convert_program(pairs.into_iter().next().unwrap())
            .expect("Failed to convert to CST");

        println!("CST structure for '{}':", source);
        println!("{}", cst.pretty_print(0));

        // Test parsing with our parser
        let result = parse_program_cst(source, CompileOptions::default());
        match result {
            Ok(parse_result) => {
                println!("\nParsed successfully:");
                println!("Variables: {:?}", parse_result.names);
                for (i, stmt) in parse_result.stmts.iter().enumerate() {
                    println!("Statement {}: {:?}", i, stmt.node);
                }
            }
            Err(e) => {
                println!("\nParsing failed: {:?}", e);
            }
        }
    }

    #[test]
    fn test_parse_cst_to_parse_conversion() {
        let source = "{a, @rest} = get_list();";
        let parse_cst = parse_program_cst(source, CompileOptions::default()).unwrap();

        // Test From trait conversion
        let parse: Parse = parse_cst.into();

        // Verify the conversion worked by checking the AST
        assert_eq!(parse.stmts.len(), 1);

        // Test that we can unparse the converted result
        let unparsed = crate::unparse(&parse).unwrap();
        assert_eq!(unparsed[0], "{a, @rest} = get_list();");
    }
}
