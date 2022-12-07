use itertools::Itertools;
use paste::expr;
use serde_derive::{Deserialize, Serialize};

use crate::compiler::ast::{Arg, BinaryOp, Expr, ScatterKind, Stmt, UnaryOp};
use crate::compiler::parse::{parse_program, Name, Names};
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
    end_label: usize,
}

pub struct State {
    pub(crate) ops: Vec<Op>,
    pub(crate) jumps: Vec<JumpLabel>,
    pub(crate) varnames: Names,
    pub(crate) literals: Vec<Var>,
    pub(crate) loops: Vec<Loop>,
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

    fn fixup_jump(&mut self, id: usize) {
        let position = self.ops.len();
        let jump = &mut self.jumps.get_mut(id).expect("Invalid jump fixup");
        let npos = position;
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
                let l = self.loops.iter().find(|l| l.start_label == eid.0);
                l.expect("Can't find loop in continue / break")
            }
        }
    }

    fn generate_assign(
        &mut self,
        left: &Box<Expr>,
        right: &Box<Expr>,
    ) -> Result<(), anyhow::Error> {
        match left.as_ref() {
            // Scattering assignment on left is special.
            Expr::Scatter(sa) => {
                todo!("Scatter assignment");
            }
            _ => {
                self.generate_lvalue(left, false)?;
                self.generate_expr(right)?;
                match left.as_ref() {
                    Expr::Range { base, from, to } => self.emit(Op::PutTemp),
                    Expr::Index(lhs, rhs) => self.emit(Op::PutTemp),
                    _ => {}
                }
                let mut is_indexed = false;
                let mut e = left;
                loop {
                    // Figure out the form of assignment, handle correctly, then walk through
                    // chained assignments
                    match left.as_ref() {
                        Expr::Range { base, from, to } => {
                            self.emit(Op::RangeSet);
                            e = base;
                            is_indexed = true;
                            continue;
                        }
                        Expr::Index(lhs, rhs) => {
                            self.emit(Op::IndexSet);
                            e = lhs;
                            is_indexed = true;
                            continue;
                        }
                        Expr::Id(name) => {
                            self.emit(Op::Put(name.0));
                            break;
                        }
                        Expr::Prop { location, property } => {
                            self.emit(Op::PutProp);
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
            }
        }

        Ok(())
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
                if indexed_above {
                    self.emit(Op::Push(id.0));
                }
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
            Expr::Id(ident) => self.emit(Op::Push(ident.0)),
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
            Expr::Call { function, args } => {
                self.generate_arg_list(args)?;
                self.emit(Op::FuncCall { id: function.0 });
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
            }
            Expr::Range { base, from, to } => {
                self.generate_expr(base.as_ref())?;
                self.generate_expr(from.as_ref())?;
                self.generate_expr(to.as_ref())?;
                self.emit(Op::RangeRef);
            }
            Expr::Cond {
                alternative,
                condition,
                consequence,
            } => {}
            Expr::Catch { .. } => {}
            Expr::List(l) => {
                self.generate_arg_list(l)?;
            }
            Expr::Scatter(_) => {}
            Expr::Length => {
                self.emit(Op::Length);
            }
            Expr::And(left, right) => {
                self.generate_expr(left.as_ref())?;
                let label = self.add_jump(None);
                self.emit(Op::And(label));
                self.generate_expr(right.as_ref())?;
                self.fixup_jump(label);
            }
            Expr::Or(left, right) => {
                self.generate_expr(left.as_ref())?;
                let label = self.add_jump(None);
                self.emit(Op::Or(label));
                self.generate_expr(right.as_ref())?;
                self.fixup_jump(label);
            }
            Expr::This => self.emit(Op::This),
            Expr::Assign { left, right } => self.generate_assign(left, right)?,
        }

        Ok(())
    }

    pub fn generate_stmt(&mut self, stmt: &Stmt) -> Result<(), anyhow::Error> {
        match stmt {
            Stmt::Cond { arms, otherwise } => {
                let mut end_label = None;
                for arm in arms {
                    self.generate_expr(&arm.condition)?;
                    let else_label = self.add_jump(None);
                    self.emit(Op::If(else_label));
                    for stmt in &arm.statements {
                        self.generate_stmt(&stmt)?;
                    }
                    end_label = Some(self.add_jump(None));
                    self.emit(Op::Jump {
                        label: end_label.unwrap(),
                    });
                    self.fixup_jump(else_label);
                }
                if !otherwise.is_empty() {
                    for stmt in otherwise {
                        self.generate_stmt(stmt)?;
                    }
                }
                if end_label.is_some() {
                    self.fixup_jump(end_label.unwrap());
                }
            }
            Stmt::ForList { id, expr, body } => {
                self.generate_expr(expr)?;
                self.emit(Op::Val(Var::Int(1))); /* loop list index... */
                let loop_top = self.add_jump(None);
                self.fixup_jump(loop_top);
                let end_label = self.add_jump(None);
                self.emit(Op::ForList {
                    id: id.0,
                    label: end_label,
                });
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::Jump { label: loop_top });
                self.fixup_jump(end_label);
            }
            Stmt::ForRange { .. } => {}
            Stmt::While {
                id,
                condition,
                body,
            } => {
                let loop_start_label = self.add_jump(*id);
                let loop_end_label = self.add_jump(None);
                self.generate_expr(condition)
                    .as_ref()
                    .expect("compile expr");
                match id {
                    None => self.emit(Op::While(loop_end_label)),
                    Some(id) => self.emit(Op::WhileId {
                        id: id.0,
                        label: loop_end_label,
                    }),
                }
                self.loops.push(Loop {
                    start_label: loop_start_label,
                    end_label: loop_end_label,
                });
                for s in body {
                    self.generate_stmt(s)?;
                }
                self.emit(Op::Jump {
                    label: loop_start_label,
                });
                self.fixup_jump(loop_end_label);
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

    fn generate_arg_list(&mut self, args: &Vec<Arg>) -> Result<(), anyhow::Error> {
        if args.is_empty() {
            self.emit(Op::MkEmptyList);
        } else {
            let mut normal_op = Op::MakeSingletonList;
            let mut splice_op = Op::CheckListForSplice;
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
                normal_op = Op::ListAddTail;
                splice_op = Op::ListAppend;
            }
        }

        Ok(())
    }
}

pub fn compile(program: &str) -> Result<Binary, anyhow::Error> {
    let parse = parse_program(program)?;
    let mut cg_state = State::new(parse.names);
    for x in parse.stmts {
        cg_state.generate_stmt(&x)?;
    }

    let binary = Binary {
        first_lineno: 0,
        literals: cg_state.literals,
        jump_labels: cg_state.jumps,
        var_names: cg_state.varnames.names,
        main_vector: cg_state.ops,
    };

    Ok(binary)
}

#[cfg(test)]
mod tests {
    use crate::vm::opcode::Op;
    use crate::vm::opcode::Op::*;

    use super::*;

    #[test]
    fn test_simple_add_expr() {
        let program = "1 + 2;";
        let parse = compile(program).unwrap();
        assert_eq!(
            parse.main_vector,
            vec![Op::Imm(0), Op::Imm(1), Op::Add, Op::Pop]
        );
    }

    #[test]
    fn test_var_assign_expr() {
        /*
                "=================", "[Bytes for labels = 1, literals = 1, forks = 1, variables = 1,
        stack refs = 1]", "[Maximum stack size = 2]",
            "  0: 124 NUM 1",
            "  1: 125 NUM 2",
            "  2: 021 * ADD",
            "  3: 052 * PUT a",
            "  4: 111 POP",
            "  5: 123 NUM 0",
            "  6: 030 010 * AND 10",
                 */
        let program = "a = 1 + 2;";
        let binary = compile(program).unwrap();
        assert_eq!(binary.var_names, vec!["a".to_string()]);
        assert_eq!(binary.literals, vec![Var::Int(1), Var::Int(2)]);
        assert_eq!(binary.main_vector, vec![Imm(0), Imm(1), Add, Put(0), Pop],);
    }

    #[test]
    fn test_var_assign_retr_expr() {
        let program = "a = 1 + 2; return a;";
        let parse = compile(program).unwrap();
        assert_eq!(
            parse.main_vector,
            vec![Imm(0), Imm(1), Add, Put(0), Pop, Push(0), Return]
        );
    }

    #[test]
    fn test_if_stmt() {
        let program = "if (1 == 2) return 5; elseif (2 == 3) return 3; else return 6; endif";
        let binary = compile(program).unwrap();
        assert_eq!(
            binary.literals,
            vec![
                Var::Int(1),
                Var::Int(2),
                Var::Int(5),
                Var::Int(3),
                Var::Int(6)
            ]
        );
        /*
         0: 124                   NUM 1
         1: 125                   NUM 2
         2: 023                 * EQ
         3: 000 009             * IF 9
         5: 128                   NUM 5
         6: 108                   RETURN
         7: 107 020               JUMP 20
         9: 125                   NUM 2
        10: 126                   NUM 3
        11: 023                 * EQ
        12: 002 018             * ELSEIF 18
        14: 126                   NUM 3
        15: 108                   RETURN
        16: 107 020               JUMP 20
        18: 129                   NUM 6
        19: 108                   RETURN

                */
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0),
                Imm(1),
                Eq,
                If(0),
                Imm(2),
                Return,
                Jump { label: 1 },
                Imm(1),
                Imm(3),
                Eq,
                If(2),
                Imm(3),
                Return,
                Jump { label: 3 },
                Imm(4),
                Return
            ]
        );
    }

    #[test]
    fn test_while_stmt() {
        let program = "while (1) x = x + 1; endwhile";
        let binary = compile(program).unwrap();
        assert_eq!(binary.var_names, vec!["x".to_string()]);
        assert_eq!(binary.literals, vec![Var::Int(1)]);

        /*
        " 0: 124                   NUM 1",
        " 1: 001 010             * WHILE 10",
        "  3: 085                   PUSH x",
        "  4: 124                    NUM 1",
        "  5: 021                 * ADD",
        "  6: 052                 * PUT x",
        "  7: 111                   POP",
        "  8: 107 000               JUMP 0",
                 */
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0),
                While(1),
                Push(0),
                Imm(0),
                Add,
                Put(0),
                Pop,
                Jump { label: 0 }
            ]
        );
        assert_eq!(binary.jump_labels[0].position, 0);
        assert_eq!(binary.jump_labels[1].position, 8);
    }

    #[test]
    fn test_while_label_stmt() {
        let program = "while chuckles (1) x = x + 1; if (x > 5) break chuckles; endif endwhile";
        let binary = compile(program).unwrap();
        assert_eq!(
            binary.var_names,
            vec!["chuckles".to_string(), "x".to_string()]
        );
        assert_eq!(binary.literals, vec![Var::Int(1), Var::Int(5)]);

        /*
                 0: 124                   NUM 1
         1: 112 010 019 024     * WHILE_ID chuckles 24
         5: 085                   PUSH x
         6: 124                   NUM 1
         7: 021                 * ADD
         8: 052                 * PUT x
         9: 111                   POP
        10: 085                   PUSH x
        11: 128                   NUM 5
        12: 027                 * GT
        13: 000 022             * IF 22
        15: 112 012 019 000 024 * EXIT_ID chuckles 0 24
        20: 107 022               JUMP 22
        22: 107 000               JUMP 0
                        */
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0), WhileId { id: 0, label: 1 }, Push(1), Imm(0), Add, Put(1), Pop, Push(1), Imm(1), Gt, If(2), Break(1), Jump { label: 3 }, Jump { label: 0 }
            ]
        );
        assert_eq!(binary.jump_labels[0].position, 0);
        assert_eq!(binary.jump_labels[1].position, 14);
    }
    #[test]
    fn test_while_break_continue_stmt() {
        let program = "while (1) x = x + 1; if (x == 5) break; else continue; endif endwhile";
        let binary = compile(program).unwrap();
        assert_eq!(binary.var_names, vec!["x".to_string()]);
        assert_eq!(binary.literals, vec![Var::Int(1), Var::Int(5)]);

        /*
        "  0: 124                   NUM 1",
        "1: 001 025             * WHILE 25",
        "  3: 085                   PUSH x",
        "  4: 124                   NUM 1",
        "  5: 021                 * ADD",
        "  6: 052                 * PUT x",
        "  7: 111                   POP",
        "  8: 085                   PUSH x",
        "  9: 128                   NUM 5",
        " 10: 023                 * EQ",
        " 11: 000 019             * IF 19",
        " 13: 112 011 000 025     * EXIT 0 25",
        " 17: 107 023               JUMP 23",
        " 19: 112 011 000 000     * EXIT 0 0",
        " 23: 107 000               JUMP 0"
                 */
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0),
                While(1),
                Push(0),
                Imm(0),
                Add,
                Put(0),
                Pop,
                Push(0),
                Imm(1),
                Eq,
                If(2),
                Break(1),
                Jump { label: 3 },
                Continue(0),
                Jump { label: 0 }
            ]
        );
        assert_eq!(binary.jump_labels[0].position, 0);
        assert_eq!(binary.jump_labels[1].position, 15);
    }
    #[test]
    fn test_for_in_list_stmt() {
        let program = "for x in ({1,2,3}) b = x + 5; endfor";
        let binary = compile(program).unwrap();
        assert_eq!(binary.var_names, vec!["x".to_string(), "b".to_string()]);

        /*
        "  0: 124                   NUM 1",
        "  1: 016                 * MAKE_SINGLETON_LIST",
        "  2: 125                   NUM 2",
        "  3: 102                   LIST_ADD_TAIL",
        "  4: 126                   NUM 3",
        "  5: 102                  LIST_ADD_TAIL",
        "  6: 124                   NUM 1",
        "  7: 005 019017         * FOR_LIST x 17",
        " 10: 086                   PUSH x",
        " 11: 128                    NUM 5",
        " 12: 021                 * ADD",
        " 13: 052                 * PUT a",
        " 14: 111                   POP",
        " 15: 107 007               JUMP 7",
                 */
        // The label for the ForList is not quite right here
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0),
                MakeSingletonList,
                Imm(1),
                ListAddTail,
                Imm(2),
                ListAddTail,
                Val(Var::Int(1)),
                ForList { id: 0, label: 1 },
                Push(0),
                Imm(3),
                Add,
                Put(1),
                Pop,
                Jump { label: 0 }
            ]
        );
        assert_eq!(binary.jump_labels[0].position, 7);
        assert_eq!(binary.jump_labels[1].position, 14);
    }

    #[test]
    fn test_builtin_call() {
        let program = "call_builtin(1, 2, 3);";
        let parse = compile(program).unwrap();
        assert_eq!(
            parse.main_vector,
            vec![
                Op::Imm(0),
                Op::MakeSingletonList,
                Op::Imm(1),
                Op::ListAddTail,
                Op::Imm(2),
                Op::ListAddTail,
                Op::FuncCall { id: 0 },
                Op::Pop
            ]
        );
    }

    // TODO: this is incorrect.
    // look at moo disassembly for the same code.
    // POP", "  8: 085                   PUSH a", "  9: 124                   NUM 1", " 10: 112 001 000           LENGTH 0", " 13:
    // 015                 * RANGE",
    #[test]
    fn test_range_length() {
        let program = "a = {1, 2, 3}; b = a[2..$];";
        let parse = compile(program).unwrap();
        assert_eq!(
            parse.main_vector,
            [
                Imm(0),
                MakeSingletonList,
                Imm(1),
                ListAddTail,
                Imm(2),
                ListAddTail,
                Put(0),
                Pop,
                Length,
                Put(1),
                Pop
            ]
        );
    }
}