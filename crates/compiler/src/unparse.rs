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

use crate::ast;
use crate::ast::{Expr, Stmt, StmtNode};
use crate::decompile::DecompileError;
use crate::parse::Parse;
use moor_common::program::names::{Name, UnboundName};
use moor_common::util::quote_str;
use moor_var::{Obj, Sequence, Var, Variant};
use std::collections::HashMap;

/// This could probably be combined with the structure for Parse.
#[derive(Debug)]
struct Unparse<'a> {
    tree: &'a Parse,
}

impl Expr {
    /// Returns the precedence of the operator. The higher the return value the higher the precedent.
    fn precedence(&self) -> u8 {
        // The table here is in reverse order from the return argument because the numbers are based
        // directly on http://en.cppreference.com/w/cpp/language/operator_precedence
        // Should be kept in sync with the pratt parser in `parse.rs`
        // Starting from lowest to highest precedence...
        // TODO: drive Pratt and this from one common precedence table.
        let cpp_ref_prep = match self {
            Expr::Scatter(_, _) | Expr::Assign { .. } => 14,
            Expr::Cond { .. } => 13,
            Expr::Or(_, _) => 12,
            Expr::And(_, _) => 11,
            Expr::Binary(op, _, _) => match op {
                ast::BinaryOp::Eq => 7,
                ast::BinaryOp::NEq => 7,
                ast::BinaryOp::Gt => 6,
                ast::BinaryOp::GtE => 6,
                ast::BinaryOp::Lt => 6,
                ast::BinaryOp::LtE => 6,
                ast::BinaryOp::In => 6,

                ast::BinaryOp::Add => 4,
                ast::BinaryOp::Sub => 4,

                ast::BinaryOp::Mul => 3,
                ast::BinaryOp::Div => 3,
                ast::BinaryOp::Mod => 3,

                ast::BinaryOp::Exp => 2,
            },

            Expr::Unary(_, _) => 1,

            Expr::Prop { .. } => 1,
            Expr::Verb { .. } => 1,
            Expr::Range { .. } => 1,
            Expr::ComprehendRange { .. } => 1,
            Expr::ComprehendList { .. } => 1,
            Expr::Index(_, _) => 2,

            Expr::Value(_) => 1,
            Expr::Error(_, _) => 1,
            Expr::Id(_) => 1,
            Expr::TypeConstant(_) => 1,
            Expr::List(_) => 1,
            Expr::Map(_) => 1,
            Expr::Flyweight(..) => 1,
            Expr::Pass { .. } => 1,
            Expr::Call { .. } => 1,
            Expr::Length => 1,
            Expr::Return(_) => 1,
            Expr::TryCatch { .. } => 1,
        };
        15 - cpp_ref_prep
    }
}

const INDENT_LEVEL: usize = 2;

impl<'a> Unparse<'a> {
    fn new(tree: &'a Parse) -> Self {
        Self { tree }
    }

    fn unparse_arg(&self, arg: &ast::Arg) -> Result<String, DecompileError> {
        match arg {
            ast::Arg::Normal(expr) => Ok(self.unparse_expr(expr).unwrap()),
            ast::Arg::Splice(expr) => Ok(format!("@{}", self.unparse_expr(expr).unwrap())),
        }
    }

    fn unparse_args(&self, args: &[ast::Arg]) -> Result<String, DecompileError> {
        Ok(args
            .iter()
            .map(|arg| self.unparse_arg(arg).unwrap())
            .collect::<Vec<String>>()
            .join(", "))
    }

    fn unparse_catch_codes(&self, codes: &ast::CatchCodes) -> Result<String, DecompileError> {
        match codes {
            ast::CatchCodes::Codes(codes) => self.unparse_args(codes),
            ast::CatchCodes::Any => Ok(String::from("ANY")),
        }
    }

    fn unparse_var(&self, var: &moor_var::Var, aggressive: bool) -> String {
        if !aggressive {
            return to_literal(var);
        }

        if let Variant::Str(s) = var.variant() {
            let s = s.as_str();

            // If the string contains anything that isn't alphanumeric and _, it's
            // not a valid ident and needs to be quoted. Likewise if it begins with a non-alpha/underscore
            let needs_quotes = s.chars().any(|c| !c.is_alphanumeric() && c != '_')
                || (s.chars().next().unwrap().is_numeric() && !s.starts_with('_'));

            if !needs_quotes {
                s.to_string()
            } else {
                format!("({})", quote_str(s))
            }
        } else {
            to_literal(var)
        }
    }

    fn unparse_expr(&self, current_expr: &Expr) -> Result<String, DecompileError> {
        let brace_if_lower = |expr: &Expr| -> String {
            if expr.precedence() < current_expr.precedence() {
                format!("({})", self.unparse_expr(expr).unwrap())
            } else {
                self.unparse_expr(expr).unwrap()
            }
        };
        let brace_if_lower_eq = |expr: &Expr| -> String {
            if expr.precedence() <= current_expr.precedence() {
                format!("({})", self.unparse_expr(expr).unwrap())
            } else {
                self.unparse_expr(expr).unwrap()
            }
        };

        match current_expr {
            Expr::Assign { left, right } => {
                let left_frag = self.unparse_expr(left)?;
                let right_frag = self.unparse_expr(right)?;
                Ok(format!("{left_frag} = {right_frag}"))
            }
            Expr::Pass { args } => {
                let mut buffer = String::new();
                buffer.push_str("pass");
                buffer.push('(');
                buffer.push_str(self.unparse_args(args).unwrap().as_str());
                buffer.push(')');
                Ok(buffer)
            }
            Expr::Error(code, value) => {
                let mut buffer: String = (*code).into();
                if let Some(value) = value {
                    let value = self.unparse_expr(value).unwrap();
                    buffer.push_str(format!("({})", value).as_str());
                }
                Ok(buffer)
            }
            Expr::Value(var) => Ok(self.unparse_var(var, false)),
            Expr::TypeConstant(vt) => Ok(vt.to_literal().to_string()),
            Expr::Id(id) => Ok(self
                .tree
                .names
                .name_of(&self.unparse_name(id))
                .unwrap()
                .to_string()),
            Expr::Binary(op, left_expr, right_expr) => Ok(format!(
                "{} {} {}",
                brace_if_lower(left_expr),
                op,
                brace_if_lower_eq(right_expr)
            )),
            Expr::And(left, right) => Ok(format!(
                "{} && {}",
                brace_if_lower(left),
                brace_if_lower_eq(right)
            )),
            Expr::Or(left, right) => Ok(format!(
                "{} || {}",
                brace_if_lower(left),
                brace_if_lower_eq(right)
            )),
            Expr::Unary(op, expr) => Ok(format!("{}{}", op, brace_if_lower(expr))),
            Expr::Prop { location, property } => {
                let location = match (&**location, &**property) {
                    (Expr::Value(var), Expr::Value(_)) if var.is_sysobj() => String::from("$"),
                    _ => format!("{}.", brace_if_lower(location)),
                };
                let prop = match &**property {
                    Expr::Value(var) => self.unparse_var(var, true).to_string(),
                    _ => format!("({})", brace_if_lower(property)),
                };
                Ok(format!("{location}{prop}"))
            }
            Expr::Verb {
                location,
                verb,
                args,
            } => {
                let location = match (&**location, &**verb) {
                    (Expr::Value(var), Expr::Value(_)) if var.is_sysobj() => String::from("$"),
                    _ => format!("{}:", brace_if_lower(location)),
                };
                let verb = match &**verb {
                    Expr::Value(var) => self.unparse_var(var, true),
                    _ => format!("({})", brace_if_lower(verb)),
                };
                let mut buffer = String::new();
                buffer.push_str(format!("{location}{verb}").as_str());
                buffer.push('(');
                buffer.push_str(self.unparse_args(args)?.as_str());
                buffer.push(')');
                Ok(buffer)
            }
            Expr::Call { function, args } => {
                let mut buffer = String::new();
                buffer.push_str(function.as_str());
                buffer.push('(');
                buffer.push_str(self.unparse_args(args)?.as_str());
                buffer.push(')');
                Ok(buffer)
            }
            Expr::Range { base, from, to } => Ok(format!(
                "{}[{}..{}]",
                brace_if_lower(base),
                self.unparse_expr(from).unwrap(),
                self.unparse_expr(to).unwrap()
            )),
            Expr::Cond {
                condition,
                consequence,
                alternative,
            } => Ok(format!(
                "{} ? {} | {}",
                brace_if_lower_eq(condition),
                self.unparse_expr(consequence)?,
                brace_if_lower_eq(alternative)
            )),
            Expr::TryCatch {
                trye,
                codes,
                except,
            } => {
                let mut buffer = String::new();
                buffer.push('`');
                buffer.push_str(self.unparse_expr(trye)?.as_str());
                buffer.push_str(" ! ");
                buffer.push_str(self.unparse_catch_codes(codes)?.to_uppercase().as_str());
                if let Some(except) = except {
                    buffer.push_str(" => ");
                    buffer.push_str(self.unparse_expr(except)?.as_str());
                }
                buffer.push('\'');
                Ok(buffer)
            }
            Expr::Return(expr) => Ok(match expr {
                None => "return".to_string(),
                Some(e) => format!("return {}", self.unparse_expr(e)?),
            }),
            Expr::Index(lvalue, index) => {
                let left = brace_if_lower(lvalue);
                let right = self.unparse_expr(index).unwrap();
                Ok(format!("{}[{}]", left, right))
            }
            Expr::List(list) => {
                let mut buffer = String::new();
                buffer.push('{');
                buffer.push_str(self.unparse_args(list)?.as_str());
                buffer.push('}');
                Ok(buffer)
            }
            Expr::Map(pairs) => {
                let mut buffer = String::new();
                buffer.push('[');
                let len = pairs.len();
                for (i, (key, value)) in pairs.iter().enumerate() {
                    buffer.push_str(self.unparse_expr(key)?.as_str());
                    buffer.push_str(" -> ");
                    buffer.push_str(self.unparse_expr(value)?.as_str());
                    if i + 1 < len {
                        buffer.push_str(", ");
                    }
                }
                buffer.push(']');
                Ok(buffer)
            }
            Expr::Scatter(vars, expr) => {
                let mut buffer = String::new();
                // If the vars are non-global scope depth, prefix with 'let' or 'const' as appropriate.
                // Note the expectation is always that the vars exist at the same scope depth,
                //   as the let scatter syntax does not have granularity per-var.
                let is_local = vars.iter().any(|var| {
                    let bound_name = self.tree.names_mapping[&var.id];
                    let scope_depth = self.tree.names.depth_of(&bound_name).unwrap();
                    scope_depth > 0
                });
                let is_const = vars
                    .iter()
                    .any(|var| self.tree.unbound_names.decl_for(&var.id).constant);
                if is_local {
                    if is_const {
                        buffer.push_str("const ");
                    } else {
                        buffer.push_str("let ");
                    }
                }
                buffer.push('{');
                let len = vars.len();
                for (i, var) in vars.iter().enumerate() {
                    match var.kind {
                        ast::ScatterKind::Required => {}
                        ast::ScatterKind::Optional => {
                            buffer.push('?');
                        }
                        ast::ScatterKind::Rest => {
                            buffer.push('@');
                        }
                    }
                    let name = self.unparse_name(&var.id);
                    buffer.push_str(
                        self.tree
                            .names
                            .name_of(&name)
                            .ok_or(DecompileError::NameNotFound(name))?
                            .as_str(),
                    );
                    if let Some(expr) = &var.expr {
                        buffer.push_str(" = ");
                        buffer.push_str(self.unparse_expr(expr)?.as_str());
                    }
                    if i + 1 < len {
                        buffer.push_str(", ");
                    }
                }
                buffer.push_str("} = ");
                buffer.push_str(self.unparse_expr(expr)?.as_str());
                Ok(buffer)
            }
            Expr::Length => Ok(String::from("$")),
            Expr::Flyweight(delegate, slots, contents) => {
                // "< #1, [ slot -> value, ...], {1, 2, 3} >"
                let mut buffer = String::new();
                buffer.push('<');
                buffer.push_str(self.unparse_expr(delegate)?.as_str());
                if !slots.is_empty() {
                    buffer.push_str(", [");
                    for (i, (slot, value)) in slots.iter().enumerate() {
                        buffer.push_str(slot.as_str());
                        buffer.push_str(" -> ");
                        buffer.push_str(self.unparse_expr(value)?.as_str());
                        if i + 1 < slots.len() {
                            buffer.push_str(", ");
                        }
                    }
                    buffer.push(']');
                }
                if !contents.is_empty() {
                    buffer.push_str(", {");
                    for (i, value) in contents.iter().enumerate() {
                        buffer.push_str(self.unparse_arg(value)?.as_str());
                        if i + 1 < contents.len() {
                            buffer.push_str(", ");
                        }
                    }
                    buffer.push('}');
                }
                buffer.push('>');
                Ok(buffer)
            }
            Expr::ComprehendRange {
                variable,
                end_of_range_register: _,
                producer_expr,
                from,
                to,
            } => {
                // { <producer_expr> for <variable> in [<from>..<to>] }
                let mut buffer = String::new();
                buffer.push_str("{ ");
                buffer.push_str(&self.unparse_expr(producer_expr)?);
                buffer.push_str(" for ");
                let name = self
                    .tree
                    .names
                    .name_of(&self.unparse_name(variable))
                    .unwrap();
                buffer.push_str(name.as_str());
                buffer.push_str(" in [");
                buffer.push_str(&self.unparse_expr(from)?);
                buffer.push_str("..");
                buffer.push_str(&self.unparse_expr(to)?);
                buffer.push_str("] }");
                Ok(buffer)
            }
            Expr::ComprehendList {
                variable,
                position_register: _,
                producer_expr,
                list,
                ..
            } => {
                // { <producer_Expr> for <variable> in (list) }
                // { <producer_expr> for <variable> in [<from>..<to>] }
                let mut buffer = String::new();
                buffer.push_str("{ ");
                buffer.push_str(&self.unparse_expr(producer_expr)?);
                buffer.push_str(" for ");
                let name = self
                    .tree
                    .names
                    .name_of(&self.unparse_name(variable))
                    .unwrap();
                buffer.push_str(name.as_str());
                buffer.push_str(" in (");
                buffer.push_str(&self.unparse_expr(list)?);
                buffer.push_str(") }");
                Ok(buffer)
            }
        }
    }

    fn unparse_stmt(&self, stmt: &Stmt, indent: usize) -> Result<Vec<String>, DecompileError> {
        let indent_frag = " ".repeat(indent);
        // Statements should not end in a newline, but should be terminated with a semicolon.
        match &stmt.node {
            StmtNode::Cond { arms, otherwise } => {
                let mut stmt_lines = Vec::with_capacity(arms.len() + 2);
                let cond_frag = self.unparse_expr(&arms[0].condition)?;
                let mut stmt_frag =
                    self.unparse_stmts(&arms[0].statements, indent + INDENT_LEVEL)?;
                stmt_lines.push(format!("{}if ({})", indent_frag, cond_frag));
                stmt_lines.append(&mut stmt_frag);
                for arm in arms.iter().skip(1) {
                    let cond_frag = self.unparse_expr(&arm.condition)?;
                    let mut stmt_frag =
                        self.unparse_stmts(&arm.statements, indent + INDENT_LEVEL)?;
                    stmt_lines.push(format!("{}elseif ({})", indent_frag, cond_frag));
                    stmt_lines.append(&mut stmt_frag);
                }
                if let Some(otherwise) = otherwise {
                    let mut stmt_frag =
                        self.unparse_stmts(&otherwise.statements, indent + INDENT_LEVEL)?;
                    stmt_lines.push(format!("{}else", indent_frag));
                    stmt_lines.append(&mut stmt_frag);
                }
                stmt_lines.push(format!("{}endif", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::ForList {
                value_binding,
                key_binding,
                expr,
                body,
                environment_width: _,
            } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let expr_frag = self.unparse_expr(expr)?;
                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;

                let v_name = self.unparse_name(value_binding);
                let v_sym = self
                    .tree
                    .names
                    .name_of(&v_name)
                    .ok_or(DecompileError::NameNotFound(v_name))?;
                let idx_clause = match key_binding {
                    None => v_sym.to_string(),
                    Some(key_binding) => {
                        let k_name = self.unparse_name(key_binding);
                        let k_sym = self
                            .tree
                            .names
                            .name_of(&k_name)
                            .ok_or(DecompileError::NameNotFound(v_name))?;
                        format!("{}, {}", v_sym, k_sym)
                    }
                };
                stmt_lines.push(format!(
                    "{}for {idx_clause} in ({})",
                    indent_frag, expr_frag
                ));
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{}endfor", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::ForRange {
                id,
                from,
                to,
                body,
                environment_width: _,
            } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let from_frag = self.unparse_expr(from)?;
                let to_frag = self.unparse_expr(to)?;
                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;
                let name = self.unparse_name(id);

                stmt_lines.push(format!(
                    "{}for {} in [{}..{}]",
                    indent_frag,
                    self.tree
                        .names
                        .name_of(&name)
                        .ok_or(DecompileError::NameNotFound(name))?,
                    from_frag,
                    to_frag
                ));
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{}endfor", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::While {
                id,
                condition,
                body,
                environment_width: _,
            } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let cond_frag = self.unparse_expr(condition)?;
                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;

                let mut base_str = "while ".to_string();
                if let Some(id) = id {
                    let id = self.unparse_name(id);

                    base_str.push_str(
                        self.tree
                            .names
                            .name_of(&id)
                            .ok_or(DecompileError::NameNotFound(id))?
                            .as_str(),
                    );
                }
                stmt_lines.push(format!("{}({})", base_str, cond_frag));
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{}endwhile", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::Fork { id, time, body } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let delay_frag = self.unparse_expr(time)?;
                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;
                let mut base_str = format!("{}fork", indent_frag);
                if let Some(id) = id {
                    base_str.push(' ');
                    let id = self.unparse_name(id);

                    base_str.push_str(
                        self.tree
                            .names
                            .name_of(&id)
                            .ok_or(DecompileError::NameNotFound(id))?
                            .as_str(),
                    );
                }
                stmt_lines.push(format!("{} ({})", base_str, delay_frag));
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{}endfork", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::TryExcept {
                body,
                excepts,
                environment_width: _,
            } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;
                stmt_lines.push(format!("{}try", indent_frag));
                stmt_lines.append(&mut stmt_frag);
                for except in excepts {
                    let mut stmt_frag =
                        self.unparse_stmts(&except.statements, indent + INDENT_LEVEL)?;
                    let mut base_str = "except ".to_string();
                    if let Some(id) = &except.id {
                        let id = self.unparse_name(id);

                        base_str.push_str(
                            self.tree
                                .names
                                .name_of(&id)
                                .ok_or(DecompileError::NameNotFound(id))?
                                .as_str(),
                        );
                        base_str.push(' ');
                    }
                    let catch_codes = self.unparse_catch_codes(&except.codes)?.to_uppercase();
                    base_str.push_str(format!("({catch_codes})").as_str());
                    stmt_lines.push(format!("{indent_frag}{base_str}"));
                    stmt_lines.append(&mut stmt_frag);
                }
                stmt_lines.push(format!("{}endtry", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::TryFinally {
                body,
                handler,
                environment_width: _,
            } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;
                let mut handler_frag = self.unparse_stmts(handler, indent + INDENT_LEVEL)?;
                stmt_lines.push("try".to_string());
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push("finally".to_string());
                stmt_lines.append(&mut handler_frag);
                stmt_lines.push(format!("{}endtry", indent_frag));
                Ok(stmt_lines)
            }
            StmtNode::Break { exit } => {
                let mut base_str = format!("{}break", indent_frag);
                if let Some(exit) = &exit {
                    base_str.push(' ');
                    let exit = self.unparse_name(exit);

                    base_str.push_str(
                        self.tree
                            .names
                            .name_of(&exit)
                            .ok_or(DecompileError::NameNotFound(exit))?
                            .as_str(),
                    );
                }
                base_str.push(';');
                Ok(vec![base_str])
            }
            StmtNode::Continue { exit } => {
                let mut base_str = format!("{}continue", indent_frag);
                if let Some(exit) = &exit {
                    base_str.push(' ');
                    let exit = self.unparse_name(exit);

                    base_str.push_str(
                        self.tree
                            .names
                            .name_of(&exit)
                            .ok_or(DecompileError::NameNotFound(exit))?
                            .as_str(),
                    );
                }
                base_str.push(';');
                Ok(vec![base_str])
            }
            StmtNode::Expr(Expr::Assign { left, right }) => {
                let left_frag = match left.as_ref() {
                    Expr::Id(id) => {
                        // If this Id is in non-zero scope, we need to prefix with "let"
                        let bound_name = self.tree.names_mapping[id];
                        let scope_depth = self.tree.names.depth_of(&bound_name).unwrap();
                        let prefix = if scope_depth > 0 {
                            "let "
                        } else {
                            // TODO: could have 'global' prefix here when in a certain mode.
                            //   instead of having it implied.
                            ""
                        };
                        let suffix = self.tree.names.name_of(&self.unparse_name(id)).unwrap();
                        format!("{}{}", prefix, suffix)
                    }
                    _ => self.unparse_expr(left)?,
                };
                let right_frag = self.unparse_expr(right)?;
                Ok(vec![format!(
                    "{}{} = {};",
                    indent_frag, left_frag, right_frag
                )])
            }
            StmtNode::Expr(expr) => Ok(vec![format!(
                "{}{};",
                indent_frag,
                self.unparse_expr(expr)?
            )]),
            StmtNode::Scope {
                num_bindings: _,
                body,
            } => {
                // Begin/End
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);
                stmt_lines.push(format!("{}begin", indent_frag));
                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{}end", indent_frag));
                Ok(stmt_lines)
            }
        }
    }

    pub fn unparse_stmts(
        &self,
        stms: &[Stmt],
        indent: usize,
    ) -> Result<Vec<String>, DecompileError> {
        let mut results = vec![];
        for stmt in stms {
            results.append(&mut self.unparse_stmt(stmt, indent)?);
        }
        Ok(results)
    }

    fn unparse_name(&self, name: &UnboundName) -> Name {
        *self.tree.names_mapping.get(name).unwrap()
    }
}

pub fn unparse(tree: &Parse) -> Result<Vec<String>, DecompileError> {
    let unparse = Unparse::new(tree);
    unparse.unparse_stmts(&tree.stmts, 0)
}

/// Walk a syntax tree and annotate each statement with line number that corresponds to what would
/// have been generated by `unparse`
/// Used for generating line number spans in the bytecode.
pub fn annotate_line_numbers(start_line_no: usize, tree: &mut [Stmt]) -> usize {
    let mut line_no = start_line_no;
    for stmt in tree.iter_mut() {
        stmt.tree_line_no = line_no;
        match &mut stmt.node {
            StmtNode::Cond { arms, otherwise } => {
                // IF & ELSEIFS
                for arm in arms.iter_mut() {
                    // IF / ELSEIF line
                    line_no += 1;
                    // Walk arm.statements ...
                    line_no = annotate_line_numbers(line_no, &mut arm.statements);
                }
                if let Some(otherwise) = otherwise {
                    // ELSE line ...
                    line_no += 1;
                    // Walk otherwise ...
                    line_no = annotate_line_numbers(line_no, &mut otherwise.statements);
                }
                // ENDIF
                line_no += 1;
            }
            StmtNode::ForList { body, .. }
            | StmtNode::ForRange { body, .. }
            | StmtNode::While { body, .. }
            | StmtNode::Fork { body, .. } => {
                // FOR/WHILE/FORK
                line_no += 1;
                // Walk body ...
                line_no = annotate_line_numbers(line_no, body);
                // ENDFOR/ENDWHILE/ENDFORK
                line_no += 1;
            }
            StmtNode::Expr(_) | StmtNode::Break { .. } | StmtNode::Continue { .. } => {
                // All single-line statements.
                line_no += 1;
            }
            StmtNode::TryExcept {
                body,
                excepts,
                environment_width: _,
            } => {
                // TRY
                line_no += 1;
                // Walk body ...
                line_no = annotate_line_numbers(line_no, body);
                // Excepts
                for except in excepts {
                    // EXCEPT <...>
                    line_no += 1;
                    line_no = annotate_line_numbers(line_no, &mut except.statements);
                }
                // ENDTRY
                line_no += 1;
            }
            StmtNode::TryFinally {
                body,
                handler,
                environment_width: _,
            } => {
                // TRY
                line_no += 1;
                // Walk body ...
                line_no = annotate_line_numbers(line_no, body);
                // FINALLY
                line_no += 1;
                // Walk handler ...
                line_no = annotate_line_numbers(line_no, handler);
                // ENDTRY
                line_no += 1;
            }
            StmtNode::Scope {
                body,
                num_bindings: _,
            } => {
                // BEGIN
                line_no += 1;
                // Walk body ...
                line_no = annotate_line_numbers(line_no, body);
                // ENDLET
                line_no += 1;
            }
        }
    }
    line_no
}

/// Utility function to produce a MOO literal from a Var/Variant.
/// This is kept in `compiler` and not in `common` because it's specific to the MOO language, and
/// other languages could have different representations.
pub fn to_literal(v: &Var) -> String {
    match v.variant() {
        Variant::None => "None".to_string(),
        Variant::Obj(oid) => {
            format!("{}", oid)
        }
        Variant::Bool(b) => {
            format!("{}", b)
        }
        Variant::Int(i) => i.to_string(),
        Variant::Float(f) => {
            format!("{f:?}")
        }
        Variant::List(l) => {
            let mut result = String::new();
            result.push('{');
            for (i, v) in l.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(to_literal(&v).as_str());
            }
            result.push('}');
            result
        }
        Variant::Str(s) => quote_str(s.as_str()),
        Variant::Map(m) => {
            let mut result = String::new();
            result.push('[');
            for (i, (k, v)) in m.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(to_literal(&k).as_str());
                result.push_str(" -> ");
                result.push_str(to_literal(&v).as_str());
            }
            result.push(']');
            result
        }
        Variant::Err(e) => e.name().to_string().to_uppercase(),
        Variant::Flyweight(fl) => {
            // If sealed, just return <sealed flyweight>
            if fl.seal().is_some() {
                return "<sealed flyweight>".to_string();
            }

            // Syntax:
            // < delegate, [ s -> v, ... ], v, v, v ... >
            let mut result = String::new();
            result.push('<');
            result.push_str(fl.delegate().to_literal().as_str());
            if !fl.slots().is_empty() {
                result.push_str(", [");
                for (i, (k, v)) in fl.slots().iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(k.as_str());
                    result.push_str(" -> ");
                    result.push_str(to_literal(v).as_str());
                }
                result.push(']');
            }
            let v = fl.contents();
            if !v.is_empty() {
                result.push_str(", {");
                for (i, v) in v.iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(to_literal(&v).as_str());
                }
                result.push('}');
            }

            result.push('>');
            result
        }
        Variant::Sym(s) => {
            format!("'{}", s.as_str())
        }
    }
}

/// Like `to_literal` but performs a tree walk assembling the string out of calls to a designated
/// function as it recursively visits each element.
pub fn to_literal_objsub(v: &Var, name_subs: &HashMap<Obj, String>) -> String {
    let f = |o: &Obj| {
        if let Some(name_sub) = name_subs.get(o) {
            name_sub.clone()
        } else {
            format!("{}", o)
        }
    };
    let mut result = String::new();
    match v.variant() {
        Variant::List(l) => {
            result.push('{');
            for (i, v) in l.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(to_literal_objsub(&v, name_subs).as_str());
            }
            result.push('}');
        }
        Variant::Map(m) => {
            result.push('[');
            for (i, (k, v)) in m.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(to_literal_objsub(&k, name_subs).as_str());
                result.push_str(" -> ");
                result.push_str(to_literal_objsub(&v, name_subs).as_str());
            }
            result.push(']');
        }
        Variant::Flyweight(fl) => {
            // TODO: sealed flyweight in object dump...
            if fl.seal().is_some() {
                return "<sealed flyweight>".to_string();
            }

            // Syntax:
            // < delegate, [ s -> v, ... ], v, v, v ... >
            result.push('<');
            result.push_str(fl.delegate().to_literal().as_str());
            if !fl.slots().is_empty() {
                result.push_str(", [");
                for (i, (k, v)) in fl.slots().iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(k.as_str());
                    result.push_str(" -> ");
                    result.push_str(to_literal_objsub(v, name_subs).as_str());
                }
                result.push(']');
            }
            let v = fl.contents();
            if !v.is_empty() {
                result.push_str(", {");
                for (i, v) in v.iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(to_literal_objsub(&v, name_subs).as_str());
                }
                result.push('}');
            }

            result.push('>');
        }
        Variant::Obj(oid) => {
            result.push_str(&f(oid));
        }
        _ => {
            result.push_str(to_literal(v).as_str());
        }
    };
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CompileOptions;
    use crate::ast::assert_trees_match_recursive;

    use pretty_assertions::assert_eq;
    use test_case::test_case;
    use unindent::unindent;

    #[test_case("a = 1;\n"; "assignment")]
    #[test_case("a = 1 + 2;\n"; "assignment with expr")]
    #[test_case("1 + 2 * 3 + 4;\n"; "binops with same precident")]
    #[test_case("1 * 2 + 3;\n"; "binops with different precident")]
    #[test_case("1 * (2 + 3);\n"; "binops with mixed precident and parens")]
    #[test_case("return;\n"; "Empty return")]
    #[test_case("return 20;\n";"return with args")]
    #[test_case(r#"
  if (expression)
    statements;
  endif
  "#; "simple if")]
    #[test_case(r#"
  if (expr)
    1;
  elseif (expr)
    2;
  elseif (expr)
    3;
  else
    4;
  endif
  "#; "if elseif chain")]
    #[test_case("`x.y ! E_PROPNF, E_PERM => 17';\n"; "catch expression")]
    #[test_case("method(a, b, c);\n"; "call function")]
    #[test_case(r#"
  try
    statements;
    statements;
  except e (E_DIV)
    statements;
  except e (E_PERM)
    statements;
  endtry
  "#; "exception handling")]
    #[test_case(r#"
  try
    basic;
  finally
    finalize();
  endtry
  "#; "try finally")]
    #[test_case(r#"return "test";"#; "string literal")]
    #[test_case(r#"return "test \"test\"";"#; "string literal with escaped quote")]
    #[test_case(r#"return #1.test;"#; "property access")]
    #[test_case(r#"return #1:test(1, 2, 3);"#; "verb call")]
    #[test_case(r#"return #1:test();"#; "verb call no args")]
    #[test_case(r#"return $test(1);"#; "sysverb")]
    #[test_case(r#"return $options;"#; "sysprop")]
    #[test_case(r#"options = "test";
  return #0.(options);"#; "sysobj prop expr")]
    #[test_case(r#"{a, b, ?c, @d} = args;"#; "scatter assign")]
    #[test_case(r#"{?a = 5} = args;"#; "scatter assign optional expression argument")]
    #[test_case(r#"5;
           fork (5)
             1;
           endfork
           2;"#; "unlabelled fork decompile")]
    #[test_case(r#"5;
           fork tst (5)
             1;
           endfork
           2;"#; "labelled fork decompile")]
    #[test_case(r#"while (1)
             continue;
             break;
           endwhile"#; "continue decompile")]
    #[test_case(r#"this:("@listgag")();"#; "verb expr escaping @")]
    #[test_case(r#"this:("listgag()")();"#; "verb expr escaping brackets ")]
    #[test_case(r#"1 ^ 2;"#; "exponents")]
    #[test_case(r#"(a + b)[1];"#; "index precedence")]
    #[test_case(r#"a ? b | (c ? d | e);"#; "conditional precedence")]
    #[test_case(r#"1 ^ (2 + 3);"#; "exponent precedence")]
    #[test_case(r#"(1 + 2) ^ (2 + 3);"#; "exponent precedence 2")]
    #[test_case(r#"verb[1..5 - 1];"#; "range precedence")]
    #[test_case(r#"1 && ((a = 5) && 3);"#; "and/or precedence")]
    #[test_case(r#"n + 10 in a;"#; "in precedence")]
    pub fn compare_parse_roundtrip(original: &str) {
        let stripped = unindent(original);
        let result = parse_and_unparse(&stripped).unwrap();

        // Compare the stripped version of the original to the stripped version of the result, they
        // should end up identical.
        assert_eq!(stripped.trim(), result.trim());

        // Now parse both again, and verify that the complete ASTs match, ignoring the parser line
        // numbers, but validating everything else.
        let parsed_original =
            crate::parse::parse_program(&stripped, CompileOptions::default()).unwrap();
        let parsed_decompiled =
            crate::parse::parse_program(&result, CompileOptions::default()).unwrap();
        assert_trees_match_recursive(&parsed_original.stmts, &parsed_decompiled.stmts)
    }

    #[test]
    pub fn unparse_complex_function() {
        let body = r#"
    brief = args && args[1];
    player:tell(this:namec_for_look_self(brief));
    things = this:visible_of(setremove(this:contents(), player));
    integrate = {};
    try
      if (this.integration_enabled)
        for i in (things)
          if (this:ok_to_integrate(i) && (!brief || !is_player(i)))
            integrate = {@integrate, i};
            things = setremove(things, i);
          endif
        endfor
        "for i in (this:obvious_exits(player))";
        for i in (this:exits())
          if (this:ok_to_integrate(i))
            integrate = setadd(integrate, i);
            "changed so prevent exits from being integrated twice in the case of doors and the like";
          endif
        endfor
      endif
    except (E_INVARG)
      player:tell("Error in integration: ");
    endtry
    if (!brief)
      desc = this:description(integrate);
      if (desc)
        player:tell_lines(desc);
      else
        player:tell("You see nothing special.");
      endif
    endif
    "there's got to be a better way to do this, but.";
    if (topic = this:topic_msg())
      if (0)
        this.topic_sign:show_topic();
      else
        player:tell(this.topic_sign:integrate_room_msg());
      endif
    endif
    "this:tell_contents(things, this.ctype);";
    this:tell_contents(things);
    "#;
        let stripped = unindent(body);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_unparse_lexical_scope_block() {
        let program = r#"b = 3;
        begin
          let a = 5;
        end"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn regress_test() {
        let program = r#"n + 10 in a;"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_local_scatter() {
        let program = r#"begin
          let {things, ?nothingstr = "nothing", ?andstr = " and ", ?commastr = ", ", ?finalcommastr = ","} = args;
        end"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_local_const() {
        let program = r#"begin
          const {things, ?nothingstr = "nothing", ?andstr = " and ", ?commastr = ", ", ?finalcommastr = ","} = args;
        end"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_flyweight() {
        let program = r#"return <#1, [slot -> "123"], {1, 2, 3}>;"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn for_range_comprehension() {
        let program = r#"return { x * 2 for x in [1..3] };"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn for_list_comprehension() {
        let program = r#"return { x * 2 for x in ({1, 2, 3}) };"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn for_v_x_in_map() {
        let program = r#" for v, k in (["a" -> "b", "c" -> "d"])
        endfor"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn for_v_x_in_list() {
        let program = r#" for v, k in ({1, 2, 3})
        endfor"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    pub fn parse_and_unparse(original: &str) -> Result<String, DecompileError> {
        let tree = crate::parse::parse_program(original, CompileOptions::default()).unwrap();
        Ok(unparse(&tree)?.join("\n"))
    }

    #[test]
    fn test_unparse_empty_map_regression() {
        let program = r#"return [];"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_type_literals() {
        let progrma = r#"return {INT, STR, OBJ, LIST, MAP, SYM, FLYWEIGHT};"#;
        let stripped = unindent(progrma);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }
}
