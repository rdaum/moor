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

use crate::{
    Op::Jump,
    ast::{Stmt, StmtNode},
    backend::control::LoopFrame,
    codegen::CodegenState,
};
use moor_common::model::CompileError;
use moor_var::program::opcode::{ForRangeOperand, ForSequenceOperand, Op};

impl CodegenState {
    pub fn generate_stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        // We use the 'canonical' tree line number here for span generation, which should match what
        // unparse generates.
        // TODO In theory we could actually provide both and generate spans for both for situations
        //   where the user is looking at their own not-decompiled copy of the source.
        let line_number = stmt.tree_line_no;
        self.current_line_col = stmt.line_col;
        self.line_number_spans
            .push((self.emitter.pc(), line_number));
        match &stmt.node {
            StmtNode::Cond { arms, otherwise } => {
                let end_label = self.make_jump_label(None);
                let mut is_else = false;
                for arm in arms {
                    self.generate_expr(&arm.condition)?;
                    let otherwise_label = self.make_jump_label(None);
                    self.emit(if !is_else {
                        Op::If(otherwise_label, arm.environment_width as u16)
                    } else {
                        Op::Eif(otherwise_label, arm.environment_width as u16)
                    });
                    is_else = true;
                    self.pop_stack(1);
                    for stmt in &arm.statements {
                        self.generate_stmt(stmt)?;
                    }
                    self.emit(Op::EndScope {
                        num_bindings: arm.environment_width as u16,
                    });
                    self.emit(Jump { label: end_label });

                    // This is where we jump to if the condition is false; either the end of the
                    // if statement, or the start of the next ('else or elseif') arm.
                    self.commit_jump_label(otherwise_label);
                }
                if let Some(otherwise) = otherwise {
                    let end_label = self.make_jump_label(None);
                    // Decompilation has to elide this begin/end scope pair, as it's not actually
                    // present in the source code.
                    self.emit(Op::BeginScope {
                        num_bindings: otherwise.environment_width as u16,
                        end_label,
                    });
                    for stmt in &otherwise.statements {
                        self.generate_stmt(stmt)?;
                    }
                    self.emit(Op::EndScope {
                        num_bindings: otherwise.environment_width as u16,
                    });
                    self.commit_jump_label(end_label);
                }
                self.commit_jump_label(end_label);
            }
            StmtNode::ForList {
                value_binding,
                key_binding,
                expr,
                body,
                environment_width,
            } => {
                self.generate_expr(expr)?;

                let value_bind = self.find_name(value_binding);
                let key_bind = key_binding.map(|id| self.find_name(&id));
                let end_label = self.make_jump_label(Some(value_bind));

                let offset = self.add_for_sequence_operand(ForSequenceOperand {
                    value_bind,
                    key_bind,
                    end_label,
                    environment_width: *environment_width as u16,
                });

                self.emit(Op::BeginForSequence { operand: offset });
                self.pop_stack(1);

                let loop_top = self.make_jump_label(Some(value_bind));
                self.commit_jump_label(loop_top);
                self.emit(Op::IterateForSequence);

                self.control.push_loop(LoopFrame {
                    loop_name: Some(value_bind),
                    top_label: loop_top,
                    top_stack: self.stack.depth().into(),
                    bottom_label: end_label,
                    bottom_stack: self.stack.depth().into(),
                });

                for stmt in body {
                    self.generate_stmt(stmt)?;
                }

                self.emit(Jump { label: loop_top });
                self.emit(Op::EndScope {
                    num_bindings: *environment_width as u16,
                });
                self.commit_jump_label(end_label);
                self.control.pop_loop();
            }
            StmtNode::ForRange {
                from,
                to,
                id,
                body,
                environment_width,
            } => {
                self.generate_expr(from)?;
                self.generate_expr(to)?;

                let end_label = self.make_jump_label(Some(self.find_name(id)));
                let offset = self.add_for_range_operand(ForRangeOperand {
                    loop_variable: self.find_name(id),
                    end_label,
                    environment_width: *environment_width as u16,
                });

                self.emit(Op::BeginForRange { operand: offset });
                self.pop_stack(2);

                let loop_top = self.make_jump_label(Some(self.find_name(id)));
                self.commit_jump_label(loop_top);
                self.emit(Op::IterateForRange);

                self.control.push_loop(LoopFrame {
                    loop_name: Some(self.find_name(id)),
                    top_label: loop_top,
                    top_stack: self.stack.depth().into(),
                    bottom_label: end_label,
                    bottom_stack: self.stack.depth().into(),
                });

                for stmt in body {
                    self.generate_stmt(stmt)?;
                }

                self.emit(Jump { label: loop_top });
                self.emit(Op::EndScope {
                    num_bindings: *environment_width as u16,
                });
                self.commit_jump_label(end_label);
                self.control.pop_loop();
            }
            StmtNode::While {
                id,
                condition,
                body,
                environment_width,
            } => {
                let loop_start_label =
                    self.make_jump_label(id.as_ref().map(|id| self.find_name(id)));
                self.commit_jump_label(loop_start_label);

                let loop_end_label = self.make_jump_label(id.as_ref().map(|id| self.find_name(id)));
                self.generate_expr(condition)?;
                match id {
                    None => self.emit(Op::While {
                        jump_label: loop_end_label,
                        environment_width: *environment_width as u16,
                    }),
                    Some(id) => self.emit(Op::WhileId {
                        id: self.find_name(id),
                        end_label: loop_end_label,
                        environment_width: *environment_width as u16,
                    }),
                }
                self.pop_stack(1);
                self.control.push_loop(LoopFrame {
                    loop_name: id.as_ref().map(|id| self.find_name(id)),
                    top_label: loop_start_label,
                    top_stack: self.stack.depth().into(),
                    bottom_label: loop_end_label,
                    bottom_stack: self.stack.depth().into(),
                });
                for s in body {
                    self.generate_stmt(s)?;
                }
                self.emit(Op::EndScope {
                    num_bindings: *environment_width as u16,
                });
                self.emit(Jump {
                    label: loop_start_label,
                });
                self.commit_jump_label(loop_end_label);
                self.control.pop_loop();
            }
            StmtNode::Fork { id, body, time } => {
                self.generate_expr(time)?;
                let fork_main_position = self.emitter.pc();

                let stashed_ops = self.emitter.take_ops();
                let stashed_line_spans = std::mem::take(&mut self.line_number_spans);

                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::Done);
                let forked_ops = self.emitter.take_ops();
                let fork_line_spans = std::mem::take(&mut self.line_number_spans);

                self.emitter.replace_ops(stashed_ops);
                self.line_number_spans = stashed_line_spans;

                let fv_id = self.add_fork_vector(fork_main_position, forked_ops, fork_line_spans);
                self.emit(Op::Fork {
                    id: id.as_ref().map(|id| self.find_name(id)),
                    fv_offset: fv_id,
                });
                self.pop_stack(1);
            }
            StmtNode::TryExcept {
                body,
                excepts,
                environment_width,
            } => {
                let mut labels = vec![];
                let num_excepts = excepts.len();
                for ex in excepts {
                    self.generate_codes(&ex.codes)?;
                    let push_label = self.make_jump_label(None);
                    self.emit(Op::PushCatchLabel(push_label));
                    labels.push(push_label);
                }
                self.pop_stack(num_excepts);
                let end_label = self.make_jump_label(None);

                self.emit(Op::TryExcept {
                    num_excepts: num_excepts as u16,
                    environment_width: *environment_width as u16,
                    end_label,
                });
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::EndExcept(end_label));
                for (i, ex) in excepts.iter().enumerate() {
                    self.commit_jump_label(labels[i]);
                    self.push_stack(1);
                    if let Some(id) = &ex.id {
                        self.emit(Op::Put(self.find_name(id)));
                    }
                    self.emit(Op::Pop);
                    self.pop_stack(1);
                    for stmt in &ex.statements {
                        self.generate_stmt(stmt)?;
                    }
                    if i + 1 < excepts.len() {
                        self.emit(Jump { label: end_label });
                    }
                }
                self.commit_jump_label(end_label);
            }
            StmtNode::TryFinally {
                body,
                handler,
                environment_width,
            } => {
                let handler_label = self.make_jump_label(None);
                self.emit(Op::TryFinally {
                    end_label: handler_label,
                    environment_width: *environment_width as u16,
                });
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::EndFinally);
                self.commit_jump_label(handler_label);
                for stmt in handler {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::FinallyContinue);
            }
            StmtNode::Scope { num_bindings, body } => {
                let end_label = self.make_jump_label(None);
                if *num_bindings > 0 {
                    self.emit(Op::BeginScope {
                        num_bindings: *num_bindings as u16,
                        end_label,
                    });
                }

                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                if *num_bindings > 0 {
                    self.emit(Op::EndScope {
                        num_bindings: *num_bindings as u16,
                    });
                }
                self.commit_jump_label(end_label);
            }
            StmtNode::Break { exit: None } => {
                let l = self.control.current_loop().expect("No loop to break/continue from");
                self.emit(Op::Exit {
                    stack: l.bottom_stack,
                    label: l.bottom_label,
                })
            }
            StmtNode::Break { exit: Some(l) } => {
                let l = self.find_name(l);
                let l = self.find_loop(&l)?;
                self.emit(Op::ExitId(l.bottom_label));
            }
            StmtNode::Continue { exit: None } => {
                let l = self.control.current_loop().expect("No loop to break/continue from");
                self.emit(Op::Exit {
                    stack: l.top_stack,
                    label: l.top_label,
                })
            }
            StmtNode::Continue { exit: Some(l) } => {
                let loop_name = self.find_name(l);
                let loop_info = self
                    .find_loop(&loop_name)
                    .expect("invalid loop for break/continue");
                self.emit(Op::ExitId(loop_info.top_label));
            }
            StmtNode::Expr(e) => {
                self.generate_expr(e)?;
                self.emit(Op::Pop);
                self.pop_stack(1);
            }
        }

        Ok(())
    }
}
