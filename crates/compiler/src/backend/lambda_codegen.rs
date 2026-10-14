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

use moor_common::model::CompileError;
use moor_var::program::names::{Name, Variable};
use moor_var::program::opcode::{Op, ScatterLabel};

use crate::{
    ast::{Expr, ScatterItem, ScatterKind, Stmt, StmtNode},
    codegen::CodegenState,
};

impl CodegenState {
    pub(crate) fn compile_lambda_body(
        &mut self,
        params: &[ScatterItem],
        body: &Stmt,
        entry_scope_count: u16,
        captures: &[Variable],
    ) -> Result<(), CompileError> {
        let base_line_offset = body.line_col.0;

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
        let stashed_declarations = std::mem::take(&mut self.declaration_sites);
        let stashed_stack = self.stack.snapshot_and_reset();
        let stashed_scopes = self.scopes.snapshot_and_reset();

        self.emitter.reset();
        self.line_number_spans = vec![];

        let mut body = body.clone();
        assign_stmt_source_line_numbers(&mut body);

        for param in params {
            if let ScatterKind::Optional = param.kind
                && let Some(default_expr) = &param.expr
            {
                let param_name = self.find_name(&param.id);
                self.emit_push_name(param_name);
                self.push_stack(1);
                self.emit(Op::ImmInt(0));
                self.push_stack(1);
                self.emit(Op::Eq);
                self.pop_stack(1);

                let skip_default = self.make_jump_label(None);
                self.emit(Op::IfQues(skip_default));
                self.pop_stack(1);

                self.generate_expr(default_expr)?;
                self.emit_put_name(param_name);
                self.emit(Op::Pop);
                self.pop_stack(1);

                self.commit_jump_label(skip_default);
            }
        }

        self.generate_stmt(&body)?;
        let lambda_max_stack = self.stack.max_depth();
        let lambda_max_scope_depth = self.scopes.max_depth();
        if self.stack.depth() != 0 || self.stack.saved_top().is_some() {
            panic!(
                "Lambda stack is not empty at end of compilation: cur_stack#: {} stack: {:?}",
                self.stack.depth(),
                self.stack.saved_top()
            )
        }
        if self.scopes.depth() != 0 {
            panic!(
                "Lambda scope stack is not empty at end of compilation: cur_scope#: {}",
                self.scopes.depth()
            )
        }

        let mut lambda_program = self
            .operands
            .take_program_parts(std::mem::take(&mut self.declaration_sites))
            .build_program(
                self.var_names.clone(),
                self.emitter.take_jumps(),
                self.emitter.take_ops(),
                lambda_max_stack,
                lambda_max_scope_depth,
                std::mem::take(&mut self.line_number_spans),
            );

        triomphe::Arc::make_mut(&mut lambda_program.0).lambda_entry_scope_count = entry_scope_count;

        self.emitter.replace_ops(stashed_ops);
        self.var_names = stashed_var_names;
        self.emitter.replace_jumps(stashed_jumps);
        self.operands.restore(stashed_operands);
        self.line_number_spans = stashed_line_number_spans;
        self.declaration_sites = stashed_declarations;
        self.stack.restore(stashed_stack);
        self.scopes.restore(stashed_scopes);

        let program_offset = self.add_lambda_program(lambda_program, base_line_offset);

        let mut captured_names: Vec<Name> = captures
            .iter()
            .filter_map(|variable| self.var_names.name_for_var(variable))
            .collect();

        captured_names.sort_unstable();
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

fn assign_stmt_source_line_numbers(stmt: &mut Stmt) {
    stmt.tree_line_no = stmt.line_col.0;
    match &mut stmt.node {
        StmtNode::Cond { arms, otherwise } => {
            for arm in arms {
                for stmt in &mut arm.statements {
                    assign_stmt_source_line_numbers(stmt);
                }
            }
            if let Some(otherwise) = otherwise {
                for stmt in &mut otherwise.statements {
                    assign_stmt_source_line_numbers(stmt);
                }
            }
        }
        StmtNode::ForList { expr, body, .. }
        | StmtNode::While {
            condition: expr,
            body,
            ..
        } => {
            assign_expr_lambda_source_lines(expr);
            for stmt in body {
                assign_stmt_source_line_numbers(stmt);
            }
        }
        StmtNode::ForRange { from, to, body, .. } => {
            assign_expr_lambda_source_lines(from);
            assign_expr_lambda_source_lines(to);
            for stmt in body {
                assign_stmt_source_line_numbers(stmt);
            }
        }
        StmtNode::Fork { id: _, time, body } => {
            assign_expr_lambda_source_lines(time);
            for stmt in body {
                assign_stmt_source_line_numbers(stmt);
            }
        }
        StmtNode::TryExcept { body, excepts, .. } => {
            for stmt in body {
                assign_stmt_source_line_numbers(stmt);
            }
            for except in excepts {
                assign_catch_codes_lambda_source_lines(&mut except.codes);
                for stmt in &mut except.statements {
                    assign_stmt_source_line_numbers(stmt);
                }
            }
        }
        StmtNode::TryFinally { body, handler, .. } => {
            for stmt in body {
                assign_stmt_source_line_numbers(stmt);
            }
            for stmt in handler {
                assign_stmt_source_line_numbers(stmt);
            }
        }
        StmtNode::Scope { body, .. } => {
            for stmt in body {
                assign_stmt_source_line_numbers(stmt);
            }
        }
        StmtNode::Expr(expr) => assign_expr_lambda_source_lines(expr),
        StmtNode::Break { .. } | StmtNode::Continue { .. } => {}
    }
}

fn assign_arg_lambda_source_lines(arg: &mut crate::ast::Arg) {
    match arg {
        crate::ast::Arg::Normal(expr) | crate::ast::Arg::Splice(expr) => {
            assign_expr_lambda_source_lines(expr)
        }
    }
}

fn assign_catch_codes_lambda_source_lines(codes: &mut crate::ast::CatchCodes) {
    match codes {
        crate::ast::CatchCodes::Codes(args) => {
            for arg in args {
                assign_arg_lambda_source_lines(arg);
            }
        }
        crate::ast::CatchCodes::Any => {}
    }
}

fn assign_expr_lambda_source_lines(expr: &mut Expr) {
    match expr {
        Expr::Assign { left, right } => {
            assign_expr_lambda_source_lines(left);
            assign_expr_lambda_source_lines(right);
        }
        Expr::Pass { args } | Expr::List(args) => {
            for arg in args {
                assign_arg_lambda_source_lines(arg);
            }
        }
        Expr::Error(_, maybe_expr) | Expr::Return(maybe_expr) => {
            if let Some(expr) = maybe_expr {
                assign_expr_lambda_source_lines(expr);
            }
        }
        Expr::Binary(_, left, right)
        | Expr::And(left, right)
        | Expr::Or(left, right)
        | Expr::Index(left, right) => {
            assign_expr_lambda_source_lines(left);
            assign_expr_lambda_source_lines(right);
        }
        Expr::Unary(_, expr)
        | Expr::Decl {
            expr: Some(expr), ..
        } => assign_expr_lambda_source_lines(expr),
        Expr::Decl { expr: None, .. }
        | Expr::TypeConstant(_)
        | Expr::Value(_)
        | Expr::Id(_)
        | Expr::Length => {}
        Expr::Prop { location, property } => {
            assign_expr_lambda_source_lines(location);
            assign_expr_lambda_source_lines(property);
        }
        Expr::Call { function, args } => {
            if let crate::ast::CallTarget::Expr(expr) = function {
                assign_expr_lambda_source_lines(expr);
            }
            for arg in args {
                assign_arg_lambda_source_lines(arg);
            }
        }
        Expr::Verb {
            location,
            verb,
            args,
        } => {
            assign_expr_lambda_source_lines(location);
            assign_expr_lambda_source_lines(verb);
            for arg in args {
                assign_arg_lambda_source_lines(arg);
            }
        }
        Expr::Range { base, from, to } => {
            assign_expr_lambda_source_lines(base);
            assign_expr_lambda_source_lines(from);
            assign_expr_lambda_source_lines(to);
        }
        Expr::Cond {
            condition,
            consequence,
            alternative,
        } => {
            assign_expr_lambda_source_lines(condition);
            assign_expr_lambda_source_lines(consequence);
            assign_expr_lambda_source_lines(alternative);
        }
        Expr::TryCatch {
            trye,
            codes,
            except,
        } => {
            assign_expr_lambda_source_lines(trye);
            assign_catch_codes_lambda_source_lines(codes);
            if let Some(expr) = except {
                assign_expr_lambda_source_lines(expr);
            }
        }
        Expr::Map(entries) => {
            for (key, value) in entries {
                assign_expr_lambda_source_lines(key);
                assign_expr_lambda_source_lines(value);
            }
        }
        Expr::Flyweight(delegate, slots, contents) => {
            assign_expr_lambda_source_lines(delegate);
            for (_, value) in slots {
                assign_expr_lambda_source_lines(value);
            }
            if let Some(expr) = contents {
                assign_expr_lambda_source_lines(expr);
            }
        }
        Expr::Scatter(items, right, _) => {
            for item in items {
                if let Some(expr) = &mut item.expr {
                    assign_expr_lambda_source_lines(expr);
                }
            }
            assign_expr_lambda_source_lines(right);
        }
        Expr::ComprehendList {
            producer_expr,
            list,
            filter,
            ..
        } => {
            assign_expr_lambda_source_lines(producer_expr);
            assign_expr_lambda_source_lines(list);
            if let Some(filter) = filter {
                assign_expr_lambda_source_lines(filter);
            }
        }
        Expr::ComprehendRange {
            producer_expr,
            from,
            to,
            filter,
            ..
        } => {
            assign_expr_lambda_source_lines(producer_expr);
            assign_expr_lambda_source_lines(from);
            assign_expr_lambda_source_lines(to);
            if let Some(filter) = filter {
                assign_expr_lambda_source_lines(filter);
            }
        }
        Expr::Lambda { params, body, .. } => {
            for param in params {
                if let Some(expr) = &mut param.expr {
                    assign_expr_lambda_source_lines(expr);
                }
            }
            assign_stmt_source_line_numbers(body);
        }
    }
}
