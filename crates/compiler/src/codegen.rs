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

//! Takes the AST and turns it into a list of opcodes.

use tracing::warn;

use moor_var::{ErrorCode, Symbol, Var, Variant, v_arc_str, v_int, v_sym};

use crate::{
    Op::{
        BeginComprehension, ComprehendList, ComprehendRange, ContinueComprehension, ImmInt, Pop,
        Put,
    },
    ast::{
        Arg, BinaryOp, CallTarget, CatchCodes, Expr, ScatterItem, ScatterKind, Stmt, StmtNode,
        UnaryOp,
    },
    backend::control::{ControlState, LoopFrame},
    backend::emitter::EmitterState,
    backend::operands::OperandState,
    backend::stack::StackState,
    compile_options::CompileOptions,
    frontend::lower::parse_program_frontend,
    parse_tree::Parse,
};
use moor_common::{
    builtins::BUILTINS,
    model::{CompileContext, CompileError, CompileError::InvalidAssignmentTarget},
};
use moor_var::program::{
    labels::{Label, Offset},
    names::{Name, Names, Variable},
    opcode::{
        ComprehensionType, ForRangeOperand, ForSequenceOperand, ListComprehend, Op, Op::Jump,
        RangeComprehend, ScatterLabel,
    },
    program::Program,
};

// Compiler code generation state.
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

    // Create an anonymous jump label at the current position and return its unique ID.
    fn make_jump_label(&mut self, name: Option<Name>) -> Label {
        self.emitter.new_jump_label(name)
    }

    // Adjust the position of a jump label to the current position.
    fn commit_jump_label(&mut self, id: Label) {
        self.emitter.bind_jump_label(id);
    }

    fn add_literal(&mut self, v: &Var) -> Label {
        self.operands.add_literal(v)
    }

    fn add_error_code_operand(&mut self, code: ErrorCode) -> Offset {
        self.operands.add_error_code_operand(code)
    }
    fn add_scatter_table(&mut self, labels: Vec<ScatterLabel>, done: Label) -> Offset {
        self.operands.add_scatter_table(labels, done)
    }

    fn add_lambda_program(&mut self, program: Program, base_line_offset: usize) -> Offset {
        self.operands.add_lambda_program(program, base_line_offset)
    }

    fn add_range_comprehension(&mut self, range_comprehension: RangeComprehend) -> Offset {
        self.operands.add_range_comprehension(range_comprehension)
    }

    fn add_list_comprehension(&mut self, list_comprehension: ListComprehend) -> Offset {
        self.operands.add_list_comprehension(list_comprehension)
    }

    fn add_for_sequence_operand(&mut self, operand: ForSequenceOperand) -> Offset {
        self.operands.add_for_sequence_operand(operand)
    }

    fn add_for_range_operand(&mut self, operand: ForRangeOperand) -> Offset {
        self.operands.add_for_range_operand(operand)
    }

    fn emit(&mut self, op: Op) {
        self.emitter.emit(op);
    }

    fn is_assignable_expr(expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::Id(..) | Expr::Index(..) | Expr::Range { .. } | Expr::Prop { .. }
        )
    }

    fn find_loop(&self, loop_label: &Name) -> Result<&LoopFrame, CompileError> {
        if let Some(loop_frame) = self.control.find_loop(loop_label) {
            return Ok(loop_frame);
        }
        // If we don't find a loop with the given name, that's an error.as
        let loop_name = self.var_names.ident_for_name(loop_label).unwrap();
        Err(CompileError::UnknownLoopLabel(
            CompileContext::new(self.current_line_col),
            loop_name.to_string(),
        ))
    }

    fn push_stack(&mut self, n: usize) {
        self.stack.push(n);
    }

    fn pop_stack(&mut self, n: usize) {
        self.stack.pop(n);
    }

    fn saved_stack_top(&self) -> Option<Offset> {
        self.stack.saved_top()
    }

    fn save_stack_top(&mut self) -> Option<Offset> {
        self.stack.save_top()
    }

    fn restore_stack_top(&mut self, old: Option<Offset>) {
        self.stack.restore_saved_top(old)
    }

    fn add_fork_vector(
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

    fn generate_assign(&mut self, left: &Expr, right: &Expr) -> Result<(), CompileError> {
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
            // Figure out the form of assignment, handle correctly, then walk through
            // chained assignments
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
                _ => {
                    panic!("Bad lvalue in generate_assign")
                }
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

    fn find_name(&self, var: &Variable) -> Name {
        self.name_for_variable
            .get(var.id as usize)
            .copied()
            .flatten()
            .expect("Variable not found")
    }

    fn generate_scatter_assign(
        &mut self,
        scatter: &[ScatterItem],
        right: &Expr,
    ) -> Result<(), CompileError> {
        self.generate_expr(right)?;
        let mut labels = Vec::with_capacity(scatter.len());
        let mut optional_defaults = Vec::new();
        for s in scatter {
            let kind_label = match s.kind {
                ScatterKind::Required => ScatterLabel::Required(self.find_name(&s.id)),
                ScatterKind::Optional => {
                    let default_label = s.expr.as_ref().map(|_| self.make_jump_label(None));
                    if let Some(label) = default_label {
                        optional_defaults.push((s, label));
                    }
                    ScatterLabel::Optional(self.find_name(&s.id), default_label)
                }
                ScatterKind::Rest => ScatterLabel::Rest(self.find_name(&s.id)),
            };
            labels.push(kind_label);
        }
        let done = self.make_jump_label(None);
        let scater_offset = self.add_scatter_table(labels, done);
        self.emit(Op::Scatter(scater_offset));
        for (s, label) in optional_defaults {
            self.commit_jump_label(label);
            self.generate_expr(s.expr.as_ref().unwrap())?;
            self.emit(Op::Put(self.find_name(&s.id)));
            self.emit(Op::Pop);
            self.pop_stack(1);
        }
        self.commit_jump_label(done);
        Ok(())
    }

    fn push_lvalue(&mut self, expr: &Expr, indexed_above: bool) -> Result<(), CompileError> {
        match expr {
            Expr::Range { from, base, to } => {
                self.push_lvalue(base.as_ref(), true)?;
                let old = self.save_stack_top();
                self.generate_expr(from.as_ref())?;
                self.generate_expr(to.as_ref())?;
                self.restore_stack_top(old);
            }
            Expr::Index(lhs, rhs) => {
                self.push_lvalue(lhs.as_ref(), true)?;
                let old = self.save_stack_top();
                self.generate_expr(rhs.as_ref())?;
                self.restore_stack_top(old);
                if indexed_above {
                    self.emit(Op::PushRef);
                    self.push_stack(1);
                }
            }
            Expr::Id(id) => {
                if indexed_above {
                    self.emit(Op::Push(self.find_name(id)));
                    self.push_stack(1);
                }
            }
            Expr::Prop { property, location } => {
                if Self::is_assignable_expr(location.as_ref()) {
                    self.push_lvalue(location.as_ref(), true)?;
                } else {
                    self.generate_expr(location.as_ref())?;
                }
                self.generate_symbol_expr(property.as_ref())?;
                if indexed_above {
                    self.emit(Op::PushGetProp);
                    self.push_stack(1);
                }
            }
            Expr::TypeConstant(c) => {
                return Err(InvalidTypeLiteralAssignment(
                    c.to_literal().to_string(),
                    CompileContext::new(self.current_line_col),
                ));
            }
            _ => {
                return Err(InvalidAssignmentTarget(CompileContext::new(
                    self.current_line_col,
                )));
            }
        }
        Ok(())
    }

    fn generate_codes(&mut self, codes: &CatchCodes) -> Result<usize, CompileError> {
        match codes {
            CatchCodes::Codes(codes) => {
                self.generate_arg_list(codes)?;
                Ok(codes.len())
            }
            CatchCodes::Any => {
                self.emit(Op::ImmInt(0));
                self.push_stack(1);
                Ok(1)
            }
        }
    }

    fn generate_expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Value(v) => {
                match v.variant() {
                    Variant::None => {
                        self.emit(Op::ImmNone);
                    }
                    Variant::Obj(oid) => {
                        self.emit(Op::ImmObjid(oid));
                    }
                    Variant::Int(i) => match i32::try_from(i) {
                        Ok(n) => self.emit(Op::ImmInt(n)),
                        Err(_) => self.emit(Op::ImmBigInt(i)),
                    },
                    Variant::Float(f) => self.emit(Op::ImmFloat(f)),
                    _ => {
                        let literal = self.add_literal(v);
                        self.emit(Op::Imm(literal));
                    }
                };
                self.push_stack(1);
            }
            Expr::Error(code, msg) => {
                // If we have a message, push it on the stack and then push MakeError.
                // Otherwise, just emit an error Literal
                if let Some(msg) = msg {
                    self.generate_expr(msg)?;
                    self.pop_stack(1);
                    let operand_offset = self.add_error_code_operand(*code);
                    self.emit(Op::MakeError(operand_offset));
                } else {
                    self.emit(Op::ImmErr(*code));
                }
                self.push_stack(1);
            }
            Expr::TypeConstant(vt) => {
                self.emit(Op::ImmType(*vt));
                self.push_stack(1);
            }
            Expr::Id(ident) => {
                self.emit(Op::Push(self.find_name(ident)));
                self.push_stack(1);
            }
            Expr::And(left, right) => {
                self.generate_expr(left.as_ref())?;
                let end_label = self.make_jump_label(None);
                self.emit(Op::And(end_label));
                self.pop_stack(1);
                self.generate_expr(right.as_ref())?;
                self.commit_jump_label(end_label);
            }
            Expr::Or(left, right) => {
                self.generate_expr(left.as_ref())?;
                let end_label = self.make_jump_label(None);
                self.emit(Op::Or(end_label));
                self.pop_stack(1);
                self.generate_expr(right.as_ref())?;
                self.commit_jump_label(end_label);
            }
            Expr::Binary(op, l, r) => {
                self.generate_expr(l)?;
                self.generate_expr(r)?;
                let binop = match op {
                    BinaryOp::Add => Op::Add,
                    BinaryOp::Sub => Op::Sub,
                    BinaryOp::Mul => Op::Mul,
                    BinaryOp::Div => Op::Div,
                    BinaryOp::Mod => Op::Mod,
                    BinaryOp::Eq => Op::Eq,
                    BinaryOp::NEq => Op::Ne,
                    BinaryOp::Gt => Op::Gt,
                    BinaryOp::GtE => Op::Ge,
                    BinaryOp::Lt => Op::Lt,
                    BinaryOp::LtE => Op::Le,
                    BinaryOp::Exp => Op::Exp,
                    BinaryOp::In => Op::In,
                    BinaryOp::BitAnd => Op::BitAnd,
                    BinaryOp::BitOr => Op::BitOr,
                    BinaryOp::BitXor => Op::BitXor,
                    BinaryOp::BitShl => Op::BitShl,
                    BinaryOp::BitShr => Op::BitShr,
                    BinaryOp::BitLShr => Op::BitLShr,
                };
                self.emit(binop);
                self.pop_stack(1);
            }
            Expr::Index(lhs, rhs) => {
                self.generate_expr(lhs.as_ref())?;
                let old = self.save_stack_top();
                self.generate_expr(rhs.as_ref())?;
                self.restore_stack_top(old);
                self.emit(Op::Ref);
                self.pop_stack(1);
            }
            Expr::Range { base, from, to } => {
                self.generate_expr(base.as_ref())?;
                let old = self.save_stack_top();
                self.generate_expr(from.as_ref())?;
                self.generate_expr(to.as_ref())?;
                self.restore_stack_top(old);
                self.emit(Op::RangeRef);
                self.pop_stack(2);
            }
            Expr::Length => {
                let saved = self.saved_stack_top().ok_or_else(|| {
                    CompileError::StringLexError(
                        CompileContext::new(self.current_line_col),
                        "Invalid use of '$'".to_string(),
                    )
                })?;
                self.emit(Op::Length(saved));
                self.push_stack(1);
            }
            Expr::Unary(op, expr) => {
                self.generate_expr(expr.as_ref())?;
                self.emit(match op {
                    UnaryOp::Neg => Op::UnaryMinus,
                    UnaryOp::Not => Op::Not,
                    UnaryOp::BitNot => Op::BitNot,
                });
            }
            Expr::Prop { location, property } => {
                self.generate_expr(location.as_ref())?;
                self.generate_symbol_expr(property.as_ref())?;
                self.emit(Op::GetProp);
                self.pop_stack(1);
            }
            Expr::Pass { args } => {
                self.generate_arg_list(args)?;
                self.emit(Op::Pass);
            }
            Expr::Call { function, args } => {
                match function {
                    CallTarget::Builtin(symbol) => {
                        // Existing builtin call logic
                        match BUILTINS.find_builtin(*symbol) {
                            Some(id) => {
                                self.generate_arg_list(args)?;
                                self.emit(Op::FuncCall { id });
                            }
                            None => {
                                if self.compile_options.call_unsupported_builtins {
                                    warn!(
                                        "Unable to resolve builtin function: {symbol}. Transforming into `call_function({symbol}, ...)`."
                                    );
                                    let call_function_id =
                                        BUILTINS.find_builtin("call_function".into()).unwrap();
                                    let mut new_args =
                                        vec![Arg::Normal(Expr::Value(v_sym(*symbol)))];
                                    new_args.extend_from_slice(args);
                                    self.generate_arg_list(&new_args)?;
                                    self.emit(Op::FuncCall {
                                        id: call_function_id,
                                    });
                                } else {
                                    return Err(CompileError::UnknownBuiltinFunction(
                                        CompileContext::new(self.current_line_col),
                                        symbol.to_string(),
                                    ));
                                }
                            }
                        }
                    }
                    CallTarget::Expr(expr) => {
                        // New lambda call logic
                        self.generate_expr(expr.as_ref())?; // Evaluate callable expression
                        self.generate_arg_list(args)?; // Push args list
                        self.emit(Op::CallLambda); // Runtime dispatch
                        self.pop_stack(1); // Pop callable, leave result
                    }
                }
            }
            Expr::Verb {
                args,
                verb,
                location,
            } => {
                self.generate_expr(location.as_ref())?;
                self.generate_symbol_expr(verb.as_ref())?;
                self.generate_arg_list(args)?;
                self.emit(Op::CallVerb);
                self.pop_stack(2);
            }
            Expr::Cond {
                alternative,
                condition,
                consequence,
            } => {
                self.generate_expr(condition.as_ref())?;
                let else_label = self.make_jump_label(None);
                self.emit(Op::IfQues(else_label));
                self.pop_stack(1);
                self.generate_expr(consequence.as_ref())?;
                let end_label = self.make_jump_label(None);
                self.emit(Jump { label: end_label });
                self.pop_stack(1);
                self.commit_jump_label(else_label);
                self.generate_expr(alternative.as_ref())?;
                self.commit_jump_label(end_label);
            }
            Expr::TryCatch {
                codes,
                except,
                trye,
            } => {
                let handler_label = self.make_jump_label(None);
                self.generate_codes(codes)?;
                self.emit(Op::PushCatchLabel(handler_label));
                self.pop_stack(1)   /* codes, catch */;
                let end_label = self.make_jump_label(None);
                self.emit(Op::TryCatch {
                    handler_label,
                    end_label,
                });
                self.generate_expr(trye.as_ref())?;
                self.emit(Op::EndCatch(end_label));
                self.commit_jump_label(handler_label);

                /* After this label, we still have a value on the stack, but now,
                 * instead of it being the value of the main expression, we have
                 * the exception pushed before entering the handler.
                 */
                match except {
                    None => {
                        self.emit(Op::ImmInt(1));
                        self.emit(Op::Ref);
                    }
                    Some(except) => {
                        self.emit(Op::Pop);
                        self.pop_stack(1);
                        self.generate_expr(except.as_ref())?;
                    }
                }
                self.commit_jump_label(end_label);
            }
            Expr::List(l) => {
                self.generate_arg_list(l)?;
            }
            Expr::Map(m) => {
                self.emit(Op::MakeMap);
                self.push_stack(1);
                for (k, v) in m {
                    self.generate_expr(k)?;
                    self.generate_expr(v)?;
                    self.emit(Op::MapInsert);
                    self.pop_stack(2);
                }
            }
            Expr::Flyweight(delegate, slots, contents) => {
                // push delegate, slots, contents. op is # of slots.
                self.generate_expr(delegate.as_ref())?;
                for (k, v) in slots {
                    self.generate_expr(v)?;
                    self.generate_expr(&Expr::Value(v_arc_str(k.as_arc_str())))?;
                }
                match contents {
                    Some(expr) => self.generate_expr(expr.as_ref())?,
                    None => {
                        self.emit(Op::ImmEmptyList);
                        self.push_stack(1);
                    }
                }
                self.emit(Op::MakeFlyweight(slots.len()));
                self.pop_stack(1 + (slots.len() * 2));
            }
            Expr::Scatter(scatter, right) => self.generate_scatter_assign(scatter, right)?,
            Expr::Assign { left, right } => self.generate_assign(left, right)?,
            Expr::Decl {
                id,
                is_const: _,
                expr,
            } => {
                // For code generation, a declaration with assignment is the same as a regular assignment
                match expr {
                    Some(rhs) => {
                        self.generate_assign(&Expr::Id(*id), rhs)?;
                    }
                    None => {
                        // Declaration without assignment - assign 0 as default
                        self.generate_assign(&Expr::Id(*id), &Expr::Value(v_int(0)))?;
                    }
                }
            }
            Expr::ComprehendRange {
                variable,
                end_of_range_register,
                producer_expr,
                from,
                to,
            } => {
                let end_label = self.make_jump_label(None);
                let loop_start_label = self.make_jump_label(None);
                assert_ne!(end_label, loop_start_label);
                let index_variable = self.find_name(variable);
                let end_of_range_register = self.find_name(end_of_range_register);

                self.emit(BeginComprehension(
                    ComprehensionType::Range,
                    end_label,
                    loop_start_label,
                ));

                // range start position set to variable
                self.generate_expr(from.as_ref())?;
                self.emit(Put(index_variable));
                self.emit(Pop);

                // And end of range to register
                self.generate_expr(to.as_ref())?;
                self.emit(Put(end_of_range_register));
                self.emit(Pop);

                self.pop_stack(2);
                self.commit_jump_label(loop_start_label);
                let offset = self.add_range_comprehension(RangeComprehend {
                    position: index_variable,
                    end_of_range_register,
                    end_label,
                });
                self.emit(ComprehendRange(offset));
                self.generate_expr(producer_expr.as_ref())?;
                self.emit(ContinueComprehension(index_variable));
                self.emit(Jump {
                    label: loop_start_label,
                });
                self.commit_jump_label(end_label);
            }
            Expr::ComprehendList {
                variable,
                position_register,
                list_register,
                producer_expr,
                list,
            } => {
                let end_label = self.make_jump_label(None);
                let position_register = self.find_name(position_register);
                let list_register = self.find_name(list_register);
                let item_variable = self.find_name(variable);

                let loop_start_label = self.make_jump_label(None);
                self.emit(BeginComprehension(
                    ComprehensionType::List,
                    end_label,
                    loop_start_label,
                ));

                // Produce the list
                self.generate_expr(list.as_ref())?;
                self.emit(Put(list_register));
                self.emit(Pop);
                self.pop_stack(1);

                // Initial position
                self.emit(ImmInt(1));
                self.emit(Put(position_register));
                self.emit(Pop);

                self.commit_jump_label(loop_start_label);
                let offset = self.add_list_comprehension(ListComprehend {
                    position_register,
                    list_register,
                    item_variable,
                    end_label,
                });
                self.emit(ComprehendList(offset));
                self.generate_expr(producer_expr.as_ref())?;
                self.emit(ContinueComprehension(position_register));
                self.emit(Jump {
                    label: loop_start_label,
                });
                self.commit_jump_label(end_label);
            }
            Expr::Return(Some(expr)) => {
                self.generate_expr(expr)?;
                self.emit(Op::Return);
            }
            Expr::Return(None) => {
                self.emit(Op::Return0);
                self.push_stack(1);
            }
            Expr::Lambda {
                params,
                body,
                self_name,
            } => {
                // Get the line number where the lambda starts
                let lambda_start_line = body.line_col.0;

                // Compile lambda body into standalone Program (following fork vector pattern)
                self.compile_lambda_body(params, body, lambda_start_line)?;

                // If this is a self-referencing lambda, update the MakeLambda opcode
                if let Some(var) = self_name {
                    let self_var_name = self.find_name(var);
                    // Update the last emitted MakeLambda opcode to include self_var
                    if let Some(Op::MakeLambda {
                        scatter_offset,
                        program_offset,
                        self_var: _,
                        num_captured,
                    }) = self.emitter.last_op_mut()
                    {
                        *self
                            .emitter
                            .last_op_mut()
                            .expect("expected last opcode to be MakeLambda") = Op::MakeLambda {
                            scatter_offset: *scatter_offset,
                            program_offset: *program_offset,
                            self_var: Some(self_var_name),
                            num_captured: *num_captured,
                        };
                    }
                }
            }
        }

        Ok(())
    }

    /// Generate code for an expression that should be a symbol.
    /// This optimizes string literals to ImmSymbol instead of Imm.
    fn generate_symbol_expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Value(v) => {
                match v.variant() {
                    Variant::Str(s) => {
                        // String literal in symbol context - emit ImmSymbol
                        let symbol = Symbol::mk(s.as_str());
                        self.emit(Op::ImmSymbol(symbol));
                        self.push_stack(1);
                        Ok(())
                    }
                    _ => {
                        // Fall back to regular expression generation
                        self.generate_expr(expr)
                    }
                }
            }
            _ => {
                // Fall back to regular expression generation
                self.generate_expr(expr)
            }
        }
    }

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
                // Generate the sequence expression
                self.generate_expr(expr)?;

                let value_bind = self.find_name(value_binding);
                let key_bind = key_binding.map(|id| self.find_name(&id));
                let end_label = self.make_jump_label(Some(value_bind));

                // Create operand for the for-sequence state
                let offset = self.add_for_sequence_operand(ForSequenceOperand {
                    value_bind,
                    key_bind,
                    end_label,
                    environment_width: *environment_width as u16,
                });

                // Begin the for-sequence loop (pops sequence, creates scope)
                self.emit(Op::BeginForSequence { operand: offset });
                self.pop_stack(1); // sequence popped by BeginForSequence

                // Iteration point - this is where we jump back to
                let loop_top = self.make_jump_label(Some(value_bind));
                self.commit_jump_label(loop_top);

                // Check bounds and iterate (or jump to end if done)
                self.emit(Op::IterateForSequence);

                // Track loop for break/continue
                self.control.push_loop(LoopFrame {
                    loop_name: Some(value_bind),
                    top_label: loop_top,
                    top_stack: self.stack.depth().into(),
                    bottom_label: end_label,
                    bottom_stack: self.stack.depth().into(), // No stack items to unwind
                });

                // Generate loop body
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }

                // Jump back to iteration check
                self.emit(Jump { label: loop_top });

                // End of loop - emit EndScope first, then the label after it
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
                // Generate from and to expressions (stack: [to, from])
                self.generate_expr(from)?;
                self.generate_expr(to)?;

                let end_label = self.make_jump_label(Some(self.find_name(id)));

                // Create operand for BeginForRange
                let offset = self.add_for_range_operand(ForRangeOperand {
                    loop_variable: self.find_name(id),
                    end_label,
                    environment_width: *environment_width as u16,
                });

                // Emit BeginForRange to pop stack values and create scope
                self.emit(Op::BeginForRange { operand: offset });
                self.pop_stack(2); // BeginForRange pops the from and to values

                // Loop top: IterateForRange checks bounds and sets loop variable
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

                // Generate loop body
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }

                // Jump back to iteration check
                self.emit(Jump { label: loop_top });

                // End of loop - emit EndScope first, then the label after it
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
                // Record the position in main vector where the fork starts
                let fork_main_position = self.emitter.pc();

                // Stash current ops and line number spans to generate fork vector separately
                let stashed_ops = self.emitter.take_ops();
                let stashed_line_spans = std::mem::take(&mut self.line_number_spans);

                // Generate fork body into separate vector
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::Done);
                let forked_ops = self.emitter.take_ops();
                let fork_line_spans = std::mem::take(&mut self.line_number_spans);

                // Restore main vector and continue from where we left off
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

                // And then the body within which the bindings are in scope.
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

    fn generate_arg_list(&mut self, args: &Vec<Arg>) -> Result<(), CompileError> {
        // TODO: Check recursion down to see if all literal common, and if so reduce to a Imm value with the full list,
        //  instead of concatenation with MkSingletonList.
        if args.is_empty() {
            self.emit(Op::ImmEmptyList);
            self.push_stack(1);
            return Ok(());
        }

        let mut normal_op = Op::MakeSingletonList;
        let mut splice_op = Op::CheckListForSplice;
        let mut pop = 0;
        for a in args {
            match a {
                Arg::Normal(a) => {
                    self.generate_expr(a)?;
                    self.emit(normal_op);
                }
                Arg::Splice(s) => {
                    self.generate_expr(s)?;
                    self.emit(splice_op);
                }
            }
            self.pop_stack(pop);
            pop = 1;
            normal_op = Op::ListAddTail;
            splice_op = Op::ListAppend;
        }

        Ok(())
    }

    fn compile_lambda_body(
        &mut self,
        params: &[ScatterItem],
        body: &Stmt,
        base_line_offset: usize,
    ) -> Result<(), CompileError> {
        // Save the current scope depth - this is the depth at which the lambda is defined.
        // Variables at this depth or lower are from the outer context and can be captured.
        let outer_scope_depth = self.control.lambda_scope_depth();

        // Increment scope depth by 2 for the lambda's param isolation scope and body scope.
        // This ensures nested lambdas have the correct outer scope level.
        self.control.push_lambda_scope_depth(2);

        // Create scatter specification for lambda parameters
        let labels: Vec<ScatterLabel> = params
            .iter()
            .map(|param| match param.kind {
                ScatterKind::Required => ScatterLabel::Required(self.find_name(&param.id)),
                ScatterKind::Optional => ScatterLabel::Optional(
                    self.find_name(&param.id),
                    // For lambdas, we don't use jump labels - defaults are handled explicitly
                    None,
                ),
                ScatterKind::Rest => ScatterLabel::Rest(self.find_name(&param.id)),
            })
            .collect();
        let done = self.make_jump_label(None);
        let scatter_offset = self.add_scatter_table(labels, done);

        // Stash current compilation state (following fork vector pattern)
        let stashed_ops = self.emitter.take_ops();
        let stashed_var_names = self.var_names.clone();
        let stashed_jumps = self.emitter.take_jumps();
        let stashed_operands = self.operands.snapshot_and_reset();
        let stashed_line_number_spans = std::mem::take(&mut self.line_number_spans);

        // Reset state for lambda compilation
        self.emitter.reset();
        self.line_number_spans = vec![];

        // Generate code to check optional parameters and evaluate defaults if needed
        // This is done at the start of the lambda body, not through scatter jump labels
        for param in params {
            if let ScatterKind::Optional = param.kind
                && let Some(default_expr) = &param.expr
            {
                let param_name = self.find_name(&param.id);

                // Check if this parameter is unset (equals 0)
                self.emit(Op::Push(param_name));
                self.push_stack(1);

                // Check if it equals 0 (the sentinel value for "needs default")
                self.emit(Op::ImmInt(0));
                self.push_stack(1);
                self.emit(Op::Eq);
                self.pop_stack(1);

                // If equal to 0, evaluate and assign the default
                let skip_default = self.make_jump_label(None);
                self.emit(Op::IfQues(skip_default));
                self.pop_stack(1);

                // Evaluate the default expression and store it
                self.generate_expr(default_expr)?;
                self.emit(Op::Put(param_name));
                self.emit(Op::Pop);
                self.pop_stack(1);

                self.commit_jump_label(skip_default);
            }
        }

        // Compile lambda body as a statement
        self.generate_stmt(body)?;

        // Build standalone Program from compiled state
        let lambda_program = self.operands.take_program_parts().build_program(
            self.var_names.clone(),
            self.emitter.take_jumps(),
            self.emitter.take_ops(),
            std::mem::take(&mut self.line_number_spans),
        );

        // Restore main compilation context
        self.emitter.replace_ops(stashed_ops);
        self.var_names = stashed_var_names;
        self.emitter.replace_jumps(stashed_jumps);
        self.operands.restore(stashed_operands);
        self.line_number_spans = stashed_line_number_spans;

        // Store compiled Program in lambda_programs table with adjusted line numbers
        let program_offset = self.add_lambda_program(lambda_program, base_line_offset);

        // Restore scope depth after lambda compilation
        self.control.set_lambda_scope_depth(outer_scope_depth);

        // Analyze which variables this lambda captures
        // Pass outer_scope_depth so parameterless lambdas know what depth they're at
        let captured_symbols = analyze_lambda_captures(params, body, outer_scope_depth)?;
        let captured_names: Vec<Name> = captured_symbols
            .iter()
            .filter_map(|sym| self.var_names.name_for_ident(*sym))
            .collect();

        // Emit Capture opcodes for each captured variable
        for &name in &captured_names {
            self.emit(Op::Capture(name));
        }

        self.emit(Op::MakeLambda {
            scatter_offset,
            program_offset,
            self_var: None, // Will be set properly for name-sugared forms
            num_captured: captured_names.len() as u16,
        });
        self.push_stack(1);

        Ok(())
    }
}

fn do_compile(parse: Parse, compile_options: CompileOptions) -> Result<Program, CompileError> {
    // Generate the code into 'cg_state'.
    let mut cg_state = CodegenState::new(compile_options, parse.names);
    for x in parse.stmts {
        cg_state.generate_stmt(&x)?;
    }
    cg_state.emit(Op::Done);

    if cg_state.stack.depth() != 0 || cg_state.stack.saved_top().is_some() {
        panic!(
            "Stack is not empty at end of compilation: cur_stack#: {} stack: {:?}",
            cg_state.stack.depth(),
            cg_state.stack.saved_top()
        )
    }

    Ok(cg_state.operands.take_program_parts().build_program(
        cg_state.var_names,
        cg_state.emitter.take_jumps(),
        cg_state.emitter.take_ops(),
        cg_state.line_number_spans,
    ))
}

/// Compile from a program string using the handwritten frontend parser and lowering path.
pub fn compile(program: &str, options: CompileOptions) -> Result<Program, CompileError> {
    let parse = parse_program_frontend(program, options.clone())?;

    do_compile(parse, options)
}

use crate::ast::AstVisitor;
use moor_common::model::CompileError::InvalidTypeLiteralAssignment;
use std::collections::HashSet;

/// A visitor that finds all variable references in lambda bodies for capture analysis
struct CaptureAnalyzer {
    captures: HashSet<Symbol>,
    /// Variables from outer scope that are assigned to (an error condition).
    /// Tracks (symbol, line_col) for better error messages.
    assigned_captures: Vec<(Symbol, (usize, usize))>,
    param_names: HashSet<Symbol>,
    /// The scope level at which the lambda is defined (parameter scope level).
    /// Only variables at this scope level or lower can be captured.
    outer_scope_level: u8,
    /// Current statement's line_col for error reporting
    current_line_col: (usize, usize),
}

impl CaptureAnalyzer {
    fn new(lambda_params: &[ScatterItem], outer_scope_depth: u8) -> Self {
        let param_names: HashSet<Symbol> = lambda_params
            .iter()
            .map(|param| param.id.to_symbol())
            .collect();

        // Determine the outer scope level.
        // For lambdas with parameters, use the parameter's scope_id.
        // For parameterless lambdas, use the passed outer_scope_depth from the codegen state.
        // This ensures variables at the definition site's depth or lower can be captured,
        // while variables defined inside the lambda body (at higher depths) are not captured.
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

    /// Check if a variable reference should be captured.
    /// Uses the Variable's actual scope_id from the AST, not a lookup by symbol name,
    /// because the same symbol can exist at different scope levels (e.g., `prop` used
    /// both in outer scope and inside the lambda's for-loop are different variables).
    fn should_capture(&self, var: &Variable) -> bool {
        // Skip if it's a lambda parameter
        if self.param_names.contains(&var.to_symbol()) {
            return false;
        }

        // Only capture if the variable's scope_id is at or below the outer scope level.
        // Variables defined inside the lambda body have higher scope_ids.
        var.scope_id as u8 <= self.outer_scope_level
    }

    /// Check if a Variable is from outer scope (for assignment error detection).
    /// Unlike should_capture, this checks the Variable's actual scope_id,
    /// so shadowed variables (e.g., `let x = 1; x = 2;`) won't trigger false positives.
    fn is_outer_scope_variable(&self, var: &Variable) -> bool {
        // Skip if it's a lambda parameter
        if self.param_names.contains(&var.to_symbol()) {
            return false;
        }

        // Check if this variable's scope_id indicates it's from outer scope.
        // Variables declared inside the lambda body have higher scope_ids.
        var.scope_id as u8 <= self.outer_scope_level
    }
}

impl AstVisitor for CaptureAnalyzer {
    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Id(var) => {
                // This is a variable reference - check if it should be captured
                // We pass the full Variable so we can use its scope_id, not just the symbol
                if self.should_capture(var) {
                    self.captures.insert(var.to_symbol());
                }
            }
            Expr::Assign { left, right: _ } => {
                // For assignments, check if the target Variable is from outer scope (an error).
                // We use is_outer_scope_variable which checks the Variable's scope_id directly,
                // so shadowed variables (let x = 1; x = 2;) won't trigger false positives.
                if let Expr::Id(var) = left.as_ref()
                    && self.is_outer_scope_variable(var)
                {
                    // Assignment to captured variable is an error - track it with location
                    self.assigned_captures
                        .push((var.to_symbol(), self.current_line_col));
                }
                // Continue walking for right side and other assignment targets
                self.walk_expr(expr);
            }
            Expr::Scatter(items, _) => {
                // Check scatter items for variable assignments to captured variables (an error)
                for item in items {
                    if self.is_outer_scope_variable(&item.id) {
                        // Assignment to captured variable is an error - track it with location
                        self.assigned_captures
                            .push((item.id.to_symbol(), self.current_line_col));
                    }
                }
                // Continue walking for the expression part
                self.walk_expr(expr);
            }
            Expr::Lambda { params, body, .. } => {
                // Visit parameter default expressions first
                for param in params {
                    if let Some(default_expr) = &param.expr {
                        self.visit_expr(default_expr);
                    }
                }

                // For transitive capture: visit the nested lambda's body to find
                // outer variables it references that we must also capture.
                //
                // The nested lambda's own params shadow outer variables with the same
                // name, so temporarily add them to param_names to exclude them.
                let nested_params: Vec<Symbol> = params.iter().map(|p| p.id.to_symbol()).collect();

                for sym in &nested_params {
                    self.param_names.insert(*sym);
                }

                // Visit the nested lambda body - should_capture will correctly filter:
                // - Nested lambda params: now in param_names, so excluded
                // - Nested lambda body locals: not in outer_names, so excluded
                // - Outer variables: in outer_names with lower scope, so captured
                self.visit_stmt(body);

                // Remove the nested params
                for sym in &nested_params {
                    self.param_names.remove(sym);
                }
            }
            _ => {
                // For all other expressions, use the default walking behavior
                self.walk_expr(expr);
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        // Track the current statement's line_col for error reporting
        self.current_line_col = stmt.line_col;
        self.walk_stmt(stmt);
    }

    fn visit_stmt_node(&mut self, stmt_node: &StmtNode) {
        self.walk_stmt_node(stmt_node);
    }
}

/// Analyze a lambda AST to find which variables it references from the outer scope.
/// Returns an error if the lambda attempts to assign to a captured variable.
fn analyze_lambda_captures(
    lambda_params: &[ScatterItem],
    lambda_body: &Stmt,
    outer_scope_depth: u8,
) -> Result<Vec<Symbol>, CompileError> {
    let mut analyzer = CaptureAnalyzer::new(lambda_params, outer_scope_depth);
    analyzer.visit_stmt(lambda_body);

    // Check if any captured variables were assigned to (an error)
    if let Some((assigned_var, line_col)) = analyzer.assigned_captures.first() {
        return Err(CompileError::AssignmentToCapturedVariable(
            CompileContext::new(*line_col),
            *assigned_var,
        ));
    }

    Ok(analyzer.captures.into_iter().collect())
}
