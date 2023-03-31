use crate::db::state::{StateError, WorldState};
use enumset::EnumSet;

use crate::model::objects::ObjFlag;
use crate::model::var::Error::{
    E_ARGS, E_INVARG, E_INVIND, E_PERM, E_PROPNF, E_RANGE, E_TYPE, E_VARNF, E_VERBNF,
};
use crate::model::var::{Error, ErrorPack, Objid, Var};
use crate::vm::activation::Activation;
use crate::vm::opcode::{Op, ScatterLabel};

#[derive(Clone, Eq, PartialEq)]
pub enum FinallyReason {
    Fallthrough,
    Raise {
        code: Error,
        msg: String,
        value: Var,
        stack: Vec<Var>,
    },
    Uncaught {
        code: Error,
        msg: String,
        value: Var,
        stack: Vec<Var>,
        backtrace: Vec<Var>,
    },
    Return(Var),
    Abort,
    Exit {
        stack: usize,
        label: usize,
    },
}
const FINALLY_REASON_RAISE: usize = 0x00;
const FINALLY_REASON_UNCAUGHT: usize = 0x01;
const FINALLY_REASON_RETURN: usize = 0x02;
const FINALLY_REASON_ABORT: usize = 0x03;
const FINALLY_REASON_EXIT: usize = 0x04;
const FINALLY_REASON_FALLTHROUGH: usize = 0x05;

impl FinallyReason {
    pub fn code(&self) -> usize {
        match *self {
            FinallyReason::Fallthrough => FINALLY_REASON_RAISE,
            FinallyReason::Raise { .. } => FINALLY_REASON_RAISE,
            FinallyReason::Uncaught { .. } => FINALLY_REASON_UNCAUGHT,
            FinallyReason::Return(_) => FINALLY_REASON_RETURN,
            FinallyReason::Abort => FINALLY_REASON_ABORT,
            FinallyReason::Exit { .. } => FINALLY_REASON_EXIT,
        }
    }
    pub fn from_code(code: usize) -> FinallyReason {
        match code {
            FINALLY_REASON_RAISE => FinallyReason::Fallthrough,
            FINALLY_REASON_UNCAUGHT => FinallyReason::Fallthrough,
            FINALLY_REASON_RETURN => FinallyReason::Fallthrough,
            FINALLY_REASON_ABORT => FinallyReason::Fallthrough,
            FINALLY_REASON_EXIT => FinallyReason::Fallthrough,
            FINALLY_REASON_FALLTHROUGH => FinallyReason::Fallthrough,
            _ => panic!("Invalid FinallyReason code"),
        }
    }
}

pub enum ExecutionOutcome {
    Done,    // Task ran successfully to completion
    Aborted, // Task aborted, either by kill_task() or by an uncaught error.
    Blocked, // Task called a blocking built-in function.
}

pub struct VM {
    // Activation stack.
    stack: Vec<Activation>,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum ExecutionResult {
    Complete(Var),
    More,
}

macro_rules! binary_bool_op {
    ( $self:ident, $op:tt ) => {
        let rhs = $self.pop();
        let lhs = $self.pop();
        let result = if lhs $op rhs { 1 } else { 0 };
        $self.push(&Var::Int(result))
    };
}

macro_rules! binary_var_op {
    ( $self:ident, $op:tt ) => {
        let rhs = $self.pop();
        let lhs = $self.pop();
        let result = lhs.$op(&rhs);
        match result {
            Ok(result) => $self.push(&result),
            Err(err_code) => return $self.push_error(err_code),
        }
    };
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    pub fn new() -> Self {
        Self { stack: vec![] }
    }

    fn find_handler_active(&mut self, raise_code: Error) -> Option<(usize, &Activation)> {
        // Scan activation frames and their stacks, looking for the first _Catch we can find.
        for a in self.stack.iter().rev() {
            let mut i = a.valstack.len();
            while i > 0 {
                if let Var::_Catch(cnt) = a.valstack[i - 1] {
                    // Found one, now scan forwards from 'cnt' backwards looking for either the first
                    // non-list value, or a list containing the error code.
                    // TODO check for 'cnt' being too large. not sure how to handle, tho
                    // TODO this actually i think is wrong, it needs to pull two values off the stack
                    for j in (i - cnt)..i {
                        if let Var::List(codes) = &a.valstack[j] {
                            if codes.contains(&Var::Err(raise_code)) {
                                return Some((i, a));
                            }
                        } else {
                            return Some((i, a));
                        }
                    }
                }
                i -= 1;
            }
        }
        None
    }

    fn make_stack_list(&self, frames: &[Activation], start_frame_num: usize) -> Vec<Var> {
        // TODO LambdaMOO had logic in here about 'root_vector' and 'line_numbers_too' that I haven't included yet.

        let mut stack_list = vec![];
        for (i, a) in frames.iter().rev().enumerate() {
            if i < start_frame_num {
                continue;
            }
            // Produce traceback line for each activation frame and append to stack_list
            // Should include line numbers (if possible), the name of the currently running verb,
            // its definer, its location, and the current player, and 'this'.
            let traceback_entry = vec![
                Var::Obj(a.this),
                Var::Str(a.verb.clone()),
                Var::Obj(a.definer),
                Var::Obj(a.verb_owner),
                Var::Obj(a.player),
                // TODO: find_line_number and add here.
            ];

            stack_list.push(Var::List(traceback_entry));
        }
        stack_list
    }

    fn error_backtrace_list(&self, raise_msg: &str) -> Vec<Var> {
        // Walk live activation frames and produce a written representation of a traceback for each
        // frame.
        let mut backtrace_list = vec![];
        for (i, a) in self.stack.iter().rev().enumerate() {
            let mut pieces = vec![];
            if i != 0 {
                pieces.push("... called from ".to_string());
            }
            pieces.push(format!("#{}:{}", a.definer.0, a.verb));
            if a.definer != a.this {
                pieces.push(format!(" (this == #{})", a.this.0));
            }
            // TODO line number
            if i == self.stack.len() - 1 {
                pieces.push(format!(": {}", raise_msg));
            }
            // TODO builtin-function name if a builtin

            let piece = pieces.join("");
            backtrace_list.push(Var::Str(piece))
        }
        backtrace_list.push(Var::Str(String::from("(End of traceback)")));
        backtrace_list
    }

    fn raise_error_pack(&mut self, p: ErrorPack) -> Result<ExecutionResult, anyhow::Error> {
        // Look for first active catch handler's activation frame and its (reverse) offset in the activation stack.
        let handler_activ = self.find_handler_active(p.code);

        let why = if let Some((handler_active_num, _)) = handler_activ {
            FinallyReason::Raise {
                code: p.code,
                msg: p.msg,
                value: p.value,
                stack: self.make_stack_list(&self.stack, handler_active_num),
            }
        } else {
            FinallyReason::Uncaught {
                code: p.code,
                msg: p.msg.clone(),
                value: p.value,
                stack: self.make_stack_list(&self.stack, 0),
                backtrace: self.error_backtrace_list(p.msg.as_str()),
            }
        };

        self.unwind_stack(why)
    }

    fn push_error(&mut self, code: Error) -> Result<ExecutionResult, anyhow::Error> {
        self.push(&Var::Err(code));
        self.raise_error_pack(code.make_error_pack())
    }

    fn raise_error(&mut self, code: Error) -> Result<ExecutionResult, anyhow::Error> {
        self.raise_error_pack(code.make_error_pack())
    }

    fn unwind_stack(&mut self, why: FinallyReason) -> Result<ExecutionResult, anyhow::Error> {
        // Walk activation stack from bottom to top, tossing frames as we go.
        while let Some(a) = self.stack.last_mut() {
            // Pop the value stack seeking finally/catch handler values.
            while let Some(v) = a.valstack.pop() {
                match v {
                    Var::_Finally(label) => {
                        /* FINALLY handler */
                        let why_num = why.code();
                        if why_num == FinallyReason::Abort.code() {
                            continue;
                        }
                        a.jump(label);
                        a.push(Var::Int(why_num as i64));
                        return Ok(ExecutionResult::More);
                    }
                    Var::_Catch(_label) => {
                        /* TRY-EXCEPT or `expr ! ...' handler */
                        let FinallyReason::Raise{code, value, ..} = &why else {
                            continue
                        };
                        // Jump further back the value stack looking for an list of errors + labels
                        // we will match on.
                        let mut found = false;
                        if a.valstack.len() >= 2 {
                            if let (Some(Var::_Label(pushed_label)), Some(Var::List(error_codes))) =
                                (a.valstack.pop(), a.valstack.pop())
                            {
                                if error_codes.contains(&Var::Err(*code)) {
                                    a.jump(pushed_label);
                                    found = true;
                                }
                            }
                        }
                        if found {
                            a.push(value.clone());
                            return Ok(ExecutionResult::More);
                        }
                    }
                    _ => continue,
                }
            }
            if let FinallyReason::Exit { label, .. } = why {
                a.jump(label);
                return Ok(ExecutionResult::More);
            }

            // If we're doing a return, and this is the last activation, we're done and just pass
            // the returned value up out of the interpreter loop.
            // Otherwise pop off this activation, and continue unwinding.
            if let FinallyReason::Return(value) = &why {
                if self.stack.len() == 1 {
                    return Ok(ExecutionResult::Complete(value.clone()));
                }
            }

            self.stack.pop().expect("Stack underflow");

            if self.stack.is_empty() {
                return Ok(ExecutionResult::Complete(Var::None));
            }
            // TODO builtin function unwinding stuff

            // If it was a return that brought us here, stick it onto the end of the next
            // activation's value stack.
            // (Unless we're the final activation, in which case that should have been handled
            // above)
            if let FinallyReason::Return(value) = why {
                self.push(&value);
                return Ok(ExecutionResult::More);
            }
        }

        // We realistically should not get here...
        panic!("Unwound stack to empty, but no exit condition was hit");
    }

    fn top_mut(&mut self) -> &mut Activation {
        self.stack.last_mut().expect("activation stack underflow")
    }

    fn top(&self) -> &Activation {
        self.stack.last().expect("activation stack underflow")
    }

    fn pop(&mut self) -> Var {
        self.top_mut().pop().expect("stack underflow")
    }

    fn push(&mut self, v: &Var) {
        self.top_mut().push(v.clone())
    }

    fn next_op(&mut self) -> Option<Op> {
        self.top_mut().next_op()
    }

    fn jump(&mut self, label: usize) {
        self.top_mut().jump(label)
    }

    fn get_env(&mut self, id: usize) -> Var {
        self.top().environment[id].clone()
    }

    fn set_env(&mut self, id: usize, v: &Var) {
        self.top_mut().environment[id] = v.clone();
    }

    fn peek(&self, amt: usize) -> Vec<Var> {
        self.top().peek(amt)
    }

    pub fn peek_at(&self, i: usize) -> Option<Var> {
        self.top().peek_at(i)
    }

    fn peek_top(&self) -> Var {
        self.top().peek_top().expect("stack underflow")
    }

    fn get_prop(
        &mut self,
        state: &dyn WorldState,
        player_flags: EnumSet<ObjFlag>,
        propname: Var,
        obj: Var,
    ) -> Result<ExecutionResult, anyhow::Error> {
        let Var::Str(propname) = propname else {
            return self.push_error(E_TYPE);
        };

        let Var::Obj(obj) = obj else {
            return self.push_error(E_INVIND);
        };

        let v = match state.retrieve_property(obj, propname.as_str(), player_flags) {
            Ok(v) => v,
            Err(e) => match e.downcast_ref::<StateError>() {
                Some(StateError::PropertyPermissionDenied(_, _)) => return self.push_error(E_PERM),
                Some(StateError::PropertyNotFound(_, _)) => return self.push_error(E_PROPNF),
                _ => {
                    panic!("Unexpected error in property retrieval: {:?}", e);
                }
            },
        };
        self.push(&v);
        Ok(ExecutionResult::More)
    }

    fn call_verb(
        &mut self,
        state: &mut impl WorldState,
        this: Objid,
        verb: String,
        args: Vec<Var>,
        do_pass: bool,
    ) -> Result<ExecutionResult, anyhow::Error> {
        let this = if do_pass {
            if !state.valid(self.top().definer)? {
                return self.push_error(E_INVIND);
            }
            state.parent_of(this)?
        } else {
            this
        };

        if !state.valid(this)? {
            return self.push_error(E_INVIND);
        }
        // find callable verb
        let Ok((binary, verbinfo)) = state.retrieve_verb(this, verb.as_str()) else {
            return self.push_error(E_VERBNF);
        };
        let top = self.top();
        let a = Activation::new_for_method(
            binary,
            top.definer,
            this,
            top.player,
            top.player_flags,
            verbinfo.attrs.owner.unwrap(),
            verbinfo.attrs.definer.unwrap(),
            verb,
            args,
        )?;

        self.stack.push(a);
        Ok(ExecutionResult::More)
    }

    pub fn do_method_verb(
        &mut self,
        state: &mut impl WorldState,
        obj: Objid,
        verb_name: &str,
        _do_pass: bool,
        this: Objid,
        player: Objid,
        player_flags: EnumSet<ObjFlag>,
        caller: Objid,
        args: Vec<Var>,
    ) -> Result<Var, anyhow::Error> {
        // TODO do_pass
        // TODO stack traces, error catch etc?

        let (binary, vi) = match state.retrieve_verb(obj, verb_name) {
            Ok(binary) => binary,
            Err(e) => {
                return match e.downcast_ref::<StateError>() {
                    Some(StateError::VerbNotFound(_, _)) => Ok(Var::Err(E_VERBNF)),
                    Some(StateError::VerbPermissionDenied(_, _)) => Ok(Var::Err(E_PERM)),
                    _ => Err(e),
                };
            }
        };

        let a = Activation::new_for_method(
            binary,
            caller,
            this,
            player,
            player_flags,
            vi.attrs.owner.unwrap(),
            vi.attrs.definer.unwrap(),
            String::from(verb_name),
            args,
        )?;

        self.stack.push(a);

        Ok(Var::Err(Error::E_NONE))
    }

    pub fn exec(&mut self, state: &mut impl WorldState) -> Result<ExecutionResult, anyhow::Error> {
        let op = self
            .next_op()
            .expect("Unexpected program termination; opcode stream should end with RETURN or DONE");
        match op {
            Op::If(label) | Op::Eif(label) | Op::IfQues(label) | Op::While(label) => {
                let cond = self.pop();
                if !cond.is_true() {
                    self.jump(label);
                }
            }
            Op::Jump { label } => {
                self.jump(label);
            }
            Op::WhileId { id, label } => {
                self.set_env(id, &self.peek_top());
                let cond = self.pop();
                if !cond.is_true() {
                    self.jump(label);
                }
            }
            Op::ForList { label, id } => {
                // Pop the count and list off the stack. We push back later when we re-enter.
                // TODO LambdaMOO had optimization here where it would only peek and update.
                // But I had some difficulty getting stack values right, so will do this simpler
                // for now and revisit later.
                let (count, list) = (&self.pop(), &self.pop());
                let Var::Int(count) = count else {
                    return self.raise_error(E_TYPE);

                    // LambdaMOO had a raise followed by jump. Not clear how that would work.
                    // Watch out for bugs here. Same below
                    // self.jump(label);
                };
                let count = *count as usize;
                let Var::List(l) = list else {
                    return self.raise_error(E_TYPE);
                    // self.jump(label);
                };

                // If we've exhausted the list, pop the count and list and jump out.
                if count >= l.len() {
                    self.jump(label);
                    return Ok(ExecutionResult::More);
                }

                // Track iteration count for range; set id to current list element for the count,
                // then increment the count, rewind the program counter to the top of the loop, and
                // continue.
                self.set_env(id, &l[count]);
                self.push(list);
                self.push(&Var::Int((count + 1) as i64));
            }
            Op::ForRange { label, id } => {
                // Pull the range ends off the stack.
                // TODO LambdaMOO had optimization here where it would only peek and update.
                // But I had some difficulty getting stack values right, so will do this simpler
                // for now and revisit later.
                let (to, from) = (&self.pop(), &self.pop());

                // TODO: LambdaMOO has special handling for MAXINT/MAXOBJ
                // Given we're 64-bit this is highly unlikely to ever be a concern for us, but
                // we also don't want to *crash* on obscene values, so impl that here.

                let next_val = match (to, from) {
                    (Var::Int(to_i), Var::Int(from_i)) => {
                        if from_i > to_i {
                            self.jump(label);
                            return Ok(ExecutionResult::More);
                        }
                        Var::Int(from_i + 1)
                    }
                    (Var::Obj(to_o), Var::Obj(from_o)) => {
                        if from_o.0 > to_o.0 {
                            self.jump(label);
                            return Ok(ExecutionResult::More);
                        }
                        Var::Obj(Objid(from_o.0 + 1))
                    }
                    (_, _) => {
                        return self.raise_error(E_TYPE);
                    }
                };

                self.set_env(id, from);
                self.push(&next_val);
                self.push(to);
            }
            Op::Pop => {
                self.pop();
            }
            Op::Val(val) => {
                self.push(&val);
            }
            Op::Imm(slot) => {
                // TODO Peek ahead to see if the next operation is 'pop' and if so, just throw away.
                // MOO uses this to optimize verbdoc/comments, etc.
                match self.top().lookahead() {
                    Some(Op::Pop) => {
                        // skip
                        self.top_mut().skip();
                        return Ok(ExecutionResult::More);
                    }
                    _ => {
                        let value = self.top().binary.literals[slot].clone();
                        self.push(&value);
                    }
                }
            }
            Op::MkEmptyList => self.push(&Var::List(vec![])),
            Op::ListAddTail => {
                let tail = self.pop();
                let list = self.pop();
                let Var::List(list) = list else {
                    return self.push_error(E_TYPE);
                };

                // TODO: quota check SVO_MAX_LIST_CONCAT -> E_QUOTA

                let mut new_list = list;
                new_list.push(tail);
                self.push(&Var::List(new_list))
            }
            Op::ListAppend => {
                let tail = self.pop();
                let list = self.pop();
                let Var::List(list) = list else {
                    return self.push_error(E_TYPE);
                };

                let Var::List(tail) = tail else {
                    return self.push_error(E_TYPE);
                };

                // TODO: quota check SVO_MAX_LIST_CONCAT -> E_QUOTA
                let new_list = list.into_iter().chain(tail.into_iter());
                self.push(&Var::List(new_list.collect()))
            }
            Op::IndexSet => {
                // collection[index] = value
                let value = self.pop(); /* rhs value */
                let index = self.pop(); /* index, must be int */
                let list = self.pop(); /* lhs except last index, should be list or str */

                let nval = match (list, index) {
                    (Var::List(l), Var::Int(i)) => {
                        if i < 0 || !i < l.len() as i64 {
                            return self.push_error(E_RANGE);
                        }

                        let mut nval = l;
                        nval[i as usize] = value;
                        Var::List(nval)
                    }
                    (Var::Str(s), Var::Int(i)) => {
                        if i < 0 || !i < s.len() as i64 {
                            return self.push_error(E_RANGE);
                        }

                        let Var::Str(value) = value else {
                            return self.push_error(E_INVARG);
                        };

                        if value.len() != 1 {
                            return self.push_error(E_INVARG);
                        }

                        let i = i as usize;
                        let (mut head, tail) = (String::from(&s[0..i]), &s[i + 1..]);
                        head.push_str(&value[0..1]);
                        head.push_str(tail);
                        Var::Str(head)
                    }
                    (_, _) => {
                        return self.push_error(E_TYPE);
                    }
                };
                self.push(&nval);
            }
            Op::MakeSingletonList => {
                let v = self.pop();
                self.push(&Var::List(vec![v]))
            }
            Op::PutTemp => {
                self.top_mut().temp = self.peek_top();
            }
            Op::PushTemp => {
                let tmp = self.top().temp.clone();
                self.push(&tmp);
                self.top_mut().temp = Var::None;
            }
            Op::Eq => {
                binary_bool_op!(self, ==);
            }
            Op::Ne => {
                binary_bool_op!(self, !=);
            }
            Op::Gt => {
                binary_bool_op!(self, >);
            }
            Op::Lt => {
                binary_bool_op!(self, <);
            }
            Op::Ge => {
                binary_bool_op!(self, >=);
            }
            Op::Le => {
                binary_bool_op!(self, <=);
            }
            Op::In => {
                let lhs = self.pop();
                let rhs = self.pop();
                let r = lhs.has_member(&rhs);
                if let Var::Err(e) = r {
                    return self.push_error(e);
                }
                self.push(&r);
            }
            Op::Mul => {
                binary_var_op!(self, mul);
            }
            Op::Sub => {
                binary_var_op!(self, sub);
            }
            Op::Div => {
                binary_var_op!(self, div);
            }
            Op::Add => {
                binary_var_op!(self, add);
            }
            Op::Exp => {
                binary_var_op!(self, pow);
            }
            Op::Mod => {
                binary_var_op!(self, modulus);
            }
            Op::And(label) => {
                let v = self.pop().is_true();
                if !v {
                    self.jump(label)
                }
            }
            Op::Or(label) => {
                let v = self.pop().is_true();
                if v {
                    self.jump(label)
                }
            }
            Op::Not => {
                let v = !self.pop().is_true();
                self.push(&Var::Int(if v { 1 } else { 0 }));
            }
            Op::UnaryMinus => {
                let v = self.pop();
                match v.negative() {
                    Err(e) => return self.push_error(e),
                    Ok(v) => self.push(&v),
                }
            }
            Op::Push(ident) => {
                let v = self.get_env(ident);
                match v {
                    Var::None => return self.push_error(E_VARNF),
                    _ => self.push(&v),
                }
            }
            Op::Put(ident) => {
                let v = self.peek_top();
                self.set_env(ident, &v);
            }
            Op::PushRef => {
                let peek = self.peek(2);
                let (index, list) = (peek[1].clone(), peek[0].clone());
                let v = match (index, list) {
                    (Var::Int(index), Var::List(list)) => {
                        if index <= 0 || !index < list.len() as i64 {
                            return self.push_error(E_RANGE);
                        } else {
                            list[index as usize].clone()
                        }
                    }
                    (_, _) => return self.push_error(E_TYPE),
                };
                self.push(&v);
            }
            Op::Ref => {
                let index = self.pop();
                let l = self.pop();
                let Var::Int(index) = index else {
                    return self.push_error(E_TYPE);
                };
                // MOO is 1-indexed.
                let index = (index - 1) as usize;
                match l.index(index) {
                    Err(e) => return self.push_error(e),
                    Ok(v) => self.push(&v),
                }
            }
            Op::RangeRef => {
                let (to, from, base) = (self.pop(), self.pop(), self.pop());
                match (to, from) {
                    (Var::Int(to), Var::Int(from)) => match base.range(from, to) {
                        Err(e) => return self.push_error(e),
                        Ok(v) => self.push(&v),
                    },
                    (_, _) => return self.push_error(E_TYPE),
                };
            }
            Op::RangeSet => {
                let (value, to, from, base) = (self.pop(), self.pop(), self.pop(), self.pop());
                match (to, from) {
                    (Var::Int(to), Var::Int(from)) => match base.rangeset(value, from, to) {
                        Err(e) => return self.push_error(e),
                        Ok(v) => self.push(&v),
                    },
                    _ => {
                        return self.push_error(E_TYPE);
                    }
                }
            }
            Op::GPut { id } => {
                self.set_env(id, &self.peek_top());
            }
            Op::GPush { id } => {
                let v = self.get_env(id);
                match v {
                    Var::None => return self.push_error(E_VARNF),
                    _ => {
                        self.push(&v);
                    }
                }
            }
            Op::Length(offset) => {
                let v = self.top().valstack[offset].clone();
                match v {
                    Var::Str(s) => self.push(&Var::Int(s.len() as i64)),
                    Var::List(l) => self.push(&Var::Int(l.len() as i64)),
                    _ => {
                        return self.push_error(E_TYPE);
                    }
                }
            }
            Op::GetProp => {
                let (propname, obj) = (self.pop(), self.pop());
                return self.get_prop(state, self.top().player_flags, propname, obj);
            }
            Op::PushGetProp => {
                let peeked = self.peek(2);
                let (propname, obj) = (peeked[0].clone(), peeked[1].clone());
                return self.get_prop(state, self.top().player_flags, propname, obj);
            }
            Op::PutProp => {
                let (rhs, propname, obj) = (self.pop(), self.pop(), self.pop());
                let (propname, obj) = match (propname, obj) {
                    (Var::Str(propname), Var::Obj(obj)) => (propname, obj),
                    (_, _) => {
                        return self.push_error(E_TYPE);
                    }
                };
                match state.update_property(obj, &propname, self.top().player_flags, &rhs) {
                    Ok(()) => {
                        self.push(&Var::None);
                    }
                    Err(e) => match e.downcast_ref::<StateError>() {
                        Some(StateError::PropertyNotFound(_, _)) => {
                            return self.push_error(E_PROPNF);
                        }
                        Some(StateError::PropertyPermissionDenied(_, _)) => {
                            return self.push_error(E_PERM);
                        }
                        _ => {
                            panic!("Unexpected error in property update: {:?}", e);
                        }
                    },
                }
                return Ok(ExecutionResult::More);
            }
            Op::Fork { id: _, f_index: _ } => {
                unimplemented!("fork")
            }
            Op::CallVerb => {
                let (args, verb, obj) = (self.pop(), self.pop(), self.pop());
                let (args, verb, obj) = match (args, verb, obj) {
                    (Var::List(l), Var::Str(s), Var::Obj(o)) => (l, s, o),
                    _ => {
                        return self.push_error(E_TYPE);
                    }
                };
                // TODO: check obj for validity, return E_INVIND if not

                return self.call_verb(state, obj, verb, args, false);
            }
            Op::Return => {
                let ret_val = self.pop();
                return self.unwind_stack(FinallyReason::Return(ret_val));
            }
            Op::Return0 => {
                return self.unwind_stack(FinallyReason::Return(Var::Int(0)));
            }
            Op::Done => {
                return self.unwind_stack(FinallyReason::Return(Var::None));
            }
            Op::FuncCall { .. } => {
                // TODO Actually perform call. For now we just raise permissions.
                return self.push_error(E_PERM);
            }
            Op::PushLabel(label) => {
                self.push(&Var::_Label(label));
            }
            Op::TryFinally(label) => {
                self.push(&Var::_Finally(label));
            }
            Op::Catch => {
                self.push(&Var::_Catch(1));
            }
            Op::TryExcept(label) => {
                self.push(&Var::_Catch(label));
            }
            Op::EndCatch(label) | Op::EndExcept(label) => {
                let is_catch = op == Op::EndCatch(label);
                let v = if is_catch { self.pop() } else { Var::None };
                let marker = self.pop();
                let Var::_Catch(marker) = marker else {
                    panic!("Stack marker is not type Catch");
                };
                for _i in 0..marker {
                    self.pop(); /* handler PC */
                    self.pop(); /* code list */
                }
                if is_catch {
                    self.push(&v);
                }
                self.jump(label);
            }
            Op::EndFinally => {
                let v = self.pop();
                let Var::_Finally(_marker) = v else {
                    panic!("Stack marker is not type Finally");
                };
                self.push(&Var::Int(0) /* fallthrough */);
                self.push(&Var::Int(0));
            }
            Op::Continue => {
                let why = self.pop();
                let Var::Int(why) = why else {
                    panic!("'why' is not an integer representing a FinallyReason");
                };
                let why = FinallyReason::from_code(why as usize);
                match why {
                    FinallyReason::Fallthrough => {
                        // Do nothing, normal case.
                        return Ok(ExecutionResult::More);
                    }
                    FinallyReason::Raise { .. }
                    | FinallyReason::Uncaught { .. }
                    | FinallyReason::Return(_)
                    | FinallyReason::Exit { .. } => {
                        return self.unwind_stack(why);
                    }
                    FinallyReason::Abort => {
                        panic!("Unexpected FINALLY_ABORT in Continue")
                    }
                }
            }
            Op::ExitId(label) => {
                self.jump(label);
                return Ok(ExecutionResult::More);
            }
            Op::Exit { stack, label } => {
                return self.unwind_stack(FinallyReason::Exit { stack, label });
            }
            Op::Scatter {
                nargs,
                nreq,
                labels,
                done,
                ..
            } => {
                let list = self.peek_top();
                let Var::List(list) = list else {
                    self.pop();
                    return self.push_error(E_TYPE);
                };

                let len = list.len();
                if len < nreq {
                    self.pop();
                    return self.push_error(E_ARGS);
                }

                assert_eq!(nargs, labels.len());

                let mut jump_where = None;
                let mut args_iter = list.into_iter();
                for label in labels.iter() {
                    match label {
                        ScatterLabel::Required(id) => {
                            let Some(arg) = args_iter.next() else {
                                return self.push_error(E_ARGS);
                            };

                            self.set_env(*id, &arg);
                        }
                        ScatterLabel::Rest(id) => {
                            let mut v = vec![];
                            for _ in 1..nargs {
                                v.push(args_iter.next().unwrap());
                            }
                            let rest = Var::List(v);
                            self.set_env(*id, &rest);
                        }
                        ScatterLabel::Optional(id, jump_to) => match args_iter.next() {
                            None => {
                                if jump_where.is_none() && jump_to.is_some() {
                                    jump_where = *jump_to;
                                }
                                break;
                            }
                            Some(v) => {
                                self.set_env(*id, &v);
                            }
                        },
                    }
                }
                match jump_where {
                    None => self.jump(done),
                    Some(jump_where) => self.jump(jump_where),
                }
            }
            Op::CheckListForSplice => {
                let Var::List(_) = self.peek_top() else {
                    self.pop();
                    return self.push_error(E_TYPE);
                };
            }
        }
        Ok(ExecutionResult::More)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use anyhow::Error;
    use enumset::EnumSet;

    use crate::compiler::codegen::compile;
    use crate::compiler::parse::Names;
    use crate::db::state::{StateError, WorldState};
    use crate::model::objects::ObjFlag;
    use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
    use crate::model::var::Error::{E_NONE, E_VERBNF};
    use crate::model::var::{Objid, Var};
    use crate::model::verbs::{VerbAttrs, VerbFlag, VerbInfo, Vid};
    use crate::vm::execute::{ExecutionResult, VM};
    use crate::vm::opcode::Op::*;
    use crate::vm::opcode::{Binary, Op};

    struct MockState {
        verbs: HashMap<(Objid, String), (Binary, VerbInfo)>,
        properties: HashMap<(Objid, String), Var>,
    }

    impl MockState {
        fn new() -> Self {
            Self {
                verbs: Default::default(),
                properties: Default::default(),
            }
        }
        fn set_verb(&mut self, o: Objid, name: &str, binary: &Binary) {
            self.verbs.insert(
                (o, name.to_string()),
                (
                    binary.clone(),
                    VerbInfo {
                        vid: Vid(0),
                        names: vec![name.to_string()],
                        attrs: VerbAttrs {
                            definer: Some(o),
                            owner: Some(o),
                            flags: Some(VerbFlag::Exec | VerbFlag::Read),
                            args_spec: Some(VerbArgsSpec {
                                dobj: ArgSpec::This,
                                prep: PrepSpec::None,
                                iobj: ArgSpec::This,
                            }),
                            program: None,
                        },
                    },
                ),
            );
        }
    }

    fn mk_binary(main_vector: Vec<Op>, literals: Vec<Var>, var_names: Names) -> Binary {
        Binary {
            literals,
            jump_labels: vec![],
            var_names,
            main_vector,
            fork_vectors: vec![],
        }
    }

    fn prepare_test_verb_with_names(
        verb_name: &str,
        state: &mut MockState,
        opcodes: Vec<Op>,
        literals: Vec<Var>,
        var_names: Names,
    ) {
        set_test_verb(verb_name, state, mk_binary(opcodes, literals, var_names));
    }

    fn prepare_test_verb(
        verb_name: &str,
        state: &mut MockState,
        opcodes: Vec<Op>,
        literals: Vec<Var>,
    ) {
        let var_names = Names::new();
        set_test_verb(verb_name, state, mk_binary(opcodes, literals, var_names));
    }

    fn set_test_verb(verb_name: &str, state: &mut MockState, binary: Binary) {
        let o = Objid(0);
        state.set_verb(o, verb_name, &binary)
    }

    fn call_verb(verb_name: &str, vm: &mut VM, state: &mut MockState) {
        let o = Objid(0);

        assert_eq!(
            vm.do_method_verb(
                state,
                o,
                verb_name,
                false,
                o,
                o,
                ObjFlag::Wizard | ObjFlag::Programmer,
                o,
                vec![],
            )
            .unwrap(),
            Var::Err(E_NONE),
        );
    }

    impl WorldState for MockState {
        fn retrieve_verb(&self, obj: Objid, vname: &str) -> Result<(Binary, VerbInfo), Error> {
            let v = self.verbs.get(&(obj, vname.to_string()));
            match v {
                None => Err(StateError::VerbNotFound(obj, vname.to_string()).into()),
                Some(v) => Ok(v.clone()),
            }
        }

        fn retrieve_property(
            &self,
            obj: Objid,
            pname: &str,
            _player_flags: EnumSet<ObjFlag>,
        ) -> Result<Var, Error> {
            let p = self.properties.get(&(obj, pname.to_string()));
            match p {
                None => Err(StateError::PropertyNotFound(obj, pname.to_string()).into()),
                Some(p) => Ok(p.clone()),
            }
        }

        fn update_property(
            &mut self,
            obj: Objid,
            pname: &str,
            _player_flags: EnumSet<ObjFlag>,
            value: &Var,
        ) -> Result<(), Error> {
            self.properties
                .insert((obj, pname.to_string()), value.clone());
            Ok(())
        }

        fn location_of(&mut self, obj: Objid) -> Result<Objid, Error> {
            unimplemented!()
        }

        fn contents_of(&mut self, obj: Objid) -> Result<Vec<Objid>, Error> {
            unimplemented!()
        }

        fn names_of(&mut self, obj: Objid) -> Result<(String, Vec<String>), Error> {
            unimplemented!()
        }
        fn parent_of(&mut self, _obj: Objid) -> Result<Objid, Error> {
            Ok(Objid(-1))
        }

        fn valid(&mut self, _obj: Objid) -> Result<bool, Error> {
            Ok(true)
        }
    }

    fn exec_vm(vm: &mut VM, state: &mut MockState) -> Var {
        // Call repeatedly into exec until we ge either an error or Complete.
        loop {
            match vm.exec(state) {
                Ok(ExecutionResult::More) => continue,
                Ok(ExecutionResult::Complete(a)) => return a,
                Err(e) => panic!("error during execution: {:?}", e),
            }
        }
    }

    #[test]
    fn test_verbnf() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        let o = Objid(0);
        assert_eq!(
            vm.do_method_verb(
                &mut state,
                o,
                "test",
                false,
                o,
                o,
                ObjFlag::Wizard | ObjFlag::Programmer,
                o,
                vec![],
            )
            .unwrap(),
            Var::Err(E_VERBNF)
        );
    }

    #[test]
    fn test_simple_vm_execute() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        prepare_test_verb("test", &mut state, vec![Imm(0), Pop, Done], vec![1.into()]);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::None);
    }

    #[test]
    fn test_string_value_simple_indexing() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        prepare_test_verb(
            "test",
            &mut state,
            vec![Imm(0), Imm(1), Ref, Return, Done],
            vec![Var::Str("hello".to_string()), 2.into()],
        );

        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::Str("e".to_string()));
    }

    #[test]
    fn test_string_value_range_indexing() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        prepare_test_verb(
            "test",
            &mut state,
            vec![Imm(0), Imm(1), Imm(2), RangeRef, Return, Done],
            vec![Var::Str("hello".to_string()), 2.into(), 4.into()],
        );

        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::Str("ell".to_string()));
    }

    #[test]
    fn test_list_value_simple_indexing() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        prepare_test_verb(
            "test",
            &mut state,
            vec![Imm(0), Imm(1), Ref, Return, Done],
            vec![
                Var::List(vec![111.into(), 222.into(), 333.into()]),
                2.into(),
            ],
        );

        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::Int(222));
    }

    #[test]
    fn test_list_value_range_indexing() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        prepare_test_verb(
            "test",
            &mut state,
            vec![Imm(0), Imm(1), Imm(2), RangeRef, Return, Done],
            vec![
                Var::List(vec![111.into(), 222.into(), 333.into()]),
                2.into(),
                3.into(),
            ],
        );

        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::List(vec![222.into(), 333.into()]));
    }

    #[test]
    fn test_list_set_range() {
        let mut vm = VM::new();
        let mut state = MockState::new();

        let mut var_names = Names::new();
        let a = var_names.find_or_add_name("a");

        prepare_test_verb_with_names(
            "test",
            &mut state,
            vec![
                Imm(0),
                Put(a.0),
                Pop,
                Push(a.0),
                Imm(1),
                Imm(2),
                Imm(3),
                PutTemp,
                RangeSet,
                Put(a.0),
                Pop,
                PushTemp,
                Pop,
                Push(a.0),
                Return,
                Done,
            ],
            vec![
                Var::List(vec![111.into(), 222.into(), 333.into()]),
                2.into(),
                3.into(),
                Var::List(vec![321.into(), 123.into()]),
            ],
            var_names,
        );
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::List(vec![111.into(), 321.into(), 123.into()]));
    }

    #[test]
    fn test_list_splice() {
        let mut vm = VM::new();
        let mut state = MockState::new();

        let program = "a = {1,2,3,4,5}; return {@a[2..4]};";
        let binary = compile(program).unwrap();
        let args = binary.find_var("args");

        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::List(vec![2.into(), 3.into(), 4.into()]));
    }

    #[test]
    fn test_list_range_length() {
        let mut vm = VM::new();
        let mut state = MockState::new();

        let program = "return {{1,2,3}[2..$], {1}[$]};";
        let binary = compile(program).unwrap();

        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(
            result,
            Var::List(vec![Var::List(vec![2.into(), 3.into()]), Var::Int(1)])
        );
    }

    #[test]
    fn test_string_set_range() {
        let mut vm = VM::new();
        let mut state = MockState::new();

        let mut var_names = Names::new();
        let a = var_names.find_or_add_name("a");

        prepare_test_verb_with_names(
            "test",
            &mut state,
            vec![
                Imm(0),
                Put(a.0),
                Pop,
                Push(a.0),
                Imm(1),
                Imm(2),
                Imm(3),
                PutTemp,
                RangeSet,
                Put(a.0),
                Pop,
                PushTemp,
                Pop,
                Push(a.0),
                Return,
                Done,
            ],
            vec![
                Var::Str("mandalorian".to_string()),
                4.into(),
                7.into(),
                Var::Str("bozo".to_string()),
            ],
            var_names,
        );
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::Str("manbozorian".to_string()));
    }

    #[test]
    fn test_property_retrieval() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        prepare_test_verb(
            "test",
            &mut state,
            vec![Imm(0), Imm(1), GetProp, Return, Done],
            vec![Var::Obj(Objid(0)), Var::Str(String::from("test_prop"))],
        );
        state
            .properties
            .insert((Objid(0), String::from("test_prop")), Var::Int(666));
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::Int(666));
    }

    #[test]
    fn test_call_verb() {
        let mut vm = VM::new();
        let mut state = MockState::new();

        // Prepare two, chained, test verbs in our environment, with simple operations.

        // The first merely returns the value "666" immediately.
        prepare_test_verb(
            "test_return_verb",
            &mut state,
            vec![Imm(0), Return],
            vec![666.into()],
        );

        // The second actually calls the first verb, and returns the result.
        prepare_test_verb(
            "test_call_verb",
            &mut state,
            vec![
                Imm(0), /* obj */
                Imm(1), /* verb */
                Imm(2), /* args */
                CallVerb,
                Return,
                Done,
            ],
            vec![
                Var::Obj(Objid(0)),
                Var::Str(String::from("test_return_verb")),
                Var::List(vec![]),
            ],
        );

        // Invoke the second verb
        call_verb("test_call_verb", &mut vm, &mut state);

        let result = exec_vm(&mut vm, &mut state);

        assert_eq!(result, Var::Int(666));
    }

    #[test]
    fn test_assignment_from_range() {
        let mut vm = VM::new();
        let mut state = MockState::new();

        let program = "x = 1; y = {1,2,3}; x = x + y[2]; return x;";
        let binary = compile(program).unwrap();
        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::Int(3));
    }

    #[test]
    fn test_while_loop() {
        let mut vm = VM::new();
        let mut state = MockState::new();

        let program =
            "x = 0; while (x<100) x = x + 1; if (x == 75) break; endif endwhile return x;";
        let binary = compile(program).unwrap();
        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::Int(75));
    }

    #[test]
    fn test_while_labelled_loop() {
        let mut vm = VM::new();
        let mut state = MockState::new();

        let program = "x = 0; while broken (1) x = x + 1; if (x == 50) break; else continue broken; endif endwhile return x;";
        let binary = compile(program).unwrap();
        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::Int(50));
    }

    #[test]
    fn test_while_breaks() {
        let mut vm = VM::new();
        let mut state = MockState::new();

        let program = "x = 0; while (1) x = x + 1; if (x == 50) break; endif endwhile return x;";
        let binary = compile(program).unwrap();
        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::Int(50));
    }

    #[test]
    fn test_for_list_loop() {
        let mut vm = VM::new();
        let mut state = MockState::new();

        let program = "x = {1,2,3,4}; z = 0; for i in (x) z = z + i; endfor return {i,z};";
        let binary = compile(program).unwrap();
        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::List(vec![Var::Int(4), Var::Int(10)]));
    }

    #[test]
    fn test_for_range_loop() {
        let mut vm = VM::new();
        let mut state = MockState::new();

        let program = "z = 0; for i in [1..4] z = z + i; endfor return {i,z};";
        let binary = compile(program).unwrap();
        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::List(vec![Var::Int(4), Var::Int(10)]));
    }

    #[test]
    fn test_basic_scatter_assign() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        let program = "{a, b, c, ?d = 4} = {1, 2, 3}; return {d, c, b, a};";
        let binary = compile(program).unwrap();
        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(
            result,
            Var::List(vec![Var::Int(4), Var::Int(3), Var::Int(2), Var::Int(1)])
        );
    }

    #[test]
    fn test_more_scatter_assign() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        let program = "{a, b, @c} = {1, 2, 3, 4}; {x, @y, ?z} = {5,6,7,8}; return {a,b,c,x,y,z};";
        let binary = compile(program).unwrap();
        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(
            result,
            Var::List(vec![
                Var::Int(1),
                Var::Int(2),
                Var::List(vec![Var::Int(3), Var::Int(4)]),
                Var::Int(5),
                Var::List(vec![Var::Int(6), Var::Int(7)]),
                Var::Int(8),
            ])
        );
    }

    #[test]
    fn test_conditional_expr() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        let program = "return 1 ? 2 | 3;";
        let binary = compile(program).unwrap();
        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::Int(2));
    }

    #[test]
    fn test_catch_expr() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        let program = "return {`x ! e_varnf => 666', `321 ! e_verbnf => 123'};";
        let binary = compile(program).unwrap();
        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::List(vec![Var::Int(666), Var::Int(321)]));
    }

    #[test]
    fn test_try_except_stmt() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        let program = "try a; except e (E_VARNF) return 666; endtry return 333;";
        let binary = compile(program).unwrap();
        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::Int(666));
    }

    #[test]
    fn test_try_finally_stmt() {
        let mut vm = VM::new();
        let mut state = MockState::new();
        let program = "try a; finally return 666; endtry return 333;";
        let binary = compile(program).unwrap();
        set_test_verb("test", &mut state, binary);
        call_verb("test", &mut vm, &mut state);
        let result = exec_vm(&mut vm, &mut state);
        assert_eq!(result, Var::Int(666));
    }
}
