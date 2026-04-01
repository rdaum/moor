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
    pub(super) fn unparse_lambda_body_inline(
        &self,
        stmts: &[Stmt],
    ) -> Result<String, DecompileError> {
        let mut parts = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            let mut stmt_buf = String::new();
            self.unparse_stmt(stmt, &mut stmt_buf, 0)?;
            parts.push(stmt_buf.trim().to_string());
        }
        Ok(if parts.is_empty() {
            String::new()
        } else {
            format!("{} ", parts.join(" "))
        })
    }

    pub(super) fn unparse_stmt<W: std::fmt::Write>(
        &self,
        stmt: &Stmt,
        writer: &mut W,
        indent: usize,
    ) -> Result<(), DecompileError> {
        let indent_str = if self.indent_width > 0 {
            " ".repeat(indent * self.indent_width)
        } else {
            String::new()
        };
        match &stmt.node {
            StmtNode::Cond { arms, otherwise } => {
                let cond_frag = self.unparse_expr(&arms[0].condition)?;
                writeln!(writer, "{indent_str}if ({cond_frag})")?;
                self.unparse_stmts(&arms[0].statements, writer, indent + 1)?;

                for arm in arms.iter().skip(1) {
                    let cond_frag = self.unparse_expr(&arm.condition)?;
                    writeln!(writer, "{indent_str}elseif ({cond_frag})")?;
                    self.unparse_stmts(&arm.statements, writer, indent + 1)?;
                }

                if let Some(otherwise) = otherwise {
                    writeln!(writer, "{indent_str}else")?;
                    self.unparse_stmts(&otherwise.statements, writer, indent + 1)?;
                }

                writeln!(writer, "{indent_str}endif")?;
                Ok(())
            }
            StmtNode::ForList {
                value_binding,
                key_binding,
                expr,
                body,
                environment_width: _,
            } => {
                let expr_frag = self.unparse_expr(expr)?;
                let v_sym = self.unparse_variable(value_binding);
                let idx_clause = match key_binding {
                    None => v_sym.to_string(),
                    Some(key_binding) => {
                        let k_sym = self.unparse_variable(key_binding);
                        format!("{v_sym}, {k_sym}")
                    }
                };
                writeln!(writer, "{indent_str}for {idx_clause} in ({expr_frag})")?;
                self.unparse_stmts(body, writer, indent + 1)?;
                writeln!(writer, "{indent_str}endfor")?;
                Ok(())
            }
            StmtNode::ForRange {
                id,
                from,
                to,
                body,
                environment_width: _,
            } => {
                let from_frag = self.unparse_expr(from)?;
                let to_frag = self.unparse_expr(to)?;
                let name = self.unparse_variable(id);

                writeln!(writer, "{indent_str}for {name} in [{from_frag}..{to_frag}]")?;
                self.unparse_stmts(body, writer, indent + 1)?;
                writeln!(writer, "{indent_str}endfor")?;
                Ok(())
            }
            StmtNode::While {
                id,
                condition,
                body,
                environment_width: _,
            } => {
                let cond_frag = self.unparse_expr(condition)?;

                let mut base_str = "while ".to_string();
                if let Some(id) = id {
                    let id = self.unparse_variable(id);
                    base_str.push_str(&id.as_arc_str());
                }
                writeln!(writer, "{indent_str}{base_str}({cond_frag})")?;
                self.unparse_stmts(body, writer, indent + 1)?;
                writeln!(writer, "{indent_str}endwhile")?;
                Ok(())
            }
            StmtNode::Fork { id, time, body } => {
                let delay_frag = self.unparse_expr(time)?;

                let mut base_str = "fork".to_string();
                if let Some(id) = id {
                    base_str.push(' ');
                    let id = self.unparse_variable(id);
                    base_str.push_str(&id.as_arc_str());
                }
                writeln!(writer, "{indent_str}{base_str} ({delay_frag})")?;
                self.unparse_stmts(body, writer, indent + 1)?;
                writeln!(writer, "{indent_str}endfork")?;
                Ok(())
            }
            StmtNode::TryExcept {
                body,
                excepts,
                environment_width: _,
            } => {
                writeln!(writer, "{indent_str}try")?;
                self.unparse_stmts(body, writer, indent + 1)?;

                for except in excepts {
                    let mut base_str = "except ".to_string();
                    if let Some(id) = &except.id {
                        let id = self.unparse_variable(id);
                        base_str.push_str(&id.as_arc_str());
                        base_str.push(' ');
                    }
                    let catch_codes = self.unparse_catch_codes(&except.codes)?.to_uppercase();
                    base_str.push_str(format!("({catch_codes})").as_str());
                    writeln!(writer, "{indent_str}{base_str}")?;
                    self.unparse_stmts(&except.statements, writer, indent + 1)?;
                }

                writeln!(writer, "{indent_str}endtry")?;
                Ok(())
            }
            StmtNode::TryFinally {
                body,
                handler,
                environment_width: _,
            } => {
                writeln!(writer, "{indent_str}try")?;
                self.unparse_stmts(body, writer, indent + 1)?;
                writeln!(writer, "{indent_str}finally")?;
                self.unparse_stmts(handler, writer, indent + 1)?;
                writeln!(writer, "{indent_str}endtry")?;
                Ok(())
            }
            StmtNode::Break { exit } => {
                write!(writer, "{indent_str}break")?;
                if let Some(exit) = &exit {
                    let exit_name = self.unparse_variable(exit);
                    write!(writer, " {}", exit_name.as_arc_str())?;
                }
                writeln!(writer, ";")?;
                Ok(())
            }
            StmtNode::Continue { exit } => {
                write!(writer, "{indent_str}continue")?;
                if let Some(exit) = &exit {
                    let exit_name = self.unparse_variable(exit);
                    write!(writer, " {}", exit_name.as_arc_str())?;
                }
                writeln!(writer, ";")?;
                Ok(())
            }
            StmtNode::Expr(Expr::Assign { left, right }) => {
                let Expr::Id(var) = left.as_ref() else {
                    let left_frag = self.unparse_expr(left)?;
                    let right_frag = self.unparse_expr(right)?;
                    writeln!(writer, "{indent_str}{left_frag} = {right_frag};")?;
                    return Ok(());
                };

                let Expr::Lambda {
                    params,
                    body,
                    self_name: Some(name),
                } = right.as_ref()
                else {
                    let var_name = self.unparse_variable(var);
                    let right_frag = self.unparse_expr(right)?;
                    writeln!(writer, "{indent_str}{var_name} = {right_frag};")?;
                    return Ok(());
                };

                let var_name = self.unparse_variable(var);
                let name_str = self.unparse_variable(name);

                if var_name != name_str {
                    let right_frag = self.unparse_expr(right)?;
                    writeln!(writer, "{indent_str}{var_name} = {right_frag};")?;
                    return Ok(());
                }

                self.unparse_named_function(params, body, &name_str, writer, indent)
            }
            StmtNode::Expr(Expr::Decl { id, is_const, expr }) => {
                let Some(expr) = expr.as_ref() else {
                    let prefix = if *is_const { "const " } else { "let " };
                    let var_name = self.unparse_variable(id);
                    writeln!(writer, "{indent_str}{prefix}{var_name};")?;
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
                    let expr_str = self.unparse_expr(expr)?;
                    writeln!(writer, "{indent_str}{prefix}{var_name} = {expr_str};")?;
                    return Ok(());
                };

                let var_name = self.unparse_variable(id);
                let name_str = self.unparse_variable(name);

                if var_name != name_str {
                    let prefix = if *is_const { "const " } else { "let " };
                    let expr_str = self.unparse_expr(expr)?;
                    writeln!(writer, "{indent_str}{prefix}{var_name} = {expr_str};")?;
                    return Ok(());
                }

                self.unparse_named_function(params, body, &name_str, writer, indent)
            }
            StmtNode::Expr(expr) => {
                let expr_str = self.unparse_expr(expr)?;
                writeln!(writer, "{indent_str}{expr_str};")?;
                Ok(())
            }
            StmtNode::Scope { num_bindings, body } => {
                writeln!(writer, "{indent_str}begin")?;
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
                        let items_frag = self.unparse_scatter_items(items)?;
                        let right_frag = self.unparse_expr(right)?;
                        let inner_indent = if self.indent_width > 0 {
                            " ".repeat((indent + 1) * self.indent_width)
                        } else {
                            String::new()
                        };
                        writeln!(
                            writer,
                            "{inner_indent}{decl_prefix}{{{items_frag}}} = {right_frag};"
                        )?;
                        remaining_bindings = remaining_bindings.saturating_sub(items.len());
                        continue;
                    }
                    self.unparse_stmt(stmt, writer, indent + 1)?;
                }
                writeln!(writer, "{indent_str}end")?;
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
        let indent_str = if self.indent_width > 0 {
            " ".repeat(indent * self.indent_width)
        } else {
            String::new()
        };

        let param_str = self.unparse_lambda_params(params)?;
        writeln!(writer, "{indent_str}fn {name}({param_str})")?;

        match &body.node {
            StmtNode::Scope {
                body: scope_body, ..
            } => self.unparse_stmts(scope_body, writer, indent + 1)?,
            _ => self.unparse_stmt(body, writer, indent + 1)?,
        }

        writeln!(writer, "{indent_str}endfn")?;
        Ok(())
    }
}
