use crate::compiler::ast::{BinaryOp, Expr, Stmt, UnaryOp};
use crate::compiler::parse::Name;
use crate::model::var::Var;
use crate::vm::opcode::{Binary, Op};
use itertools::Itertools;

// Fixup for a jump label
struct JumpLabel {
    // The unique id for the jump label, which is also its offset in the jump vector.
    id: usize,

    // If there's a unique identifier assigned to this label, it goes here.
    label: Option<Name>,

    // The temporary and then final resolved position of the label in terms of PC offsets.
    position: usize,
}

// References to vars using the name idx.
struct VarRef {
    id: usize,
    name: Name,
}

struct State {
    ops: Vec<Op>,
    jumps: Vec<JumpLabel>,
    vars: Vec<VarRef>,
    literals: Vec<Var>,
    cur_stack: usize,
    saved_stack: usize,
    loops: Vec<Loop>,
}

impl State {}

impl State {
    pub fn new() -> Self {
        Self {
            ops: vec![],
            jumps: vec![],
            vars: vec![],
            literals: vec![],
            cur_stack: 0,
            saved_stack: 0,
            loops: vec![],
        }
    }

    pub fn generate(&mut self, stmts: &Vec<Stmt>) -> Result<(), anyhow::Error> {
        for stmt in stmts {
            self.generate_stmt(stmt)?;
        }

        todo!()
    }

    fn push_stack(&mut self, n: usize) {
        self.cur_stack += n
    }

    fn pop_stack(&mut self, n: usize) {
        self.cur_stack -= n
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

    fn find_named_jump(&self, name: &Name) -> Option<JumpLabel> {
        self.jumps.into_iter().find(|j| {
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

    fn add_var_ref(&mut self, n: &Name) -> usize {
        let vr_pos = self.vars.iter().position(|vr| return vr.name.eq(n));
        match vr_pos {
            None => {
                let idx = self.literals.len();
                self.vars.push(VarRef {
                    id: idx,
                    name: n.clone(),
                });
                idx
            }
            Some(idx) => idx,
        }
    }

    fn emit(&mut self, op: Op) {
        self.ops.push(op);
    }

    fn save_stack_top(&mut self) -> usize {
        let old = self.saved_stack;
        self.saved_stack = self.cur_stack - 1;
        old
    }

    fn restore_stack_top(&mut self, old: usize) {
        self.saved_stack = old;
    }

    fn generate_lvalue(&mut self, expr: &Expr, indexed_above: bool) -> Result<(), anyhow::Error> {
        match expr {
            Expr::Range { from, base, to } => {
                self.generate_lvalue(base.as_ref(), true)?;
                let old = self.save_stack_top();
                self.generate_expr(from.as_ref())?;
                self.generate_expr(to.as_ref())?;
                self.restore_stack_top(old);
            }
            Expr::Index(lhs, rhs) => {
                self.generate_lvalue(lhs.as_ref(), true)?;
                let old = self.save_stack_top();
                self.generate_expr(rhs.as_ref())?;
                self.restore_stack_top(old);
                if indexed_above {
                    self.emit(Op::PushRef);
                    self.push_stack(1);
                }
            }
            Expr::Id(id) => {
                let v = self.add_var_ref(id);
                self.emit(Op::Push(v));
                self.push_stack(1);
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
    fn generate_expr(&mut self, expr: &Expr) -> Result<(), anyhow::Error> {
        match expr {
            Expr::VarExpr(v) => {
                let literal = self.add_literal(v);
                self.emit(Op::Imm(literal));
                self.push_stack(1);
            }
            Expr::Id(ident) => {
                let ident = self.add_var_ref(ident);
                self.emit(Op::Push(ident))
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
                self.pop_stack(1);
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
                self.pop_stack(1);
                self.generate_expr(right.as_ref())?;
                self.commit_jump_fixup(label);
            }
            Expr::Or(left, right) => {
                self.generate_expr(left.as_ref())?;
                let label = self.add_jump(None);
                self.emit(Op::Or(label));
                self.pop_stack(1);
                self.generate_expr(right.as_ref())?;
                self.commit_jump_fixup(label);
            }
        }

        Ok(())
    }

    fn generate_stmt(&mut self, stmt: &Stmt) -> Result<(), anyhow::Error> {
        match stmt {
            Stmt::Cond { arms, otherwise } => {}
            Stmt::List { .. } => {}
            Stmt::Range { .. } => {}
            Stmt::While {
                id,
                condition,
                body,
            } => {
                let loop_label = self.add_jump(*id);
                // Push condition ops
                self.generate_expr(condition).as_ref()?;
                match id {
                    None => {
                        self.emit(Op::While(loop_label))
                    }
                    Some(id) => {
                        let vr = self.add_var_ref(id);
                        self.emit(Op::WhileId {id: id.0, label: loop_label})
                    }
                }
                let end_label = self.add_jump(None);
                self.pop_stack(1);
                self.enter_loop(loop_label);

            }
            Stmt::Fork { .. } => {}
            Stmt::Catch { .. } => {}
            Stmt::Finally { .. } => {}
            Stmt::Break { exit } | Stmt::Continue { exit } => {
                let lp = match exit {
                    None => {
                        self.emit(Op::Exit);
                        let l = self.loops.last_mut().expect("No loop to exit in codegen");
                        l
                    }
                    Some(eid) => {
                        let loop_label = self.add_var_ref(eid);
                        self.emit(Op::ExitId { id: loop_label });
                        let l = self
                            .loops
                            .iter_mut()
                            .find(|l| l.id == eid.0)
                            .expect("Can't find loop in CONTINUE_LOOP");
                        l
                    }
                };

                if let Stmt::Continue { .. } = stmt {
                    self.add_stack_ref(lp.top_stack);
                    self.add_known_label(lp.top_label);
                } else {
                    self.add_stack_ref(lp.bottom_stack);
                    lp.bottom_label = self.add_linked_label(lp.bottom_label);
                }
            }
            Stmt::Return { expr } => match expr {
                Some(expr) => {
                    self.generate_expr(expr)?;
                    self.emit(Op::Return);
                    self.pop_stack(1);
                }
                None => {
                    self.emit(Op::Return);
                }
            },
            Stmt::Expr(e) => {
                self.generate_expr(e)?;
                self.emit(Op::Pop);
                self.pop_stack(1);
            }
            Stmt::Exit(_) => {}
        }

        Ok(())
    }
}
