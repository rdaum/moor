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
            Stmt::Break { exit: None } => {
                let l = self.loops.last().expect("No loop to break/continue from");
                self.emit(Op::Exit {
                    stack: l.bottom_stack,
                    label: l.bottom_label,
                })
            }
            Stmt::Break { exit: Some(l) } => {
                let l = self.find_loop(l).expect("invalid loop for break/continue");
                self.emit(Op::ExitId(l.bottom_label));
            }
            Stmt::Continue { exit: None } => {
                let l = self.loops.last().expect("No loop to break/continue from");
                self.emit(Op::Exit {
                    stack: l.top_stack,
                    label: l.top_label,
                })
            }
            Stmt::Continue { exit: Some(l) } => {
                let l = self.find_loop(l).expect("invalid loop for break/continue");
                self.emit(Op::ExitId(l.top_label));
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
