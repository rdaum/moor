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

//! Lexical capture resolution for lambda expressions.

use std::collections::HashSet;

use moor_common::model::{CompileContext, CompileError};
use moor_var::{Symbol, program::names::Variable};

use crate::ast::{AstVisitor, Expr, ScatterItem, Stmt, StmtNode};

struct CaptureAnalyzer {
    captures: HashSet<Variable>,
    assigned_captures: Vec<(Symbol, (usize, usize))>,
    parameter_scope: u16,
    current_line_col: (usize, usize),
}

impl CaptureAnalyzer {
    fn new(parameter_scope: u16) -> Self {
        Self {
            captures: HashSet::new(),
            assigned_captures: Vec::new(),
            parameter_scope,
            current_line_col: (0, 0),
        }
    }

    fn is_outer_scope_variable(&self, var: &Variable) -> bool {
        // Scope IDs are allocated monotonically. Parameters and every scope
        // inside this lambda are allocated at or after its parameter scope.
        var.scope_id < self.parameter_scope
    }
}

impl AstVisitor for CaptureAnalyzer {
    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Id(var) => {
                if self.is_outer_scope_variable(var) {
                    self.captures.insert(*var);
                }
            }
            Expr::Assign { left, right: _ } => {
                if let Expr::Id(var) = left.as_ref()
                    && self.is_outer_scope_variable(var)
                {
                    self.assigned_captures
                        .push((var.to_symbol(), self.current_line_col));
                }
                self.walk_expr(expr);
            }
            Expr::Scatter(items, _, _) => {
                for item in items {
                    if self.is_outer_scope_variable(&item.id) {
                        self.assigned_captures
                            .push((item.id.to_symbol(), self.current_line_col));
                    }
                }
                self.walk_expr(expr);
            }
            _ => self.walk_expr(expr),
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        self.current_line_col = stmt.line_col;
        self.walk_stmt(stmt);
    }

    fn visit_stmt_node(&mut self, stmt_node: &StmtNode) {
        self.walk_stmt_node(stmt_node);
    }
}

/// Resolve captures while lowering still has the lambda's lexical scope ID.
pub(crate) fn analyze_lambda_captures(
    lambda_params: &[ScatterItem],
    lambda_body: &Stmt,
    parameter_scope: u16,
) -> Result<Vec<Variable>, CompileError> {
    let mut analyzer = CaptureAnalyzer::new(parameter_scope);
    for param in lambda_params {
        if let Some(default) = &param.expr {
            analyzer.visit_expr(default);
        }
    }
    analyzer.visit_stmt(lambda_body);

    if let Some((assigned_var, line_col)) = analyzer.assigned_captures.first() {
        return Err(CompileError::AssignmentToCapturedVariable(
            CompileContext::new(*line_col),
            *assigned_var,
        ));
    }

    let mut captures: Vec<_> = analyzer.captures.into_iter().collect();
    captures.sort_unstable_by_key(|var| var.id);
    Ok(captures)
}
