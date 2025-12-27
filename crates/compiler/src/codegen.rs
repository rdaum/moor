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

//! Takes the AST and turns it into a list of opcodes.

use std::sync::Arc;
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
    parse::{CompileOptions, Parse, parse_program},
};
use moor_common::{
    builtins::BUILTINS,
    model::{CompileContext, CompileError, CompileError::InvalidAssignmentTarget},
};
use moor_var::program::{
    labels::{JumpLabel, Label, Offset},
    names::{Name, Names, Variable},
    opcode::{
        ComprehensionType, ForRangeOperand, ForSequenceOperand, ListComprehend, Op, Op::Jump,
        RangeComprehend, ScatterArgs, ScatterLabel,
    },
    program::{PrgInner, Program},
};

pub struct Loop {
    loop_name: Option<Name>,
    top_label: Label,
    top_stack: Offset,
    bottom_label: Label,
    bottom_stack: Offset,
}

// Compiler code generation state.
pub struct CodegenState {
    pub(crate) ops: Vec<Op>,
    pub(crate) jumps: Vec<JumpLabel>,
    pub(crate) var_names: Names,
    pub(crate) literals: Vec<Var>,
    pub(crate) loops: Vec<Loop>,
    pub(crate) saved_stack: Option<Offset>,
    pub(crate) scatter_tables: Vec<ScatterArgs>,
    pub(crate) for_sequence_operands: Vec<ForSequenceOperand>,
    pub(crate) for_range_operands: Vec<ForRangeOperand>,
    pub(crate) range_comprehensions: Vec<RangeComprehend>,
    pub(crate) list_comprehensions: Vec<ListComprehend>,
    pub(crate) error_operands: Vec<ErrorCode>,
    pub(crate) lambda_programs: Vec<Program>,
    pub(crate) cur_stack: usize,
    pub(crate) max_stack: usize,
    pub(crate) fork_vectors: Vec<(usize, Vec<Op>)>,
    pub(crate) line_number_spans: Vec<(usize, usize)>,
    pub(crate) fork_line_number_spans: Vec<Vec<(usize, usize)>>,
    pub(crate) current_line_col: (usize, usize),
    pub(crate) compile_options: CompileOptions,
}

impl CodegenState {
    pub fn new(compile_options: CompileOptions, var_names: Names) -> Self {
        Self {
            ops: vec![],
            jumps: vec![],
            var_names,
            literals: vec![],
            loops: vec![],
            saved_stack: None,
            cur_stack: 0,
            max_stack: 0,
            fork_vectors: vec![],
            scatter_tables: vec![],
            for_sequence_operands: vec![],
            for_range_operands: vec![],
            range_comprehensions: vec![],
            line_number_spans: vec![],
            fork_line_number_spans: vec![],
            current_line_col: (0, 0),
            compile_options,
            list_comprehensions: vec![],
            error_operands: vec![],
            lambda_programs: vec![],
        }
    }

    // Create an anonymous jump label at the current position and return its unique ID.
    fn make_jump_label(&mut self, name: Option<Name>) -> Label {
        let id = Label(self.jumps.len() as u16);
        let position = self.ops.len().into();
        self.jumps.push(JumpLabel { id, name, position });
        id
    }

    // Adjust the position of a jump label to the current position.
    fn commit_jump_label(&mut self, id: Label) {
        let position = self.ops.len();
        let jump = &mut self
            .jumps
            .get_mut(id.0 as usize)
            .expect("Invalid jump fixup");
        let npos = position;
        jump.position = npos.into();
    }

    fn add_literal(&mut self, v: &Var) -> Label {
        // This comparison needs to be done with case sensitivity for strings.
        let lv_pos = self.literals.iter().position(|lv| lv.eq_case_sensitive(v));
        let pos = lv_pos.unwrap_or_else(|| {
            let idx = self.literals.len();
            self.literals.push(v.clone());
            idx
        });
        Label(pos as u16)
    }

    fn add_error_code_operand(&mut self, code: ErrorCode) -> Offset {
        let err_pos = self.error_operands.len();
        self.error_operands.push(code);
        Offset(err_pos as u16)
    }
    fn add_scatter_table(&mut self, labels: Vec<ScatterLabel>, done: Label) -> Offset {
        let st_pos = self.scatter_tables.len();
        self.scatter_tables.push(ScatterArgs { labels, done });
        Offset(st_pos as u16)
    }

    fn add_lambda_program(&mut self, mut program: Program, base_line_offset: usize) -> Offset {
        // Adjust lambda's line number spans to be relative to parent source
        let adjusted_spans: Vec<(usize, usize)> = program
            .line_number_spans()
            .iter()
            .map(|(offset, line_num)| (*offset, line_num + base_line_offset))
            .collect();

        // Update the lambda program's line number spans
        Arc::make_mut(&mut program.0).line_number_spans = adjusted_spans;

        let lp_pos = self.lambda_programs.len();
        self.lambda_programs.push(program);
        Offset(lp_pos as u16)
    }

    fn add_range_comprehension(&mut self, range_comprehension: RangeComprehend) -> Offset {
        let rc_pos = self.range_comprehensions.len();
        self.range_comprehensions.push(range_comprehension);
        Offset(rc_pos as u16)
    }

    fn add_list_comprehension(&mut self, list_comprehension: ListComprehend) -> Offset {
        let lc_pos = self.list_comprehensions.len();
        self.list_comprehensions.push(list_comprehension);
        Offset(lc_pos as u16)
    }

    fn add_for_sequence_operand(&mut self, operand: ForSequenceOperand) -> Offset {
        let fs_pos = self.for_sequence_operands.len();
        self.for_sequence_operands.push(operand);
        Offset(fs_pos as u16)
    }

    fn add_for_range_operand(&mut self, operand: ForRangeOperand) -> Offset {
        let fr_pos = self.for_range_operands.len();
        self.for_range_operands.push(operand);
        Offset(fr_pos as u16)
    }

    fn emit(&mut self, op: Op) {
        self.ops.push(op);
    }

    fn find_loop(&self, loop_label: &Name) -> Result<&Loop, CompileError> {
        for l in self.loops.iter() {
            if l.loop_name.is_none() {
                continue;
            }
            if let Some(name) = &l.loop_name
                && name.eq(loop_label)
            {
                return Ok(l);
            }
        }
        // If we don't find a loop with the given name, that's an error.as
        let loop_name = self.var_names.ident_for_name(loop_label).unwrap();
        Err(CompileError::UnknownLoopLabel(
            CompileContext::new(self.current_line_col),
            loop_name.to_string(),
        ))
    }

    fn push_stack(&mut self, n: usize) {
        self.cur_stack += n;
        if self.cur_stack > self.max_stack {
            self.max_stack = self.cur_stack;
        }
    }

    fn pop_stack(&mut self, n: usize) {
        if self.cur_stack < n {
            panic!(
                "Stack underflow: trying to pop {} items but stack only has {} items",
                n, self.cur_stack
            );
        }
        self.cur_stack -= n;
    }

    fn saved_stack_top(&self) -> Option<Offset> {
        self.saved_stack
    }

    fn save_stack_top(&mut self) -> Option<Offset> {
        let old = self.saved_stack;
        self.saved_stack = Some((self.cur_stack - 1).into());
        old
    }

    fn restore_stack_top(&mut self, old: Option<Offset>) {
        self.saved_stack = old
    }

    fn add_fork_vector(
        &mut self,
        offset: usize,
        opcodes: Vec<Op>,
        line_spans: Vec<(usize, usize)>,
    ) -> Offset {
        let fv = self.fork_vectors.len();
        self.fork_vectors.push((offset, opcodes));
        self.fork_line_number_spans.push(line_spans);
        Offset(fv as u16)
    }

    fn generate_assign(&mut self, left: &Expr, right: &Expr) -> Result<(), CompileError> {
        self.push_lvalue(left, false)?;
        self.generate_expr(right)?;
        match left {
            Expr::Range { .. } => self.emit(Op::PutTemp),
            Expr::Index(..) => self.emit(Op::PutTemp),
            _ => {}
        }
        let mut is_indexed = false;
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
                    self.emit(Op::RangeSet);
                    self.pop_stack(3);
                    e = base;
                    is_indexed = true;
                    continue;
                }
                Expr::Index(lhs, _rhs) => {
                    self.emit(Op::IndexSet);
                    self.pop_stack(2);
                    e = lhs;
                    is_indexed = true;
                    continue;
                }
                Expr::Id(name) => {
                    self.emit(Op::Put(self.find_name(name)));
                    break;
                }
                Expr::Prop {
                    location: _,
                    property: _,
                } => {
                    self.emit(Op::PutProp);
                    self.pop_stack(2);
                    break;
                }
                _ => {
                    panic!("Bad lvalue in generate_assign")
                }
            }
        }
        if is_indexed {
            self.emit(Op::Pop);
            self.emit(Op::PushTemp);
        }

        Ok(())
    }

    fn find_name(&self, var: &Variable) -> Name {
        self.var_names
            .name_for_var(var)
            .expect("Variable not found")
    }

    fn generate_scatter_assign(
        &mut self,
        scatter: &[ScatterItem],
        right: &Expr,
    ) -> Result<(), CompileError> {
        self.generate_expr(right)?;
        let labels: Vec<(&ScatterItem, ScatterLabel)> = scatter
            .iter()
            .map(|s| {
                let kind_label = match s.kind {
                    ScatterKind::Required => ScatterLabel::Required(self.find_name(&s.id)),
                    ScatterKind::Optional => ScatterLabel::Optional(
                        self.find_name(&s.id),
                        if s.expr.is_some() {
                            Some(self.make_jump_label(None))
                        } else {
                            None
                        },
                    ),
                    ScatterKind::Rest => ScatterLabel::Rest(self.find_name(&s.id)),
                };
                (s, kind_label)
            })
            .collect();
        let done = self.make_jump_label(None);
        let scater_offset =
            self.add_scatter_table(labels.iter().map(|(_, l)| l.clone()).collect(), done);
        self.emit(Op::Scatter(scater_offset));
        for (s, label) in labels {
            if let ScatterLabel::Optional(_, Some(label)) = label {
                if s.expr.is_none() {
                    continue;
                }
                self.commit_jump_label(label);
                self.generate_expr(s.expr.as_ref().unwrap())?;
                self.emit(Op::Put(self.find_name(&s.id)));
                self.emit(Op::Pop);
                self.pop_stack(1);
            }
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
                self.generate_expr(location.as_ref())?;
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
                let saved = self.saved_stack_top();
                self.emit(Op::Length(saved.expect("Missing saved stack for '$'")));
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
                    }) = self.ops.last_mut()
                    {
                        *self.ops.last_mut().unwrap() = Op::MakeLambda {
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
        self.line_number_spans.push((self.ops.len(), line_number));
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
                self.loops.push(Loop {
                    loop_name: Some(value_bind),
                    top_label: loop_top,
                    top_stack: self.cur_stack.into(),
                    bottom_label: end_label,
                    bottom_stack: self.cur_stack.into(), // No stack items to unwind
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
                self.loops.pop();
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

                self.loops.push(Loop {
                    loop_name: Some(self.find_name(id)),
                    top_label: loop_top,
                    top_stack: self.cur_stack.into(),
                    bottom_label: end_label,
                    bottom_stack: self.cur_stack.into(),
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
                self.loops.pop();
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
                self.loops.push(Loop {
                    loop_name: id.as_ref().map(|id| self.find_name(id)),
                    top_label: loop_start_label,
                    top_stack: self.cur_stack.into(),
                    bottom_label: loop_end_label,
                    bottom_stack: self.cur_stack.into(),
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
                self.loops.pop();
            }
            StmtNode::Fork { id, body, time } => {
                self.generate_expr(time)?;
                // Record the position in main vector where the fork starts
                let fork_main_position = self.ops.len();

                // Stash current ops and line number spans to generate fork vector separately
                let stashed_ops = std::mem::take(&mut self.ops);
                let stashed_line_spans = std::mem::take(&mut self.line_number_spans);

                // Generate fork body into separate vector
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::Done);
                let forked_ops = std::mem::take(&mut self.ops);
                let fork_line_spans = std::mem::take(&mut self.line_number_spans);

                // Restore main vector and continue from where we left off
                self.ops = stashed_ops;
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
                    if ex.id.is_some() {
                        self.emit(Op::Put(self.find_name(ex.id.as_ref().unwrap())));
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
                let l = self.loops.last().expect("No loop to break/continue from");
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
                let l = self.loops.last().expect("No loop to break/continue from");
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
        let scatter_offset = self.add_scatter_table(labels.clone(), done);

        // Stash current compilation state (following fork vector pattern)
        let stashed_ops = std::mem::take(&mut self.ops);
        let stashed_literals = std::mem::take(&mut self.literals);
        let stashed_var_names = self.var_names.clone();
        let stashed_jumps = std::mem::take(&mut self.jumps);
        let stashed_scatter_tables = std::mem::take(&mut self.scatter_tables);
        let stashed_for_sequence_operands = std::mem::take(&mut self.for_sequence_operands);
        let stashed_for_range_operands = std::mem::take(&mut self.for_range_operands);
        let stashed_range_comprehensions = std::mem::take(&mut self.range_comprehensions);
        let stashed_list_comprehensions = std::mem::take(&mut self.list_comprehensions);
        let stashed_error_operands = std::mem::take(&mut self.error_operands);
        let stashed_lambda_programs = std::mem::take(&mut self.lambda_programs);
        let stashed_fork_vectors = std::mem::take(&mut self.fork_vectors);
        let stashed_line_number_spans = std::mem::take(&mut self.line_number_spans);
        let stashed_fork_line_number_spans = std::mem::take(&mut self.fork_line_number_spans);

        // Reset state for lambda compilation
        self.ops = vec![];
        self.literals = vec![];
        self.jumps = vec![];
        self.scatter_tables = vec![];
        self.for_sequence_operands = vec![];
        self.for_range_operands = vec![];
        self.range_comprehensions = vec![];
        self.list_comprehensions = vec![];
        self.error_operands = vec![];
        self.lambda_programs = vec![];
        self.fork_vectors = vec![];
        self.line_number_spans = vec![];
        self.fork_line_number_spans = vec![];

        // Generate code to check optional parameters and evaluate defaults if needed
        // This is done at the start of the lambda body, not through scatter jump labels
        for (param, label) in params.iter().zip(labels.iter()) {
            if let ScatterKind::Optional = param.kind
                && let Some(default_expr) = &param.expr
            {
                // Extract the already-resolved parameter name from the scatter label
                let param_name = match label {
                    ScatterLabel::Optional(name, _) => *name,
                    _ => panic!("Expected Optional label for optional parameter"),
                };

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
        let lambda_program = Program(Arc::new(PrgInner {
            literals: std::mem::take(&mut self.literals),
            jump_labels: std::mem::take(&mut self.jumps),
            var_names: self.var_names.clone(),
            scatter_tables: std::mem::take(&mut self.scatter_tables),
            for_sequence_operands: std::mem::take(&mut self.for_sequence_operands),
            for_range_operands: std::mem::take(&mut self.for_range_operands),
            range_comprehensions: std::mem::take(&mut self.range_comprehensions),
            list_comprehensions: std::mem::take(&mut self.list_comprehensions),
            error_operands: std::mem::take(&mut self.error_operands),
            lambda_programs: std::mem::take(&mut self.lambda_programs),
            main_vector: std::mem::take(&mut self.ops),
            fork_vectors: std::mem::take(&mut self.fork_vectors),
            line_number_spans: std::mem::take(&mut self.line_number_spans),
            fork_line_number_spans: std::mem::take(&mut self.fork_line_number_spans),
        }));

        // Restore main compilation context
        self.ops = stashed_ops;
        self.literals = stashed_literals;
        self.var_names = stashed_var_names;
        self.jumps = stashed_jumps;
        self.scatter_tables = stashed_scatter_tables;
        self.for_sequence_operands = stashed_for_sequence_operands;
        self.for_range_operands = stashed_for_range_operands;
        self.range_comprehensions = stashed_range_comprehensions;
        self.list_comprehensions = stashed_list_comprehensions;
        self.error_operands = stashed_error_operands;
        self.lambda_programs = stashed_lambda_programs;
        self.fork_vectors = stashed_fork_vectors;
        self.line_number_spans = stashed_line_number_spans;
        self.fork_line_number_spans = stashed_fork_line_number_spans;

        // Store compiled Program in lambda_programs table with adjusted line numbers
        let program_offset = self.add_lambda_program(lambda_program, base_line_offset);

        // Analyze which variables this lambda captures
        let captured_symbols = analyze_lambda_captures(params, body, &self.var_names)?;
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

    if cg_state.cur_stack != 0 || cg_state.saved_stack.is_some() {
        panic!(
            "Stack is not empty at end of compilation: cur_stack#: {} stack: {:?}",
            cg_state.cur_stack, cg_state.saved_stack
        )
    }

    let program = Arc::new(PrgInner {
        literals: cg_state.literals,
        jump_labels: cg_state.jumps,
        var_names: cg_state.var_names,
        scatter_tables: cg_state.scatter_tables,
        range_comprehensions: cg_state.range_comprehensions,
        list_comprehensions: cg_state.list_comprehensions,
        for_sequence_operands: cg_state.for_sequence_operands,
        for_range_operands: cg_state.for_range_operands,
        error_operands: cg_state.error_operands,
        lambda_programs: cg_state.lambda_programs,
        main_vector: cg_state.ops,
        fork_vectors: cg_state.fork_vectors,
        line_number_spans: cg_state.line_number_spans,
        fork_line_number_spans: cg_state.fork_line_number_spans,
    });
    let program = Program(program);
    Ok(program)
}

/// Compile from a program string, starting at the "program" rule.
pub fn compile(program: &str, options: CompileOptions) -> Result<Program, CompileError> {
    let parse = parse_program(program, options.clone())?;

    do_compile(parse, options)
}

use crate::ast::AstVisitor;
use moor_common::model::CompileError::InvalidTypeLiteralAssignment;
use std::collections::HashSet;

/// A visitor that finds all variable references in lambda bodies for capture analysis
struct CaptureAnalyzer<'a> {
    captures: HashSet<Symbol>,
    /// Variables from outer scope that are assigned to (an error condition).
    /// Tracks (symbol, line_col) for better error messages.
    assigned_captures: Vec<(Symbol, (usize, usize))>,
    param_names: HashSet<Symbol>,
    outer_names: &'a Names,
    /// The scope level at which the lambda is defined (parameter scope level).
    /// Only variables at this scope level or lower can be captured.
    outer_scope_level: u8,
    /// Current statement's line_col for error reporting
    current_line_col: (usize, usize),
}

impl<'a> CaptureAnalyzer<'a> {
    fn new(lambda_params: &[ScatterItem], outer_names: &'a Names) -> Self {
        let param_names: HashSet<Symbol> = lambda_params
            .iter()
            .map(|param| param.id.to_symbol())
            .collect();

        // Determine the outer scope level from the parameter scope_id.
        // Parameters are at the lambda's scope, so anything at higher scopes is local to the body.
        let outer_scope_level = lambda_params
            .first()
            .map(|p| p.id.scope_id as u8)
            .unwrap_or(0);

        Self {
            captures: HashSet::new(),
            assigned_captures: Vec::new(),
            param_names,
            outer_names,
            outer_scope_level,
            current_line_col: (0, 0),
        }
    }

    fn should_capture(&self, var_symbol: &Symbol) -> bool {
        // Skip if it's a lambda parameter
        if self.param_names.contains(var_symbol) {
            return false;
        }

        // Check if this variable exists in the outer names
        let Some(name) = self.outer_names.name_for_ident(*var_symbol) else {
            return false;
        };

        // Only capture if the variable is at the outer scope level or lower.
        // Variables at higher scope levels are local to the lambda body.
        name.1 <= self.outer_scope_level
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

impl<'a> AstVisitor for CaptureAnalyzer<'a> {
    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Id(var) => {
                // This is a variable reference - check if it should be captured
                let var_symbol = var.to_symbol();
                if self.should_capture(&var_symbol) {
                    self.captures.insert(var_symbol);
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
            Expr::Lambda { params, .. } => {
                // Nested lambdas are compiled separately with their own capture analysis.
                // Only visit parameter default expressions, not the lambda body.
                for param in params {
                    if let Some(default_expr) = &param.expr {
                        self.visit_expr(default_expr);
                    }
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
    outer_names: &Names,
) -> Result<Vec<Symbol>, CompileError> {
    let mut analyzer = CaptureAnalyzer::new(lambda_params, outer_names);
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
