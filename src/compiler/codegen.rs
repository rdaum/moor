use std::collections::HashMap;

use anyhow::anyhow;
use serde_derive::{Deserialize, Serialize};
use thiserror::Error;

use crate::compiler::ast::{Arg, BinaryOp, Expr, Stmt, UnaryOp};
use crate::compiler::parse::{parse_program, Name, Names};
use crate::model::var::Var;
use crate::vm::opcode::Op::Jump;
use crate::vm::opcode::{Binary, Op};

#[derive(Error, Debug)]
pub enum CompileError {
    #[error("Unknown built-in function: {0}")]
    UnknownBuiltinFunction(String),
    #[error("Could not find loop with id: {0}")]
    UnknownLoopLabel(String),
}

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

// Compiler code generation state.
pub struct CodegenState {
    pub(crate) ops: Vec<Op>,
    pub(crate) jumps: Vec<JumpLabel>,
    pub(crate) var_names: Names,
    pub(crate) literals: Vec<Var>,
    pub(crate) loops: Vec<Loop>,
    pub(crate) saved_stack: Option<usize>,
    pub(crate) cur_stack: usize,
    pub(crate) max_stack: usize,
    pub(crate) builtins: HashMap<String, usize>,
    pub(crate) fork_vectors: Vec<Vec<Op>>,
}

impl CodegenState {
    pub fn new(var_names: Names, builtins: HashMap<String, usize>) -> Self {
        Self {
            ops: vec![],
            jumps: vec![],
            var_names,
            literals: vec![],
            loops: vec![],
            saved_stack: None,
            cur_stack: 0,
            max_stack: 0,
            builtins,
            fork_vectors: vec![],
        }
    }

    // Create an anonymous jump label at the current position and return its unique ID.
    fn add_label(&mut self, name: Option<Name>) -> usize {
        let id = self.jumps.len();
        let position = self.ops.len();
        self.jumps.push(JumpLabel {
            id,
            label: name,
            position,
        });
        id
    }

    fn find_label(&self, name: &Name) -> Option<&JumpLabel> {
        self.jumps.iter().find(|j| {
            if let Some(label) = j.label {
                label.eq(name)
            } else {
                false
            }
        })
    }

    // Adjust the position of a jump label to the current position.
    fn define_label(&mut self, id: usize) {
        let position = self.ops.len();
        let jump = &mut self.jumps.get_mut(id).expect("Invalid jump fixup");
        let npos = position;
        jump.position = npos;
    }

    fn add_literal(&mut self, v: &Var) -> usize {
        let lv_pos = self.literals.iter().position(|lv| lv.eq(v));
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

    fn find_loop(&self, loop_label: &Option<Name>) -> Result<&Loop, anyhow::Error>  {
        match loop_label {
            None => {
                let l = self.loops.last().expect("No loop to exit in codegen");
                Ok(l)
            }
            Some(eid) => {
                match self.find_label(eid) {
                    None => {
                        let loop_name = self.var_names.names[eid.0].clone();
                        return Err(anyhow!(CompileError::UnknownLoopLabel(loop_name)));
                    }
                    Some(label) => {
                        Ok(&self.loops[label.id])
                    }
                }
            }
        }
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

    fn save_stack_top(&mut self) -> Option<usize> {
        let old = self.saved_stack;
        self.saved_stack = Some(self.cur_stack - 1);
        old
    }

    fn saved_stack_top(&self) -> Option<usize> {
        self.saved_stack
    }

    fn restore_stack_top(&mut self, old: Option<usize>) {
        self.saved_stack = old
    }

    fn add_fork_vector(&mut self, opcodes: Vec<Op>) -> usize {
        let fv = self.fork_vectors.len();
        self.fork_vectors.push(opcodes);
        fv
    }

    fn generate_assign(
        &mut self,
        left: &Box<Expr>,
        right: &Box<Expr>,
    ) -> Result<(), anyhow::Error> {
        match left.as_ref() {
            // Scattering assignment on left is special.
            Expr::Scatter(_sa) => {
                todo!("Scatter assignment");
            }
            _ => {
                self.push_lvalue(left, false)?;
                self.generate_expr(right)?;
                match left.as_ref() {
                    Expr::Range {
                        base: _,
                        from: _,
                        to: _,
                    } => self.emit(Op::PutTemp),
                    Expr::Index(_lhs, _rhs) => self.emit(Op::PutTemp),
                    _ => {}
                }
                let mut is_indexed = false;
                let mut e = left;
                loop {
                    // Figure out the form of assignment, handle correctly, then walk through
                    // chained assignments
                    match left.as_ref() {
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
                            self.emit(Op::Put(name.0));
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
            }
        }

        Ok(())
    }

    fn push_lvalue(&mut self, expr: &Expr, indexed_above: bool) -> Result<(), anyhow::Error> {
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
                    self.emit(Op::Push(id.0));
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

    fn generate_codes(&mut self, arg_list: &Vec<Arg>) -> Result<(), anyhow::Error> {
        if !arg_list.is_empty() {
            self.generate_arg_list(arg_list)?;
        } else {
            self.emit(Op::Push(0));
            self.push_stack(0);
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
                self.emit(Op::Push(ident.0));
                self.push_stack(1);
            }
            Expr::And(left, right) => {
                self.generate_expr(left.as_ref())?;
                let end_label = self.add_label(None);
                self.emit(Op::And(end_label));
                self.pop_stack(1);
                self.generate_expr(right.as_ref())?;
                self.define_label(end_label);
            }
            Expr::Or(left, right) => {
                self.generate_expr(left.as_ref())?;
                let end_label = self.add_label(None);
                self.emit(Op::Or(end_label));
                self.pop_stack(1);
                self.generate_expr(right.as_ref())?;
                self.define_label(end_label);
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
                let old;
                self.generate_expr(base.as_ref())?;
                old = self.save_stack_top();
                self.generate_expr(from.as_ref())?;
                self.generate_expr(to.as_ref())?;
                self.restore_stack_top(old);
                self.emit(Op::RangeRef);
                self.pop_stack(2);
            }
            Expr::Length => {
                let saved = self.save_stack_top();
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
            }
            Expr::Call { function, args } => {
                // Lookup builtin.
                let Some(builtin) = self.builtins.get(function) else {
                    return Err(CompileError::UnknownBuiltinFunction(function.clone()).into());
                };
                let builtin = *builtin;
                self.generate_arg_list(args)?;
                self.emit(Op::FuncCall { id: builtin });
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
                let else_label = self.add_label(None);
                self.emit(Op::IfQues(else_label));
                self.pop_stack(1);
                self.generate_expr(consequence.as_ref())?;
                let end_label = self.add_label(None);
                self.emit(Op::Jump { label: end_label });
                self.pop_stack(1);
                self.define_label(else_label);
                self.generate_expr(alternative.as_ref())?;
                self.define_label(end_label);
            }
            Expr::Catch {
                codes,
                except,
                trye,
            } => {
                self.generate_codes(codes)?;
                let handler_label = self.add_label(None);
                self.emit(Op::PushLabel(handler_label));
                self.push_stack(1);
                self.emit(Op::Catch);
                self.push_stack(1);
                self.generate_expr(trye.as_ref())?;
                let end_label = self.add_label(None);
                self.emit(Op::EndCatch(end_label));
                self.pop_stack(3)   /* codes, label, catch */;

                /* After this label, we still have a value on the stack, but now,
                 * instead of it being the value of the main expression, we have
                 * the exception tuple pushed before entering the handler.
                 */
                match except {
                    Some(except) => {
                        self.emit(Op::Pop);
                        self.pop_stack(1);
                        self.generate_expr(except.as_ref())?;
                    }
                    None => {
                        self.emit(Op::Val(Var::Int(1)));
                        self.emit(Op::Ref);
                    }
                }
                self.define_label(end_label);
            }
            Expr::List(l) => {
                self.generate_arg_list(l)?;
            }
            Expr::Scatter(_) => {}
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
                    let else_label = self.add_label(None);
                    self.emit(Op::If(else_label));
                    self.pop_stack(1);
                    for stmt in &arm.statements {
                        self.generate_stmt(stmt)?;
                    }
                    end_label = Some(self.add_label(None));
                    self.emit(Op::Jump {
                        label: end_label.unwrap(),
                    });
                    self.define_label(else_label);
                }
                if !otherwise.is_empty() {
                    for stmt in otherwise {
                        self.generate_stmt(stmt)?;
                    }
                }
                if end_label.is_some() {
                    self.define_label(end_label.unwrap());
                }
            }
            Stmt::ForList { id, expr, body } => {
                self.generate_expr(expr)?;
                self.emit(Op::Val(Var::Int(1))); /* loop list index... */
                self.push_stack(1);
                let loop_top = self.add_label(None);
                self.define_label(loop_top);
                let end_label = self.add_label(None);
                // TODO self.enter_loop/exit_loop needed?
                self.emit(Op::ForList {
                    id: id.0,
                    label: end_label,
                });
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::Jump { label: loop_top });
                self.define_label(end_label);
                self.pop_stack(2);
            }
            Stmt::ForRange { from, to, id, body } => {
                self.generate_expr(from)?;
                self.generate_expr(to)?;
                let loop_top = self.add_label(None);
                let end_label = self.add_label(None);
                self.emit(Op::ForRange {
                    id: id.0,
                    label: end_label,
                });
                // TODO self.enter_loop/exit_loop needed?
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Jump { label: loop_top });
                self.define_label(end_label);
                self.pop_stack(2);
            }
            Stmt::While {
                id,
                condition,
                body,
            } => {
                let loop_start_label = self.add_label(*id);
                let loop_end_label = self.add_label(None);
                self.generate_expr(condition)?;
                match id {
                    None => self.emit(Op::While(loop_end_label)),
                    Some(id) => self.emit(Op::WhileId {
                        id: id.0,
                        label: loop_end_label,
                    }),
                }
                self.pop_stack(1);
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
                self.define_label(loop_end_label);
            }
            Stmt::Fork { id, body, time } => {
                self.generate_expr(time)?;
                // Stash all of main vector in a temporary buffer, then begin compilation of the forked code.
                // Once compiled, we can create a fork vector from the new buffer, and then restore the main vector.
                let stashed_ops = std::mem::take(&mut self.ops);
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                let forked_ops = std::mem::take(&mut self.ops);
                let fv_id = self.add_fork_vector(forked_ops);
                self.ops = stashed_ops;
                self.emit(Op::Fork {
                    id: id.map(|i| i.0),
                    f_index: fv_id,
                });
                self.pop_stack(1);
            }
            Stmt::TryExcept { body, excepts } => {
                for ex in excepts {
                    self.generate_codes(&ex.codes)?;
                    let push_label = self.add_label(None);
                    self.emit(Op::PushLabel(push_label));
                    self.push_stack(1);
                }
                let arm_count = excepts.len();
                self.emit(Op::TryExcept(arm_count));
                self.push_stack(1);
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                let end_label = self.add_label(None);
                self.emit(Op::EndExcept(end_label));
                self.pop_stack(2 * arm_count + 1);
                for (i, ex) in excepts.iter().enumerate() {
                    // let label = self.add_jump(ex.id);  TODO hmm
                    self.push_stack(1);
                    if ex.id.is_some() {
                        self.emit(Op::Put(ex.id.unwrap().0));
                    }
                    self.emit(Op::Pop);
                    self.pop_stack(1);
                    for stmt in &ex.statements {
                        self.generate_stmt(stmt)?;
                    }
                    if i + 1 < excepts.len() {
                        let arm_end_label = self.add_label(None);
                        self.emit(Op::Jump {
                            label: arm_end_label,
                        });
                        self.define_label(arm_end_label);
                    }
                }
                self.define_label(end_label);
            }
            Stmt::TryFinally { body, handler } => {
                let handler_label = self.add_label(None);
                self.emit(Op::TryFinally(handler_label));
                self.push_stack(1);
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::EndFinally);
                self.pop_stack(1);
                self.define_label(handler_label);
                self.push_stack(2); /* continuation value, reason */
                for stmt in handler {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::Continue);
                self.pop_stack(2);
            }
            Stmt::Break { exit } => {
                let lp = self.find_loop(exit)?;
                self.emit(Op::Exit(Some(lp.end_label)));
            }
            Stmt::Continue { exit } => {
                let lp = self.find_loop(exit)?;
                self.emit(Op::Exit(Some(lp.start_label)));
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
        }

        Ok(())
    }

    fn generate_arg_list(&mut self, args: &Vec<Arg>) -> Result<(), anyhow::Error> {
        if args.is_empty() {
            self.emit(Op::MkEmptyList);
            self.push_stack(1);
        } else {
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
        }

        Ok(())
    }
}

pub fn compile(program: &str, builtins: HashMap<String, usize>) -> Result<Binary, anyhow::Error> {
    let parse = parse_program(program)?;
    let mut cg_state = CodegenState::new(parse.names, builtins);
    for x in parse.stmts {
        cg_state.generate_stmt(&x)?;
    }

    cg_state.emit(Op::Done);

    if cg_state.cur_stack != 0 {
        return Err(anyhow!("stack not entirely popped after code generation"));
    }
    if cg_state.saved_stack.is_some() {
        return Err(anyhow!("saved stack still present after code generation"));
    }

    let binary = Binary {
        literals: cg_state.literals,
        jump_labels: cg_state.jumps,
        var_names: cg_state.var_names.names,
        main_vector: cg_state.ops,
        fork_vectors: cg_state.fork_vectors,
    };

    Ok(binary)
}

#[cfg(test)]
mod tests {
    use crate::model::var::Error::{E_INVARG, E_PERM, E_PROPNF};
    use crate::vm::opcode::Op::*;

    use super::*;

    #[test]
    fn test_simple_add_expr() {
        let program = "1 + 2;";
        let binary = compile(program, HashMap::new()).unwrap();
        let one = binary.find_literal(1.into());
        let two = binary.find_literal(2.into());
        assert_eq!(binary.main_vector, vec![Imm(one), Imm(two), Add, Pop, Done]);
    }

    #[test]
    fn test_var_assign_expr() {

        let program = "a = 1 + 2;";
        let binary = compile(program, HashMap::new()).unwrap();

        let a = binary.find_var("a");
        let one = binary.find_literal(1.into());
        let two = binary.find_literal(2.into());
        /*
        "  0: 124 NUM 1",
        "  1: 125 NUM 2",
        "  2: 021 * ADD",
        "  3: 052 * PUT a",
        "  4: 111 POP",
        "  5: 123 NUM 0",
        "  6: 030 010 * AND 10",
     */
        assert_eq!(
            binary.main_vector,
            vec![Imm(one), Imm(two), Add, Put(a), Pop, Done],
        );
    }

    #[test]
    fn test_var_assign_retr_expr() {
        let program = "a = 1 + 2; return a;";
        let binary = compile(program, HashMap::new()).unwrap();

        let a = binary.find_var("a");
        let one = binary.find_literal(1.into());
        let two = binary.find_literal(2.into());

        assert_eq!(
            binary.main_vector,
            vec![Imm(one), Imm(two), Add, Put(a), Pop, Push(a), Return, Done]
        );
    }

    #[test]
    fn test_if_stmt() {
        let program = "if (1 == 2) return 5; elseif (2 == 3) return 3; else return 6; endif";
        let binary = compile(program, HashMap::new()).unwrap();

        let one = binary.find_literal(1.into());
        let two = binary.find_literal(2.into());
        let three = binary.find_literal(3.into());
        let five = binary.find_literal(5.into());
        let six = binary.find_literal(6.into());

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
                Imm(one),
                Imm(two),
                Eq,
                If(0),
                Imm(five),
                Return,
                Jump { label: 1 },
                Imm(two),
                Imm(three),
                Eq,
                If(2),
                Imm(three),
                Return,
                Jump { label: 3 },
                Imm(six),
                Return,
                Done
            ]
        );
    }

    #[test]
    fn test_while_stmt() {
        let program = "while (1) x = x + 1; endwhile";
        let binary = compile(program, HashMap::new()).unwrap();

        let x = binary.find_var("x");
        let one = binary.find_literal(1.into());

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
                Imm(one),
                While(1),
                Push(x),
                Imm(one),
                Add,
                Put(x),
                Pop,
                Jump { label: 0 },
                Done
            ]
        );
        assert_eq!(binary.jump_labels[0].position, 0);
        assert_eq!(binary.jump_labels[1].position, 8);
    }

    #[test]
    fn test_while_label_stmt() {
        let program = "while chuckles (1) x = x + 1; if (x > 5) break chuckles; endif endwhile";
        let binary = compile(program, HashMap::new()).unwrap();

        let x = binary.find_var("x");
        let chuckles = binary.find_var("chuckles");
        let one = binary.find_literal(1.into());
        let five = binary.find_literal(5.into());

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
                Imm(one),
                WhileId { id: chuckles, label: 1 },
                Push(x),
                Imm(one),
                Add,
                Put(x),
                Pop,
                Push(x),
                Imm(five),
                Gt,
                If(2),
                Exit(Some(1)),
                Jump { label: 3 },
                Jump { label: 0 },
                Done,
            ]
        );
        assert_eq!(binary.jump_labels[0].position, 0);
        assert_eq!(binary.jump_labels[1].position, 14);
    }
    #[test]
    fn test_while_break_continue_stmt() {
        let program = "while (1) x = x + 1; if (x == 5) break; else continue; endif endwhile";
        let binary = compile(program, HashMap::new()).unwrap();

        let x = binary.find_var("x");
        let one = binary.find_literal(1.into());
        let five = binary.find_literal(5.into());


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
                Imm(one),
                While(1),
                Push(x),
                Imm(one),
                Add,
                Put(x),
                Pop,
                Push(x),
                Imm(five),
                Eq,
                If(2),
                Exit(Some(1)),
                Jump { label: 3 },
                Exit(Some(0)),
                Jump { label: 0 },
                Done
            ]
        );
        assert_eq!(binary.jump_labels[0].position, 0);
        assert_eq!(binary.jump_labels[1].position, 15);
    }
    #[test]
    fn test_for_in_list_stmt() {
        let program = "for x in ({1,2,3}) b = x + 5; endfor";
        let binary = compile(program, HashMap::new()).unwrap();

        let b = binary.find_var("b");
        let x = binary.find_var("x");
        let one = binary.find_literal(1.into());
        let two = binary.find_literal(2.into());
        let three = binary.find_literal(3.into());
        let five = binary.find_literal(5.into());

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
        " 13: 052                 * PUT b",
        " 14: 111                   POP",
        " 15: 107 007               JUMP 7",
                 */
        // The label for the ForList is not quite right here
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(one),
                MakeSingletonList,
                Imm(two),
                ListAddTail,
                Imm(three),
                ListAddTail,
                Val(Var::Int(1)),
                ForList { id: x, label: 1 },
                Push(x),
                Imm(five),
                Add,
                Put(b),
                Pop,
                Jump { label: 0 },
                Done
            ]
        );
        assert_eq!(binary.jump_labels[0].position, 7);
        assert_eq!(binary.jump_labels[1].position, 14);
    }

    #[test]
    fn test_for_range() {
        let program = "for n in [1..5] player:tell(a); endfor";
        let binary = compile(program, HashMap::new()).unwrap();

        let player = binary.find_var("player");
        let a = binary.find_var("a".into());
        let n = binary.find_var("n".into());
        let tell = binary.find_literal("tell".into());
        let one = binary.find_literal(1.into());
        let five = binary.find_literal(5.into());

        /*
         0: 124                   NUM 1
         1: 128                   NUM 5
         2: 006 019 014         * FOR_RANGE n 14
         5: 072                   PUSH player
         6: 100 000               PUSH_LITERAL "tell"
         8: 085                   PUSH a
         9: 016                 * MAKE_SINGLETON_LIST
        10: 010                 * CALL_VERB
        11: 111                   POP
        12: 107 002               JUMP 2
        */
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(one),
                Imm(five),
                ForRange { id: n, label: 1 },
                Push(player),
                Imm(tell),
                Push(a),
                MakeSingletonList,
                CallVerb,
                Pop,
                Jump { label: 0 },
                Done
            ]
        );
    }

    #[test]
    fn test_fork() {
        let program = "fork (5) player:tell(\"a\"); endfork";
        let binary = compile(program, HashMap::new()).unwrap();

        let player = binary.find_var("player");
        let a = binary.find_literal("a".into());
        let tell = binary.find_literal("tell".into());

        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0),
                Fork {
                    f_index: 0,
                    id: None
                },
                Done
            ]
        );
        assert_eq!(
            binary.fork_vectors[0],
            vec![
                Push(player), // player
                Imm(tell),  // tell
                Imm(a),  // 'a'
                MakeSingletonList,
                CallVerb,
                Pop
            ]
        );
    }

    #[test]
    fn test_fork_id() {
        let program = "fork fid (5) player:tell(fid); endfork";
        let binary = compile(program, HashMap::new()).unwrap();

        let player = binary.find_var("player");
        let fid = binary.find_var("fid");
        let five = binary.find_literal(5.into());
        let tell = binary.find_literal("tell".into());

        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0),
                Fork {
                    f_index: 0,
                    id: Some(fid)
                },
                Done
            ]
        );
        assert_eq!(
            binary.fork_vectors[0],
            vec![
                Push(player), // player
                Imm(tell),  // tell
                Push(fid), // fid
                MakeSingletonList,
                CallVerb,
                Pop
            ]
        );
    }

    #[test]
    fn test_and_or() {
        let program = "a = (1 && 2 || 3);";
        let binary = compile(program, HashMap::new()).unwrap();

        let a = binary.find_var("a");
        let one = binary.find_literal(1.into());
        let two = binary.find_literal(2.into());
        let three = binary.find_literal(3.into());

        /*
         0: 124                   NUM 1
         1: 030 004             * AND 4
         3: 125                   NUM 2
         4: 031 007             * OR 7
         6: 126                   NUM 3
         7: 052                 * PUT a
         8: 111                   POP
        */
        assert_eq!(
            binary.main_vector,
            vec![Imm(one), And(0), Imm(two), Or(1), Imm(three), Put(a), Pop, Done]
        );
        assert_eq!(binary.jump_labels[0].position, 3);
        assert_eq!(binary.jump_labels[1].position, 5);
    }

    #[test]
    fn test_unknown_builtin_call() {
        let program = "call_builtin(1, 2, 3);";
        let parse = compile(program, HashMap::new());
        assert!(parse.is_err());
        match parse.err().unwrap().downcast_ref::<CompileError>() {
            Some(CompileError::UnknownBuiltinFunction(name)) => {
                assert_eq!(name, "call_builtin");
            }
            None => {
                panic!("Missing error");
            }
            Some(_) => {
                panic!("Wrong error type")
            }
        }
    }

    #[test]
    fn test_known_builtin() {
        let program = "disassemble(player, \"test\");";
        let mut builtins = HashMap::new();
        builtins.insert(String::from("disassemble"), 0);
        let binary = compile(program, builtins).unwrap();

        let player = binary.find_var("player");
        let test = binary.find_literal("test".into());
        /*
         0: 072                   PUSH player
         1: 016                 * MAKE_SINGLETON_LIST
         2: 100 000               PUSH_LITERAL "test"
         4: 102                   LIST_ADD_TAIL
         5: 012 000             * CALL_FUNC disassemble
         7: 111                   POP
        */
        assert_eq!(
            binary.main_vector,
            vec![
                Push(player), // Player
                MakeSingletonList,
                Imm(test),
                ListAddTail,
                FuncCall { id: 0 },
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_cond_expr() {
        let program = "a = (1 == 2 ? 3 | 4);";
        let binary = compile(program, HashMap::new()).unwrap();

        let a = binary.find_var("a");
        let one = binary.find_literal(1.into());
        let two = binary.find_literal(2.into());
        let three = binary.find_literal(3.into());
        let four = binary.find_literal(4.into());

        /*
         0: 124                   NUM 1
         1: 125                   NUM 2
         2: 023                 * EQ
         3: 013 008             * IF_EXPR 8
         5: 126                   NUM 3
         6: 107 009               JUMP 9
         8: 127                   NUM 4
         9: 052                 * PUT a
        10: 111                   POP
        */
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(one),
                Imm(two),
                Eq,
                IfQues(0),
                Imm(three),
                Jump { label: 1 },
                Imm(four),
                Put(a),
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_verb_call() {
        let program = "player:tell(\"test\");";
        let binary = compile(program, HashMap::new()).unwrap();

        let player = binary.find_var("player");
        let tell = binary.find_literal("tell".into());
        let test = binary.find_literal("test".into());

        /*
              0: 072                   PUSH player
              1: 100 000               PUSH_LITERAL "tell"
              3: 100 001               PUSH_LITERAL "test"
              5: 016                 * MAKE_SINGLETON_LIST
              6: 010                 * CALL_VERB
              7: 111                   POP
        */
        assert_eq!(
            binary.main_vector,
            vec![
                Push(player), // Player
                Imm(tell),
                Imm(test),
                MakeSingletonList,
                CallVerb,
                Pop,
                Done,
            ]
        );
    }

    #[test]
    fn test_range_length() {
        let program = "a = {1, 2, 3}; b = a[2..$];";
        let binary = compile(program, HashMap::new()).unwrap();

        let a = binary.find_var("a");
        let b = binary.find_var("b");
        let one = binary.find_literal(1.into());
        let two = binary.find_literal(2.into());
        let three = binary.find_literal(3.into());

        /*
         0: 124                   NUM 1
         1: 016                 * MAKE_SINGLETON_LIST
         2: 125                   NUM 2
         3: 102                   LIST_ADD_TAIL
         4: 126                   NUM 3
         5: 102                   LIST_ADD_TAIL
         6: 052                 * PUT a
         7: 111                   POP
         8: 085                   PUSH a
         9: 125                   NUM 2
        10: 112 001 000           LENGTH 0
        13: 015                 * RANGE
        14: 053                 * PUT b
        15: 111                   POP
        16: 123                   NUM 0
        17: 030 021             * AND 21
                */
        assert_eq!(
            binary.main_vector,
            [
                Imm(one),
                MakeSingletonList,
                Imm(two),
                ListAddTail,
                Imm(three),
                ListAddTail,
                Put(a),
                Pop,
                Push(a),
                Imm(two),
                Length(0),
                RangeRef,
                Put(b),
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_try_finally() {
        let program = "try a=1; finally a=2; endtry";
        let binary = compile(program, HashMap::new()).unwrap();

        let a = binary.find_var("a");
        let one = binary.find_literal(1.into());
        let two = binary.find_literal(2.into());
        /*
         0: 112 009 008         * TRY_FINALLY 8
         3: 124                   NUM 1
         4: 052                 * PUT a
         5: 111                   POP
         6: 112 005               END_FINALLY
         8: 125                   NUM 2
         9: 052                 * PUT a
        10: 111                   POP
        11: 112 006               CONTINUE
        */
        assert_eq!(
            binary.main_vector,
            vec![
                TryFinally(0),
                Imm(one),
                Put(a),
                Pop,
                EndFinally,
                Imm(two),
                Put(a),
                Pop,
                Continue,
                Done
            ]
        );
    }

    #[test]
    fn test_try_excepts() {
        let program = "try a=1; except a (E_INVARG) a=2; except b (E_PROPNF) a=3; endtry";
        let binary = compile(program, HashMap::new()).unwrap();

        let a = binary.find_var("a");
        let b = binary.find_var("b");
        let e_invarg = binary.find_literal(E_INVARG.into());
        let e_propnf = binary.find_literal(E_PROPNF.into());
        let one = binary.find_literal(1.into());
        let two = binary.find_literal(2.into());
        let three = binary.find_literal(3.into());

        /*
          0: 100 000               PUSH_LITERAL E_INVARG
          2: 016                 * MAKE_SINGLETON_LIST
          3: 112 002 021           PUSH_LABEL 21
          6: 100 001               PUSH_LITERAL E_PROPNF
          8: 016                 * MAKE_SINGLETON_LIST
          9: 112 002 028           PUSH_LABEL 28
         12: 112 008 002         * TRY_EXCEPT 2
         15: 124                   NUM 1
         16: 052                 * PUT a
         17: 111                   POP
         18: 112 004 033           END_EXCEPT 33
         21: 052                 * PUT a
         22: 111                   POP
         23: 125                   NUM 2
         24: 052                 * PUT a
         25: 111                   POP
         26: 107 033               JUMP 33
         28: 053                 * PUT b
         29: 111                   POP
         30: 126                   NUM 3
         31: 052                 * PUT a
         32: 111                   POP

        */
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(e_invarg),
                MakeSingletonList,
                PushLabel(0),
                Imm(e_propnf),
                MakeSingletonList,
                PushLabel(1),
                TryExcept(2),
                Imm(one),
                Put(a),
                Pop,
                EndExcept(2),
                Put(a),
                Pop,
                Imm(two),
                Put(a),
                Pop,
                Jump { label: 3 },
                Put(b),
                Pop,
                Imm(three),
                Put(a),
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_catch_expr() {
        let program = "x = `x + 1 ! e_propnf, E_PERM => 17';";
        let binary = compile(program, HashMap::new()).unwrap();
        /**
          0: 100 000               PUSH_LITERAL E_PROPNF
         2: 016                 * MAKE_SINGLETON_LIST
         3: 100 001               PUSH_LITERAL E_PERM
         5: 102                   LIST_ADD_TAIL
         6: 112 002 017           PUSH_LABEL 17
         9: 112 007             * CATCH
        11: 085                   PUSH x
        12: 124                   NUM 1
        13: 021                 * ADD
        14: 112 003 019           END_CATCH 19
        17: 111                   POP
        18: 140                   NUM 17
        19: 052                 * PUT x
        20: 111                   POP

         */
        let x = binary.find_var("x");
        let e_propnf = binary.find_literal(E_PROPNF.into());
        let e_perm = binary.find_literal(E_PERM.into());
        let one = binary.find_literal(1.into());
        let svntn = binary.find_literal(17.into());

        assert_eq!(
            binary.main_vector,
            vec![
                Imm(e_propnf),
                MakeSingletonList,
                Imm(e_perm),
                ListAddTail,
                PushLabel(0),
                Catch,
                Push(x),
                Imm(one),
                Add,
                EndCatch(1),
                Pop,
                Imm(svntn),
                Put(x),
                Pop,
                Done
            ]
        )
    }
}
