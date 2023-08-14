use std::collections::{HashMap, VecDeque};

use tracing::{debug, trace};

use moor_value::var::variant::Variant;
use moor_value::var::Var;

use crate::compiler::ast::{
    Arg, BinaryOp, CatchCodes, CondArm, ExceptArm, Expr, ScatterItem, ScatterKind, Stmt, UnaryOp,
};
use crate::compiler::builtins::make_labels_builtins;
use crate::compiler::decompile::DecompileError::{MalformedProgram, NameNotFound};
use crate::compiler::labels::{JumpLabel, Label, Name};
use crate::compiler::parse::Parse;
use crate::vm::opcode::{Op, Program, ScatterLabel};

#[derive(Debug, thiserror::Error)]
pub enum DecompileError {
    #[error("unexpected program end")]
    UnexpectedProgramEnd,
    #[error("name not found: {0:?}")]
    NameNotFound(Name),
    #[error("label not found: {0:?}")]
    LabelNotFound(Label),
    #[error("malformed program: {0}")]
    MalformedProgram(String),
    #[error("could not decompile statement")]
    CouldNotDecompileStatement,
}

struct Decompile {
    program: Program,
    position: usize,
    expr_stack: VecDeque<Expr>,
    builtins: HashMap<Name, String>,
    s: Vec<Stmt>,
}

impl Decompile {
    fn next(&mut self) -> Result<Op, DecompileError> {
        if self.position >= self.program.main_vector.len() {
            return Err(DecompileError::UnexpectedProgramEnd);
        }
        let op = self.program.main_vector[self.position].clone();
        self.position += 1;
        Ok(op)
    }
    fn pop_expr(&mut self) -> Result<Expr, DecompileError> {
        self.expr_stack.pop_front().ok_or_else(|| {
            DecompileError::MalformedProgram("expected expression on stack".to_string())
        })
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
        let old_len = self.s.len();
        while self.position < self.program.main_vector.len() {
            let op = &self.program.main_vector[self.position];
            trace!("check: {}: {:?}", self.position, op);
            if predicate(self.position, op) {
                // We'll need a copy of the matching opcode we terminated at.
                let final_op = self.next()?;
                if self.s.len() > old_len {
                    return Ok((self.s.split_off(old_len), final_op));
                } else {
                    trace!("Stopping @ {}: {:?}", self.position, final_op);
                    return Ok((vec![], final_op));
                };
            }
            self.decompile()?;
        }
        Err(DecompileError::UnexpectedProgramEnd)
    }

    fn decompile_statements_up_to(&mut self, label: &Label) -> Result<Vec<Stmt>, DecompileError> {
        let jump_label = self.find_jump(label)?; // check that the label exists
        let old_len = self.s.len();

        trace!("seek up to pos {}", jump_label.position.0);
        while self.position + 1 < jump_label.position.0 {
            self.decompile()?;
        }
        trace!(
            "seek done @ pos {}: {} stmts",
            self.position,
            self.s.len() - old_len
        );
        if self.s.len() > old_len {
            Ok(self.s.split_off(old_len))
        } else {
            Ok(vec![])
        }
    }

    /// Scan forward until we hit the label's position, decompiling as we go and returning the
    /// new statements produced.
    fn decompile_statements_until(&mut self, label: &Label) -> Result<Vec<Stmt>, DecompileError> {
        let jump_label = self.find_jump(label)?; // check that the label exists
        let old_len = self.s.len();

        trace!("seek up to pos {}", jump_label.position.0);
        while self.position < jump_label.position.0 {
            self.decompile()?;
        }
        trace!(
            "seek done @ pos {}: {} stmts",
            self.position,
            self.s.len() - old_len
        );
        if self.s.len() > old_len {
            Ok(self.s.split_off(old_len))
        } else {
            Ok(vec![])
        }
    }

    fn decompile_until_branch_end(
        &mut self,
        label: &Label,
    ) -> Result<(Vec<Stmt>, Label), DecompileError> {
        let jump_label = self.find_jump(label)?; // check that the label exists
        let old_len = self.s.len();

        trace!("seek up to pos {}", jump_label.position.0);
        while self.position + 1 < jump_label.position.0 {
            self.decompile()?;
        }
        // Next opcode must be the jump to the end of the whole branch
        let opcode = self.next()?;
        let Op::Jump {label} = opcode else {
            return Err(MalformedProgram("expected jump opcode at branch end".to_string()));
        };
        trace!(
            "seek done @ pos {}: {} stmts",
            self.position,
            self.s.len() - old_len
        );
        if self.s.len() > old_len {
            Ok((self.s.split_off(old_len), label))
        } else {
            Ok((vec![], label))
        }
    }
    fn decompile(&mut self) -> Result<(), DecompileError> {
        let opcode = self.next()?;

        debug!("decompile @ pos {}: {:?}", self.position, opcode);
        match opcode {
            Op::If(otherwise_label) => {
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
                };
                self.s.push(Stmt::Cond {
                    arms: vec![cond_arm],
                    otherwise: vec![],
                });

                // Decompile to the 'end_of_otherwise' label to get the statements for the
                // otherwise branch.
                let otherwise_stmts = self.decompile_statements_until(&end_of_otherwise)?;
                let Some(Stmt::Cond{arms:_, otherwise}) = self.s.last_mut() else {
                    trace!("s: {:?}", self.s);
                    return Err(MalformedProgram("expected Cond as working tree".to_string()));
                };
                *otherwise = otherwise_stmts;
            }
            Op::Eif(end_label) => {
                trace!("Begin elseif branch @ pos {}", self.position);

                let cond = self.pop_expr()?;
                // decompile statements until the position marked in `label`, which is the
                // end of the branch statement
                let (cond_statements, _) = self.decompile_until_branch_end(&end_label)?;
                let cond_arm = CondArm {
                    condition: cond,
                    statements: cond_statements,
                };
                // Add the arm
                let Some(Stmt::Cond{arms, otherwise: _}) = self.s.last_mut() else {
                    trace!("s: {:?}", self.s);
                    return Err(MalformedProgram("expected Cond as working tree".to_string()));
                };
                arms.push(cond_arm);
            }
            Op::ForList {
                id,
                end_label: label,
            } => {
                let one = self.pop_expr()?;
                let Expr::VarExpr(v) = one else {
                    return Err(MalformedProgram("expected literal '0' in for loop".to_string()));
                };
                let Variant::Int(0) = v.variant() else {
                    return Err(MalformedProgram("expected literal '0' in for loop".to_string()));
                };
                let list = self.pop_expr()?;
                let (body, _) = self.decompile_until_branch_end(&label)?;
                self.s.push(Stmt::ForList {
                    id,
                    expr: list,
                    body,
                });
            }
            Op::ForRange { id, end_label } => {
                let to = self.pop_expr()?;
                let from = self.pop_expr()?;
                let (body, _) = self.decompile_until_branch_end(&end_label)?;
                self.s.push(Stmt::ForRange { id, from, to, body });
            }
            Op::While(loop_end_label) => {
                // A "while" is actually a:
                //      a conditional expression
                //      this While opcode (with end label)
                //      a series of statements
                //      a jump back to the conditional expression
                let cond = self.pop_expr()?;
                let (body, _) = self.decompile_until_branch_end(&loop_end_label)?;
                self.s.push(Stmt::While {
                    id: None,
                    condition: cond,
                    body,
                });
            }
            // Same as above, but with id.
            // TODO: we may want to consider collapsing these two VM opcodes
            Op::WhileId {
                id,
                end_label: loop_end_label,
            } => {
                // A "while" is actually a:
                //      a conditional expression
                //      this While opcode (with end label)
                //      a series of statements
                //      a jump back to the conditional expression
                let cond = self.pop_expr()?;
                let (body, _) = self.decompile_until_branch_end(&loop_end_label)?;
                self.s.push(Stmt::While {
                    id: Some(id),
                    condition: cond,
                    body,
                });
            }
            Op::Exit { stack: _, label } => {
                let position = self.find_jump(&label)?.position;
                if position.0 < self.position {
                    self.s.push(Stmt::Continue { exit: None });
                } else {
                    self.s.push(Stmt::Break { exit: None });
                }
            }
            Op::ExitId(label) => {
                let jump_label = self.find_jump(&label)?;
                // Whether it's a break or a continue depends on whether the jump is forward or
                // backward from the current position.
                let s = if jump_label.position.0 < self.position {
                    Stmt::Continue {
                        exit: Some(jump_label.name.unwrap()),
                    }
                } else {
                    Stmt::Break {
                        exit: Some(jump_label.name.unwrap()),
                    }
                };

                self.s.push(s);
            }
            Op::Fork { .. } => {
                unimplemented!("decompile fork");
            }
            Op::Pop => {
                let expr = self.pop_expr()?;
                self.s.push(Stmt::Expr(expr));
            }
            Op::Return => {
                let expr = self.pop_expr()?;
                self.s.push(Stmt::Return { expr: Some(expr) });
            }
            Op::Return0 => {
                self.s.push(Stmt::Return { expr: None });
            }
            Op::Done => {
                if self.position != self.program.main_vector.len() {
                    return Err(MalformedProgram("expected end of program".to_string()));
                }
            }
            Op::Imm(literal_label) => {
                self.push_expr(Expr::VarExpr(self.find_literal(&literal_label)?));
            }
            Op::Push(varname) => {
                self.push_expr(Expr::Id(varname));
            }
            Op::Val(value) => {
                self.push_expr(Expr::VarExpr(value));
            }
            Op::Put(varname) => {
                let expr = self.pop_expr()?;
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
                while self.position < self.program.main_vector.len() {
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
                while self.position < self.program.main_vector.len() {
                    let op = self.next()?;
                    if let Op::PushTemp = op {
                        break;
                    }
                }
            }
            Op::FuncCall { id } => {
                let args = self.pop_expr()?;
                let Some(builtin) = self.builtins.get(&id) else {
                    return Err(NameNotFound(id));
                };

                // Have to reconstruct arg list ...
                let Expr::List(args) = args else {
                    return Err(MalformedProgram("expected list of args".to_string()));
                };
                self.push_expr(Expr::Call {
                    function: builtin.clone(),
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
            Op::MkEmptyList => {
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
            Op::Scatter {
                nargs: _,
                nreq: _,
                labels,
                rest: _,
                done: _,
            } => {
                let mut scatter_items = vec![];
                for scatter_label in labels.iter() {
                    let scatter_item = match scatter_label {
                        ScatterLabel::Required(id) => ScatterItem {
                            kind: ScatterKind::Required,
                            id: *id,
                            expr: None,
                        },
                        ScatterLabel::Rest(id) => ScatterItem {
                            kind: ScatterKind::Rest,
                            id: *id,
                            expr: None,
                        },
                        ScatterLabel::Optional(id, assign_id) => {
                            let opt_assign = if let Some(_label_b) = assign_id {
                                let Expr::Assign {left: _, right} = self.pop_expr()? else {
                                    return Err(MalformedProgram("expected assign for optional scatter assignment".to_string()));
                                };
                                Some(*right)
                            } else {
                                None
                            };
                            ScatterItem {
                                kind: ScatterKind::Optional,
                                id: *id,
                                expr: opt_assign,
                            }
                        }
                    };
                    scatter_items.push(scatter_item);
                }
                let e = self.pop_expr()?;
                self.push_expr(Expr::Scatter(scatter_items, Box::new(e)));
            }
            Op::PushLabel(_) => {
                // ignore and consume, we don't need it.
            }
            Op::TryExcept { num_excepts } => {
                let mut except_arms = Vec::with_capacity(num_excepts);
                for _ in 0..num_excepts {
                    let codes_expr = self.pop_expr()?;
                    let catch_codes = match codes_expr {
                        Expr::VarExpr(_) => CatchCodes::Any,
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
                        arm.id = Some(varname);
                        next_opcode = self.next()?;
                    }
                    let Op::Pop = next_opcode else {
                        return Err(MalformedProgram("expected Pop".to_string()));
                    };

                    // Scan forward until the jump, decompiling as we go.
                    trace!("Decompiling except arm @ pos {}", self.position);
                    let end_label_position = self.find_jump(&end_label)?.position.0;
                    let (statements, _) =
                        self.decompile_statements_until_match(|position, o| {
                            if position == end_label_position {
                                return true;
                            }
                            if let Op::Jump { label } = o {
                                label == &end_label
                            } else {
                                false
                            }
                        })?;
                    trace!("Decompiled up to pos {}", self.position);
                    arm.statements = statements;
                }

                // We need to rewind the position by one opcode, it seems.
                // TODO this is not the most elegant. we're being too greedy above
                self.position -= 1;
                self.s.push(Stmt::TryExcept {
                    body,
                    excepts: except_arms,
                });
            }
            Op::TryFinally(_label) => {
                // decompile body up until the EndFinally
                let (body, _) =
                    self.decompile_statements_until_match(|_, op| matches!(op, Op::EndFinally))?;
                let (handler, _) =
                    self.decompile_statements_until_match(|_, op| matches!(op, Op::Continue))?;
                self.s.push(Stmt::TryFinally { body, handler });
            }
            Op::Catch(label) => {
                let codes_expr = self.pop_expr()?;
                let catch_codes = match codes_expr {
                    Expr::VarExpr(_) => CatchCodes::Any,
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

                // There's either an except (Pop, then expr) or not (Val, Ref).
                let except = match self.next()? {
                    Op::Pop => {
                        self.decompile_statements_until(&end_label)?;
                        Some(Box::new(self.pop_expr()?))
                    }
                    Op::Ref => {
                        let Op::Ref = self.next()? else {
                            return Err(MalformedProgram("expected Ref".to_string()));
                        };
                        None
                    }
                    _ => {
                        return Err(MalformedProgram("bad end to catch expr".to_string()));
                    }
                };
                self.push_expr(Expr::Catch {
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
                let label_position = self.find_jump(&label)?.position.0;
                let (_, _) =
                    self.decompile_statements_until_match(|position, o| {
                        if position == label_position {
                            return true;
                        }
                        if let Op::Jump { label } = o {
                            label == label
                        } else {
                            false
                        }
                    })?;
                let consequent = self.pop_expr();
                self.decompile()?;
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
                let e = Expr::List(
                    vec![Arg::Splice(sp_expr)],
                );
                self.push_expr(e);
            }
            Op::GPut { id } => {
               let e = Expr::Assign {
                   left: Box::new(Expr::Id(id)),
                   right: Box::new(self.pop_expr()?),
               };
                self.push_expr(e);
            }
            Op::GPush { id } => {
                let e = Expr::Id(id);
                self.push_expr(e)
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
            Op::EndCatch(_) | Op::Continue | Op::EndExcept(_) | Op::EndFinally  => {
                // Early exit; main logic is in TRY_FINALLY or CATCH etc case, above
                // TODO: MOO has "return ptr - 2;"  -- doing something with the iteration, that
                //   I may not be able to do with the current structure. See if I need to
                unreachable!("should have been handled other decompilation branches")
            }
        }
        Ok(())
    }
}

/// Reconstruct a parse tree from opcodes.
pub fn program_to_tree(program: &Program) -> Result<Parse, anyhow::Error> {
    let builtins = make_labels_builtins();
    let mut decompile = Decompile {
        program: program.clone(),
        position: 0,
        expr_stack: Default::default(),
        builtins,
        s: vec![],
    };
    while decompile.position < decompile.program.main_vector.len() {
        decompile.decompile()?;
    }

    Ok(Parse {
        stmts: decompile.s,
        names: program.var_names.clone(),
    })
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;
    use crate::compiler::codegen::compile;
    use crate::compiler::decompile::program_to_tree;
    use crate::compiler::parse::parse_program;
    use crate::compiler::parse::Parse;
    use crate::vm::opcode::Program;

    fn parse_decompile(program_text: &str) -> (Parse, Parse, Program) {
        let parse_1 = parse_program(program_text).unwrap();
        let binary = compile(program_text).unwrap();
        let parse_2 = program_to_tree(&binary).unwrap();
        (parse_1, parse_2, binary)
    }

    #[test]
    fn test_if() {
        let (parse, decompiled, binary) = parse_decompile("if (1) return 2; endif");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_if_else() {
        let (parse, decompiled, binary) = parse_decompile("if (1) return 2; else return 3; endif");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_if_elseif() {
        let (parse, decompiled, binary) =
            parse_decompile("if (1) return 2; elseif (2) return 3; elseif (3) return 4; endif");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_while() {
        let (parse, decompiled, binary) = parse_decompile("while (1) return 2; endwhile");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_while_break_continue() {
        let (parse, decompiled, binary) =
            parse_decompile("while (1) if (1 == 2) break; else continue; endif endwhile");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_while_labelled() {
        let (parse, decompiled, binary) = parse_decompile("while chuckles (1) return 2; endwhile");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_for_in() {
        let (parse, decompiled, binary) = parse_decompile("for x in (a) return 2; endfor");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_for_range() {
        let (parse, decompiled, binary) = parse_decompile("for x in [1..5] return 2; endfor");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_builtin() {
        let (parse, decompiled, binary) = parse_decompile("return setadd({1,2}, 3);");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_list() {
        let (parse, decompiled, binary) = parse_decompile("return {1,2,3};");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_list_splice() {
        let (parse, decompiled, binary) = parse_decompile("return {1,2,3,@{1,2,3}};");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_arithmetic_expression() {
        let (parse, decompiled, binary) = parse_decompile("return -(1 + 2 * (3 - 4) / 5 % 6);");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_equality_inequality_relational() {
        let (parse, decompiled, binary) = parse_decompile("return 1 == 2 != 3 < 4 <= 5 > 6 >= 7;");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_logical_and_or() {
        let (parse, decompiled, binary) = parse_decompile("return 1 && 2 || 3 && 4;");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_assignment() {
        let (parse, decompiled, binary) = parse_decompile("x = 1; return x;");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_index() {
        let (parse, decompiled, binary) = parse_decompile("return x[1];");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_range() {
        let (parse, decompiled, binary) = parse_decompile("return x[1..2];");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_call_verb() {
        let (parse, decompiled, binary) = parse_decompile("return x:y(1,2,3);");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_call_verb_expr() {
        let (parse, decompiled, binary) = parse_decompile(r#"return x:("y")(1,2,3);"#);
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_simple_scatter() {
        let (parse, decompiled, binary) = parse_decompile("{connection} = args;");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_scatter_required() {
        let (parse, decompiled, binary) = parse_decompile("{a,b,c} = args;");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_scatter_mixed() {
        let (parse, decompiled, binary) = parse_decompile("{a,b,?c,@d} = args;");
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_try_except() {
        let program = "try a=1; except a (E_INVARG) a=2; except b (E_PROPNF) a=3; endtry";
        let (parse, decompiled, binary) = parse_decompile(program);
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_try_finally() {
        let program = "try a=1; finally a=2; endtry";
        let (parse, decompiled, binary) = parse_decompile(program);
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_catch_expr() {
        let program = "x = `x + 1 ! e_propnf, E_PERM => 17';";
        let (parse, decompiled, binary) = parse_decompile(program);
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_range_set() {
        let program = "a[1..2] = {3,4};";
        let (parse, decompiled, binary) = parse_decompile(program);
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_index_set() {
        let program = "a[1] = {3,4};";
        let (parse, decompiled, binary) = parse_decompile(program);
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_if_ques() {
        let program = "1 ? 2 | 3;";
        let (parse, decompiled, binary) = parse_decompile(program);
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_prop_assign() {
        let program = "x.y = 1;";
        let (parse, decompiled, binary) = parse_decompile(program);
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_labelled_break() {
        let program = "while bozo (1) break bozo; tostr(5);  endwhile;";
        let (parse, decompiled, binary) = parse_decompile(program);
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    fn test_labelled_continue() {
        let program = "while bozo (1) continue bozo; tostr(5); endwhile;";
        let (parse, decompiled, binary) = parse_decompile(program);
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    #[traced_test]
    fn test_if_after_try() {
        let program = "try return x; except (E_VARNF) endtry; if (x) return 1; endif;";
        let (parse, decompiled, binary) = parse_decompile(program);
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }

    #[test]
    #[traced_test]
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
        let (parse, decompiled, binary) = parse_decompile(program);
        assert_eq!(
            parse.stmts, decompiled.stmts,
            "Decompile mismatch for {}",
            binary
        );
    }
}
