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

use std::collections::HashSet;

use moor_common::model::{CompileContext, CompileError};
use moor_var::Symbol;
use moor_var::program::names::{Name, Variable};
use moor_var::program::opcode::{Op, ScatterLabel};

use crate::{
    ast::{AstVisitor, Expr, ScatterItem, ScatterKind, Stmt, StmtNode},
    codegen::CodegenState,
};

impl CodegenState {
    pub(crate) fn compile_lambda_body(
        &mut self,
        params: &[ScatterItem],
        body: &Stmt,
        base_line_offset: usize,
    ) -> Result<(), CompileError> {
        let outer_scope_depth = self.control.lambda_scope_depth();
        self.control.push_lambda_scope_depth(2);

        let labels: Vec<ScatterLabel> = params
            .iter()
            .map(|param| match param.kind {
                ScatterKind::Required => ScatterLabel::Required(self.find_name(&param.id)),
                ScatterKind::Optional => ScatterLabel::Optional(self.find_name(&param.id), None),
                ScatterKind::Rest => ScatterLabel::Rest(self.find_name(&param.id)),
            })
            .collect();
        let done = self.make_jump_label(None);
        let scatter_offset = self.add_scatter_table(labels, done);

        let stashed_ops = self.emitter.take_ops();
        let stashed_var_names = self.var_names.clone();
        let stashed_jumps = self.emitter.take_jumps();
        let stashed_operands = self.operands.snapshot_and_reset();
        let stashed_line_number_spans = std::mem::take(&mut self.line_number_spans);

        self.emitter.reset();
        self.line_number_spans = vec![];

        for param in params {
            if let ScatterKind::Optional = param.kind
                && let Some(default_expr) = &param.expr
            {
                let param_name = self.find_name(&param.id);
                self.emit(Op::Push(param_name));
                self.push_stack(1);
                self.emit(Op::ImmInt(0));
                self.push_stack(1);
                self.emit(Op::Eq);
                self.pop_stack(1);

                let skip_default = self.make_jump_label(None);
                self.emit(Op::IfQues(skip_default));
                self.pop_stack(1);

                self.generate_expr(default_expr)?;
                self.emit(Op::Put(param_name));
                self.emit(Op::Pop);
                self.pop_stack(1);

                self.commit_jump_label(skip_default);
            }
        }

        self.generate_stmt(body)?;

        let lambda_program = self.operands.take_program_parts().build_program(
            self.var_names.clone(),
            self.emitter.take_jumps(),
            self.emitter.take_ops(),
            std::mem::take(&mut self.line_number_spans),
        );

        self.emitter.replace_ops(stashed_ops);
        self.var_names = stashed_var_names;
        self.emitter.replace_jumps(stashed_jumps);
        self.operands.restore(stashed_operands);
        self.line_number_spans = stashed_line_number_spans;

        let program_offset = self.add_lambda_program(lambda_program, base_line_offset);
        self.control.set_lambda_scope_depth(outer_scope_depth);

        let captured_symbols = analyze_lambda_captures(params, body, outer_scope_depth)?;
        let captured_names: Vec<Name> = captured_symbols
            .iter()
            .filter_map(|sym| self.var_names.name_for_ident(*sym))
            .collect();

        for &name in &captured_names {
            self.emit(Op::Capture(name));
        }

        self.emit(Op::MakeLambda {
            scatter_offset,
            program_offset,
            self_var: None,
            num_captured: captured_names.len() as u16,
        });
        self.push_stack(1);

        Ok(())
    }
}

struct CaptureAnalyzer {
    captures: HashSet<Symbol>,
    assigned_captures: Vec<(Symbol, (usize, usize))>,
    param_names: HashSet<Symbol>,
    outer_scope_level: u8,
    current_line_col: (usize, usize),
}

impl CaptureAnalyzer {
    fn new(lambda_params: &[ScatterItem], outer_scope_depth: u8) -> Self {
        let param_names: HashSet<Symbol> = lambda_params
            .iter()
            .map(|param| param.id.to_symbol())
            .collect();
        let outer_scope_level = lambda_params
            .first()
            .map(|p| p.id.scope_id as u8)
            .unwrap_or(outer_scope_depth);

        Self {
            captures: HashSet::new(),
            assigned_captures: Vec::new(),
            param_names,
            outer_scope_level,
            current_line_col: (0, 0),
        }
    }

    fn should_capture(&self, var: &Variable) -> bool {
        if self.param_names.contains(&var.to_symbol()) {
            return false;
        }

        var.scope_id as u8 <= self.outer_scope_level
    }

    fn is_outer_scope_variable(&self, var: &Variable) -> bool {
        if self.param_names.contains(&var.to_symbol()) {
            return false;
        }

        var.scope_id as u8 <= self.outer_scope_level
    }
}

impl AstVisitor for CaptureAnalyzer {
    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Id(var) => {
                if self.should_capture(var) {
                    self.captures.insert(var.to_symbol());
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
            Expr::Scatter(items, _) => {
                for item in items {
                    if self.is_outer_scope_variable(&item.id) {
                        self.assigned_captures
                            .push((item.id.to_symbol(), self.current_line_col));
                    }
                }
                self.walk_expr(expr);
            }
            Expr::Lambda { params, body, .. } => {
                for param in params {
                    if let Some(default_expr) = &param.expr {
                        self.visit_expr(default_expr);
                    }
                }

                let nested_params: Vec<Symbol> = params.iter().map(|p| p.id.to_symbol()).collect();
                for sym in &nested_params {
                    self.param_names.insert(*sym);
                }

                self.visit_stmt(body);

                for sym in &nested_params {
                    self.param_names.remove(sym);
                }
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

fn analyze_lambda_captures(
    lambda_params: &[ScatterItem],
    lambda_body: &Stmt,
    outer_scope_depth: u8,
) -> Result<Vec<Symbol>, CompileError> {
    let mut analyzer = CaptureAnalyzer::new(lambda_params, outer_scope_depth);
    analyzer.visit_stmt(lambda_body);

    if let Some((assigned_var, line_col)) = analyzer.assigned_captures.first() {
        return Err(CompileError::AssignmentToCapturedVariable(
            CompileContext::new(*line_col),
            *assigned_var,
        ));
    }

    Ok(analyzer.captures.into_iter().collect())
}
