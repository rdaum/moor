use std::collections::{HashMap, VecDeque};

use crate::compiler::ast::{Arg, BinaryOp, CondArm, Expr, Stmt, UnaryOp};
use crate::compiler::builtins::make_labels_builtins;
use crate::compiler::decompile::DecompileError::{LabelNotFound, MalformedProgram};
use crate::compiler::labels::{JumpLabel, Label, Name};
use crate::compiler::Parse;
use crate::var::{Var, Variant};
use crate::vm::opcode::{Binary, Op};

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
            .get(label.0 as usize).cloned()
            .ok_or(DecompileError::LabelNotFound(*label))
    }
    pub fn find_var(&self, label: &Label) -> Result<Name, DecompileError> {
        self.program
            .var_names
            .find_label(label)
            .ok_or(DecompileError::LabelNotFound(*label))
    }

    /// Scan forward until we hit the label's position, decompiling as we go and returning the
    /// new statements produced.
    fn decompile_statements_until(&mut self, label: &Label) -> Result<Vec<Stmt>, DecompileError> {
        let jump_label = self.find_jump(label)?; // check that the label exists
        let old_len = self.s.len();

        eprintln!("seek up to pos {}", jump_label.position.0);
        while self.position < jump_label.position.0 as usize {
            self.decompile()?;
        }
        eprintln!(
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

        eprintln!("seek up to pos {}", jump_label.position.0);
        while self.position + 1 < jump_label.position.0 as usize {
            self.decompile()?;
        }
        // Next opcode must be the jump to the end of the whole branch
        let opcode = self.next()?;
        let Op::Jump {label} = opcode else {
            return Err(MalformedProgram("expected jump opcode at branch end".to_string()));
        };
        eprintln!(
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

        eprintln!("decompile @ pos {}: {:?}", self.position, opcode);
        match opcode {
            Op::If(otherwise_label) => {
                eprintln!("Begin if statement @ pos {}", self.position);
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
                    eprintln!("s: {:?}", self.s);
                    return Err(MalformedProgram("expected Cond as working tree".to_string()));
                };
                *otherwise = otherwise_stmts;
            }
            Op::Eif(end_label) => {
                eprintln!("Begin elseif branch @ pos {}", self.position);

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
                    eprintln!("s: {:?}", self.s);
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
            Op::Imm(label) => {
                self.push_expr(Expr::VarExpr(self.find_literal(&label)?));
            }
            Op::Push(label) => {
                self.push_expr(Expr::Id(self.find_var(&label)?));
            }
            Op::Val(value) => {
                self.push_expr(Expr::VarExpr(value));
            }
            Op::And(label) | Op::Or(label) => {
                let left = self.pop_expr()?;
                // read forward til we hit label position, only then can we read `right`
                while self.position < label.0 as usize {
                    // This should push into the expression stack...
                    self.decompile()?;
                }
                let right = self.pop_expr()?;
                self.push_expr(Expr::And(Box::new(left), Box::new(right)));
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
}
