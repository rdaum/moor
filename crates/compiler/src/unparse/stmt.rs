// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use super::Unparse;
use crate::{
    ast,
    ast::{Expr, Stmt, StmtNode},
    decompile::DecompileError,
};
use moor_var::{Symbol, program::names::Variable};

impl<'a> Unparse<'a> {
    /// Format lambda body statements inline (space-separated, trimmed).
    /// Used for inline lambda representation in expressions.
    pub(super) fn write_lambda_body_inline<W: std::fmt::Write>(
        &self,
        stmts: &[Stmt],
        writer: &mut W,
    ) -> Result<(), DecompileError> {
        for (i, stmt) in stmts.iter().enumerate() {
            let mut stmt_buf = String::new();
            self.unparse_stmt(stmt, &mut stmt_buf, 0)?;
            let trimmed = stmt_buf.trim();
            if trimmed.is_empty() {
                continue;
            }
            if i > 0 {
                write!(writer, " ")?;
            }
            write!(writer, "{trimmed}")?;
        }
        if !stmts.is_empty() {
            write!(writer, " ")?;
        }
        Ok(())
    }

    pub(super) fn unparse_stmt<W: std::fmt::Write>(
        &self,
        stmt: &Stmt,
        writer: &mut W,
        indent: usize,
    ) -> Result<(), DecompileError> {
        match &stmt.node {
            StmtNode::Cond { arms, otherwise } => {
                self.write_indent(indent, writer)?;
                write!(writer, "if (")?;
                self.write_expr(&arms[0].condition, writer)?;
                writeln!(writer, ")")?;
                self.unparse_stmts(&arms[0].statements, writer, indent + 1)?;

                for arm in arms.iter().skip(1) {
                    self.write_indent(indent, writer)?;
                    write!(writer, "elseif (")?;
                    self.write_expr(&arm.condition, writer)?;
                    writeln!(writer, ")")?;
                    self.unparse_stmts(&arm.statements, writer, indent + 1)?;
                }

                if let Some(otherwise) = otherwise {
                    self.write_indent(indent, writer)?;
                    writeln!(writer, "else")?;
                    self.unparse_stmts(&otherwise.statements, writer, indent + 1)?;
                }

                self.write_indent(indent, writer)?;
                writeln!(writer, "endif")?;
                Ok(())
            }
            StmtNode::ForList {
                value_binding,
                key_binding,
                expr,
                body,
                environment_width: _,
            } => {
                self.write_indent(indent, writer)?;
                write!(writer, "for {}", self.unparse_variable(value_binding))?;
                if let Some(key_binding) = key_binding {
                    write!(writer, ", {}", self.unparse_variable(key_binding))?;
                }
                write!(writer, " in (")?;
                self.write_expr(expr, writer)?;
                writeln!(writer, ")")?;
                self.unparse_stmts(body, writer, indent + 1)?;
                self.write_indent(indent, writer)?;
                writeln!(writer, "endfor")?;
                Ok(())
            }
            StmtNode::ForRange {
                id,
                from,
                to,
                body,
                environment_width: _,
            } => {
                let name = self.unparse_variable(id);
                self.write_indent(indent, writer)?;
                write!(writer, "for {name} in [")?;
                self.write_expr(from, writer)?;
                write!(writer, "..")?;
                self.write_expr(to, writer)?;
                writeln!(writer, "]")?;
                self.unparse_stmts(body, writer, indent + 1)?;
                self.write_indent(indent, writer)?;
                writeln!(writer, "endfor")?;
                Ok(())
            }
            StmtNode::While {
                id,
                condition,
                body,
                environment_width: _,
            } => {
                self.write_indent(indent, writer)?;
                write!(writer, "while ")?;
                if let Some(id) = id {
                    write!(writer, "{}", self.unparse_variable(id))?;
                }
                write!(writer, "(")?;
                self.write_expr(condition, writer)?;
                writeln!(writer, ")")?;
                self.unparse_stmts(body, writer, indent + 1)?;
                self.write_indent(indent, writer)?;
                writeln!(writer, "endwhile")?;
                Ok(())
            }
            StmtNode::Fork { id, time, body } => {
                self.write_indent(indent, writer)?;
                write!(writer, "fork")?;
                if let Some(id) = id {
                    write!(writer, " {}", self.unparse_variable(id))?;
                }
                write!(writer, " (")?;
                self.write_expr(time, writer)?;
                writeln!(writer, ")")?;
                self.unparse_stmts(body, writer, indent + 1)?;
                self.write_indent(indent, writer)?;
                writeln!(writer, "endfork")?;
                Ok(())
            }
            StmtNode::TryExcept {
                body,
                excepts,
                environment_width: _,
            } => {
                self.write_indent(indent, writer)?;
                writeln!(writer, "try")?;
                self.unparse_stmts(body, writer, indent + 1)?;

                for except in excepts {
                    self.write_indent(indent, writer)?;
                    write!(writer, "except ")?;
                    if let Some(id) = &except.id {
                        let id = self.unparse_variable(id);
                        write!(writer, "{} ", id.as_arc_str())?;
                    }
                    write!(writer, "(")?;
                    self.write_catch_codes(&except.codes, writer)?;
                    writeln!(writer, ")")?;
                    self.unparse_stmts(&except.statements, writer, indent + 1)?;
                }

                self.write_indent(indent, writer)?;
                writeln!(writer, "endtry")?;
                Ok(())
            }
            StmtNode::TryFinally {
                body,
                handler,
                environment_width: _,
            } => {
                self.write_indent(indent, writer)?;
                writeln!(writer, "try")?;
                self.unparse_stmts(body, writer, indent + 1)?;
                self.write_indent(indent, writer)?;
                writeln!(writer, "finally")?;
                self.unparse_stmts(handler, writer, indent + 1)?;
                self.write_indent(indent, writer)?;
                writeln!(writer, "endtry")?;
                Ok(())
            }
            StmtNode::Break { exit } => {
                self.write_indent(indent, writer)?;
                write!(writer, "break")?;
                if let Some(exit) = &exit {
                    let exit_name = self.unparse_variable(exit);
                    write!(writer, " {}", exit_name.as_arc_str())?;
                }
                writeln!(writer, ";")?;
                Ok(())
            }
            StmtNode::Continue { exit } => {
                self.write_indent(indent, writer)?;
                write!(writer, "continue")?;
                if let Some(exit) = &exit {
                    let exit_name = self.unparse_variable(exit);
                    write!(writer, " {}", exit_name.as_arc_str())?;
                }
                writeln!(writer, ";")?;
                Ok(())
            }
            StmtNode::Expr(Expr::Assign { left, right }) => {
                let Expr::Id(var) = left.as_ref() else {
                    self.write_indent(indent, writer)?;
                    self.write_expr(left, writer)?;
                    write!(writer, " = ")?;
                    self.write_expr(right, writer)?;
                    writeln!(writer, ";")?;
                    return Ok(());
                };

                let Expr::Lambda {
                    params,
                    body,
                    self_name: Some(name),
                } = right.as_ref()
                else {
                    let var_name = self.unparse_variable(var);
                    self.write_indent(indent, writer)?;
                    write!(writer, "{var_name} = ")?;
                    self.write_expr(right, writer)?;
                    writeln!(writer, ";")?;
                    return Ok(());
                };

                let var_name = self.unparse_variable(var);
                let name_str = self.unparse_variable(name);

                if var_name != name_str {
                    self.write_indent(indent, writer)?;
                    write!(writer, "{var_name} = ")?;
                    self.write_expr(right, writer)?;
                    writeln!(writer, ";")?;
                    return Ok(());
                }

                self.unparse_named_function(params, body, &name_str, writer, indent)
            }
            StmtNode::Expr(Expr::Decl { id, is_const, expr }) => {
                let Some(expr) = expr.as_ref() else {
                    let prefix = if *is_const { "const " } else { "let " };
                    let var_name = self.unparse_variable(id);
                    self.write_indent(indent, writer)?;
                    writeln!(writer, "{prefix}{var_name};")?;
                    return Ok(());
                };

                let Expr::Lambda {
                    params,
                    body,
                    self_name: Some(name),
                } = expr.as_ref()
                else {
                    let prefix = if *is_const { "const " } else { "let " };
                    let var_name = self.unparse_variable(id);
                    self.write_indent(indent, writer)?;
                    write!(writer, "{prefix}{var_name} = ")?;
                    self.write_expr(expr, writer)?;
                    writeln!(writer, ";")?;
                    return Ok(());
                };

                let var_name = self.unparse_variable(id);
                let name_str = self.unparse_variable(name);

                if var_name != name_str {
                    let prefix = if *is_const { "const " } else { "let " };
                    self.write_indent(indent, writer)?;
                    write!(writer, "{prefix}{var_name} = ")?;
                    self.write_expr(expr, writer)?;
                    writeln!(writer, ";")?;
                    return Ok(());
                }

                self.unparse_named_function(params, body, &name_str, writer, indent)
            }
            StmtNode::Expr(expr) => {
                self.write_indent(indent, writer)?;
                self.write_expr(expr, writer)?;
                writeln!(writer, ";")?;
                Ok(())
            }
            StmtNode::Scope { num_bindings, body } => {
                self.write_indent(indent, writer)?;
                writeln!(writer, "begin")?;
                let mut remaining_bindings = *num_bindings;
                for stmt in body {
                    if let StmtNode::Expr(Expr::Scatter(items, right)) = &stmt.node
                        && remaining_bindings > 0
                    {
                        let decl_prefix = if items
                            .iter()
                            .all(|item| self.tree.variables.decl_for(&item.id).constant)
                        {
                            "const "
                        } else {
                            "let "
                        };
                        self.write_indent(indent + 1, writer)?;
                        write!(writer, "{decl_prefix}{{")?;
                        self.write_scatter_items(items, writer)?;
                        write!(writer, "}} = ")?;
                        self.write_expr(right, writer)?;
                        writeln!(writer, ";")?;
                        remaining_bindings = remaining_bindings.saturating_sub(items.len());
                        continue;
                    }
                    self.unparse_stmt(stmt, writer, indent + 1)?;
                }
                self.write_indent(indent, writer)?;
                writeln!(writer, "end")?;
                Ok(())
            }
        }
    }

    pub(super) fn unparse_stmts<W: std::fmt::Write>(
        &self,
        stms: &[Stmt],
        writer: &mut W,
        indent: usize,
    ) -> Result<(), DecompileError> {
        for stmt in stms {
            self.unparse_stmt(stmt, writer, indent)?;
        }
        Ok(())
    }

    pub(super) fn unparse_variable(&self, variable: &Variable) -> Symbol {
        self.tree
            .variables
            .variables
            .iter()
            .find(|d| d.identifier.eq(variable))
            .unwrap()
            .identifier
            .to_symbol()
    }

    pub(super) fn unparse_named_function<W: std::fmt::Write>(
        &self,
        params: &[ast::ScatterItem],
        body: &Stmt,
        name: &Symbol,
        writer: &mut W,
        indent: usize,
    ) -> Result<(), DecompileError> {
        self.write_indent(indent, writer)?;
        write!(writer, "fn {name}(")?;
        self.write_lambda_params(params, writer)?;
        writeln!(writer, ")")?;

        match &body.node {
            StmtNode::Scope {
                body: scope_body, ..
            } => self.unparse_stmts(scope_body, writer, indent + 1)?,
            _ => self.unparse_stmt(body, writer, indent + 1)?,
        }

        self.write_indent(indent, writer)?;
        writeln!(writer, "endfn")?;
        Ok(())
    }
}
