// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

/// Takes the AST and turns it into a list of opcodes.
use std::collections::HashMap;
use std::sync::Arc;

use tracing::error;

use moor_values::var::Var;
use moor_values::var::Variant;

use crate::ast::{
    Arg, BinaryOp, CatchCodes, Expr, ScatterItem, ScatterKind, Stmt, StmtNode, UnaryOp,
};
use crate::builtins::BUILTINS;
use crate::labels::{JumpLabel, Label, Offset};
use crate::names::{Name, Names, UnboundName};
use crate::opcode::Op::Jump;
use crate::opcode::{Op, ScatterArgs, ScatterLabel};
use crate::parse::{parse_program, CompileOptions};
use crate::program::Program;
use moor_values::model::CompileError;

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
    pub(crate) binding_mappings: HashMap<UnboundName, Name>,
    pub(crate) literals: Vec<Var>,
    pub(crate) loops: Vec<Loop>,
    pub(crate) saved_stack: Option<Offset>,
    pub(crate) cur_stack: usize,
    pub(crate) max_stack: usize,
    pub(crate) fork_vectors: Vec<Vec<Op>>,
    pub(crate) line_number_spans: Vec<(usize, usize)>,
}

impl CodegenState {
    pub fn new(var_names: Names, binding_mappings: HashMap<UnboundName, Name>) -> Self {
        Self {
            ops: vec![],
            jumps: vec![],
            binding_mappings,
            var_names,
            literals: vec![],
            loops: vec![],
            saved_stack: None,
            cur_stack: 0,
            max_stack: 0,
            fork_vectors: vec![],
            line_number_spans: vec![],
        }
    }

    // Create an anonymous jump label at the current position and return its unique ID.
    fn make_jump_label(&mut self, name: Option<Name>) -> Label {
        let id = Label(self.jumps.len() as u16);
        let position = (self.ops.len()).into();
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

    fn emit(&mut self, op: Op) {
        self.ops.push(op);
    }

    fn find_loop(&self, loop_label: &Name) -> Result<&Loop, CompileError> {
        let Some(l) = self.loops.iter().find(|l| {
            if let Some(name) = &l.loop_name {
                name.eq(loop_label)
            } else {
                false
            }
        }) else {
            let loop_name = self.var_names.name_of(loop_label).unwrap();
            return Err(CompileError::UnknownLoopLabel(loop_name.to_string()));
        };
        Ok(l)
    }

    fn push_stack(&mut self, n: usize) {
        self.cur_stack += n;
        if self.cur_stack > self.max_stack {
            self.max_stack = self.cur_stack;
        }
    }

    fn pop_stack(&mut self, n: usize) {
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

    fn add_fork_vector(&mut self, opcodes: Vec<Op>) -> Offset {
        let fv = self.fork_vectors.len();
        self.fork_vectors.push(opcodes);
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
                    self.emit(Op::Put(self.binding_mappings[name]));
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
                    ScatterKind::Required => ScatterLabel::Required(self.binding_mappings[&s.id]),
                    ScatterKind::Optional => ScatterLabel::Optional(
                        self.binding_mappings[&s.id],
                        if s.expr.is_some() {
                            Some(self.make_jump_label(None))
                        } else {
                            None
                        },
                    ),
                    ScatterKind::Rest => ScatterLabel::Rest(self.binding_mappings[&s.id]),
                };
                (s, kind_label)
            })
            .collect();
        let done = self.make_jump_label(None);
        self.emit(Op::Scatter(Box::new(ScatterArgs {
            labels: labels.iter().map(|(_, l)| l.clone()).collect(),
            done,
        })));
        for (s, label) in labels {
            if let ScatterLabel::Optional(_, Some(label)) = label {
                if s.expr.is_none() {
                    continue;
                }
                self.commit_jump_label(label);
                self.generate_expr(s.expr.as_ref().unwrap())?;
                self.emit(Op::Put(self.binding_mappings[&s.id]));
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
                    self.emit(Op::Push(self.binding_mappings[id]));
                    self.push_stack(1);
                }
            }
            Expr::Prop { property, location } => {
                self.generate_expr(location.as_ref())?;
                self.generate_expr(property.as_ref())?;
                if indexed_above {
                    self.emit(Op::PushGetProp);
                    self.push_stack(1);
                }
            }
            _ => {
                panic!("Invalid expr for lvalue: {:?}", expr);
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
                        self.emit(Op::ImmObjid(*oid));
                    }
                    Variant::Int(i) => match i32::try_from(*i) {
                        Ok(n) => self.emit(Op::ImmInt(n)),
                        Err(_) => self.emit(Op::ImmBigInt(*i)),
                    },
                    Variant::Float(f) => self.emit(Op::ImmFloat(*f)),
                    Variant::Err(e) => {
                        self.emit(Op::ImmErr(*e));
                    }
                    _ => {
                        let literal = self.add_literal(v);
                        self.emit(Op::Imm(literal));
                    }
                };
                self.push_stack(1);
            }
            Expr::Id(ident) => {
                self.emit(Op::Push(self.binding_mappings[ident]));
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
                });
            }
            Expr::Prop { location, property } => {
                self.generate_expr(location.as_ref())?;
                self.generate_expr(property.as_ref())?;
                self.emit(Op::GetProp);
                self.pop_stack(1);
            }
            Expr::Pass { args } => {
                self.generate_arg_list(args)?;
                self.emit(Op::Pass);
            }
            Expr::Call { function, args } => {
                // Lookup builtin.
                let Some(id) = BUILTINS.find_builtin(*function) else {
                    error!("Unknown builtin function: {}({:?}", function, args);
                    return Err(CompileError::UnknownBuiltinFunction(function.to_string()));
                };
                self.generate_arg_list(args)?;
                self.emit(Op::FuncCall { id });
            }
            Expr::Verb {
                args,
                verb,
                location,
            } => {
                self.generate_expr(location.as_ref())?;
                self.generate_expr(verb.as_ref())?;
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
                self.emit(Op::Jump { label: end_label });
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
                self.emit(Op::TryCatch { handler_label });
                self.generate_expr(trye.as_ref())?;
                let end_label = self.make_jump_label(None);
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
            Expr::Scatter(scatter, right) => self.generate_scatter_assign(scatter, right)?,
            Expr::Assign { left, right } => self.generate_assign(left, right)?,
        }

        Ok(())
    }

    pub fn generate_stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        // We use the 'canonical' tree line number here for span generation, which should match what
        // unparse generates.
        // TODO In theory we could actually provide both and generate spans for both for situations
        //   where the user is looking at their own not-decompiled copy of the source.
        let line_number = stmt.tree_line_no;
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
                    self.emit(Op::Jump { label: end_label });

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
                id,
                expr,
                body,
                environment_width,
            } => {
                self.generate_expr(expr)?;

                // Note that MOO is 1-indexed, so this is counter value is 1 in LambdaMOO;
                // we use 0 here to make it easier to implement the ForList instruction.
                self.emit(Op::ImmInt(0)); /* loop list index... */
                self.push_stack(1);
                let loop_top = self.make_jump_label(Some(self.binding_mappings[id]));
                self.commit_jump_label(loop_top);
                let end_label = self.make_jump_label(Some(self.binding_mappings[id]));
                self.emit(Op::ForList {
                    id: self.binding_mappings[id],
                    end_label,
                    environment_width: *environment_width as u16,
                });
                self.loops.push(Loop {
                    loop_name: Some(self.binding_mappings[id]),
                    top_label: loop_top,
                    top_stack: self.cur_stack.into(),
                    bottom_label: end_label,
                    bottom_stack: (self.cur_stack - 2).into(),
                });
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::Jump { label: loop_top });
                self.commit_jump_label(end_label);
                self.emit(Op::EndScope {
                    num_bindings: *environment_width as u16,
                });
                self.pop_stack(2);
                self.loops.pop();
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
                let loop_top = self.make_jump_label(Some(self.binding_mappings[id]));
                let end_label = self.make_jump_label(Some(self.binding_mappings[id]));
                self.commit_jump_label(loop_top);
                self.emit(Op::ForRange {
                    id: self.binding_mappings[id],
                    end_label,
                    environment_width: *environment_width as u16,
                });
                self.loops.push(Loop {
                    loop_name: Some(self.binding_mappings[id]),
                    top_label: loop_top,
                    top_stack: self.cur_stack.into(),
                    bottom_label: end_label,
                    bottom_stack: (self.cur_stack - 2).into(),
                });
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Jump { label: loop_top });
                self.commit_jump_label(end_label);
                self.emit(Op::EndScope {
                    num_bindings: *environment_width as u16,
                });
                self.pop_stack(2);
                self.loops.pop();
            }
            StmtNode::While {
                id,
                condition,
                body,
                environment_width,
            } => {
                let loop_start_label =
                    self.make_jump_label(id.as_ref().map(|id| self.binding_mappings[id]));
                self.commit_jump_label(loop_start_label);

                let loop_end_label =
                    self.make_jump_label(id.as_ref().map(|id| self.binding_mappings[id]));
                self.generate_expr(condition)?;
                match id {
                    None => self.emit(Op::While {
                        jump_label: loop_end_label,
                        environment_width: *environment_width as u16,
                    }),
                    Some(id) => self.emit(Op::WhileId {
                        id: self.binding_mappings[id],
                        end_label: loop_end_label,
                        environment_width: *environment_width as u16,
                    }),
                }
                self.pop_stack(1);
                self.loops.push(Loop {
                    loop_name: id.as_ref().map(|id| self.binding_mappings[id]),
                    top_label: loop_start_label,
                    top_stack: self.cur_stack.into(),
                    bottom_label: loop_end_label,
                    bottom_stack: self.cur_stack.into(),
                });
                for s in body {
                    self.generate_stmt(s)?;
                }
                self.emit(Op::Jump {
                    label: loop_start_label,
                });
                self.commit_jump_label(loop_end_label);
                self.emit(Op::EndScope {
                    num_bindings: *environment_width as u16,
                });
                self.loops.pop();
            }
            StmtNode::Fork { id, body, time } => {
                self.generate_expr(time)?;
                // Stash all of main vector in a temporary buffer, then begin compilation of the forked code.
                // Once compiled, we can create a fork vector from the new buffer, and then restore the main vector.
                let stashed_ops = std::mem::take(&mut self.ops);
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::Done);
                let forked_ops = std::mem::take(&mut self.ops);
                let fv_id = self.add_fork_vector(forked_ops);
                self.ops = stashed_ops;
                self.emit(Op::Fork {
                    id: id.as_ref().map(|id| self.binding_mappings[id]),
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
                self.emit(Op::TryExcept {
                    num_excepts: num_excepts as u16,
                    environment_width: *environment_width as u16,
                });
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                let end_label = self.make_jump_label(None);
                self.emit(Op::EndExcept(end_label));
                for (i, ex) in excepts.iter().enumerate() {
                    self.commit_jump_label(labels[i]);
                    self.push_stack(1);
                    if ex.id.is_some() {
                        self.emit(Op::Put(self.binding_mappings[ex.id.as_ref().unwrap()]));
                    }
                    self.emit(Op::Pop);
                    self.pop_stack(1);
                    for stmt in &ex.statements {
                        self.generate_stmt(stmt)?;
                    }
                    if i + 1 < excepts.len() {
                        self.emit(Op::Jump { label: end_label });
                    }
                }
                self.emit(Op::EndScope {
                    num_bindings: *environment_width as u16,
                });
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
                self.push_stack(2); /* continuation value, reason */
                for stmt in handler {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::FinallyContinue);
                self.pop_stack(2);
            }
            StmtNode::Scope { num_bindings, body } => {
                let end_label = self.make_jump_label(None);
                self.emit(Op::BeginScope {
                    num_bindings: *num_bindings as u16,
                    end_label,
                });

                // And then the body within which the bindings are in scope.
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }

                self.emit(Op::EndScope {
                    num_bindings: *num_bindings as u16,
                });
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
                let l = self.binding_mappings[l];
                let l = self.find_loop(&l).expect("invalid loop for break/continue");
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
                let l = self.binding_mappings[l];
                let l = self.find_loop(&l).expect("invalid loop for break/continue");
                self.emit(Op::ExitId(l.top_label));
            }
            StmtNode::Return(Some(expr)) => {
                self.generate_expr(expr)?;
                self.emit(Op::Return);
                self.pop_stack(1);
            }
            StmtNode::Return(None) => self.emit(Op::Return0),
            StmtNode::Expr(e) => {
                self.generate_expr(e)?;
                self.emit(Op::Pop);
                self.pop_stack(1);
            }
        }

        Ok(())
    }

    fn generate_arg_list(&mut self, args: &Vec<Arg>) -> Result<(), CompileError> {
        // TODO: Check recursion down to see if all literal values, and if so reduce to a Imm value with the full list,
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
                    self.emit(normal_op.clone());
                }
                Arg::Splice(s) => {
                    self.generate_expr(s)?;
                    self.emit(splice_op.clone());
                }
            }
            self.pop_stack(pop);
            pop = 1;
            normal_op = Op::ListAddTail;
            splice_op = Op::ListAppend;
        }

        Ok(())
    }
}

pub fn compile(program: &str, options: CompileOptions) -> Result<Program, CompileError> {
    let compile_span = tracing::trace_span!("compile");
    let _compile_guard = compile_span.enter();

    let parse = parse_program(program, options)?;

    // Generate the code into 'cg_state'.
    let mut cg_state = CodegenState::new(parse.names, parse.names_mapping);
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

    let binary = Program {
        literals: cg_state.literals,
        jump_labels: cg_state.jumps,
        var_names: cg_state.var_names,
        main_vector: Arc::new(cg_state.ops),
        fork_vectors: cg_state.fork_vectors,
        line_number_spans: cg_state.line_number_spans,
    };

    Ok(binary)
}
