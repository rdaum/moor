// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use crate::{
    BUILTINS,
    ast::{
        Arg, BinaryOp, CatchCodes, CondArm, ElseArm, ExceptArm, Expr, Expr::ComprehendRange,
        ScatterItem, ScatterKind, Stmt, StmtNode, UnaryOp,
    },
    decompile::DecompileError::{BuiltinNotFound, MalformedProgram},
    parse::Parse,
    var_scope::VarScope,
};
use moor_common::builtins::BuiltinId;
use moor_var::{
    Symbol, Var, Variant,
    program::{
        DeclType,
        labels::{JumpLabel, Label, Offset},
        names::{Name, Variable},
        opcode::{
            ComprehensionType, ForRangeOperand, ForSequenceOperand, ListComprehend, Op,
            RangeComprehend, ScatterLabel,
        },
        program::Program,
    },
    v_float, v_int, v_none, v_obj, v_str,
};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, thiserror::Error)]
pub enum DecompileError {
    #[error("unexpected program end")]
    UnexpectedProgramEnd,
    #[error("name not found: {0:?}")]
    NameNotFound(Name),
    #[error("builtin function not found: #{0:?}")]
    BuiltinNotFound(BuiltinId),
    #[error("label not found: {0:?}")]
    LabelNotFound(Label),
    #[error("malformed program: {0}")]
    MalformedProgram(String),
    #[error("could not decompile statement")]
    CouldNotDecompileStatement,
    #[error("unsupported construct: {0}")]
    UnsupportedConstruct(String),
}

impl From<std::fmt::Error> for DecompileError {
    fn from(_: std::fmt::Error) -> Self {
        DecompileError::MalformedProgram("Format error during unparsing".to_string())
    }
}

struct Decompile {
    /// The program we are decompiling.
    program: Program,
    /// The fork vector # we're decompiling, or None if from the main stream.
    fork_vector: Option<usize>,
    /// The current position in the opcode stream as it is being decompiled.
    position: usize,
    expr_stack: VecDeque<Expr>,
    statements: Vec<Stmt>,
    /// Track which variables have been assigned to in each scope to detect first assignments
    assigned_vars: HashSet<(u16, u16)>, // (variable_id, scope_id)
}

impl Decompile {
    fn opcode_vector(&self) -> &[Op] {
        match self.fork_vector {
            Some(fv) => self.program.fork_vector(Offset(fv as u16)),
            None => self.program.main_vector(),
        }
    }

    /// Returns the next opcode in the program, or an error if the program is malformed.
    fn next(&mut self) -> Result<Op, DecompileError> {
        let opcode_vector = &self.opcode_vector();
        if self.position >= opcode_vector.len() {
            return Err(DecompileError::UnexpectedProgramEnd);
        }
        let op = opcode_vector[self.position].clone();
        self.position += 1;
        Ok(op)
    }
    fn pop_expr(&mut self) -> Result<Expr, DecompileError> {
        self.expr_stack.pop_front().ok_or_else(|| {
            MalformedProgram(format!(
                "expected expression on stack at decompile position {}",
                self.position
            ))
        })
    }
    fn push_expr(&mut self, expr: Expr) {
        self.expr_stack.push_front(expr);
    }
    fn remove_expr_at(&mut self, depth: usize) -> Result<Expr, DecompileError> {
        self.expr_stack.remove(depth).ok_or_else(|| {
            MalformedProgram(format!(
                "expected expression on stack at decompile position {}, depth {}",
                self.position, depth
            ))
        })
    }

    fn find_jump(&self, label: &Label) -> Result<JumpLabel, DecompileError> {
        self.program
            .find_jump(label)
            .ok_or(DecompileError::LabelNotFound(*label))
    }

    pub fn find_literal(&self, label: &Label) -> Result<Var, DecompileError> {
        self.program
            .find_literal(label)
            .ok_or(DecompileError::LabelNotFound(*label))
    }

    fn decompile_statements_until_match<F: Fn(usize, &Op) -> bool>(
        &mut self,
        predicate: F,
    ) -> Result<(Vec<Stmt>, Op), DecompileError> {
        let old_len = self.statements.len();
        let opcode_vector_len = self.opcode_vector().len();
        while self.position < opcode_vector_len {
            let op = &self.opcode_vector()[self.position];
            if predicate(self.position, op) {
                // We'll need a copy of the matching opcode we terminated at.
                let final_op = self.next()?;
                return if self.statements.len() > old_len {
                    Ok((self.statements.split_off(old_len), final_op))
                } else {
                    Ok((vec![], final_op))
                };
            }
            self.decompile()?;
        }
        Err(DecompileError::UnexpectedProgramEnd)
    }

    fn decompile_statements_sub_offset(
        &mut self,
        label: &Label,
        offset: usize,
    ) -> Result<Vec<Stmt>, DecompileError> {
        let jump_label = self.find_jump(label)?; // check that the label exists
        let old_len = self.statements.len();

        while self.position + offset < jump_label.position.0 as usize {
            self.decompile()?;
        }
        if self.statements.len() > old_len {
            Ok(self.statements.split_off(old_len))
        } else {
            Ok(vec![])
        }
    }

    // Decompile statements up to the given label, but not including it.
    fn decompile_statements_up_to(&mut self, label: &Label) -> Result<Vec<Stmt>, DecompileError> {
        self.decompile_statements_sub_offset(label, 1)
    }

    /// Decompile statements up to the given label, including it.
    fn decompile_statements_until(&mut self, label: &Label) -> Result<Vec<Stmt>, DecompileError> {
        self.decompile_statements_sub_offset(label, 0)
    }

    fn can_skip_short_circuit_cleanup(&self, from: usize, to: usize) -> bool {
        if to <= from + 1 {
            return false;
        }
        let opcode_vector = self.opcode_vector();
        if to > opcode_vector.len() {
            return false;
        }
        opcode_vector[from + 1..to]
            .iter()
            .all(|op| matches!(op, Op::PutTemp | Op::Pop | Op::PushTemp | Op::Jump { .. }))
    }

    fn decompile_until_branch_end(
        &mut self,
        label: &Label,
    ) -> Result<(Vec<Stmt>, Label), DecompileError> {
        let jump_label = self.find_jump(label)?; // check that the label exists
        let old_len = self.statements.len();

        while self.position + 1 < jump_label.position.0 as usize {
            self.decompile()?;
        }
        // Next opcode must be the jump to the end of the whole branch
        let opcode = self.next()?;
        let Op::Jump { label } = opcode else {
            return Err(MalformedProgram(
                format!("expected jump opcode at branch end, got: {opcode:?}").to_string(),
            ));
        };
        if self.statements.len() > old_len {
            Ok((self.statements.split_off(old_len), label))
        } else {
            Ok((vec![], label))
        }
    }

    fn decompile(&mut self) -> Result<(), DecompileError> {
        let opcode = self.next()?;

        let line_num = (self.program.line_num_for_position(self.position, 0), 0);
        match opcode {
            Op::If(otherwise_label, environment_width) => {
                let cond = self.pop_expr()?;

                // decompile statements until the position marked in `label`, which is the
                // otherwise branch
                // We scan forward in exclusive mode to avoid the jump to the end of the otherwise
                // branch. That's part of the program flow, but not meaningful for construction
                // of the parse tree.
                let (arm, end_of_otherwise) = self.decompile_until_branch_end(&otherwise_label)?;
                let cond_arm = CondArm {
                    condition: cond,
                    statements: arm,
                    environment_width: environment_width as usize,
                };
                self.statements.push(Stmt::new(
                    StmtNode::Cond {
                        arms: vec![cond_arm],
                        otherwise: None,
                    },
                    line_num,
                ));

                // Decompile to the 'end_of_otherwise' label to get the statements for the
                // otherwise branch.
                let mut otherwise_stmts = self.decompile_statements_until(&end_of_otherwise)?;

                // Resulting thing should be a Scope, or empty, or the scope itself may have been
                // optimized out (no scope-local variables)
                let else_arm = if otherwise_stmts.is_empty() {
                    None
                } else {
                    let (num_bindings, body) = match otherwise_stmts.pop() {
                        Some(Stmt {
                            node: StmtNode::Scope { num_bindings, body },
                            ..
                        }) => (num_bindings, body),
                        Some(body) => (0, vec![body]),
                        None => (0, vec![]),
                    };

                    Some(ElseArm {
                        statements: body,
                        environment_width: num_bindings,
                    })
                };

                let Some(Stmt {
                    node: StmtNode::Cond { arms: _, otherwise },
                    ..
                }) = self.statements.last_mut()
                else {
                    return Err(MalformedProgram(
                        "expected Cond as working tree".to_string(),
                    ));
                };
                *otherwise = else_arm;
            }
            Op::Eif(end_label, environment_width) => {
                let cond = self.pop_expr()?;
                // decompile statements until the position marked in `label`, which is the
                // end of the branch statement
                let (cond_statements, _) = self.decompile_until_branch_end(&end_label)?;
                let cond_arm = CondArm {
                    condition: cond,
                    statements: cond_statements,
                    environment_width: environment_width as usize,
                };
                // Add the arm
                let Some(Stmt {
                    node: StmtNode::Cond { arms, otherwise: _ },
                    ..
                }) = self.statements.last_mut()
                else {
                    return Err(MalformedProgram(
                        "expected Cond as working tree".to_string(),
                    ));
                };
                arms.push(cond_arm);
            }
            Op::BeginForSequence { operand } => {
                let list = self.pop_expr()?;
                let ForSequenceOperand {
                    value_bind,
                    key_bind,
                    end_label: label,
                    environment_width,
                } = self.program.for_sequence_operand(operand).clone();

                // Next opcode should be IterateForSequence
                let Op::IterateForSequence = self.next()? else {
                    return Err(MalformedProgram(
                        "expected IterateForSequence after BeginForSequence".to_string(),
                    ));
                };

                let body = self.decompile_statements_until(&label)?;
                let value_id = self.decompile_name(&value_bind)?;
                let key_id = match key_bind {
                    None => None,
                    Some(key_bind) => Some(self.decompile_name(&key_bind)?),
                };

                self.statements.push(Stmt::new(
                    StmtNode::ForList {
                        value_binding: value_id,
                        key_binding: key_id,
                        expr: list,
                        body,
                        environment_width: environment_width as usize,
                    },
                    line_num,
                ));
            }
            Op::IterateForSequence => {
                // This should have been handled by BeginForSequence
                return Err(MalformedProgram(
                    "IterateForSequence without preceding BeginForSequence".to_string(),
                ));
            }
            Op::BeginForRange { operand } => {
                let to = self.pop_expr()?;
                let from = self.pop_expr()?;
                let ForRangeOperand {
                    loop_variable,
                    end_label: label,
                    environment_width,
                } = self.program.for_range_operand(operand).clone();

                // Next opcode should be IterateForRange
                let Op::IterateForRange = self.next()? else {
                    return Err(MalformedProgram(
                        "expected IterateForRange after BeginForRange".to_string(),
                    ));
                };

                let body = self.decompile_statements_until(&label)?;
                let id = self.decompile_name(&loop_variable)?;

                self.statements.push(Stmt::new(
                    StmtNode::ForRange {
                        id,
                        from,
                        to,
                        body,
                        environment_width: environment_width as usize,
                    },
                    line_num,
                ));
            }
            Op::IterateForRange => {
                // This should have been handled by BeginForRange
                return Err(MalformedProgram(
                    "IterateForRange without preceding BeginForRange".to_string(),
                ));
            }
            Op::While {
                jump_label: loop_end_label,
                environment_width,
            } => {
                // A "while" is actually a:
                //      a conditional expression
                //      this While opcode (with end label)
                //      a series of statements
                //      a jump back to the conditional expression
                let cond = self.pop_expr()?;
                let body = self.decompile_statements_until(&loop_end_label)?;
                self.statements.push(Stmt::new(
                    StmtNode::While {
                        id: None,
                        condition: cond,
                        body,
                        environment_width: environment_width as usize,
                    },
                    line_num,
                ));
            }
            // Same as above, but with id.
            // TODO: we may want to consider collapsing these two VM opcodes
            Op::WhileId {
                id,
                end_label: loop_end_label,
                environment_width,
            } => {
                // A "while" is actually a:
                //      a conditional expression
                //      this While opcode (with end label)
                //      a series of statements
                //      a jump back to the conditional expression
                let cond = self.pop_expr()?;
                let body = self.decompile_statements_until(&loop_end_label)?;
                let id = self.decompile_name(&id)?;
                self.statements.push(Stmt::new(
                    StmtNode::While {
                        id: Some(id),
                        condition: cond,
                        body,
                        environment_width: environment_width as usize,
                    },
                    line_num,
                ));
            }
            Op::Exit { stack: _, label } => {
                let position = self.find_jump(&label)?.position;
                if position.0 < self.position as u16 {
                    self.statements
                        .push(Stmt::new(StmtNode::Continue { exit: None }, line_num));
                } else {
                    self.statements
                        .push(Stmt::new(StmtNode::Break { exit: None }, line_num));
                }
            }
            Op::ExitId(label) => {
                let jump_label = self.find_jump(&label)?;
                // Whether it's a break or a continue depends on whether the jump is forward or
                // backward from the current position.
                let jump_label_name =
                    self.decompile_name(&jump_label.name.expect("jump label must have name"))?;
                let s = if jump_label.position.0 < self.position as u16 {
                    StmtNode::Continue {
                        exit: Some(jump_label_name),
                    }
                } else {
                    StmtNode::Break {
                        exit: Some(jump_label_name),
                    }
                };

                self.statements.push(Stmt::new(s, line_num));
            }
            Op::Fork { fv_offset, id } => {
                // Delay time should be on stack.
                let delay_time = self.pop_expr()?;

                // Grab the fork vector at `fv_offset` and start decompilation from there, using
                // a brand new decompiler
                let mut fork_decompile = Decompile {
                    program: self.program.clone(),
                    fork_vector: Some(fv_offset.0 as _),
                    position: 0,
                    expr_stack: self.expr_stack.clone(),
                    statements: vec![],
                    assigned_vars: self.assigned_vars.clone(),
                };
                let fv_len = self.program.fork_vector(fv_offset).len();
                while fork_decompile.position < fv_len {
                    fork_decompile.decompile()?;
                }
                let id = id.map(|x| self.decompile_name(&x).unwrap());
                self.statements.push(Stmt::new(
                    StmtNode::Fork {
                        id,
                        time: delay_time,
                        body: fork_decompile.statements,
                    },
                    line_num,
                ));
            }
            Op::Pop => {
                let expr = self.pop_expr()?;
                self.statements
                    .push(Stmt::new(StmtNode::Expr(expr), line_num));
            }
            Op::Dup => {
                let expr =
                    self.expr_stack.front().cloned().ok_or_else(|| {
                        MalformedProgram("expected expression on stack".to_string())
                    })?;
                self.push_expr(expr);
            }
            Op::Swap => {
                let a = self.pop_expr()?;
                let b = self.pop_expr()?;
                self.push_expr(a);
                self.push_expr(b);
            }
            Op::Return => {
                let expr = self.pop_expr()?;
                self.push_expr(Expr::Return(Some(Box::new(expr))));
            }
            Op::Return0 => {
                self.push_expr(Expr::Return(None));
            }
            Op::Done => {
                let opcode_vector = &self.opcode_vector();
                if self.position != opcode_vector.len() {
                    return Err(MalformedProgram("expected end of program".to_string()));
                }
            }
            Op::Imm(literal_label) => {
                self.push_expr(Expr::Value(self.find_literal(&literal_label)?));
            }
            Op::Push(varname) => {
                let varname = self.decompile_name(&varname)?;
                self.push_expr(Expr::Id(varname));
            }
            Op::Put(varname) => {
                let expr = self.pop_expr()?;
                let varname = self.decompile_name(&varname)?;

                // Check if this is the first assignment to this variable in this scope
                let var_key = (varname.id, varname.scope_id);
                let is_first_assignment = !self.assigned_vars.contains(&var_key);

                // Look up the declaration info
                let name = self.program.var_names().name_for_var(&varname);
                let decl_info = name.and_then(|n| self.program.var_names().decls.get(&n));

                // Check if this is a named function (lambda with self_name matching the variable)
                let is_named_function = matches!(
                    &expr,
                    Expr::Lambda { self_name: Some(self_var), .. }
                    if self_var.id == varname.id && self_var.scope_id == varname.scope_id
                );

                // Check if this should be treated as a declaration:
                // 1. It's the first assignment to this variable in this scope
                // 2. Either:
                //    a. The variable was declared with DeclType::Let and is a local (scope_id != 0)
                //    b. It's a named function (lambda with self_name) - these are always declarations
                let should_be_declaration = is_first_assignment
                    && (is_named_function
                        || (varname.scope_id != 0
                            && decl_info
                                .map(|d| d.decl_type == DeclType::Let)
                                .unwrap_or(false)));

                if should_be_declaration {
                    // Mark as assigned
                    self.assigned_vars.insert(var_key);

                    let is_const = decl_info.map(|d| d.constant).unwrap_or(false);

                    self.push_expr(Expr::Decl {
                        id: varname,
                        is_const,
                        expr: Some(Box::new(expr)),
                    });
                } else {
                    // Mark as assigned even for subsequent assignments
                    self.assigned_vars.insert(var_key);

                    self.push_expr(Expr::Assign {
                        left: Box::new(Expr::Id(varname)),
                        right: Box::new(expr),
                    });
                }
            }
            Op::And(label) => {
                let left = self.pop_expr()?;
                self.decompile_statements_until(&label)?;
                let right = self.pop_expr()?;
                self.push_expr(Expr::And(Box::new(left), Box::new(right)));
            }
            Op::Or(label) => {
                let left = self.pop_expr()?;
                self.decompile_statements_until(&label)?;
                let right = self.pop_expr()?;
                self.push_expr(Expr::Or(Box::new(left), Box::new(right)));
            }
            Op::UnaryMinus => {
                let expr = self.pop_expr()?;
                self.push_expr(Expr::Unary(UnaryOp::Neg, Box::new(expr)));
            }
            Op::Not => {
                let expr = self.pop_expr()?;
                self.push_expr(Expr::Unary(UnaryOp::Not, Box::new(expr)));
            }
            Op::BitNot => {
                let expr = self.pop_expr()?;
                self.push_expr(Expr::Unary(UnaryOp::BitNot, Box::new(expr)));
            }
            Op::GetProp | Op::PushGetProp => {
                let prop = self.pop_expr()?;
                let obj = self.pop_expr()?;
                self.push_expr(Expr::Prop {
                    location: Box::new(obj),
                    property: Box::new(prop),
                });
            }
            Op::Eq
            | Op::Ne
            | Op::Lt
            | Op::Le
            | Op::Gt
            | Op::Ge
            | Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Mod
            | Op::Exp
            | Op::In
            | Op::BitAnd
            | Op::BitOr
            | Op::BitXor
            | Op::BitShl
            | Op::BitShr
            | Op::BitLShr => {
                let right = self.pop_expr()?;
                let left = self.pop_expr()?;
                let operator = BinaryOp::from_binary_opcode(opcode);
                self.push_expr(Expr::Binary(operator, Box::new(left), Box::new(right)));
            }
            Op::Ref | Op::PushRef => {
                let right = self.pop_expr()?;
                let left = self.pop_expr()?;
                self.push_expr(Expr::Index(Box::new(left), Box::new(right)));
            }
            Op::RangeRef => {
                let e1 = self.pop_expr()?;
                let e2 = self.pop_expr()?;
                let base = self.pop_expr()?;
                self.push_expr(Expr::Range {
                    base: Box::new(base),
                    from: Box::new(e2),
                    to: Box::new(e1),
                });
            }
            Op::PutTemp => {}
            Op::IndexSet => {
                let rval = self.pop_expr()?;
                let index = self.pop_expr()?;
                let base = self.pop_expr()?;
                self.push_expr(Expr::Assign {
                    left: Box::new(Expr::Index(Box::new(base), Box::new(index))),
                    right: Box::new(rval),
                });

                // skip forward to and beyond PushTemp
                let opcode_vector_len = self.opcode_vector().len();
                while self.position < opcode_vector_len {
                    let op = self.next()?;
                    if let Op::PushTemp = op {
                        break;
                    }
                }
            }
            Op::IndexSetAt(offset) => {
                let offset = offset.0 as usize;
                let base = self.remove_expr_at(offset + 2)?;
                let index = self.remove_expr_at(offset + 1)?;
                let rval = self.remove_expr_at(offset)?;
                if offset > 0 {
                    let _ = self.remove_expr_at(0)?;
                }
                self.push_expr(Expr::Assign {
                    left: Box::new(Expr::Index(Box::new(base), Box::new(index))),
                    right: Box::new(rval),
                });

                let opcode_vector_len = self.opcode_vector().len();
                while self.position < opcode_vector_len {
                    let op = self.next()?;
                    if let Op::Swap = op {
                        if self.position < opcode_vector_len {
                            match &self.opcode_vector()[self.position] {
                                Op::Pop => {
                                    self.position += 1;
                                }
                                Op::Put(_) => {
                                    self.position += 1;
                                    if self.position < opcode_vector_len
                                        && matches!(self.opcode_vector()[self.position], Op::Pop)
                                    {
                                        self.position += 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                        break;
                    }
                }

                if self.position < opcode_vector_len
                    && let Op::Jump { label } = self.opcode_vector()[self.position]
                {
                    let jump = self.find_jump(&label)?;
                    let target = jump.position.0 as usize;
                    if self.can_skip_short_circuit_cleanup(self.position, target) {
                        self.position = target;
                    }
                }
            }
            Op::RangeSet => {
                let rval = self.pop_expr()?;
                let (to, from, base) = (self.pop_expr()?, self.pop_expr()?, self.pop_expr()?);
                self.push_expr(Expr::Assign {
                    left: Box::new(Expr::Range {
                        base: Box::new(base),
                        from: Box::new(from),
                        to: Box::new(to),
                    }),
                    right: Box::new(rval),
                });

                // skip forward to and beyond PushTemp
                let opcode_vector_len = self.opcode_vector().len();
                while self.position < opcode_vector_len {
                    let op = self.next()?;
                    if let Op::PushTemp = op {
                        break;
                    }
                }
            }
            Op::RangeSetAt(offset) => {
                let offset = offset.0 as usize;
                let base = self.remove_expr_at(offset + 3)?;
                let from = self.remove_expr_at(offset + 2)?;
                let to = self.remove_expr_at(offset + 1)?;
                let rval = self.remove_expr_at(offset)?;
                if offset > 0 {
                    let _ = self.remove_expr_at(0)?;
                }
                self.push_expr(Expr::Assign {
                    left: Box::new(Expr::Range {
                        base: Box::new(base),
                        from: Box::new(from),
                        to: Box::new(to),
                    }),
                    right: Box::new(rval),
                });

                let opcode_vector_len = self.opcode_vector().len();
                while self.position < opcode_vector_len {
                    let op = self.next()?;
                    if let Op::Swap = op {
                        if self.position < opcode_vector_len {
                            match &self.opcode_vector()[self.position] {
                                Op::Pop => {
                                    self.position += 1;
                                }
                                Op::Put(_) => {
                                    self.position += 1;
                                    if self.position < opcode_vector_len
                                        && matches!(self.opcode_vector()[self.position], Op::Pop)
                                    {
                                        self.position += 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                        break;
                    }
                }

                if self.position < opcode_vector_len
                    && let Op::Jump { label } = self.opcode_vector()[self.position]
                {
                    let jump = self.find_jump(&label)?;
                    let target = jump.position.0 as usize;
                    if self.can_skip_short_circuit_cleanup(self.position, target) {
                        self.position = target;
                    }
                }
            }
            Op::FuncCall { id } => {
                let args = self.pop_expr()?;
                let Some(function) = BUILTINS.name_of(id) else {
                    return Err(BuiltinNotFound(id));
                };

                // Have to reconstruct arg list ...
                let Expr::List(args) = args else {
                    return Err(MalformedProgram(
                        format!("expected list of args, got {args:?} instead").to_string(),
                    ));
                };
                self.push_expr(Expr::Call {
                    function: crate::ast::CallTarget::Builtin(function),
                    args,
                })
            }
            Op::CallVerb => {
                let args = self.pop_expr()?;
                let verb = self.pop_expr()?;
                let obj = self.pop_expr()?;
                let Expr::List(args) = args else {
                    return Err(MalformedProgram("expected list of args".to_string()));
                };
                self.push_expr(Expr::Verb {
                    location: Box::new(obj),
                    verb: Box::new(verb),
                    args,
                })
            }
            Op::ImmEmptyList => {
                self.push_expr(Expr::List(vec![]));
            }
            Op::MakeSingletonList => {
                let expr = self.pop_expr()?;
                self.push_expr(Expr::List(vec![Arg::Normal(expr)]));
            }
            Op::MakeMap => {
                self.push_expr(Expr::Map(vec![]));
            }
            Op::MapInsert => {
                let (v, k) = (self.pop_expr()?, self.pop_expr()?);
                let map = self.pop_expr()?;
                let Expr::Map(mut map) = map else {
                    return Err(MalformedProgram("expected map".to_string()));
                };
                map.push((k, v));
                self.push_expr(Expr::Map(map));
            }
            Op::ListAddTail | Op::ListAppend => {
                let e = self.pop_expr()?;
                let list = self.pop_expr()?;
                let Expr::List(mut list) = list else {
                    return Err(MalformedProgram("expected list".to_string()));
                };
                let arg = if opcode == Op::ListAddTail {
                    Arg::Normal(e)
                } else {
                    Arg::Splice(e)
                };
                list.push(arg);
                self.push_expr(Expr::List(list));
            }
            Op::MakeFlyweight(num_slots) => {
                let mut slots = Vec::with_capacity(num_slots);
                let contents = self.pop_expr()?;
                // Contents can be any expression now, not just a list
                let contents = match contents {
                    Expr::List(ref list) if list.is_empty() => None,
                    _ => Some(Box::new(contents)),
                };

                for _ in 0..num_slots {
                    let k = self.pop_expr()?;
                    let v = self.pop_expr()?;
                    let k = match k {
                        Expr::Value(s) => match s.variant() {
                            Variant::Str(s) => Symbol::mk(s.as_str()),
                            _ => {
                                return Err(MalformedProgram(
                                    "expected string for flyweight slot name".to_string(),
                                ));
                            }
                        },
                        _ => {
                            return Err(MalformedProgram(
                                "expected string for flyweight slot name".to_string(),
                            ));
                        }
                    };
                    slots.push((k, v));
                }
                // To maintain equivalency for testing, these need to be reversed.
                slots.reverse();
                let delegate = self.pop_expr()?;
                self.push_expr(Expr::Flyweight(Box::new(delegate), slots, contents));
            }
            Op::MakeError(offset) => {
                let error_code = *self.program.error_operand(offset);
                // The value for the error is on the stack.
                let value = self.pop_expr()?;
                self.push_expr(Expr::Error(error_code, Some(Box::new(value))))
            }
            Op::Pass => {
                let args = self.pop_expr()?;
                let Expr::List(args) = args else {
                    return Err(MalformedProgram("expected list of args".to_string()));
                };
                self.push_expr(Expr::Pass { args });
            }
            Op::Scatter(sa) => {
                let mut scatter_items = vec![];
                // We need to go through and collect the jump labels for the expressions in
                // optional scatters. We will use this later to compute the end of optional
                // assignment expressions in the scatter.
                let mut opt_jump_labels = vec![];
                let scatter_table = self.program.scatter_table(sa).clone();
                for scatter_label in scatter_table.labels.iter() {
                    if let ScatterLabel::Optional(_, Some(label)) = scatter_label {
                        opt_jump_labels.push(label);
                    }
                }
                opt_jump_labels.push(&scatter_table.done);

                let mut label_pos = 0;
                for scatter_label in scatter_table.labels.iter() {
                    let scatter_item = match scatter_label {
                        ScatterLabel::Required(id) => {
                            let id = self.decompile_name(id)?;
                            ScatterItem {
                                kind: ScatterKind::Required,
                                id,
                                expr: None,
                            }
                        }
                        ScatterLabel::Rest(id) => {
                            let id = self.decompile_name(id)?;
                            ScatterItem {
                                kind: ScatterKind::Rest,
                                id,
                                expr: None,
                            }
                        }
                        ScatterLabel::Optional(id, Some(_)) => {
                            // The labels inside each optional scatters are jumps to the _start_ of the
                            // expression inside it, so to know the end of the expression we will look at the
                            // next label after it (if any), or done.
                            let next_label = opt_jump_labels[label_pos + 1];
                            label_pos += 1;
                            let _ = self.decompile_statements_up_to(next_label)?;
                            let assign_expr = self.pop_expr()?;
                            let Expr::Assign { left: _, right } = assign_expr else {
                                return Err(MalformedProgram(
                                    format!(
                                        "expected assign for optional scatter assignment; got {assign_expr:?}"
                                    )
                                    .to_string(),
                                ));
                            };
                            // We need to eat the 'pop' after us that is present in the program
                            // stream.
                            // It's not clear to me why we have to do this vs the way LambdaMOO
                            // is decompiling this, but this is what works, otherwise we get
                            // a hanging pop.
                            let _ = self.next()?;

                            let id = self.decompile_name(id)?;
                            ScatterItem {
                                kind: ScatterKind::Optional,
                                id,
                                expr: Some(*right),
                            }
                        }
                        ScatterLabel::Optional(id, None) => {
                            let id = self.decompile_name(id)?;
                            ScatterItem {
                                kind: ScatterKind::Optional,
                                id,
                                expr: None,
                            }
                        }
                    };
                    scatter_items.push(scatter_item);
                }
                let e = self.pop_expr()?;
                self.push_expr(Expr::Scatter(scatter_items, Box::new(e)));
            }
            Op::PushCatchLabel(_) => {
                // ignore and consume, we don't need it.
            }
            Op::TryExcept {
                num_excepts,
                environment_width,
                ..
            } => {
                let mut except_arms = Vec::with_capacity(num_excepts as usize);
                for _ in 0..num_excepts {
                    let codes_expr = self.pop_expr()?;
                    let catch_codes = match codes_expr {
                        Expr::Value(_) => CatchCodes::Any,
                        Expr::List(codes) => CatchCodes::Codes(codes),
                        _ => {
                            return Err(MalformedProgram("invalid try/except codes".to_string()));
                        }
                    };

                    // Each arm has a statement, but we will get to that later.
                    except_arms.push(ExceptArm {
                        id: None,
                        codes: catch_codes,
                        statements: vec![],
                    });
                }
                // Decompile the body.
                // Means decompiling until we hit EndExcept, so scan forward for that.
                // TODO: make sure that this doesn't fail with nested try/excepts?
                let (body, end_except) =
                    self.decompile_statements_until_match(|_, o| matches!(o, Op::EndExcept(_)))?;
                let Op::EndExcept(end_label) = end_except else {
                    return Err(MalformedProgram("expected EndExcept".to_string()));
                };

                // Order of except arms is reversed in the program, so reverse it back before we
                // decompile the except arm statements.
                except_arms.reverse();

                // Now each of the arms has a statement potentially with an assignment label.
                // So it can look like:  Put, Pop, Statements, Jump (end_except), ...
                // or   Pop, Statements, Jump (end_except).
                // So first look for the Put
                for arm in &mut except_arms {
                    let mut next_opcode = self.next()?;
                    if let Op::Put(varname) = next_opcode
                        && let Ok(varname) = self.decompile_name(&varname)
                    {
                        arm.id = Some(varname);
                        next_opcode = self.next()?;
                    }
                    let Op::Pop = next_opcode else {
                        return Err(MalformedProgram("expected Pop".to_string()));
                    };

                    // Scan forward until the jump, decompiling as we go.
                    let end_label_position = self.find_jump(&end_label)?.position.0;
                    let (statements, _) =
                        self.decompile_statements_until_match(|position, o| {
                            if position == (end_label_position as usize) {
                                return true;
                            }
                            if let Op::Jump { label } = o {
                                label == &end_label
                            } else {
                                false
                            }
                        })?;
                    arm.statements = statements;
                }

                // We need to rewind the position by one opcode, it seems.
                // TODO this is not the most elegant. we're being too greedy above
                self.position -= 1;
                self.statements.push(Stmt::new(
                    StmtNode::TryExcept {
                        body,
                        excepts: except_arms,
                        environment_width: environment_width as usize,
                    },
                    line_num,
                ));
            }
            Op::TryFinally {
                end_label: _label,
                environment_width,
            } => {
                // decompile body up until the EndFinally
                let (body, _) =
                    self.decompile_statements_until_match(|_, op| matches!(op, Op::EndFinally))?;
                let (handler, _) = self
                    .decompile_statements_until_match(|_, op| matches!(op, Op::FinallyContinue))?;
                self.statements.push(Stmt::new(
                    StmtNode::TryFinally {
                        body,
                        handler,
                        environment_width: environment_width as usize,
                    },
                    line_num,
                ));
            }
            Op::TryCatch {
                handler_label: label,
                ..
            } => {
                let codes_expr = self.pop_expr()?;
                let catch_codes = match codes_expr {
                    Expr::Value(_) => CatchCodes::Any,
                    Expr::List(codes) => CatchCodes::Codes(codes),
                    _ => {
                        return Err(MalformedProgram("invalid try/except codes".to_string()));
                    }
                };
                // decompile forward to the EndCatch
                let _handler = self.decompile_statements_up_to(&label)?;
                let Op::EndCatch(end_label) = self.next()? else {
                    return Err(MalformedProgram("expected EndCatch".to_string()));
                };
                let try_expr = self.pop_expr()?;

                // There's either an except (Pop, then expr) or not (Val(1), Ref).
                let next = self.next()?;
                let except = match next {
                    Op::Pop => {
                        self.decompile_statements_until(&end_label)?;
                        Some(Box::new(self.pop_expr()?))
                    }
                    Op::ImmInt(v) => {
                        // V must be '1' and next opcode must be ref
                        if v != 1 {
                            return Err(MalformedProgram(
                                "expected literal '1' in catch".to_string(),
                            ));
                        };
                        let Op::Ref = self.next()? else {
                            return Err(MalformedProgram("expected Ref".to_string()));
                        };
                        None
                    }
                    _ => {
                        return Err(MalformedProgram(
                            format!("bad end to catch expr (expected Pop or Val/Ref, got {next:?}")
                                .to_string(),
                        ));
                    }
                };
                self.push_expr(Expr::TryCatch {
                    trye: Box::new(try_expr),
                    codes: catch_codes,
                    except,
                });
            }
            Op::Length(_) => {
                self.push_expr(Expr::Length);
            }
            Op::IfQues(label) => {
                let condition = self.pop_expr();
                // Read up to the jump, decompiling as we go.
                self.decompile_statements_up_to(&label)?;

                // Check if this is a full ternary (with Jump) or a degenerate one-armed
                // conditional (used for optional parameter defaults in lambdas)
                let label_position = self.find_jump(&label)?.position.0 as usize;
                let current_opcode = &self.opcode_vector()[self.position];

                if let Op::Jump { label: jump_label } = current_opcode {
                    // Full ternary: condition ? consequence | alternative
                    let jump_label = *jump_label;
                    self.position += 1; // consume the Jump
                    let consequent = self.pop_expr();
                    // Now decompile up to and including jump_label's offset
                    self.decompile_statements_until(&jump_label)?;
                    let alternate = self.pop_expr();
                    let e = Expr::Cond {
                        condition: Box::new(condition?),
                        consequence: Box::new(consequent?),
                        alternative: Box::new(alternate?),
                    };
                    self.push_expr(e);
                } else if self.position + 1 == label_position {
                    // Degenerate one-armed conditional (no alternative):
                    // Used for optional parameter defaults: if (param == 0) param = default;
                    // decompile_statements_up_to stops 1 position before the label.
                    // The consequence was already decompiled and should be an assignment
                    // statement on the statement list (Put followed by Pop made it a stmt).
                    // Since this doesn't produce a value, we don't push anything.
                    // The assignment statement was already added by the Put/Pop handling.
                } else {
                    return Err(MalformedProgram(format!(
                        "expected Jump at position {} for IfQues, label at {}, got {:?}",
                        self.position, label_position, current_opcode
                    )));
                }
            }
            Op::CheckListForSplice => {
                let sp_expr = self.pop_expr()?;
                let e = Expr::List(vec![Arg::Splice(sp_expr)]);
                self.push_expr(e);
            }
            Op::PutProp => {
                let rvalue = self.pop_expr()?;
                let propname = self.pop_expr()?;
                let e = self.pop_expr()?;
                let assign = Expr::Assign {
                    left: Box::new(Expr::Prop {
                        location: Box::new(e),
                        property: Box::new(propname),
                    }),
                    right: Box::new(rvalue),
                };
                self.push_expr(assign);
            }
            Op::PutPropAt {
                offset,
                jump_if_object: _,
            } => {
                let offset = offset.0 as usize;
                let location = self.remove_expr_at(offset + 2)?;
                let property = self.remove_expr_at(offset + 1)?;
                let rvalue = self.remove_expr_at(offset)?;
                if offset > 0 {
                    let _ = self.remove_expr_at(0)?;
                }
                let assign = Expr::Assign {
                    left: Box::new(Expr::Prop {
                        location: Box::new(location),
                        property: Box::new(property),
                    }),
                    right: Box::new(rvalue),
                };
                self.push_expr(assign);

                let opcode_vector_len = self.opcode_vector().len();
                while self.position < opcode_vector_len {
                    let op = self.next()?;
                    if let Op::Swap = op {
                        if self.position < opcode_vector_len {
                            match &self.opcode_vector()[self.position] {
                                Op::Pop => {
                                    self.position += 1;
                                }
                                Op::Put(_) => {
                                    self.position += 1;
                                    if self.position < opcode_vector_len
                                        && matches!(self.opcode_vector()[self.position], Op::Pop)
                                    {
                                        self.position += 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                        break;
                    }
                }

                if self.position < opcode_vector_len
                    && let Op::Jump { label } = self.opcode_vector()[self.position]
                {
                    let jump = self.find_jump(&label)?;
                    let target = jump.position.0 as usize;
                    if self.can_skip_short_circuit_cleanup(self.position, target) {
                        self.position = target;
                    }
                }
            }
            Op::Jump { .. } | Op::PushTemp => {
                // unreachable!("should have been handled other decompilation branches")
            }
            Op::EndCatch(_) | Op::FinallyContinue | Op::EndExcept(_) | Op::EndFinally => {
                // Early exit; main logic is in TRY_FINALLY or CATCH etc case, above
                // TODO: MOO has "return ptr - 2;"  -- doing something with the iteration, that
                //   I may not be able to do with the current structure. See if I need to
                unreachable!("should have been handled other decompilation branches")
            }
            Op::ImmNone => {
                self.push_expr(Expr::Value(v_none()));
            }
            Op::ImmInt(i) => {
                self.push_expr(Expr::Value(v_int(i as i64)));
            }
            Op::ImmBigInt(i) => {
                self.push_expr(Expr::Value(v_int(i)));
            }
            Op::ImmFloat(f) => {
                self.push_expr(Expr::Value(v_float(f)));
            }
            Op::ImmErr(e) => {
                self.push_expr(Expr::Error(e, None));
            }
            Op::ImmObjid(oid) => {
                self.push_expr(Expr::Value(v_obj(oid)));
            }
            Op::ImmSymbol(sym) => {
                self.push_expr(Expr::Value(v_str(&sym.as_string())));
            }
            Op::ImmType(t) => self.push_expr(Expr::TypeConstant(t)),
            Op::BeginScope {
                num_bindings,
                end_label,
            } => {
                let block_statements = self.decompile_statements_until(&end_label)?;
                self.statements.push(Stmt::new(
                    StmtNode::Scope {
                        num_bindings: num_bindings as usize,
                        body: block_statements,
                    },
                    line_num,
                ));
            }
            Op::EndScope { .. } => {
                // Noop.
            }
            Op::BeginComprehension(comprehension_type, _, loop_start_label) => {
                let assign_statements = self.decompile_statements_until(&loop_start_label)?;
                match comprehension_type {
                    ComprehensionType::Range => {
                        // We should have two assignments -- begin and end range
                        assert_eq!(assign_statements.len(), 2);

                        // Next must be ComprehendRange
                        let next = self.next()?;

                        let Op::ComprehendRange(offset) = next else {
                            return Err(MalformedProgram(
                                "malformed range comprehension".to_string(),
                            ));
                        };
                        let RangeComprehend {
                            position,
                            end_of_range_register,
                            end_label,
                        } = self.program.range_comprehension(offset).clone();
                        self.decompile_statements_until(&end_label)?;
                        let producer_expr = self.pop_expr()?;

                        let StmtNode::Expr(Expr::Assign {
                            left: _,
                            right: from,
                        }) = &assign_statements[0].node
                        else {
                            return Err(MalformedProgram(
                                "malformed range comprehension".to_string(),
                            ));
                        };

                        let StmtNode::Expr(Expr::Assign { left: _, right: to }) =
                            &assign_statements[1].node
                        else {
                            return Err(MalformedProgram(
                                "malformed range comprehension".to_string(),
                            ));
                        };

                        let variable = self.decompile_name(&position)?;
                        let end_of_range_register = self.decompile_name(&end_of_range_register)?;
                        self.push_expr(ComprehendRange {
                            variable,
                            end_of_range_register,
                            producer_expr: Box::new(producer_expr),
                            from: from.clone(),
                            to: to.clone(),
                        })
                    }
                    ComprehensionType::List => {
                        // we have two assignments, the list and initial position. we only care
                        // about the value of the list
                        assert_eq!(assign_statements.len(), 2);

                        let next_opcode = self.next()?;
                        let Op::ComprehendList(offset) = next_opcode else {
                            return Err(MalformedProgram(
                                "malformed list comprehension".to_string(),
                            ));
                        };
                        let ListComprehend {
                            position_register,
                            list_register,
                            item_variable,
                            end_label,
                        } = self.program.list_comprehension(offset).clone();
                        self.decompile_statements_until(&end_label)?;
                        let producer_expr = self.pop_expr()?;

                        let StmtNode::Expr(Expr::Assign {
                            left: _,
                            right: list,
                        }) = &assign_statements[0].node
                        else {
                            return Err(MalformedProgram(
                                "malformed list comprehension".to_string(),
                            ));
                        };

                        let variable = self.decompile_name(&item_variable)?;
                        let list_register = self.decompile_name(&list_register)?;
                        let position_register = self.decompile_name(&position_register)?;
                        self.push_expr(Expr::ComprehendList {
                            variable,
                            position_register,
                            list_register,
                            producer_expr: Box::new(producer_expr),
                            list: list.clone(),
                        })
                    }
                }
            }
            Op::ContinueComprehension(..)
            | Op::ComprehendRange { .. }
            | Op::ComprehendList { .. } => {
                // noop, handled above
            }
            Op::Capture(_) => {
                // Capture opcodes are handled implicitly during lambda creation
                // They don't produce expressions themselves
            }
            Op::MakeLambda {
                scatter_offset,
                program_offset,
                self_var,
                ..
            } => {
                // Retrieve lambda program and scatter specification
                let lambda_program = self.program.lambda_program(program_offset);
                let scatter_spec = self.program.scatter_table(scatter_offset).clone();

                // Decompile lambda body from standalone Program
                let lambda_body = self.decompile_lambda_program(lambda_program)?;

                // Convert scatter spec to parameter list
                let params = self.decompile_scatter_params(&scatter_spec)?;

                // Extract self_var name if present (indicates named function)
                let self_name = self_var
                    .and_then(|name| self.program.var_names().find_variable(&name).cloned());

                self.push_expr(Expr::Lambda {
                    params,
                    body: Box::new(lambda_body),
                    self_name,
                });
            }
            Op::CallLambda => {
                let args = self.pop_expr()?;
                let lambda_expr = self.pop_expr()?;

                // Convert args expression to argument list
                let args = match args {
                    Expr::List(args) => args,
                    _ => {
                        return Err(MalformedProgram(
                            "expected list of args for lambda call".to_string(),
                        ));
                    }
                };

                // For decompilation, check if the lambda expression is a simple variable reference
                // If so, generate a direct function call instead of using __lambda_call__
                match lambda_expr {
                    Expr::Id(_) => {
                        // Direct function call to a named function
                        self.push_expr(Expr::Call {
                            function: crate::ast::CallTarget::Expr(Box::new(lambda_expr)),
                            args,
                        });
                    }
                    _ => {
                        // Complex lambda expression, generate lambda call syntax
                        self.push_expr(Expr::Call {
                            function: crate::ast::CallTarget::Expr(Box::new(lambda_expr)),
                            args,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn decompile_name(&self, name: &Name) -> Result<Variable, DecompileError> {
        self.program
            .var_names()
            .find_variable(name)
            .cloned()
            .ok_or(DecompileError::NameNotFound(*name))
    }

    fn decompile_lambda_program(&self, lambda_program: &Program) -> Result<Stmt, DecompileError> {
        // Create separate decompiler for lambda's standalone Program
        let mut lambda_decompile = Decompile {
            program: lambda_program.clone(),
            fork_vector: None,
            position: 0,
            expr_stack: VecDeque::new(),
            statements: vec![],
            assigned_vars: HashSet::new(),
        };

        // Decompile lambda body
        let opcode_vector_len = lambda_decompile.opcode_vector().len();
        while lambda_decompile.position < opcode_vector_len {
            lambda_decompile.decompile()?;
        }

        // Handle both single-statement (expression) lambdas and multi-statement lambdas
        if lambda_decompile.statements.len() == 1 {
            // Single statement - return directly (expression lambda like `{x} => x + 1`)
            Ok(lambda_decompile.statements.into_iter().next().unwrap())
        } else if lambda_decompile.statements.is_empty() {
            // Empty lambda body - wrap in scope with no statements
            Ok(Stmt::new(
                StmtNode::Scope {
                    num_bindings: 0,
                    body: vec![],
                },
                (0, 0),
            ))
        } else {
            // Multi-statement lambda (fn () ... endfn) - wrap in Scope
            // Note: We set num_bindings to 0 because we cannot reliably determine the
            // correct count from bytecode. The bytecode doesn't distinguish between new
            // declarations and reassignments to existing variables. When num_bindings is 0,
            // codegen will not emit BeginScope/EndScope opcodes, which is safe because the
            // lambda already has its own scope in the VM.
            Ok(Stmt::new(
                StmtNode::Scope {
                    num_bindings: 0,
                    body: lambda_decompile.statements,
                },
                (0, 0),
            ))
        }
    }

    fn decompile_scatter_params(
        &self,
        scatter_spec: &moor_var::program::opcode::ScatterArgs,
    ) -> Result<Vec<ScatterItem>, DecompileError> {
        let mut params = Vec::new();

        for label in &scatter_spec.labels {
            let (kind, id, expr) = match label {
                ScatterLabel::Required(name) => {
                    let var = self.decompile_name(name)?;
                    (ScatterKind::Required, var, None)
                }
                ScatterLabel::Optional(name, _label) => {
                    let var = self.decompile_name(name)?;
                    // TODO: Handle optional parameter default expressions
                    // For now, we don't decompile the default expression
                    (ScatterKind::Optional, var, None)
                }
                ScatterLabel::Rest(name) => {
                    let var = self.decompile_name(name)?;
                    (ScatterKind::Rest, var, None)
                }
            };

            params.push(ScatterItem { kind, id, expr });
        }

        Ok(params)
    }
}

/// Reconstruct a parse tree from opcodes.
pub fn program_to_tree(program: &Program) -> Result<Parse, DecompileError> {
    let variables = VarScope {
        variables: program.var_names().decls.values().cloned().collect(),
        scopes: vec![],
        scope_id_stack: vec![],
        num_registers: 0,
        scope_id_seq: 0,
    };
    let mut decompile = Decompile {
        program: program.clone(),
        fork_vector: None,
        position: 0,
        expr_stack: Default::default(),
        statements: vec![],
        assigned_vars: HashSet::new(),
    };
    let opcode_vector_len = decompile.opcode_vector().len();
    while decompile.position < opcode_vector_len {
        decompile.decompile()?;
    }

    Ok(Parse {
        stmts: decompile.statements,
        names: program.var_names().clone(),
        variables,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        CompileOptions,
        ast::assert_trees_match_recursive,
        codegen::compile,
        decompile::program_to_tree,
        parse::{Parse, parse_program},
        unparse::annotate_line_numbers,
    };
    use test_case::test_case;

    fn parse_decompile(program_text: &str) -> (Parse, Parse) {
        let parse_1 = parse_program(program_text, CompileOptions::default()).unwrap();
        let binary = compile(program_text, CompileOptions::default()).unwrap();
        let mut parse_2 = program_to_tree(&binary).unwrap();
        annotate_line_numbers(1, &mut parse_2.stmts);
        (parse_1, parse_2)
    }

    // Test that multi-statement lambdas parse, compile, and decompile successfully
    #[test]
    fn test_multi_statement_lambda_parses_compiles_and_decompiles() {
        let program = r#"f = fn ()
            let x = 1;
            return x + 1;
        endfn;
        return f();"#;

        // Parse should succeed
        let parsed = parse_program(program, CompileOptions::default());
        assert!(parsed.is_ok(), "Parse should succeed: {:?}", parsed.err());

        // Compile should succeed
        let compiled = compile(program, CompileOptions::default());
        assert!(
            compiled.is_ok(),
            "Compile should succeed: {:?}",
            compiled.err()
        );

        // Decompile should now succeed (bug fixed)
        let binary = compiled.unwrap();
        let decompiled = program_to_tree(&binary);
        assert!(
            decompiled.is_ok(),
            "Decompile should succeed: {:?}",
            decompiled.err()
        );
    }

    #[test_case("if (1) return 2; endif"; "simple if")]
    #[test_case("if (1) return 2; else return 3; endif"; "if_else")]
    #[test_case("if (1) return 2; elseif (2) return 3; endif"; "if_elseif")]
    #[test_case(
        "if (1) return 2; elseif (2) return 3; else return 4; endif";
        "if_elseif_else"
    )]
    #[test_case("while (1) return 2; endwhile"; "simple while")]
    #[test_case(
        "while (1) if (1 == 2) break; else continue; endif endwhile";
        "while_break_continue"
    )]
    #[test_case("while chuckles (1) return 2; endwhile"; "while_labelled")]
    #[test_case(
        "while chuckles (1) if (1 == 2) break chuckles; else continue chuckles; endif endwhile";
        "while_labelled_break_continue"
    )]
    #[test_case("for x in (1) return 2; endfor"; "simple for in")]
    #[test_case("for x in (1) if (1 == 2) break; else continue; endif endfor"; "for_in_break_continue")]
    #[test_case("for x in (1) if (1 == 2) break x; else continue x; endif endfor"; "for_in_labelled_break_continue")]
    #[test_case("for x in [1..5] return 2; endfor"; "for_range")]
    #[test_case("try return 1; except a (E_INVARG) return 2; endtry"; "try_except")]
    #[test_case("try return 1; except a (E_INVARG) return 2; except b (E_PROPNF) return 3; endtry"; "try_except_2")]
    #[test_case("try return 1; finally return 2; endtry"; "try_finally")]
    #[test_case("return setadd({1,2}, 3);"; "builtin")]
    #[test_case("return {1,2,3};"; "list")]
    #[test_case("return {1,2,3,@{1,2,3}};"; "list_splice")]
    #[test_case("return {1,2,3,@{1,2,3},4};"; "list_splice_2")]
    #[test_case("return -1;"; "unary")]
    #[test_case("return 1 + 2;"; "binary")]
    #[test_case("return 1 + 2 * 3;"; "binary_precedence")]
    #[test_case(
        "return -(1 + 2 * (3 - 4) / 5 % 6);";
        "unary_and_binary_and_paren_precedence"
    )]
    #[test_case(
        "return 1 == 2 != 3 < 4 <= 5 > 6 >= 7;";
        "equality_inequality_relational"
    )]
    #[test_case("return 1 && 2 || 3 && 4;"; "logical_and_or")]
    #[test_case("x = 1; return x;"; "assignment")]
    #[test_case("return x[1];"; "index")]
    #[test_case("return x[1..2];"; "range")]
    #[test_case("return x:y(1,2,3);"; "call_verb")]
    #[test_case(r#"return x:("y")(1,2,3);"#; "call_verb_expr")]
    #[test_case(r#"return $ansi:(this.some_function)();"#; "call_verb_computed_prop")]
    #[test_case("{connection} = args;"; "scatter")]
    #[test_case("{connection, player} = args;"; "scatter_2")]
    #[test_case("{connection, player, ?arg3} = args;"; "scatter_3")]
    #[test_case("{connection, player, ?arg3, @arg4} = args;"; "scatter_4")]
    #[test_case("x = `x + 1 ! e_propnf, E_PERM => 17';"; "catch_expr")]
    #[test_case("x = `x + 1 ! e_propnf, E_PERM';"; "catch_expr_no_result")]
    #[test_case("x = `x + 1 ! ANY => 17';"; "any_catch_expr")]
    #[test_case("x = `x + 1 ! ANY';"; "any_catch_expr_no_result")]
    #[test_case("a[1..2] = {3,4};"; "range_set")]
    #[test_case("a[1] = {3,4};"; "index_set")]
    #[test_case("1 ? 2 | 3;"; "ternary")]
    #[test_case("x.y = 1;"; "prop_assign")]
    #[test_case("try return x; except (E_VARNF) endtry; if (x) return 1; endif"; "if_after_try")]
    #[test_case("2 ? 0 | caller_perms();"; "regression_builtin_after_ternary")]
    #[test_case(r#"options="test"; return #0.(options);"#; "sysprop expr")]
    #[test_case(r#"{?package = 5} = args;"#; "scatter optional assignment")]
    #[test_case(r#"{?package = $nothing} = args;"#; "scatter optional assignment from property")]
    #[test_case(r#"5; fork (5) 1; endfork 2;"#; "unlabelled fork decompile")]
    #[test_case(r#"5; fork tst (5) 1; endfork 2;"#; "labelled fork decompile")]
    #[test_case(r#"[ 1 -> 2, 3 -> 4 ];"#; "map")]
    fn test_case_decompile_matches(prg: &str) {
        let (parse, decompiled) = parse_decompile(prg);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    fn test_decompile_lexical_scope_block() {
        let program = r#"begin
            let a = 5;
        end"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    // A big verb to verify that decompilation works for more than just simple cases.
    fn test_a_complicated_function() {
        let program = r#"
        brief = args && args[1];
        player:tell(this:namec_for_look_self(brief));
        things = this:visible_of(setremove(this:contents(), player));
        integrate = {};
        try
            if (this.integration_enabled)
              for i in (things)
                if (this:ok_to_integrate(i) && (!brief || !is_player(i)))
                  integrate = {@integrate, i};
                  things = setremove(things, i);
                endif
              endfor
              "for i in (this:obvious_exits(player))";
              for i in (this:exits())
                if (this:ok_to_integrate(i))
                  integrate = setadd(integrate, i);
                  "changed so prevent exits from being integrated twice in the case of doors and the like";
                endif
              endfor
            endif
        except (E_INVARG)
            player:tell("Error in integration: ");
        endtry
        if (!brief)
          desc = this:description(integrate);
          if (desc)
            player:tell_lines(desc);
          else
            player:tell("You see nothing special.");
          endif
        endif
        "there's got to be a better way to do this, but.";
        if (topic = this:topic_msg())
          if (0)
            this.topic_sign:show_topic();
          else
            player:tell(this.topic_sign:integrate_room_msg());
          endif
        endif
        "this:tell_contents(things, this.ctype);";
        this:tell_contents(things);
        "#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    fn regress() {
        let program = r#""Usage:  @make-setter <object>.<property>";
"Write the standard :set_foo verb for a property.";
"Works by copying $code_utils:standard_set_property";
if (!player.programmer)
player:tell("I don't understand that.");
elseif ((!dobjstr) || (!(spec = $code_utils:parse_propref(dobjstr))))
player:tell_lines($code_utils:verb_usage());
return;
elseif ($command_utils:object_match_failed(what = player:my_match_object(whatname = spec[1]), whatname))
elseif (!$perm_utils:controls(player, what))
player:tell("You don't own ", what:title(), ".");
elseif (!(info = property_info(what, propname = spec[2])))
player:tell(what:titlec(), " has no \"", propname, "\" property.");
elseif ((index(propname, " ") || index(propname, "\"")) || index(propname, "*"))
player:tell("The standard setter verb won't work; you can't have a space, a quotation mark, or an asterisk in a verb name.");
elseif (!$perm_utils:controls_prop(player, what, propname))
player:tell("You don't own ", what:title(), ".", propname, ".");
elseif (index(info[2], "c") && (!player.wizard))
player:tell(what:titlec(), ".", propname, " is +c, so the standard setter verb won't work.  @chmod it, or write your own verb.");
elseif ($code_utils:find_verb_named_1_based(what, verbname = "set_" + propname))
player:tell(what:titlec(), " already has a ", verbname, " verb.");
else
code = listdelete(verb_code($code_utils, "standard_set_property"), 2);
for v in (verbs(what))
if (match(v, "^%(set_[^ ]+%|.* set_[^ ]+%)"))
vname = strsub($string_utils:explode(v)[1], "*", "");
if (((verb_code(what, vname) == code) && ((oldinfo = verb_info(what, vname))[1] == player)) && (oldinfo[2] == "rx"))
set_task_perms(player);
set_verb_info(what, vname, {player, "rx", (v + " ") + verbname});
player:tell(what:titlec(), " already had a standard setter verb; it's now named \"", (v + " ") + verbname, "\".");
return;
endif
endif
endfor
set_task_perms(player);
add_verb(what, {player, "rx", verbname}, {"this", "none", "this"});
set_verb_code(what, verbname, code);
if ((player.wizard && (!index(info[2], "c"))) && (!info[1].wizard))
player:tell(what:titlec(), ".", propname, " is !c and owned by ", info[1]:name(), ", so this verb doesn't need wizard permissions.  \"@chown ", what, ":", verbname, " to ", info[1], "\" to give ", info[1]:name(), " the verb.");
endif
player:tell("Wrote ", what:title(), ":", verbname, ".");
endif
return 0 && "Automatically Added Return";
"Metadata 202106";"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    fn regression_scatter() {
        let program = r#"{things, ?nothingstr = "nothing", ?andstr = " and ", ?commastr = ", ", ?finalcommastr = ","} = args;"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    fn test_local_scatter() {
        let program = r#"begin
            let {things, ?nothingstr = "nothing", ?andstr = " and ", ?commastr = ", ", ?finalcommastr = ","} = args;
        end"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    fn test_flyweight() {
        let program = r#"flywt = < #1, .colour = "orange", .z = 5, {#2, #4, "a"}>;"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    fn test_map_decompile() {
        let program = r#"[ 1 -> 2, 3 -> 4 ];"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    fn test_for_range_comprehension() {
        let program = r#"return { x * 2 for x in [1..3] };"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    fn test_for_list_comprehension() {
        let program = r#"return { x * 2 for x in ({1,2,3}) };"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    // Tests for multi-statement lambda decompilation
    // These test that fn () ... endfn syntax with multiple statements can be decompiled
    // Note: We use named function syntax (fn name() ... endfn) because it produces
    // consistent Decl nodes in both parse and decompile. The assignment form
    // (f = fn() ... endfn) produces Decl in parse but Assign in decompile.

    #[test]
    fn test_multi_statement_lambda_decompile() {
        // Multi-statement lambda using named fn syntax
        // Note: Must use `let` for local variables inside lambdas
        let program = r#"fn f()
            let x = 1;
            return x + 1;
        endfn
        return f();"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    fn test_multi_statement_lambda_with_params_decompile() {
        // Multi-statement lambda with parameters
        // Note: Must use `let` for local variables inside lambdas
        let program = r#"fn f(a, b)
            let sum = a + b;
            return sum * 2;
        endfn
        return f(1, 2);"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    fn test_multi_statement_lambda_with_conditionals_decompile() {
        // Multi-statement lambda with control flow
        let program = r#"fn f(x)
            if (x > 0)
                return x;
            endif
            return -x;
        endfn
        return f(-5);"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    fn test_nested_multi_statement_lambdas_decompile() {
        // Nested multi-statement lambdas using named function syntax
        let program = r#"fn outer(x)
            fn inner(y)
                return y * 2;
            endfn
            return inner(x) + 1;
        endfn
        return outer(5);"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    #[test]
    fn test_lambda_in_list_with_multi_statements_decompile() {
        // Lambda stored in a list (common pattern in OMeta parsers)
        // Using anonymous fn syntax inside list literal
        // Note: Must use `let` for local variables inside lambdas since bare
        // assignments create global variables which would be flagged as captured
        let program = r#"handlers = {
            fn ()
                let x = 1;
                return x + 1;
            endfn,
            fn ()
                let y = 2;
                return y + 2;
            endfn
        };
        f = handlers[1];
        return f();"#;
        let (parse, decompiled) = parse_decompile(program);
        assert_trees_match_recursive(&parse.stmts, &decompiled.stmts);
    }

    // Regression test: lambdas with optional parameters use IfQues without an
    // alternative branch for default value assignment. The decompiler previously
    // expected all IfQues to have a Jump between consequence and alternative.
    // Issue: decompilation of fn...endfn with optional params failed with
    // "malformed program: expected Jump"
    #[test]
    fn test_lambda_optional_param_decompile() {
        let program = r#"fn f(?x = 0)
                return x + 1;
            endfn
            return f();"#;

        let compiled = compile(program, CompileOptions::default());
        assert!(
            compiled.is_ok(),
            "Compile should succeed: {:?}",
            compiled.err()
        );
        let binary = compiled.unwrap();
        let decompiled = program_to_tree(&binary);
        assert!(
            decompiled.is_ok(),
            "Decompile should succeed: {:?}",
            decompiled.err()
        );
    }

    #[test]
    fn regression_server_started_decompile() {
        let program = r#"
            callers() && !caller_perms().wizard && return E_PERM;
            server_log("Core starting...");
            player_class = $login.default_player_class;
            $login.player_setup_capability = $player:issue_capability(player_class, {'create_child, 'make_player}, 0, 0);
            server_log("Issued player creation capability to $login");
            $scheduler:resume_if_needed();
        "#;
        let compiled = compile(program, CompileOptions::default()).unwrap();
        let decompiled = program_to_tree(&compiled);
        assert!(
            decompiled.is_ok(),
            "Decompile should succeed: {:?}",
            decompiled.err()
        );
    }
}
