/// Takes the AST and turns it into a list of opcodes.
use std::collections::HashMap;

use anyhow::anyhow;
use itertools::Itertools;
use thiserror::Error;
use tracing::error;

use moor_value::var::{v_int, Var};

use crate::compiler::ast::{
    Arg, BinaryOp, CatchCodes, Expr, ScatterItem, ScatterKind, Stmt, UnaryOp,
};
use crate::compiler::builtins::make_builtin_labels;
use crate::compiler::labels::{JumpLabel, Label, Name, Names, Offset};
use crate::compiler::parse::parse_program;
use crate::vm::opcode::Op::Jump;
use crate::vm::opcode::{Op, Program, ScatterLabel};

#[derive(Error, Debug)]
pub enum CompileError {
    #[error("Unknown built-in function: {0}")]
    UnknownBuiltinFunction(String),
    #[error("Could not find loop with id: {0}")]
    UnknownLoopLabel(String),
}

pub struct Loop {
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
    pub(crate) literals: Vec<Var>,
    pub(crate) loops: Vec<Loop>,
    pub(crate) saved_stack: Option<Offset>,
    pub(crate) cur_stack: usize,
    pub(crate) max_stack: usize,
    pub(crate) builtins: HashMap<String, Name>,
    pub(crate) fork_vectors: Vec<Vec<Op>>,
}

impl CodegenState {
    pub fn new(var_names: Names, builtins: HashMap<String, Name>) -> Self {
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
    fn make_label(&mut self, name: Option<Name>) -> Label {
        let id = Label(self.jumps.len() as u32);
        let position = (self.ops.len()).into();
        self.jumps.push(JumpLabel { id, name, position });
        id
    }

    fn find_label(&self, name: &Name) -> Option<&JumpLabel> {
        self.jumps.iter().find(|j| {
            if let Some(label) = &j.name {
                label.eq(name)
            } else {
                false
            }
        })
    }

    // Adjust the position of a jump label to the current position.
    fn commit_label(&mut self, id: Label) {
        let position = self.ops.len();
        let jump = &mut self
            .jumps
            .get_mut(id.0 as usize)
            .expect("Invalid jump fixup");
        let npos = position;
        jump.position = npos.into();
    }

    fn add_literal(&mut self, v: &Var) -> Label {
        let lv_pos = self.literals.iter().position(|lv| lv.eq(v));
        let pos = match lv_pos {
            None => {
                let idx = self.literals.len();
                self.literals.push(v.clone());
                idx
            }
            Some(idx) => idx,
        };
        Label(pos as u32)
    }

    fn emit(&mut self, op: Op) {
        self.ops.push(op);
    }

    fn find_loop(&self, loop_label: &Name) -> Result<&Loop, anyhow::Error> {
        match self.find_label(loop_label) {
            None => {
                let loop_name = self.var_names.names[loop_label.0 as usize].clone();
                Err(anyhow!(CompileError::UnknownLoopLabel(loop_name)))
            }
            Some(label) => {
                let l = self.loops.iter().find(|l| l.top_label == label.id);
                let Some(l) = l else {
                      return Err(anyhow!(CompileError::UnknownLoopLabel(loop_label.0.to_string())));
                    };
                Ok(l)
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
        Offset(fv)
    }

    fn generate_assign(&mut self, left: &Expr, right: &Expr) -> Result<(), anyhow::Error> {
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
                    self.emit(Op::Put(*name));
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
        scatter: &Vec<ScatterItem>,
        right: &Expr,
    ) -> Result<(), anyhow::Error> {
        self.generate_expr(right)?;
        let nargs = scatter.len();
        let nreq = scatter
            .iter()
            .filter(|s| s.kind == ScatterKind::Required)
            .count();
        let nrest = match scatter
            .iter()
            .positions(|s| s.kind == ScatterKind::Rest)
            .last()
        {
            None => nargs + 1,
            Some(rest) => rest + 1,
        };
        let labels: Vec<(&ScatterItem, ScatterLabel)> = scatter
            .iter()
            .map(|s| {
                let kind_label = match s.kind {
                    ScatterKind::Required => ScatterLabel::Required(s.id),
                    ScatterKind::Optional if s.expr.is_some() => {
                        ScatterLabel::Optional(s.id, Some(self.make_label(None)))
                    }
                    ScatterKind::Optional => ScatterLabel::Optional(s.id, None),
                    ScatterKind::Rest => ScatterLabel::Rest(s.id),
                };
                (s, kind_label)
            })
            .collect();
        let done = self.make_label(None);
        self.emit(Op::Scatter {
            nargs,
            nreq,
            rest: nrest,
            labels: labels.iter().map(|(_, l)| l.clone()).collect(),
            done,
        });
        for (s, label) in labels {
            if let ScatterLabel::Optional(_, Some(label)) = label {
                if s.expr.is_none() {
                    continue;
                }
                self.commit_label(label);
                self.generate_expr(s.expr.as_ref().unwrap())?;
                self.emit(Op::Put(s.id));
                self.emit(Op::Pop);
                self.pop_stack(1);
            }
        }
        self.commit_label(done);
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
                    self.emit(Op::Push(*id));
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

    fn generate_codes(&mut self, codes: &CatchCodes) -> Result<(), anyhow::Error> {
        match codes {
            CatchCodes::Codes(codes) => {
                self.generate_arg_list(codes)?;
            }
            CatchCodes::Any => {
                self.emit(Op::Val(v_int(0)));
                self.push_stack(1);
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
                self.emit(Op::Push(*ident));
                self.push_stack(1);
            }
            Expr::And(left, right) => {
                self.generate_expr(left.as_ref())?;
                let end_label = self.make_label(None);
                self.emit(Op::And(end_label));
                self.pop_stack(1);
                self.generate_expr(right.as_ref())?;
                self.commit_label(end_label);
            }
            Expr::Or(left, right) => {
                self.generate_expr(left.as_ref())?;
                let end_label = self.make_label(None);
                self.emit(Op::Or(end_label));
                self.pop_stack(1);
                self.generate_expr(right.as_ref())?;
                self.commit_label(end_label);
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
                let Some(builtin) = self.builtins.get(function) else {
                    error!("Unknown builtin function: {}({:?}", function, args);
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
                let else_label = self.make_label(None);
                self.emit(Op::IfQues(else_label));
                self.pop_stack(1);
                self.generate_expr(consequence.as_ref())?;
                let end_label = self.make_label(None);
                self.emit(Op::Jump { label: end_label });
                self.pop_stack(1);
                self.commit_label(else_label);
                self.generate_expr(alternative.as_ref())?;
                self.commit_label(end_label);
            }
            Expr::Catch {
                codes,
                except,
                trye,
            } => {
                self.generate_codes(codes)?;
                let handler_label = self.make_label(None);
                self.emit(Op::PushLabel(handler_label));
                self.emit(Op::Catch(handler_label));
                self.generate_expr(trye.as_ref())?;
                let end_label = self.make_label(None);
                self.emit(Op::EndCatch(end_label));
                self.pop_stack(1)   /* codes, catch */;
                self.commit_label(handler_label);

                /* After this label, we still have a value on the stack, but now,
                 * instead of it being the value of the main expression, we have
                 * the exception pushed before entering the handler.
                 */
                if let Some(except) = except {
                    self.emit(Op::Pop);
                    self.pop_stack(1);
                    self.generate_expr(except.as_ref())?;
                }
                self.commit_label(end_label);
            }
            Expr::List(l) => {
                self.generate_arg_list(l)?;
            }
            Expr::Scatter(scatter, right) => self.generate_scatter_assign(scatter, right)?,
            Expr::Assign { left, right } => self.generate_assign(left, right)?,
        }

        Ok(())
    }

    pub fn generate_stmt(&mut self, stmt: &Stmt) -> Result<(), anyhow::Error> {
        match stmt {
            Stmt::Cond { arms, otherwise } => {
                let end_label = self.make_label(None);
                let mut is_else = false;
                for arm in arms {
                    self.generate_expr(&arm.condition)?;
                    let otherwise_label = self.make_label(None);
                    self.emit(if !is_else {
                        Op::If(otherwise_label)
                    } else {
                        Op::Eif(otherwise_label)
                    });
                    is_else = true;
                    self.pop_stack(1);
                    for stmt in &arm.statements {
                        self.generate_stmt(stmt)?;
                    }
                    self.emit(Op::Jump { label: end_label });

                    // This is where we jump to if the condition is false; either the end of the
                    // if statement, or the start of the next ('else or elseif') arm.
                    self.commit_label(otherwise_label);
                }
                if !otherwise.is_empty() {
                    for stmt in otherwise {
                        self.generate_stmt(stmt)?;
                    }
                }
                self.commit_label(end_label);
            }
            Stmt::ForList { id, expr, body } => {
                self.generate_expr(expr)?;

                // Note that MOO is 1-indexed, so this is counter value is 1 in LambdaMOO;
                // we use 0 here to make it easier to implement the ForList instruction.
                self.emit(Op::Val(v_int(0))); /* loop list index... */
                self.push_stack(1);
                let loop_top = self.make_label(Some(*id));
                self.commit_label(loop_top);
                let end_label = self.make_label(None);
                // TODO self.enter_loop/exit_loop needed?
                self.emit(Op::ForList { id: *id, end_label });
                self.loops.push(Loop {
                    top_label: loop_top,
                    top_stack: self.cur_stack.into(),
                    bottom_label: end_label,
                    bottom_stack: (self.cur_stack - 2).into(),
                });
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::Jump { label: loop_top });
                self.commit_label(end_label);
                self.pop_stack(2);
            }
            Stmt::ForRange { from, to, id, body } => {
                self.generate_expr(from)?;
                self.generate_expr(to)?;
                let loop_top = self.make_label(Some(*id));
                let end_label = self.make_label(None);
                self.emit(Op::ForRange { id: *id, end_label });
                self.loops.push(Loop {
                    top_label: loop_top,
                    top_stack: self.cur_stack.into(),
                    bottom_label: end_label,
                    bottom_stack: (self.cur_stack - 2).into(),
                });
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Jump { label: loop_top });
                self.commit_label(end_label);
                self.pop_stack(2);
            }
            Stmt::While {
                id,
                condition,
                body,
            } => {
                let loop_start_label = self.make_label(*id);
                let loop_end_label = self.make_label(None);
                self.generate_expr(condition)?;
                match id {
                    None => self.emit(Op::While(loop_end_label)),
                    Some(id) => self.emit(Op::WhileId {
                        id: *id,
                        end_label: loop_end_label,
                    }),
                }
                self.pop_stack(1);
                self.loops.push(Loop {
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
                self.commit_label(loop_end_label);
            }
            Stmt::Fork { id, body, time } => {
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
                    id: *id,
                    fv_offset: fv_id,
                });
                self.pop_stack(1);
            }
            Stmt::TryExcept { body, excepts } => {
                let mut labels = vec![];
                for ex in excepts {
                    self.generate_codes(&ex.codes)?;
                    let push_label = self.make_label(None);
                    self.emit(Op::PushLabel(push_label));
                    labels.push(push_label);
                }
                let num_excepts = excepts.len();
                self.emit(Op::TryExcept { num_excepts });
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                let end_label = self.make_label(None);
                self.emit(Op::EndExcept(end_label));
                self.pop_stack(num_excepts);
                for (i, ex) in excepts.iter().enumerate() {
                    self.commit_label(labels[i]);
                    self.push_stack(1);
                    if ex.id.is_some() {
                        self.emit(Op::Put(ex.id.unwrap()));
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
                self.commit_label(end_label);
            }
            Stmt::TryFinally { body, handler } => {
                let handler_label = self.make_label(None);
                self.emit(Op::TryFinally(handler_label));
                for stmt in body {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::EndFinally);
                self.commit_label(handler_label);
                self.push_stack(2); /* continuation value, reason */
                for stmt in handler {
                    self.generate_stmt(stmt)?;
                }
                self.emit(Op::Continue);
                self.pop_stack(2);
            }
            Stmt::Break { exit } | Stmt::Continue { exit } => {
                let l = match exit {
                    None => self.loops.last().expect("No loop to break/continue from"),
                    Some(ln) => self.find_loop(ln).expect("invalid loop for break/continue"),
                };
                if let Some(exit) = exit {
                    let jump_label = self
                        .find_label(exit)
                        .expect("invalid label for break/continue");
                    self.emit(Op::ExitId(jump_label.id));
                } else if let Stmt::Continue { .. } = stmt {
                    self.emit(Op::Exit {
                        stack: l.top_stack,
                        label: l.top_label,
                    })
                } else {
                    self.emit(Op::Exit {
                        stack: l.bottom_stack,
                        label: l.bottom_label,
                    })
                }
            }
            Stmt::Return { expr } => match expr {
                Some(expr) => {
                    self.generate_expr(expr)?;
                    self.emit(Op::Return);
                    self.pop_stack(1);
                }
                None => {
                    self.emit(Op::Return0);
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

pub fn compile(program: &str) -> Result<Program, anyhow::Error> {
    let compile_span = tracing::trace_span!("compile");
    let _compile_guard = compile_span.enter();

    let builtins = make_builtin_labels();
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

    let binary = Program {
        literals: cg_state.literals,
        jump_labels: cg_state.jumps,
        var_names: cg_state.var_names,
        main_vector: cg_state.ops,
        fork_vectors: cg_state.fork_vectors,
    };

    Ok(binary)
}

#[cfg(test)]
mod tests {
    use crate::compiler::builtins::BUILTIN_DESCRIPTORS;
    use moor_value::var::error::Error::{E_INVARG, E_PERM, E_PROPNF};
    use moor_value::var::objid::Objid;
    use moor_value::var::v_obj;

    use crate::vm::opcode::Op::*;
    use crate::vm::opcode::ScatterLabel;

    use super::*;

    #[test]
    fn test_simple_add_expr() {
        let program = "1 + 2;";
        let binary = compile(program).unwrap();
        let one = binary.find_literal(1.into());
        let two = binary.find_literal(2.into());
        assert_eq!(binary.main_vector, vec![Imm(one), Imm(two), Add, Pop, Done]);
    }

    #[test]
    fn test_var_assign_expr() {
        let program = "a = 1 + 2;";
        let binary = compile(program).unwrap();

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
        let binary = compile(program).unwrap();

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
        let binary = compile(program).unwrap();

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
                If(1.into()),
                Imm(five),
                Return,
                Jump { label: 0.into() },
                Imm(two),
                Imm(three),
                Eq,
                Eif(2.into()),
                Imm(three),
                Return,
                Jump { label: 0.into() },
                Imm(six),
                Return,
                Done
            ]
        );
    }

    #[test]
    fn test_while_stmt() {
        let program = "while (1) x = x + 1; endwhile";
        let binary = compile(program).unwrap();

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
                While(1.into()),
                Push(x),
                Imm(one),
                Add,
                Put(x),
                Pop,
                Jump { label: 0.into() },
                Done
            ]
        );
        assert_eq!(binary.jump_labels[0].position.0, 0);
        assert_eq!(binary.jump_labels[1].position.0, 8);
    }

    #[test]
    fn test_while_label_stmt() {
        let program = "while chuckles (1) x = x + 1; if (x > 5) break chuckles; endif endwhile";
        let binary = compile(program).unwrap();

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
                WhileId {
                    id: chuckles,
                    end_label: 1.into()
                },
                Push(x),
                Imm(one),
                Add,
                Put(x),
                Pop,
                Push(x),
                Imm(five),
                Gt,
                If(3.into()),
                ExitId(0.into()),
                Jump { label: 2.into() },
                Jump { label: 0.into() },
                Done,
            ]
        );
        assert_eq!(binary.jump_labels[0].position.0, 0);
        assert_eq!(binary.jump_labels[1].position.0, 14);
    }
    #[test]
    fn test_while_break_continue_stmt() {
        let program = "while (1) x = x + 1; if (x == 5) break; else continue; endif endwhile";
        let binary = compile(program).unwrap();

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
                While(1.into()),
                Push(x),
                Imm(one),
                Add,
                Put(x),
                Pop,
                Push(x),
                Imm(five),
                Eq,
                If(3.into()),
                Exit {
                    stack: 0.into(),
                    label: 1.into()
                },
                Jump { label: 2.into() },
                Exit {
                    stack: 0.into(),
                    label: 0.into()
                },
                Jump { label: 0.into() },
                Done
            ]
        );
        assert_eq!(binary.jump_labels[0].position.0, 0);
        assert_eq!(binary.jump_labels[1].position.0, 15);
    }
    #[test]
    fn test_for_in_list_stmt() {
        let program = "for x in ({1,2,3}) b = x + 5; endfor";
        let binary = compile(program).unwrap();

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
                Val(v_int(0)), // Differs from LambdaMOO, which uses 1-indexed lists internally, too.
                ForList {
                    id: x,
                    end_label: 1.into()
                },
                Push(x),
                Imm(five),
                Add,
                Put(b),
                Pop,
                Jump { label: 0.into() },
                Done
            ]
        );
        assert_eq!(binary.jump_labels[0].position.0, 7);
        assert_eq!(binary.jump_labels[1].position.0, 14);
    }

    #[test]
    fn test_for_range() {
        let program = "for n in [1..5] player:tell(a); endfor";
        let binary = compile(program).unwrap();

        let player = binary.find_var("player");
        let a = binary.find_var("a");
        let n = binary.find_var("n");
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
                ForRange {
                    id: n,
                    end_label: 1.into()
                },
                Push(player),
                Imm(tell),
                Push(a),
                MakeSingletonList,
                CallVerb,
                Pop,
                Jump { label: 0.into() },
                Done
            ]
        );
    }

    #[test]
    fn test_fork() {
        let program = "fork (5) player:tell(\"a\"); endfork";
        let binary = compile(program).unwrap();

        let player = binary.find_var("player");
        let a = binary.find_literal("a".into());
        let tell = binary.find_literal("tell".into());

        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0.into()),
                Fork {
                    fv_offset: 0.into(),
                    id: None
                },
                Done
            ]
        );
        assert_eq!(
            binary.fork_vectors[0],
            vec![
                Push(player), // player
                Imm(tell),    // tell
                Imm(a),       // 'a'
                MakeSingletonList,
                CallVerb,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_fork_id() {
        let program = "fork fid (5) player:tell(fid); endfork";
        let binary = compile(program).unwrap();

        let player = binary.find_var("player");
        let fid = binary.find_var("fid");
        let five = binary.find_literal(5.into());
        let tell = binary.find_literal("tell".into());

        assert_eq!(
            binary.main_vector,
            vec![
                Imm(five),
                Fork {
                    fv_offset: 0.into(),
                    id: Some(fid)
                },
                Done
            ]
        );
        assert_eq!(
            binary.fork_vectors[0],
            vec![
                Push(player), // player
                Imm(tell),    // tell
                Push(fid),    // fid
                MakeSingletonList,
                CallVerb,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_and_or() {
        let program = "a = (1 && 2 || 3);";
        let binary = compile(program).unwrap();

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
            vec![
                Imm(one),
                And(0.into()),
                Imm(two),
                Or(1.into()),
                Imm(three),
                Put(a),
                Pop,
                Done
            ]
        );
        assert_eq!(binary.jump_labels[0].position.0, 3);
        assert_eq!(binary.jump_labels[1].position.0, 5);
    }

    #[test]
    fn test_unknown_builtin_call() {
        let program = "bad_builtin(1, 2, 3);";
        let parse = compile(program);
        assert!(parse.is_err());
        match parse.err().unwrap().downcast_ref::<CompileError>() {
            Some(CompileError::UnknownBuiltinFunction(name)) => {
                assert_eq!(name, "bad_builtin");
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
        let binary = compile(program).unwrap();

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
                FuncCall { id: Name(0) },
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_cond_expr() {
        let program = "a = (1 == 2 ? 3 | 4);";
        let binary = compile(program).unwrap();

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
                IfQues(0.into()),
                Imm(three),
                Jump { label: 1.into() },
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
        let binary = compile(program).unwrap();

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
    fn test_string_get() {
        let program = "return \"test\"[1];";
        let binary = compile(program).unwrap();
        assert_eq!(
            binary.main_vector,
            vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done]
        );
    }

    #[test]
    fn test_string_get_range() {
        let program = "return \"test\"[1..2];";
        let binary = compile(program).unwrap();
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0.into()),
                Imm(1.into()),
                Imm(2.into()),
                RangeRef,
                Return,
                Done
            ]
        );
    }

    #[test]
    fn test_index_set() {
        let program = "a[2] = \"3\";";
        let binary = compile(program).unwrap();
        let a = binary.find_var("a");

        assert_eq!(
            binary.main_vector,
            vec![
                Push(a),
                Imm(0.into()),
                Imm(1.into()),
                PutTemp,
                IndexSet,
                Put(a),
                Pop,
                PushTemp,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_range_set() {
        let program = "a[2..4] = \"345\";";
        let binary = compile(program).unwrap();
        let a = binary.find_var("a");

        assert_eq!(
            binary.main_vector,
            vec![
                Push(a),
                Imm(0.into()),
                Imm(1.into()),
                Imm(2.into()),
                PutTemp,
                RangeSet,
                Put(a),
                Pop,
                PushTemp,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_list_get() {
        let program = "return {1,2,3}[1];";
        let binary = compile(program).unwrap();
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0.into()),
                MakeSingletonList,
                Imm(1.into()),
                ListAddTail,
                Imm(2.into()),
                ListAddTail,
                Imm(0.into()),
                Ref,
                Return,
                Done
            ]
        );
    }

    #[test]
    fn test_list_get_range() {
        let program = "return {1,2,3}[1..2];";
        let binary = compile(program).unwrap();
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0.into()),
                MakeSingletonList,
                Imm(1.into()),
                ListAddTail,
                Imm(2.into()),
                ListAddTail,
                Imm(0.into()),
                Imm(1.into()),
                RangeRef,
                Return,
                Done
            ]
        );
    }

    #[test]
    fn test_range_length() {
        let program = "a = {1, 2, 3}; b = a[2..$];";
        let binary = compile(program).unwrap();

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
                Length(0.into()),
                RangeRef,
                Put(b),
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_list_splice() {
        let program = "return {@args[1..2]};";
        let binary = compile(program).unwrap();

        /*
          0: 076                   PUSH args
          1: 124                   NUM 1
          2: 125                   NUM 2
          3: 015                 * RANGE
          4: 017                 * CHECK_LIST_FOR_SPLICE
          5: 108                   RETURN
         12: 110                   DONE
        */
        let args = binary.find_var("args");
        assert_eq!(
            binary.main_vector,
            vec![
                Push(args),
                Imm(0.into()),
                Imm(1.into()),
                RangeRef,
                CheckListForSplice,
                Return,
                Done
            ]
        );
    }

    #[test]
    fn test_try_finally() {
        let program = "try a=1; finally a=2; endtry";
        let binary = compile(program).unwrap();

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
                TryFinally(0.into()),
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
        let binary = compile(program).unwrap();

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
                PushLabel(0.into()),
                Imm(e_propnf),
                MakeSingletonList,
                PushLabel(1.into()),
                TryExcept { num_excepts: 2 },
                Imm(one),
                Put(a),
                Pop,
                EndExcept(2.into()),
                Put(a),
                Pop,
                Imm(two),
                Put(a),
                Pop,
                Jump { label: 2.into() },
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
        let binary = compile(program).unwrap();
        /*
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
                PushLabel(0.into()),
                Catch(0.into()),
                Push(x),
                Imm(one),
                Add,
                EndCatch(1.into()),
                Pop,
                Imm(svntn),
                Put(x),
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_catch_any_expr() {
        let program = "return `raise(E_INVARG) ! ANY';";
        let binary = compile(program).unwrap();

        /*
          0: 123                   NUM 0
          1: 112 002 014           PUSH_LABEL 14
          4: 112 007             * CATCH
          6: 100 000               PUSH_LITERAL E_INVARG
          8: 016                 * MAKE_SINGLETON_LIST
          9: 012 004             * CALL_FUNC raise
         11: 112 003 016           END_CATCH 16
         14: 124                   NUM 1
         15: 014                 * INDEX
         16: 108                   RETURN
        */
        let raise_num = BUILTIN_DESCRIPTORS
            .iter()
            .position(|b| b.name == "raise")
            .unwrap();
        let e_invarg = binary.find_literal(E_INVARG.into());
        assert_eq!(
            binary.main_vector,
            vec![
                Val(0.into()),
                PushLabel(0.into()),
                Catch(0.into()),
                Imm(e_invarg),
                MakeSingletonList,
                FuncCall {
                    id: Name(raise_num as u32)
                },
                EndCatch(1.into()),
                Return,
                Done
            ]
        )
    }

    #[test]
    fn test_sysobjref() {
        let program = "$string_utils:from_list(test_string);";
        let binary = compile(program).unwrap();

        let string_utils = binary.find_literal("string_utils".into());
        let from_list = binary.find_literal("from_list".into());
        let test_string = binary.find_var("test_string");
        let sysobj = binary.find_literal(Objid(0).into());
        /*
         0: 100 000               PUSH_LITERAL #0
         2: 100 001               PUSH_LITERAL "string_utils"
         4: 009                 * GET_PROP
         5: 100 002               PUSH_LITERAL "from_list"
         7: 085                   PUSH test_string
         8: 016                 * MAKE_SINGLETON_LIST
         9: 010                 * CALL_VERB
        */
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(sysobj),
                Imm(string_utils),
                GetProp,
                Imm(from_list),
                Push(test_string),
                MakeSingletonList,
                CallVerb,
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_basic_scatter_assign() {
        let program = "{a, b, c} = args;";
        let binary = compile(program).unwrap();
        let (a, b, c) = (
            binary.find_var("a"),
            binary.find_var("b"),
            binary.find_var("c"),
        );
        /*
         0: 076                   PUSH args
         1: 112 013 001 001 002
            018 000 009         * SCATTER 3/3/4: args/0 9
         9: 111                   POP
        */

        assert_eq!(
            binary.main_vector,
            vec![
                Push(binary.find_var("args")),
                Scatter {
                    nargs: 3,
                    nreq: 3,
                    rest: 4,
                    labels: vec![
                        ScatterLabel::Required(a),
                        ScatterLabel::Required(b),
                        ScatterLabel::Required(c),
                    ],
                    done: 0.into(),
                },
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_more_scatter_assign() {
        let program = "{first, second, ?third = 0} = args;";
        let binary = compile(program).unwrap();
        let (first, second, third) = (
            binary.find_var("first"),
            binary.find_var("second"),
            binary.find_var("third"),
        );
        /*
          0: 076                   PUSH args
          1: 112 013 003 002 004
             018 000 019 000 020
             013 016             * SCATTER 3/2/4: first/0 second/0 third/13 16
         13: 123                   NUM 0
         14: 054                 * PUT third
         15: 111                   POP
        */
        assert_eq!(
            binary.main_vector,
            vec![
                Push(binary.find_var("args")),
                Scatter {
                    nargs: 3,
                    nreq: 2,
                    rest: 4,
                    labels: vec![
                        ScatterLabel::Required(first),
                        ScatterLabel::Required(second),
                        ScatterLabel::Optional(third, Some(0.into())),
                    ],
                    done: 1.into(),
                },
                Imm(binary.find_literal(0.into())),
                Put(binary.find_var("third")),
                Pop,
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_some_more_scatter_assign() {
        let program = "{a, b, ?c = 8, @d} = args;";
        let binary = compile(program).unwrap();
        /*
         0: 076                   PUSH args
         1: 112 013 004 002 004
            018 000 019 000 020
            015 021 000 018     * SCATTER 4/2/4: a/0 b/0 c/15 d/0 18
        15: 131                   NUM 8
        16: 054                 * PUT c
        17: 111                   POP
        18: 111                   POP

                */
        let (a, b, c, d) = (
            binary.find_var("a"),
            binary.find_var("b"),
            binary.find_var("c"),
            binary.find_var("d"),
        );
        assert_eq!(
            binary.main_vector,
            vec![
                Push(binary.find_var("args")),
                Scatter {
                    nargs: 4,
                    nreq: 2,
                    rest: 4,
                    labels: vec![
                        ScatterLabel::Required(a),
                        ScatterLabel::Required(b),
                        ScatterLabel::Optional(c, Some(0.into())),
                        ScatterLabel::Rest(d),
                    ],
                    done: 1.into(),
                },
                Imm(binary.find_literal(8.into())),
                Put(binary.find_var("c")),
                Pop,
                Pop,
                Done
            ]
        );
    }

    #[test]
    fn test_even_more_scatter_assign() {
        let program = "{a, ?b, ?c = 8, @d, ?e = 9, f} = args;";
        let binary = compile(program).unwrap();
        let (a, b, c, d, e, f) = (
            binary.find_var("a"),
            binary.find_var("b"),
            binary.find_var("c"),
            binary.find_var("d"),
            binary.find_var("e"),
            binary.find_var("f"),
        );
        /*
          0: 076                   PUSH args
          1: 112 013 006 002 004
             018 000 019 001 020
             019 021 000 022 022
             023 000 025         * SCATTER 6/2/4: a/0 b/1 c/19 d/0 e/22 f/0 25
         19: 131                   NUM 8
         20: 054                 * PUT c
         21: 111                   POP
         22: 132                   NUM 9
         23: 056                 * PUT e
         24: 111                   POP
         25: 111                   POP
        */
        assert_eq!(
            binary.main_vector,
            vec![
                Push(binary.find_var("args")),
                Scatter {
                    nargs: 6,
                    nreq: 2,
                    rest: 4,
                    labels: vec![
                        ScatterLabel::Required(a),
                        ScatterLabel::Optional(b, None),
                        ScatterLabel::Optional(c, Some(0.into())),
                        ScatterLabel::Rest(d),
                        ScatterLabel::Optional(e, Some(1.into())),
                        ScatterLabel::Required(f),
                    ],
                    done: 2.into(),
                },
                Imm(binary.find_literal(8.into())),
                Put(binary.find_var("c")),
                Pop,
                Imm(binary.find_literal(9.into())),
                Put(binary.find_var("e")),
                Pop,
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_scatter_precedence() {
        let program = "{a,b,?c, @d} = {{1,2,player:kill(b)}}[1]; return {a,b,c};";
        let binary = compile(program).unwrap();
        let (a, b, c, d) = (
            binary.find_var("a"),
            binary.find_var("b"),
            binary.find_var("c"),
            binary.find_var("d"),
        );
        /*
         0: 124                   NUM 1
         1: 016                 * MAKE_SINGLETON_LIST
         2: 125                   NUM 2
         3: 102                   LIST_ADD_TAIL
         4: 072                   PUSH player
         5: 100 000               PUSH_LITERAL "kill"
         7: 086                   PUSH b
         8: 016                 * MAKE_SINGLETON_LIST
         9: 010                 * CALL_VERB
        10: 102                   LIST_ADD_TAIL
        11: 016                 * MAKE_SINGLETON_LIST
        12: 124                   NUM 1
        13: 014                 * INDEX
        14: 112 013 004 002 004
            018 000 019 000 020
            001 021 000 028     * SCATTER 4/2/4: a/0 b/0 c/1 d/0 28
        28: 111                   POP
        29: 085                   PUSH a
        30: 016                 * MAKE_SINGLETON_LIST
        31: 086                   PUSH b
        32: 102                   LIST_ADD_TAIL
        33: 087                   PUSH c
        34: 102                   LIST_ADD_TAIL
        35: 108                   RETURN
               */

        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0.into()),
                MakeSingletonList,
                Imm(1.into()),
                ListAddTail,
                Push(binary.find_var("player")),
                Imm(binary.find_literal("kill".into())),
                Push(b),
                MakeSingletonList,
                CallVerb,
                ListAddTail,
                MakeSingletonList,
                Imm(0.into()),
                Ref,
                Scatter {
                    nargs: 4,
                    nreq: 2,
                    rest: 4,
                    labels: vec![
                        ScatterLabel::Required(a),
                        ScatterLabel::Required(b),
                        ScatterLabel::Optional(c, None),
                        ScatterLabel::Rest(d),
                    ],
                    done: 0.into(),
                },
                Pop,
                Push(a),
                MakeSingletonList,
                Push(b),
                ListAddTail,
                Push(c),
                ListAddTail,
                Return,
                Done
            ]
        )
    }

    #[test]
    fn test_indexed_assignment() {
        let program = r#"this.stack[5] = 5;"#;
        let binary = compile(program).unwrap();

        /*
                  0: 073                   PUSH this
                  1: 100 000               PUSH_LITERAL "stack"
                  3: 008                 * PUSH_GET_PROP
                  4: 128                   NUM 5
                  5: 128                   NUM 5
                  6: 105                   PUT_TEMP
                  7: 007                 * INDEXSET
                  8: 011                 * PUT_PROP
                  9: 111                   POP
                 10: 106                   PUSH_TEMP
        */
        assert_eq!(
            binary.main_vector,
            vec![
                Push(binary.find_var("this")),
                Imm(binary.find_literal("stack".into())),
                PushGetProp,
                Imm(binary.find_literal(5.into())),
                Imm(binary.find_literal(5.into())),
                PutTemp,
                IndexSet,
                PutProp,
                Pop,
                PushTemp,
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_assignment_from_range() {
        let program = r#"x = 1; y = {1,2,3}; x = x + y[2];"#;
        let binary = compile(program).unwrap();

        let x = binary.find_var("x");
        let y = binary.find_var("y");

        /*
         0: 124                   NUM 1
         1: 052                 * PUT x
         2: 111                   POP
         3: 124                   NUM 1
         4: 016                 * MAKE_SINGLETON_LIST
         5: 125                   NUM 2
         6: 102                   LIST_ADD_TAIL
         7: 126                   NUM 3
         8: 102                   LIST_ADD_TAIL
         9: 053                 * PUT y
        10: 111                   POP
        11: 085                   PUSH x
        12: 086                   PUSH y
        13: 125                   NUM 2
        14: 014                 * INDEX
        15: 021                 * ADD
        16: 052                 * PUT x
        17: 111                   POP
        30: 110                   DONE
        */

        assert_eq!(
            binary.main_vector,
            vec![
                Imm(0.into()),
                Put(x),
                Pop,
                Imm(0.into()),
                MakeSingletonList,
                Imm(1.into()),
                ListAddTail,
                Imm(2.into()),
                ListAddTail,
                Put(y),
                Pop,
                Push(x),
                Push(y),
                Imm(1.into()),
                Ref,
                Add,
                Put(x),
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_get_property() {
        let program = r#"return this.stack;"#;
        let binary = compile(program).unwrap();

        assert_eq!(
            binary.main_vector,
            vec![
                Push(binary.find_var("this")),
                Imm(binary.find_literal("stack".into())),
                GetProp,
                Return,
                Done
            ]
        )
    }

    #[test]
    fn test_call_verb() {
        let program = r#"#0:test_verb();"#;
        let binary = compile(program).unwrap();
        assert_eq!(
            binary.main_vector,
            vec![
                Imm(binary.find_literal(v_obj(0))),
                Imm(binary.find_literal("test_verb".into())),
                MkEmptyList,
                CallVerb,
                Pop,
                Done
            ]
        )
    }

    #[test]
    fn test_0_arg_return() {
        let program = r#"return;"#;
        let binary = compile(program).unwrap();
        assert_eq!(binary.main_vector, vec![Return0, Done])
    }

    #[test]
    fn test_pass() {
        let program = r#"
            result = pass(@args);
            result = pass();
            result = pass(1,2,3,4);
            pass = blop;
            return pass;
        "#;
        let binary = compile(program).unwrap();
        let result = binary.find_var("result");
        let pass = binary.find_var("pass");
        let args = binary.find_var("args");
        let blop = binary.find_var("blop");
        assert_eq!(
            binary.main_vector,
            vec![
                Push(args),
                CheckListForSplice,
                Pass,
                Put(result),
                Pop,
                MkEmptyList,
                Pass,
                Put(result),
                Pop,
                Imm(0.into()),
                MakeSingletonList,
                Imm(1.into()),
                ListAddTail,
                Imm(2.into()),
                ListAddTail,
                Imm(3.into()),
                ListAddTail,
                Pass,
                Put(result),
                Pop,
                Push(blop),
                Put(pass),
                Pop,
                Push(pass),
                Return,
                Done
            ]
        )
    }

    #[test]
    fn test_regression_length_expr_inside_try_except() {
        let program = compile(
            r#"
        try
          return "hello world"[2..$];
        except (E_RANGE)
        endtry
        "#,
        )
        .unwrap();

        /*
  0: 100 000               PUSH_LITERAL E_RANGE
  2: 016                 * MAKE_SINGLETON_LIST
  3: 112 002 020           PUSH_LABEL 20
  6: 112 008 001         * TRY_EXCEPT 1
  9: 100 001               PUSH_LITERAL "hello world"
 11: 125                   NUM 2
 12: 112 001 003           LENGTH 3
 15: 015                 * RANGE
 16: 108                   RETURN
 17: 112 004 021           END_EXCEPT 21
 20: 111                   POP
 33: 110                   DONE
         */
        assert_eq!(
            program.main_vector,
            vec![
                Imm(0.into()),
                MakeSingletonList,
                PushLabel(Label(0)),
                TryExcept {
                  num_excepts: 1,
                },
                Imm(1.into()),
                Imm(2.into()),
                // Our offset is different because we don't count PushLabel in the stack.
                Op::Length(Offset(1)),
                RangeRef,
                Return,
                EndExcept(Label(1)),
                Pop,
                Done
            ]);
    }
}
