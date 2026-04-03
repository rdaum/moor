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

use moor_common::model::{CompileContext, CompileError};
use moor_var::{ErrorCode, Var};
use moor_var::program::{
    labels::{Label, Offset},
    names::{Name, Names, Variable},
    opcode::{
        ForRangeOperand, ForSequenceOperand, ListComprehend, Op, RangeComprehend, ScatterLabel,
    },
    program::Program,
};

use crate::{
    ast::Expr,
    backend::control::{ControlState, LoopFrame},
    backend::emitter::EmitterState,
    backend::operands::OperandState,
    backend::stack::StackState,
    compile_options::CompileOptions,
};

pub struct CodegenState {
    pub(crate) emitter: EmitterState,
    pub(crate) var_names: Names,
    pub(crate) name_for_variable: Vec<Option<Name>>,
    pub(crate) operands: OperandState,
    pub(crate) control: ControlState,
    pub(crate) stack: StackState,
    pub(crate) line_number_spans: Vec<(usize, usize)>,
    pub(crate) current_line_col: (usize, usize),
    pub(crate) compile_options: CompileOptions,
}

impl CodegenState {
    pub fn new(compile_options: CompileOptions, var_names: Names) -> Self {
        let max_variable_id = var_names
            .decls
            .values()
            .map(|decl| decl.identifier.id as usize)
            .max()
            .unwrap_or(0);
        let mut name_for_variable = vec![None; max_variable_id + 1];
        for (name, decl) in &var_names.decls {
            name_for_variable[decl.identifier.id as usize] = Some(*name);
        }
        Self {
            emitter: EmitterState::new(),
            var_names,
            name_for_variable,
            operands: OperandState::new(),
            control: ControlState::new(),
            stack: StackState::new(),
            line_number_spans: vec![],
            current_line_col: (0, 0),
            compile_options,
        }
    }

    pub(crate) fn make_jump_label(&mut self, name: Option<Name>) -> Label {
        self.emitter.new_jump_label(name)
    }

    pub(crate) fn commit_jump_label(&mut self, id: Label) {
        self.emitter.bind_jump_label(id);
    }

    pub(crate) fn add_literal(&mut self, v: &Var) -> Label {
        self.operands.add_literal(v)
    }

    pub(crate) fn add_error_code_operand(&mut self, code: ErrorCode) -> Offset {
        self.operands.add_error_code_operand(code)
    }

    pub(crate) fn add_scatter_table(&mut self, labels: Vec<ScatterLabel>, done: Label) -> Offset {
        self.operands.add_scatter_table(labels, done)
    }

    pub(crate) fn add_lambda_program(&mut self, program: Program, base_line_offset: usize) -> Offset {
        self.operands.add_lambda_program(program, base_line_offset)
    }

    pub(crate) fn add_range_comprehension(&mut self, range_comprehension: RangeComprehend) -> Offset {
        self.operands.add_range_comprehension(range_comprehension)
    }

    pub(crate) fn add_list_comprehension(&mut self, list_comprehension: ListComprehend) -> Offset {
        self.operands.add_list_comprehension(list_comprehension)
    }

    pub(crate) fn add_for_sequence_operand(&mut self, operand: ForSequenceOperand) -> Offset {
        self.operands.add_for_sequence_operand(operand)
    }

    pub(crate) fn add_for_range_operand(&mut self, operand: ForRangeOperand) -> Offset {
        self.operands.add_for_range_operand(operand)
    }

    pub(crate) fn emit(&mut self, op: Op) {
        self.emitter.emit(op);
    }

    pub(crate) fn is_assignable_expr(expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::Id(..) | Expr::Index(..) | Expr::Range { .. } | Expr::Prop { .. }
        )
    }

    pub(crate) fn find_loop(&self, loop_label: &Name) -> Result<&LoopFrame, CompileError> {
        if let Some(loop_frame) = self.control.find_loop(loop_label) {
            return Ok(loop_frame);
        }
        let loop_name = self.var_names.ident_for_name(loop_label).unwrap();
        Err(CompileError::UnknownLoopLabel(
            CompileContext::new(self.current_line_col),
            loop_name.to_string(),
        ))
    }

    pub(crate) fn push_stack(&mut self, n: usize) {
        self.stack.push(n);
    }

    pub(crate) fn pop_stack(&mut self, n: usize) {
        self.stack.pop(n);
    }

    pub(crate) fn saved_stack_top(&self) -> Option<Offset> {
        self.stack.saved_top()
    }

    pub(crate) fn save_stack_top(&mut self) -> Option<Offset> {
        self.stack.save_top()
    }

    pub(crate) fn restore_stack_top(&mut self, old: Option<Offset>) {
        self.stack.restore_saved_top(old)
    }

    pub(crate) fn add_fork_vector(
        &mut self,
        offset: usize,
        opcodes: Vec<Op>,
        line_spans: Vec<(usize, usize)>,
    ) -> Offset {
        self.operands.add_fork_vector(offset, opcodes, line_spans)
    }

    fn lvalue_stack_footprint(expr: &Expr, indexed_above: bool) -> usize {
        match expr {
            Expr::Range { base, .. } => Self::lvalue_stack_footprint(base.as_ref(), true) + 2,
            Expr::Index(lhs, ..) => {
                Self::lvalue_stack_footprint(lhs.as_ref(), true) + 1 + usize::from(indexed_above)
            }
            Expr::Id(..) => usize::from(indexed_above),
            Expr::Prop { location, .. } => {
                let loc = if Self::is_assignable_expr(location.as_ref()) {
                    Self::lvalue_stack_footprint(location.as_ref(), true)
                } else {
                    1
                };
                loc + 1 + usize::from(indexed_above)
            }
            _ => 0,
        }
    }

    pub(crate) fn generate_assign(&mut self, left: &Expr, right: &Expr) -> Result<(), CompileError> {
        self.push_lvalue(left, false)?;
        self.generate_expr(right)?;
        let uses_set = matches!(
            left,
            Expr::Range { .. } | Expr::Index(..) | Expr::Prop { .. }
        );
        if uses_set {
            self.emit(Op::Dup);
            self.push_stack(1);
        }
        let mut used_set = false;
        let mut handled_stack = false;
        let mut prop_short_circuit_blocks: Vec<(Label, usize, usize)> = vec![];
        let mut e = left;
        loop {
            match e {
                Expr::Range {
                    base,
                    from: _,
                    to: _,
                } => {
                    self.emit(Op::RangeSetAt(Offset(1)));
                    self.pop_stack(3);
                    e = base;
                    used_set = true;
                    continue;
                }
                Expr::Index(lhs, _rhs) => {
                    self.emit(Op::IndexSetAt(Offset(1)));
                    self.pop_stack(2);
                    e = lhs;
                    used_set = true;
                    continue;
                }
                Expr::Id(name) => {
                    if used_set {
                        self.emit(Op::Swap);
                        self.emit(Op::Put(self.find_name(name)));
                        self.emit(Op::Pop);
                        self.pop_stack(1);
                        handled_stack = true;
                    } else {
                        self.emit(Op::Put(self.find_name(name)));
                    }
                    break;
                }
                Expr::Prop {
                    location,
                    property: _,
                } => {
                    let needs_prop_short_circuit = matches!(location.as_ref(), Expr::Prop { .. });
                    let jump_if_object = self.make_jump_label(None);
                    self.emit(Op::PutPropAt {
                        offset: Offset(1),
                        jump_if_object,
                    });
                    self.pop_stack(2);
                    used_set = true;
                    if Self::is_assignable_expr(location.as_ref()) {
                        if !needs_prop_short_circuit {
                            self.commit_jump_label(jump_if_object);
                        }
                        if needs_prop_short_circuit {
                            let cleanup_slots =
                                Self::lvalue_stack_footprint(location.as_ref(), true);
                            prop_short_circuit_blocks.push((
                                jump_if_object,
                                cleanup_slots,
                                self.stack.depth(),
                            ));
                        }
                        e = location;
                        continue;
                    }
                    if !needs_prop_short_circuit {
                        self.commit_jump_label(jump_if_object);
                    }
                    break;
                }
                _ => panic!("Bad lvalue in generate_assign"),
            }
        }
        if used_set && !handled_stack {
            self.emit(Op::Swap);
            self.emit(Op::Pop);
            self.pop_stack(1);
        }

        if !prop_short_circuit_blocks.is_empty() {
            let done_label = self.make_jump_label(None);
            self.emit(Op::Jump { label: done_label });
            let normal_path_stack = self.stack.depth();
            for (label, cleanup_slots, entry_stack) in prop_short_circuit_blocks {
                self.commit_jump_label(label);
                self.stack.set_depth(entry_stack);
                self.emit(Op::PutTemp);
                for _ in 0..=cleanup_slots {
                    self.emit(Op::Pop);
                    self.pop_stack(1);
                }
                self.emit(Op::PushTemp);
                self.push_stack(1);
                self.emit(Op::Jump { label: done_label });
                self.stack.set_depth(normal_path_stack);
            }
            self.commit_jump_label(done_label);
        }

        Ok(())
    }

    pub(crate) fn find_name(&self, var: &Variable) -> Name {
        self.name_for_variable
            .get(var.id as usize)
            .copied()
            .flatten()
            .expect("Variable not found")
    }
}
