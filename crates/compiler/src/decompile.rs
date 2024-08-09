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

use moor_values::var::Symbol;
use moor_values::var::{v_err, v_int, v_none, v_objid, Var};
use moor_values::var::{v_float, Variant};
use std::collections::{HashMap, VecDeque};

use crate::ast::{
    Arg, BinaryOp, CatchCodes, CondArm, ElseArm, ExceptArm, Expr, ScatterItem, ScatterKind, Stmt,
    StmtNode, UnaryOp,
};
use crate::builtins::make_offsets_builtins;
use crate::decompile::DecompileError::{BuiltinNotFound, MalformedProgram};
use crate::labels::{JumpLabel, Label};
use crate::names::{Name, UnboundName, UnboundNames};
use crate::opcode::{Op, ScatterLabel};
use crate::parse::Parse;
use crate::program::Program;

#[derive(Debug, thiserror::Error)]
pub enum DecompileError {
    #[error("unexpected program end")]
    UnexpectedProgramEnd,
    #[error("name not found: {0:?}")]
    NameNotFound(Name),
    #[error("builtin function not found: #{0:?}")]
    BuiltinNotFound(usize),
    #[error("label not found: {0:?}")]
    LabelNotFound(Label),
    #[error("malformed program: {0}")]
    MalformedProgram(String),
    #[error("could not decompile statement")]
    CouldNotDecompileStatement,
}

struct Decompile {
    /// The program we are decompiling.
    program: Program,
    /// The fork vector # we're decompiling, or None if from the main stream.
    fork_vector: Option<usize>,
    /// The current position in the opcode stream as it is being decompiled.
    position: usize,
    expr_stack: VecDeque<Expr>,
    builtins: HashMap<usize, Symbol>,
    statements: Vec<Stmt>,
    names_mapping: HashMap<Name, UnboundName>,
}

impl Decompile {
    fn opcode_vector(&self) -> &[Op] {
        match self.fork_vector {
            Some(fv) => &self.program.fork_vectors[fv],
            None => &self.program.main_vector,
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
        self.expr_stack
            .pop_front()
            .ok_or_else(|| MalformedProgram("expected expression on stack".to_string()))
    }
    fn push_expr(&mut self, expr: Expr) {
        self.expr_stack.push_front(expr);
    }

    fn find_jump(&self, label: &Label) -> Result<JumpLabel, DecompileError> {
        self.program
            .jump_labels
            .iter()
            .find(|j| &j.id == label)
            .ok_or(DecompileError::LabelNotFound(*label))
            .cloned()
    }

    pub fn find_literal(&self, label: &Label) -> Result<Var, DecompileError> {
        self.program
            .literals
            .get(label.0 as usize)
            .cloned()
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
                "expected jump opcode at branch end".to_string(),
            ));
        };
        if self.statements.len() > old_len {
            Ok((self.statements.split_off(old_len), label))
        } else {
            Ok((vec![], label))
        }
    }

    fn line_num_for_position(&self) -> usize {
        let mut last_line_num = 1;
        for (offset, line_no) in &self.program.line_number_spans {
            if *offset >= self.position {
                return last_line_num;
            }
            last_line_num = *line_no
        }
        last_line_num
    }

    fn decompile(&mut self) -> Result<(), DecompileError> {
        let opcode = self.next()?;

        let line_num = self.line_num_for_position();
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

                // Resulting thing should be a Scope, or empty.
                let else_arm = if otherwise_stmts.is_empty() {
                    None
                } else {
                    let Some(Stmt {
                        node: StmtNode::Scope { num_bindings, body },
                        ..
                    }) = otherwise_stmts.pop()
                    else {
                        return Err(MalformedProgram(
                            "expected Scope as otherwise branch".to_string(),
                        ));
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
            Op::ForList {
                id,
                end_label: label,
                environment_width,
            } => {
                let one = self.pop_expr()?;
                let Expr::Value(v) = one else {
                    return Err(MalformedProgram(
                        "expected literal '0' in for loop".to_string(),
                    ));
                };
                let Variant::Int(0) = v.variant() else {
                    return Err(MalformedProgram(
                        "expected literal '0' in for loop".to_string(),
                    ));
                };
                let list = self.pop_expr()?;
                let (body, _) = self.decompile_until_branch_end(&label)?;
                let id = self.decompile_name(&id)?;
                self.statements.push(Stmt::new(
                    StmtNode::ForList {
                        id,
                        expr: list,
                        body,
                        environment_width: environment_width as usize,
                    },
                    line_num,
                ));
            }
            Op::ForRange {
                id,
                end_label,
                environment_width,
            } => {
                let to = self.pop_expr()?;
                let from = self.pop_expr()?;
                let (body, _) = self.decompile_until_branch_end(&end_label)?;
                let id = self.decompile_name(&id)?;
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
                let (body, _) = self.decompile_until_branch_end(&loop_end_label)?;
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
                let (body, _) = self.decompile_until_branch_end(&loop_end_label)?;
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
                    builtins: self.builtins.clone(),
                    statements: vec![],
                    names_mapping: self.names_mapping.clone(),
                };
                let fv_len = self.program.fork_vectors[fv_offset.0 as usize].len();
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
            Op::Return => {
                let expr = self.pop_expr()?;
                self.statements
                    .push(Stmt::new(StmtNode::Return(Some(expr)), line_num));
            }
            Op::Return0 => {
                self.statements
                    .push(Stmt::new(StmtNode::Return(None), line_num));
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
                self.push_expr(Expr::Assign {
                    left: Box::new(Expr::Id(varname)),
                    right: Box::new(expr),
                });
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
            | Op::In => {
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
            Op::FuncCall { id } => {
                let args = self.pop_expr()?;
                let Some(builtin) = self.builtins.get(&(id as usize)) else {
                    return Err(BuiltinNotFound(id as usize));
                };

                // Have to reconstruct arg list ...
                let Expr::List(args) = args else {
                    return Err(MalformedProgram(
                        format!("expected list of args, got {:?} instead", args).to_string(),
                    ));
                };
                self.push_expr(Expr::Call {
                    function: *builtin,
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
                for scatter_label in sa.labels.iter() {
                    if let ScatterLabel::Optional(_, Some(label)) = scatter_label {
                        opt_jump_labels.push(label);
                    }
                }
                opt_jump_labels.push(&sa.done);

                let mut label_pos = 0;
                for scatter_label in sa.labels.iter() {
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
                                        "expected assign for optional scatter assignment; got {:?}",
                                        assign_expr
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
                    if let Op::Put(varname) = next_opcode {
                        let varname = self.decompile_name(&varname)?;
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
                            if position == end_label_position as _ {
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
                            format!(
                                "bad end to catch expr (expected Pop or Val/Ref, got {:?}",
                                next
                            )
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
                // We should be findin' a jump now.
                let Op::Jump { label: jump_label } = self.next()? else {
                    return Err(MalformedProgram("expected Jump".to_string()));
                };
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
            Op::Jump { .. } | Op::PushTemp => {
                unreachable!("should have been handled other decompilation branches")
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
                self.push_expr(Expr::Value(v_err(e)));
            }
            Op::ImmObjid(oid) => {
                self.push_expr(Expr::Value(v_objid(oid)));
            }
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
        }
        Ok(())
    }

    fn decompile_name(&self, name: &Name) -> Result<UnboundName, DecompileError> {
        self.names_mapping
            .get(name)
            .cloned()
            .ok_or(DecompileError::NameNotFound(*name))
    }
}

/// Reconstruct a parse tree from opcodes.
pub fn program_to_tree(program: &Program) -> Result<Parse, DecompileError> {
    // Reconstruct a fake "unbound names" from the program's var_names.
    // TODO: this is broken for scopes, but we don't have a way to represent that yet
    //  inside bound names.
    let mut unbound_names = UnboundNames::new();
    let mut bound_to_unbound = HashMap::new();
    let mut unbound_to_bound = HashMap::new();
    for bound_name in program.var_names.names() {
        let n = program.var_names.name_of(&bound_name).unwrap();
        let ub = unbound_names.find_or_add_name_global(n.as_str());
        bound_to_unbound.insert(bound_name, ub);
        unbound_to_bound.insert(ub, bound_name);
    }

    let builtins = make_offsets_builtins();
    let mut decompile = Decompile {
        program: program.clone(),
        fork_vector: None,
        position: 0,
        expr_stack: Default::default(),
        builtins,
        statements: vec![],
        names_mapping: bound_to_unbound,
    };
    let opcode_vector_len = decompile.opcode_vector().len();
    while decompile.position < opcode_vector_len {
        decompile.decompile()?;
    }

    Ok(Parse {
        stmts: decompile.statements,
        names: program.var_names.clone(),
        unbound_names,
        names_mapping: unbound_to_bound,
    })
}

#[cfg(test)]
mod tests {
    use crate::ast::assert_trees_match_recursive;
    use crate::codegen::compile;
    use crate::decompile::program_to_tree;
    use crate::parse::parse_program;
    use crate::parse::Parse;
    use crate::unparse::annotate_line_numbers;
    use test_case::test_case;

    fn parse_decompile(program_text: &str) -> (Parse, Parse) {
        let parse_1 = parse_program(program_text).unwrap();
        let binary = compile(program_text).unwrap();
        let mut parse_2 = program_to_tree(&binary).unwrap();
        annotate_line_numbers(1, &mut parse_2.stmts);
        (parse_1, parse_2)
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
}
