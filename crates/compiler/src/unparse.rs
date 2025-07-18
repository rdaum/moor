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
use crate::parsers::parse::Parse;
use crate::precedence::get_precedence;
use base64::{Engine, engine::general_purpose};
use moor_common::util::quote_str;
use moor_var::program::names::Variable;
use moor_var::program::opcode::ScatterLabel;
use moor_var::{Obj, Sequence, Symbol, Var, Variant};
use std::collections::HashMap;

/// This could probably be combined with the structure for Parse.
#[derive(Debug)]
struct Unparse<'a> {
    tree: &'a Parse,
}

impl Expr {
    /// Returns the precedence of the operator. The higher the return value the higher the precedent.
    fn precedence(&self) -> u8 {
        get_precedence(self)
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

        if let Some(s) = var.as_string() {
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
                    buffer.push_str(format!("({value})").as_str());
                }
                Ok(buffer)
            }
            Expr::Value(var) => Ok(self.unparse_var(var, false)),
            Expr::TypeConstant(vt) => Ok(vt.to_literal().to_string()),
            Expr::Id(id) => Ok(self.unparse_variable(id).to_string()),
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
                match function {
                    crate::ast::CallTarget::Builtin(symbol) => {
                        buffer.push_str(&symbol.as_arc_string());
                    }
                    crate::ast::CallTarget::Expr(expr) => {
                        buffer.push_str(self.unparse_expr(expr)?.as_str());
                    }
                }
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
                Ok(format!("{left}[{right}]"))
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
                // TODO: this is currently broken and will unparse all locals as lets, even when
                //   they are re-assigning to an existing declared variable.
                let is_local = vars.iter().any(|var| var.id.scope_id != 0);
                let is_const = vars
                    .iter()
                    .any(|var| self.tree.variables.decl_for(&var.id).constant);
                if is_local && is_const {
                    buffer.push_str("const ");
                } else if is_local {
                    buffer.push_str("let ");
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
                    let name = self.unparse_variable(&var.id);
                    buffer.push_str(&name.as_arc_string());
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
            Expr::Decl { id, is_const, expr } => {
                let prefix = if *is_const { "const " } else { "let " };
                let var_name = self.unparse_variable(id);
                match expr {
                    Some(e) => Ok(format!(
                        "{}{} = {}",
                        prefix,
                        var_name,
                        self.unparse_expr(e)?
                    )),
                    None => Ok(format!("{prefix}{var_name}")),
                }
            }
            Expr::Flyweight(delegate, slots, contents) => {
                // "< #1, [ slot -> value, ...], {1, 2, 3} >"
                let mut buffer = String::new();
                buffer.push('<');
                buffer.push_str(self.unparse_expr(delegate)?.as_str());
                if !slots.is_empty() {
                    buffer.push_str(", [");
                    for (i, (slot, value)) in slots.iter().enumerate() {
                        buffer.push_str(&slot.as_arc_string());
                        buffer.push_str(" -> ");
                        buffer.push_str(self.unparse_expr(value)?.as_str());
                        if i + 1 < slots.len() {
                            buffer.push_str(", ");
                        }
                    }
                    buffer.push(']');
                }
                if let Some(contents_expr) = contents {
                    buffer.push_str(", ");
                    buffer.push_str(self.unparse_expr(contents_expr)?.as_str());
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
                let name = self.unparse_variable(variable);
                buffer.push_str(&name.as_arc_string());
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
                let name = self.unparse_variable(variable);
                buffer.push_str(&name.as_arc_string());
                buffer.push_str(" in (");
                buffer.push_str(&self.unparse_expr(list)?);
                buffer.push_str(") }");
                Ok(buffer)
            }
            Expr::Lambda { params, body, .. } => {
                // Lambda syntax: {param1, ?param2, @param3} => expr
                let mut buffer = String::new();
                buffer.push('{');

                let len = params.len();
                for (i, param) in params.iter().enumerate() {
                    match param.kind {
                        ast::ScatterKind::Required => {
                            // No prefix for required parameters
                        }
                        ast::ScatterKind::Optional => {
                            buffer.push('?');
                        }
                        ast::ScatterKind::Rest => {
                            buffer.push('@');
                        }
                    }
                    let name = self.unparse_variable(&param.id);
                    buffer.push_str(&name.as_arc_string());
                    if let Some(expr) = &param.expr {
                        buffer.push_str(" = ");
                        buffer.push_str(self.unparse_expr(expr)?.as_str());
                    }
                    if i + 1 < len {
                        buffer.push_str(", ");
                    }
                }

                buffer.push_str("} => ");

                // Handle different types of lambda bodies
                match &body.node {
                    // Expression lambda: return expr; → just show the expr
                    StmtNode::Expr(Expr::Return(Some(expr))) => {
                        buffer.push_str(&self.unparse_expr(expr)?);
                    }
                    // Statement lambda: show the full statement (like begin/end blocks)
                    _ => {
                        let stmt_lines = self.unparse_stmt(body, 0)?;
                        buffer.push_str(&stmt_lines.join("\n"));
                    }
                }
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
                stmt_lines.push(format!("{indent_frag}if ({cond_frag})"));
                stmt_lines.append(&mut stmt_frag);
                for arm in arms.iter().skip(1) {
                    let cond_frag = self.unparse_expr(&arm.condition)?;
                    let mut stmt_frag =
                        self.unparse_stmts(&arm.statements, indent + INDENT_LEVEL)?;
                    stmt_lines.push(format!("{indent_frag}elseif ({cond_frag})"));
                    stmt_lines.append(&mut stmt_frag);
                }
                if let Some(otherwise) = otherwise {
                    let mut stmt_frag =
                        self.unparse_stmts(&otherwise.statements, indent + INDENT_LEVEL)?;
                    stmt_lines.push(format!("{indent_frag}else"));
                    stmt_lines.append(&mut stmt_frag);
                }
                stmt_lines.push(format!("{indent_frag}endif"));
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

                let v_sym = self.unparse_variable(value_binding);
                let idx_clause = match key_binding {
                    None => v_sym.to_string(),
                    Some(key_binding) => {
                        let k_sym = self.unparse_variable(key_binding);
                        format!("{v_sym}, {k_sym}")
                    }
                };
                stmt_lines.push(format!("{indent_frag}for {idx_clause} in ({expr_frag})"));
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{indent_frag}endfor"));
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
                let name = self.unparse_variable(id);

                stmt_lines.push(format!(
                    "{indent_frag}for {name} in [{from_frag}..{to_frag}]"
                ));
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{indent_frag}endfor"));
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
                    let id = self.unparse_variable(id);

                    base_str.push_str(&id.as_arc_string());
                }
                stmt_lines.push(format!("{base_str}({cond_frag})"));
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{indent_frag}endwhile"));
                Ok(stmt_lines)
            }
            StmtNode::Fork { id, time, body } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let delay_frag = self.unparse_expr(time)?;
                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;
                let mut base_str = format!("{indent_frag}fork");
                if let Some(id) = id {
                    base_str.push(' ');
                    let id = self.unparse_variable(id);

                    base_str.push_str(&id.as_arc_string());
                }
                stmt_lines.push(format!("{base_str} ({delay_frag})"));
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{indent_frag}endfork"));
                Ok(stmt_lines)
            }
            StmtNode::TryExcept {
                body,
                excepts,
                environment_width: _,
            } => {
                let mut stmt_lines = Vec::with_capacity(body.len() + 3);

                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;
                stmt_lines.push(format!("{indent_frag}try"));
                stmt_lines.append(&mut stmt_frag);
                for except in excepts {
                    let mut stmt_frag =
                        self.unparse_stmts(&except.statements, indent + INDENT_LEVEL)?;
                    let mut base_str = "except ".to_string();
                    if let Some(id) = &except.id {
                        let id = self.unparse_variable(id);
                        base_str.push_str(&id.as_arc_string());
                        base_str.push(' ');
                    }
                    let catch_codes = self.unparse_catch_codes(&except.codes)?.to_uppercase();
                    base_str.push_str(format!("({catch_codes})").as_str());
                    stmt_lines.push(format!("{indent_frag}{base_str}"));
                    stmt_lines.append(&mut stmt_frag);
                }
                stmt_lines.push(format!("{indent_frag}endtry"));
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
                stmt_lines.push(format!("{indent_frag}endtry"));
                Ok(stmt_lines)
            }
            StmtNode::Break { exit } => {
                let mut base_str = format!("{indent_frag}break");
                if let Some(exit) = &exit {
                    base_str.push(' ');
                    let exit = self.unparse_variable(exit);
                    base_str.push_str(&exit.as_arc_string());
                }
                base_str.push(';');
                Ok(vec![base_str])
            }
            StmtNode::Continue { exit } => {
                let mut base_str = format!("{indent_frag}continue");
                if let Some(exit) = &exit {
                    base_str.push(' ');
                    let exit = self.unparse_variable(exit);

                    base_str.push_str(&exit.as_arc_string());
                }
                base_str.push(';');
                Ok(vec![base_str])
            }
            StmtNode::Expr(Expr::Assign { left, right }) => {
                let left_frag = match left.as_ref() {
                    Expr::Id(id) => {
                        let suffix = self.unparse_variable(id);
                        suffix.to_string()
                    }
                    _ => self.unparse_expr(left)?,
                };
                let right_frag = self.unparse_expr(right)?;
                Ok(vec![format!(
                    "{}{} = {};",
                    indent_frag, left_frag, right_frag
                )])
            }
            StmtNode::Expr(Expr::Decl { id, is_const, expr }) => {
                let prefix = if *is_const { "const " } else { "let " };
                let var_name = self.unparse_variable(id);
                match expr {
                    Some(e) => Ok(vec![format!(
                        "{}{}{} = {};",
                        indent_frag,
                        prefix,
                        var_name,
                        self.unparse_expr(e)?
                    )]),
                    None => Ok(vec![format!("{}{}{};", indent_frag, prefix, var_name)]),
                }
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
                stmt_lines.push(format!("{indent_frag}begin"));
                let mut stmt_frag = self.unparse_stmts(body, indent + INDENT_LEVEL)?;
                stmt_lines.append(&mut stmt_frag);
                stmt_lines.push(format!("{indent_frag}end"));
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

    fn unparse_variable(&self, variable: &Variable) -> Symbol {
        self.tree
            .variables
            .variables
            .iter()
            .find(|d| d.identifier.eq(variable))
            .unwrap()
            .identifier
            .to_symbol()
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
            format!("{oid}")
        }
        Variant::Bool(b) => {
            format!("{b}")
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
                    result.push_str(&k.as_arc_string());
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
            format!("'{}", s.as_arc_string())
        }
        Variant::Binary(b) => {
            let encoded = general_purpose::URL_SAFE.encode(b.as_bytes());
            format!("b\"{encoded}\"")
        }
        Variant::Lambda(l) => {
            use crate::decompile;
            use moor_var::program::opcode::ScatterLabel;

            // Build parameter list with proper names and syntax
            let param_strings: Vec<String> =
                l.0.params
                    .labels
                    .iter()
                    .map(|label| match label {
                        ScatterLabel::Required(name) => {
                            // Find the variable in lambda body's var_names and get its symbol
                            if let Some(var) = l.0.body.var_names().find_variable(name) {
                                var.to_symbol().as_arc_string().to_string()
                            } else {
                                format!("param_{}", name.0) // Fallback if name not found
                            }
                        }
                        ScatterLabel::Optional(name, _) => {
                            if let Some(var) = l.0.body.var_names().find_variable(name) {
                                format!("?{}", var.to_symbol().as_arc_string())
                            } else {
                                format!("?param_{}", name.0)
                            }
                        }
                        ScatterLabel::Rest(name) => {
                            if let Some(var) = l.0.body.var_names().find_variable(name) {
                                format!("@{}", var.to_symbol().as_arc_string())
                            } else {
                                format!("@param_{}", name.0)
                            }
                        }
                    })
                    .collect();
            let param_str = param_strings.join(", ");

            // Just manually construct the lambda syntax - simpler than reconstructing AST
            let decompiled_tree = decompile::program_to_tree(&l.0.body).unwrap();
            let lambda_body = &decompiled_tree.stmts[0];

            let temp_unparse = Unparse::new(&decompiled_tree);
            let body_str = match &lambda_body.node {
                // Expression lambda: return expr; → just show the expr
                crate::ast::StmtNode::Expr(crate::ast::Expr::Return(Some(expr))) => {
                    temp_unparse.unparse_expr(expr).unwrap()
                }
                // Statement lambda: show the full statement
                _ => {
                    let stmt_lines = temp_unparse.unparse_stmt(lambda_body, 0).unwrap();
                    stmt_lines.join("\n")
                }
            };

            // Build metadata string for captured environment and self-reference
            let mut metadata_parts = vec![];

            if !l.0.captured_env.is_empty() {
                let mut captured_vars: Vec<String> = vec![];

                for (scope_depth, frame) in l.0.captured_env.iter().enumerate() {
                    for (var_offset, var_value) in frame.iter().enumerate() {
                        // Only include variables that are not None/v_none
                        if var_value.is_none() {
                            continue;
                        }

                        // Search for variable names in the lambda body's name table that match this scope and offset
                        let var_names = l.0.body.var_names();
                        let maybe_name = var_names
                            .names()
                            .iter()
                            .filter_map(|name| {
                                // Check if this name corresponds to our scope depth and offset
                                if name.1 as usize == scope_depth && name.0 as usize == var_offset {
                                    var_names.ident_for_name(name)
                                } else {
                                    None
                                }
                            })
                            .next();

                        match maybe_name {
                            Some(symbol) => {
                                // Include both variable name and value for clarity
                                captured_vars.push(format!(
                                    "{}: {}",
                                    symbol.as_arc_string(),
                                    to_literal(var_value)
                                ));
                            }
                            None => {
                                // Fall back to just the value if no name is found
                                captured_vars.push(to_literal(var_value));
                            }
                        }
                    }
                }

                if !captured_vars.is_empty() {
                    metadata_parts.push(format!("captured [{}]", captured_vars.join(", ")));
                }
            }

            if let Some(_self_var) = l.0.self_var {
                // For now, represent self-reference as a simple marker
                metadata_parts.push("self 1".to_string());
            }

            if metadata_parts.is_empty() {
                format!("{{{param_str}}} => {body_str}")
            } else {
                format!(
                    "{{{param_str}}} => {body_str} with {}",
                    metadata_parts.join(" ")
                )
            }
        }
    }
}

/// Like `to_literal_objsub` but formats lists whose literal form would exceed 80 characters
/// into multiple lines with indentation.
pub fn to_literal_objsub(v: &Var, name_subs: &HashMap<Obj, String>, indent_depth: usize) -> String {
    let f = |o: &Obj| {
        if let Some(name_sub) = name_subs.get(o) {
            name_sub.clone()
        } else {
            format!("{o}")
        }
    };
    let mut result = String::new();
    let indent_str = " ".repeat(indent_depth);
    let inner_indent_str = " ".repeat(indent_depth + INDENT_LEVEL);

    match v.variant() {
        Variant::List(l) => {
            // First, try to format on one line
            let mut single_line = String::new();
            single_line.push('{');
            for (i, v) in l.iter().enumerate() {
                if i > 0 {
                    single_line.push_str(", ");
                }
                single_line.push_str(
                    to_literal_objsub(&v, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                );
            }
            single_line.push('}');

            // If single line exceeds 80 characters, format multiline
            if single_line.len() > 80 {
                result.push('{');
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        result.push(',');
                    }
                    result.push('\n');
                    result.push_str(&inner_indent_str);
                    result.push_str(
                        to_literal_objsub(&v, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                    );
                }
                result.push('\n');
                result.push_str(&indent_str);
                result.push('}');
            } else {
                result = single_line;
            }
        }
        Variant::Map(m) => {
            // First, try to format on one line
            let mut single_line = String::new();
            single_line.push('[');
            for (i, (k, v)) in m.iter().enumerate() {
                if i > 0 {
                    single_line.push_str(", ");
                }
                single_line.push_str(
                    to_literal_objsub(&k, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                );
                single_line.push_str(" -> ");
                single_line.push_str(
                    to_literal_objsub(&v, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                );
            }
            single_line.push(']');

            // If single line exceeds 80 characters, format multiline
            if single_line.len() > 80 {
                result.push('[');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        result.push(',');
                    }
                    result.push('\n');
                    result.push_str(&inner_indent_str);
                    result.push_str(
                        to_literal_objsub(&k, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                    );
                    result.push_str(" -> ");
                    result.push_str(
                        to_literal_objsub(&v, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                    );
                }
                result.push('\n');
                result.push_str(&indent_str);
                result.push(']');
            } else {
                result = single_line;
            }
        }
        Variant::Flyweight(fl) => {
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
                    result.push_str(&k.as_arc_string());
                    result.push_str(" -> ");
                    result.push_str(
                        to_literal_objsub(v, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                    );
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
                    result.push_str(
                        to_literal_objsub(&v, name_subs, indent_depth + INDENT_LEVEL).as_str(),
                    );
                }
                result.push('}');
            }

            result.push('>');
        }
        Variant::Obj(oid) => {
            result.push_str(&f(oid));
        }
        Variant::Lambda(l) => {
            // Special objdef formatting for lambdas - needs to match the grammar:
            // lambda_captured = { "captured" ~ "[" ~ (captured_var_map ~ ("," ~ captured_var_map)*)? ~ "]" }
            // captured_var_map = { "{" ~ (captured_var_entry ~ ("," ~ captured_var_entry)*)? ~ "}" }
            // captured_var_entry = { ident ~ ":" ~ literal }

            // Build parameter string
            let param_str =
                l.0.params
                    .labels
                    .iter()
                    .map(|label| match label {
                        ScatterLabel::Required(name) => {
                            l.0.body
                                .var_names()
                                .ident_for_name(name)
                                .map(|s| s.as_arc_string().to_string())
                                .unwrap_or_else(|| "x".to_string())
                        }
                        ScatterLabel::Optional(name, _) => {
                            let var_name =
                                l.0.body
                                    .var_names()
                                    .ident_for_name(name)
                                    .map(|s| s.as_arc_string().to_string())
                                    .unwrap_or_else(|| "x".to_string());
                            format!("?{var_name}")
                        }
                        ScatterLabel::Rest(name) => {
                            let var_name =
                                l.0.body
                                    .var_names()
                                    .ident_for_name(name)
                                    .map(|s| s.as_arc_string().to_string())
                                    .unwrap_or_else(|| "x".to_string());
                            format!("@{var_name}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

            // Build body string - for objdef we need to reconstruct the lambda body
            // For now, use a simple placeholder that's valid MOO syntax
            let body_str = "1"; // Simple placeholder

            // Build metadata for objdef format
            let mut metadata_parts = vec![];

            if !l.0.captured_env.is_empty() {
                let mut scope_maps = vec![];

                for (scope_depth, frame) in l.0.captured_env.iter().enumerate() {
                    let mut captured_vars: Vec<String> = vec![];

                    for (var_offset, var_value) in frame.iter().enumerate() {
                        if var_value.is_none() {
                            continue;
                        }

                        let var_names = l.0.body.var_names();
                        let maybe_name = var_names
                            .names()
                            .iter()
                            .filter_map(|name| {
                                if name.1 as usize == scope_depth && name.0 as usize == var_offset {
                                    var_names.ident_for_name(name)
                                } else {
                                    None
                                }
                            })
                            .next();

                        if let Some(symbol) = maybe_name {
                            captured_vars.push(format!(
                                "{}: {}",
                                symbol.as_arc_string(),
                                to_literal_objsub(
                                    var_value,
                                    name_subs,
                                    indent_depth + INDENT_LEVEL
                                )
                            ));
                        }
                    }

                    if !captured_vars.is_empty() {
                        scope_maps.push(format!("{{{}}}", captured_vars.join(", ")));
                    }
                }

                if !scope_maps.is_empty() {
                    metadata_parts.push(format!("captured [{}]", scope_maps.join(", ")));
                }
            }

            if let Some(_self_var) = l.0.self_var {
                metadata_parts.push("self 1".to_string());
            }

            if metadata_parts.is_empty() {
                result.push_str(&format!("{{{param_str}}} => {body_str}"));
            } else {
                result.push_str(&format!(
                    "{{{param_str}}} => {body_str} with {}",
                    metadata_parts.join(" ")
                ));
            }
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
    basic = 5;
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
    #[test_case(r#"begin
  let a = 5;
  a = a + 3;
end"#; "variable declaration and reassignment")]
    #[test_case(r#"begin
  let {a, b} = {1, 2};
  a = 5;
  b = 2;
end"#; "scatter declaration and reassignment")]
    #[test_case(r#"begin
  let {a, ?b = 5, @c} = {1, 2, 3};
  b = 2;
  c = 1;
end"#; "complex scatter declaration with optional and rest")]
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
        let tree = crate::parsers::parse::parse_program(original, CompileOptions::default()).unwrap();
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

    #[test]
    fn test_lambda_unparse_simple() {
        let program = r#"return {x} => x + 1;"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_lambda_unparse_optional() {
        let program = r#"return {x, ?y} => x + y;"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_lambda_unparse_optional_with_default() {
        let program = r#"return {x, ?y = 5} => x + y;"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_lambda_unparse_rest() {
        let program = r#"return {x, @rest} => x + length(rest);"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_lambda_unparse_complex() {
        let program = r#"return {x, ?y = 5, @rest} => x + y + length(rest);"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }

    #[test]
    fn test_lambda_unparse_no_params() {
        let program = r#"return {} => 42;"#;
        let stripped = unindent(program);
        let result = parse_and_unparse(&stripped).unwrap();
        assert_eq!(stripped.trim(), result.trim());
    }
}
