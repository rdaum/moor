use itertools::Itertools;
use serde_derive::{Deserialize, Serialize};

use crate::compiler::ast::{BinaryOp, Expr, Stmt, UnaryOp};
use crate::compiler::parse::{Name, Names, parse_program};
use crate::model::var::Var;
use crate::vm::opcode::{Binary, Op};

// Fixup for a jump label
#[derive(Clone, Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct JumpLabel {
    // The unique id for the jump label, which is also its offset in the jump vector.
    pub(crate) id: usize,

    // If there's a unique identifier assigned to this label, it goes here.
    label: Option<Name>,

    // The temporary and then final resolved position of the label in terms of PC offsets.
    pub(crate) position: usize,
}

// References to vars using the name idx.
pub struct VarRef {
    pub(crate) id: usize,
    pub(crate) name: Name,
}

pub struct Loop {
    start_label: usize,
    end_label: usize
}

pub struct State {
    pub (crate) ops: Vec<Op>,
    pub (crate) jumps: Vec<JumpLabel>,
    pub (crate) varnames: Names,
    pub (crate) literals: Vec<Var>,
    pub (crate) loops: Vec<Loop>,
}

impl State {}

impl State {
    pub fn new(varnames: Names) -> Self {
        Self {
            ops: vec![],
            jumps: vec![],
            varnames,
            literals: vec![],
            loops: vec![],
        }
    }

    pub fn generate(&mut self, stmts: &Vec<Stmt>) -> Result<(), anyhow::Error> {
        for stmt in stmts {
            self.generate_stmt(stmt)?;
        }

        todo!()
    }

    // Create an anonymous jump label and return its unique ID.
    fn add_jump(&mut self, name: Option<Name>) -> usize {
        let id = self.jumps.len();
        let position = self.ops.len();
        self.jumps.push(JumpLabel {
            id,
            label: name,
            position,
        });
        id
    }

    fn find_named_jump(&self, name: &Name) -> Option<&JumpLabel> {
        self.jumps.iter().find(|j| {
            if let Some(label) = j.label {
                label.eq(name)
            } else {
                false
            }
        })
    }

    fn commit_jump_fixup(&mut self, id: usize) {
        let position = self.ops.len();
        let jump = &mut self.jumps.get_mut(id).expect("Invalid jump fixup");
        let npos = position - jump.position;
        jump.position = npos;
    }

    fn add_literal(&mut self, v: &Var) -> usize {
        let lv_pos = self.literals.iter().position(|lv| return lv.eq(v));
        match lv_pos {
            None => {
                let idx = self.literals.len();
                self.literals.push(v.clone());
                idx
            }
            Some(idx) => idx,
        }
    }

    fn emit(&mut self, op: Op) {
        self.ops.push(op);
    }

    fn find_loop(&self, loop_label: &Option<Name>) -> &Loop {
        match loop_label {
            None => {
                let l = self.loops.last().expect("No loop to exit in codegen");
                l
            }
            Some(eid) => {
                let l = self.loops.iter().find(|l| {
                    l.start_label == eid.0
                });
                l.expect("Can't find loop in continue / break")
            }
        }
    }

    fn generate_lvalue(&mut self, expr: &Expr, indexed_above: bool) -> Result<(), anyhow::Error> {
        match expr {
            Expr::Range { from, base, to } => {
                self.generate_lvalue(base.as_ref(), true)?;
                self.generate_expr(from.as_ref())?;
                self.generate_expr(to.as_ref())?;
            }
            Expr::Index(lhs, rhs) => {
                self.generate_lvalue(lhs.as_ref(), true)?;
                self.generate_expr(rhs.as_ref())?;
                if indexed_above {
                    self.emit(Op::PushRef);
                }
            }
            Expr::Id(id) => {
                self.emit(Op::Push(id.0));
            }
            Expr::Prop { property, location } => {
                self.generate_expr(location.as_ref())?;
                self.generate_expr(property.as_ref())?;
                if indexed_above {
                    self.emit(Op::PushGetProp);
                }
            }
            _ => {
                panic!("Invalid expr for lvalue: {:?}", expr);
            }
        }
        Ok(())
    }
    fn generate_expr(&mut self, expr: &Expr) -> Result<(), anyhow::Error> {
        match expr {
            Expr::VarExpr(v) => {
                let literal = self.add_literal(v);
                self.emit(Op::Imm(literal));
            }
            Expr::Id(ident) => {
                self.emit(Op::Push(ident.0))
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
            }
            Expr::Index(lhs, rhs) => {}
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
            }
            Expr::Call { .. } => {}
            Expr::Verb { .. } => {}
            Expr::Range { .. } => {}
            Expr::Cond { .. } => {}
            Expr::Catch { .. } => {}
            Expr::List(_) => {}
            Expr::Scatter(_) => {}
            Expr::Length => {}
            Expr::And(left, right) => {
                self.generate_expr(left.as_ref())?;
                let label = self.add_jump(None);
                self.emit(Op::And(label));
                self.generate_expr(right.as_ref())?;
                self.commit_jump_fixup(label);
            }
            Expr::Or(left, right) => {
                self.generate_expr(left.as_ref())?;
                let label = self.add_jump(None);
                self.emit(Op::Or(label));
                self.generate_expr(right.as_ref())?;
                self.commit_jump_fixup(label);
            }
            Expr::This => {
                self.emit(Op::This)
            }
        }

        Ok(())
    }

    pub fn generate_stmt(&mut self, stmt: &Stmt) -> Result<(), anyhow::Error> {
        match stmt {
            Stmt::Cond { arms, otherwise } => {}
            Stmt::ForList { .. } => {}
            Stmt::ForRange { .. } => {}
            Stmt::While {
                id,
                condition,
                body,
            } => {
                let loop_start_label = self.add_jump(*id);
                // Push condition ops
                self.generate_expr(condition).as_ref().expect("compile expr");
                match id {
                    None => {
                        self.emit(Op::While(loop_start_label))
                    }
                    Some(id) => {
                        self.emit(Op::WhileId {id: id.0, label: loop_start_label })
                    }
                }
                let loop_end_label = self.add_jump(None);
                self.loops.push(Loop {
                    start_label: loop_start_label,
                    end_label: loop_end_label,
                });
            }
            Stmt::Fork { .. } => {}
            Stmt::Catch { .. } => {}
            Stmt::Finally { .. } => {}
            Stmt::Break { exit } => {
                let lp = self.find_loop(exit);
                self.emit(Op::Break(lp.end_label));
            }
            Stmt::Continue { exit } => {
                let lp = self.find_loop(exit);
                self.emit(Op::Continue(lp.start_label));
            }
            Stmt::Return { expr } => match expr {
                Some(expr) => {
                    self.generate_expr(expr)?;
                    self.emit(Op::Return);
                }
                None => {
                    self.emit(Op::Return);
                }
            },
            Stmt::Expr(e) => {
                self.generate_expr(e)?;
                self.emit(Op::Pop);
            }
            Stmt::Exit(_) => {}
        }

        Ok(())
    }
}

pub fn compile(program : &str) -> Result<Binary, anyhow::Error> {
    let parse = parse_program(program)?;
    let mut cg_state = State::new(parse.names);
    for x in parse.stmts{
        cg_state.generate_stmt(&x)?;
    }

    let binary = Binary {
        first_lineno: 0,
        ref_count: 0,
        literals: cg_state.literals,
        jump_labels: cg_state.jumps,
        var_names: cg_state.varnames.names,
        main_vector: cg_state.ops
    };

    Ok(binary)
}

#[cfg(test)]
mod tests {
    use crate::vm::opcode::Op;

    use super::*;

    #[test]
    fn test_compile_simple_expr() {
        let program = "1 + 2;";
        let parse = compile(program).unwrap();
        assert_eq!(parse.main_vector, vec![Op::Imm(0), Op::Imm(1), Op::Add, Op::Pop]);
    }

    #[test]
    fn test_compile_simple_expr_with_var() {
        let program = "a = 1 + 2;";
        let parse = compile(program).unwrap();
        assert_eq!(parse.main_vector, vec![Op::Imm(0), Op::Imm(1), Op::Add, Op::Pop]);
    }

    #[test]
    fn test_compile_simple_expr_with_var_and_assign() {
        let program = "a = 1 + 2;";
        let parse = compile(program).unwrap();
        assert_eq!(parse.main_vector, vec![Op::Imm(0), Op::Imm(1), Op::Add, Op::Pop]);
    }

    #[test]
    fn test_compile_simple_expr_with_var_and_assign_and_return() {
        let program = "a = 1 + 2; return a;";
        let parse = compile(program).unwrap();
        assert_eq!(parse.main_vector, vec![Op::Imm(0), Op::Imm(1), Op::Add, Op::Pop, Op::Push(0), Op::Return]);
    }
}
