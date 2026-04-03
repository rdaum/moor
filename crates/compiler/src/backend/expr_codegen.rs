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

use tracing::warn;

use crate::{
    Op::{BeginComprehension, ComprehendList, ComprehendRange, ContinueComprehension, ImmInt, Put},
    ast::{Arg, BinaryOp, CallTarget, CatchCodes, Expr, ScatterItem, ScatterKind, UnaryOp},
    codegen::CodegenState,
};
use moor_common::{
    builtins::BUILTINS,
    model::{CompileContext, CompileError, CompileError::InvalidAssignmentTarget},
};
use moor_var::{Symbol, Variant, v_arc_str, v_int, v_sym};
use moor_var::program::opcode::{ComprehensionType, ListComprehend, Op, Op::Jump, RangeComprehend, ScatterLabel};

impl CodegenState {
    pub(crate) fn generate_codes(&mut self, codes: &CatchCodes) -> Result<usize, CompileError> {
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

    pub(crate) fn generate_expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Value(v) => {
                match v.variant() {
                    Variant::None => self.emit(Op::ImmNone),
                    Variant::Obj(oid) => self.emit(Op::ImmObjid(oid)),
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
            Expr::Call { function, args } => match function {
                CallTarget::Builtin(symbol) => match BUILTINS.find_builtin(*symbol) {
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
                            let mut new_args = vec![Arg::Normal(Expr::Value(v_sym(*symbol)))];
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
                },
                CallTarget::Expr(expr) => {
                    self.generate_expr(expr.as_ref())?;
                    self.generate_arg_list(args)?;
                    self.emit(Op::CallLambda);
                    self.pop_stack(1);
                }
            },
            Expr::Verb { args, verb, location } => {
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
            Expr::TryCatch { codes, except, trye } => {
                let handler_label = self.make_jump_label(None);
                self.generate_codes(codes)?;
                self.emit(Op::PushCatchLabel(handler_label));
                self.pop_stack(1);
                let end_label = self.make_jump_label(None);
                self.emit(Op::TryCatch {
                    handler_label,
                    end_label,
                });
                self.generate_expr(trye.as_ref())?;
                self.emit(Op::EndCatch(end_label));
                self.commit_jump_label(handler_label);
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
            Expr::List(l) => self.generate_arg_list(l)?,
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
            Expr::Decl { id, is_const: _, expr } => match expr {
                Some(rhs) => self.generate_assign(&Expr::Id(*id), rhs)?,
                None => self.generate_assign(&Expr::Id(*id), &Expr::Value(v_int(0)))?,
            },
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
                self.generate_expr(from.as_ref())?;
                self.emit(Put(index_variable));
                self.emit(Op::Pop);
                self.generate_expr(to.as_ref())?;
                self.emit(Put(end_of_range_register));
                self.emit(Op::Pop);
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
                self.generate_expr(list.as_ref())?;
                self.emit(Put(list_register));
                self.emit(Op::Pop);
                self.pop_stack(1);
                self.emit(ImmInt(1));
                self.emit(Put(position_register));
                self.emit(Op::Pop);
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
                self.compile_lambda_body(params, body)?;
                if let Some(var) = self_name {
                    let self_var_name = self.find_name(var);
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

    pub(crate) fn generate_symbol_expr(&mut self, expr: &Expr) -> Result<(), CompileError> {
        match expr {
            Expr::Value(v) => match v.variant() {
                Variant::Str(s) => {
                    let symbol = Symbol::mk(s.as_str());
                    self.emit(Op::ImmSymbol(symbol));
                    self.push_stack(1);
                    Ok(())
                }
                _ => self.generate_expr(expr),
            },
            _ => self.generate_expr(expr),
        }
    }

    pub(crate) fn generate_arg_list(&mut self, args: &Vec<Arg>) -> Result<(), CompileError> {
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

    pub(crate) fn generate_scatter_assign(
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

    pub(crate) fn push_lvalue(
        &mut self,
        expr: &Expr,
        indexed_above: bool,
    ) -> Result<(), CompileError> {
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
                return Err(CompileError::InvalidTypeLiteralAssignment(
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
}
