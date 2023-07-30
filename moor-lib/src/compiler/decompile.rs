use std::collections::{HashMap, VecDeque};
use tracing::trace;

use crate::compiler::ast::{
    Arg, BinaryOp, CatchCodes, CondArm, ExceptArm, Expr, ScatterItem, ScatterKind, Stmt, UnaryOp,
};
use crate::compiler::builtins::make_labels_builtins;
use crate::compiler::decompile::DecompileError::{LabelNotFound, MalformedProgram};
use crate::compiler::labels::{JumpLabel, Label, Name};
use crate::compiler::Parse;
use crate::var::{v_label, Var, Variant};
use crate::vm::opcode::{Binary, Op, ScatterLabel};

#[derive(Debug, thiserror::Error)]
pub enum DecompileError {
    #[error("unexpected program end")]
    UnexpectedProgramEnd,
    #[error("label not found: {0:?}")]
    LabelNotFound(Label),
    #[error("malformed program: {0}")]
    MalformedProgram(String),
    #[error("could not decompile statement")]
    CouldNotDecompileStatement,
}

struct Decompile {
    program: Binary,
    position: usize,
    expr_stack: VecDeque<Expr>,
    builtins: HashMap<Label, String>,
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
    pub fn find_var(&self, label: &Label) -> Result<Name, DecompileError> {
        self.program
            .var_names
            .find_label(label)
            .ok_or(DecompileError::LabelNotFound(*label))
    }

    fn decompile_statements_until_match<F: Fn(usize, &Op) -> bool>(
        &mut self,
        predicate: F,
    ) -> Result<(Vec<Stmt>, Op), DecompileError> {
        let old_len = self.s.len();
        while self.position < self.program.main_vector.len() {
            let op = &self.program.main_vector[self.position];
            if predicate(self.position, op) {
                // We'll need a copy of the matching opcode we terminated at.
                let final_op = self.next()?;
                if self.s.len() > old_len {
                    return Ok((self.s.split_off(old_len), final_op));
                } else {
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
        while self.position + 1 < jump_label.position.0 as usize {
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
        while self.position < jump_label.position.0 as usize {
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
        while self.position + 1 < jump_label.position.0 as usize {
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

        trace!("decompile @ pos {}: {:?}", self.position, opcode);
        match opcode {
            Op::If(otherwise_label) => {
                trace!("Begin if statement @ pos {}", self.position);
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
                    id: self.find_var(&id)?,
                    expr: list,
                    body,
                });
            }
            Op::ForRange { id, end_label } => {
                let to = self.pop_expr()?;
                let from = self.pop_expr()?;
                let (body, _) = self.decompile_until_branch_end(&end_label)?;
                self.s.push(Stmt::ForRange {
                    id: self.find_var(&id)?,
                    from,
                    to,
                    body,
                });
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
                    id: Some(self.find_var(&id)?),
                    condition: cond,
                    body,
                });
            }
            Op::Exit { stack: _, label } => {
                let position = self.find_jump(&label)?.position;
                if position.0 < self.position as u32 {
                    self.s.push(Stmt::Continue { exit: None });
                } else {
                    self.s.push(Stmt::Break { exit: None });
                }
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
            Op::Push(label) => {
                self.push_expr(Expr::Id(self.find_var(&label)?));
            }
            Op::Val(value) => {
                self.push_expr(Expr::VarExpr(value));
            }
            Op::Put(label) => {
                let expr = self.pop_expr()?;
                self.push_expr(Expr::Assign {
                    left: Box::new(Expr::Id(self.find_var(&label)?)),
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
            Op::FuncCall { id } => {
                let args = self.pop_expr()?;
                let Some(builtin) = self.builtins.get(&id) else {
                    return Err(LabelNotFound(id));
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
                        ScatterLabel::Required(label) => ScatterItem {
                            kind: ScatterKind::Required,
                            id: self.find_var(label)?,
                            expr: None,
                        },
                        ScatterLabel::Rest(label) => ScatterItem {
                            kind: ScatterKind::Rest,
                            id: self.find_var(label)?,
                            expr: None,
                        },
                        ScatterLabel::Optional(label_a, label_b) => {
                            let opt_assign = if let Some(_label_b) = label_b {
                                let Expr::Assign {left: _, right} = self.pop_expr()? else {
                                    return Err(MalformedProgram("expected assign for optional scatter assignment".to_string()));
                                };
                                Some(*right)
                            } else {
                                None
                            };
                            ScatterItem {
                                kind: ScatterKind::Optional,
                                id: self.find_var(label_a)?,
                                expr: opt_assign,
                            }
                        }
                    };
                    scatter_items.push(scatter_item);
                }
                let e = self.pop_expr()?;
                self.push_expr(Expr::Scatter(scatter_items, Box::new(e)));
            }
            Op::PushLabel(label) => {
                self.push_expr(Expr::VarExpr(v_label(label)));
            }
            Op::TryExcept { num_excepts } => {
                let mut except_arms = Vec::with_capacity(num_excepts);
                for _ in 0..num_excepts {
                    // Inverse of generate_codes. Jump label first.
                    let Expr::VarExpr(label) = self.pop_expr()? else {
                        return Err(MalformedProgram("missing try/except jump label".to_string()));
                    };
                    let Variant::_Label(_) = label.variant() else {
                        return Err(MalformedProgram("invalid try/except jump label".to_string()));
                    };

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
                let (body, end_except) = self.decompile_statements_until_match(|_, o| {
                    if let Op::EndExcept(_) = o {
                        true
                    } else {
                        false
                    }
                })?;
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
                    if let Op::Put(put_label) = next_opcode {
                        arm.id = Some(self.find_var(&put_label)?);
                        next_opcode = self.next()?;
                    }
                    let Op::Pop = next_opcode else {
                        return Err(MalformedProgram("expected Pop".to_string()));
                    };

                    // Scan forward until the jump, decompiling as we go.
                    trace!("Decompiling except arm @ pos {}", self.position);
                    let end_label_position = self.find_jump(&end_label)?.position.0 as usize;
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
                    arm.statements = statements;
                }

                self.s.push(Stmt::TryExcept {
                    body,
                    excepts: except_arms,
                });
            }
            Op::TryFinally(_label) => {
                // decompile body up until the EndFinally
                let (body, _) = self.decompile_statements_until_match(|_, op| {
                    if let Op::EndFinally = op {
                        true
                    } else {
                        false
                    }
                })?;
                let (handler, _) = self.decompile_statements_until_match(|_, op| {
                    if let Op::Continue = op {
                        true
                    } else {
                        false
                    }
                })?;
                self.s.push(Stmt::TryFinally { body, handler });
            }
            Op::Catch => {
                let Expr::VarExpr(label) = self.pop_expr()? else {
                    return Err(MalformedProgram("missing catch jump label".to_string()));
                };
                let Variant::_Label(label) = label.variant() else {
                    return Err(MalformedProgram("invalid catch jump label".to_string()));
                };

                let codes_expr = self.pop_expr()?;
                let catch_codes = match codes_expr {
                    Expr::VarExpr(_) => CatchCodes::Any,
                    Expr::List(codes) => CatchCodes::Codes(codes),
                    _ => {
                        return Err(MalformedProgram("invalid try/except codes".to_string()));
                    }
                };
                // decompile forward to the EndCatch
                let _handler = self.decompile_statements_up_to(label)?;
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
            _ => {
                todo!("decompile for {:?}", opcode);
            }
        }
        Ok(())
    }
}

/// Reconstruct a parse tree from opcodes.
pub fn program_to_tree(program: &Binary) -> Result<Parse, anyhow::Error> {
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
    use crate::compiler::codegen::compile;
    use crate::compiler::decompile::program_to_tree;
    use crate::compiler::parse::parse_program;
    use crate::compiler::Parse;
    use crate::vm::opcode::Binary;

    fn parse_decompile(program_text: &str) -> (Parse, Parse, Binary) {
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
}
