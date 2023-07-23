use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::trace;

use crate::db::state::WorldState;

use crate::model::ObjectError::{PropertyNotFound, PropertyPermissionDenied};

use crate::tasks::Sessions;

use crate::var::error::Error::{E_ARGS, E_INVARG, E_PERM, E_PROPNF, E_RANGE, E_TYPE, E_VARNF};

use crate::var::{
    v_bool, v_catch, v_finally, v_int, v_label, v_list, v_obj, v_str, Variant, VAR_NONE,
};

use crate::vm::opcode::{Op, ScatterLabel};
use crate::vm::vm::{ExecutionResult, FinallyReason, VM};

macro_rules! binary_bool_op {
    ( $self:ident, $op:tt ) => {
        let rhs = $self.pop();
        let lhs = $self.pop();
        let result = if lhs $op rhs { 1 } else { 0 };
        $self.push(&v_int(result))
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

impl VM {
    pub async fn exec(
        &mut self,
        state: &mut dyn WorldState,
        client_connection: Arc<RwLock<dyn Sessions>>,
    ) -> Result<ExecutionResult, anyhow::Error> {
        let op = self
            .next_op()
            .expect("Unexpected program termination; opcode stream should end with RETURN or DONE");

        trace!("exec: {:?} stack: {:?}", op, self.top().valstack);
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
                let Variant::Int(count) = count.variant() else {
                    return self.raise_error(E_TYPE);

                    // LambdaMOO had a raise followed by jump. Not clear how that would work.
                    // Watch out for bugs here. Same below
                    // self.jump(label);
                };
                let count = *count as usize;
                let Variant::List(l) = list.variant() else {
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
                self.push(&v_int((count + 1) as i64));
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

                let next_val = match (to.variant(), from.variant()) {
                    (Variant::Int(to_i), Variant::Int(from_i)) => {
                        if from_i > to_i {
                            self.jump(label);
                            return Ok(ExecutionResult::More);
                        }
                        v_int(from_i + 1)
                    }
                    (Variant::Obj(to_o), Variant::Obj(from_o)) => {
                        if from_o.0 > to_o.0 {
                            self.jump(label);
                            return Ok(ExecutionResult::More);
                        }
                        v_obj(from_o.0 + 1)
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
                        let value = self.top().binary.literals[slot.0 as usize].clone();
                        self.push(&value);
                    }
                }
            }
            Op::MkEmptyList => self.push(&v_list(vec![])),
            Op::ListAddTail => {
                let tail = self.pop();
                let list = self.pop();
                let Variant::List(list) = list.variant() else {
                    return self.push_error(E_TYPE);
                };

                // TODO: quota check SVO_MAX_LIST_CONCAT -> E_QUOTA

                let mut new_list = list.clone();
                new_list.push(tail);
                self.push(&v_list(new_list))
            }
            Op::ListAppend => {
                let tail = self.pop();
                let list = self.pop();
                let Variant::List(list) = list.variant() else {
                    return self.push_error(E_TYPE);
                };

                let Variant::List(tail) = tail.variant() else {
                    return self.push_error(E_TYPE);
                };

                // TODO: quota check SVO_MAX_LIST_CONCAT -> E_QUOTA
                let new_list = list.iter().chain(tail.iter());
                self.push(&v_list(new_list.cloned().collect()))
            }
            Op::IndexSet => {
                // collection[index] = value
                let value = self.pop(); /* rhs value */
                let index = self.pop(); /* index, must be int */
                let list = self.pop(); /* lhs except last index, should be list or str */

                let nval = match (list.variant(), index.variant()) {
                    (Variant::List(l), Variant::Int(i)) => {
                        // Adjust for 1 indexing.
                        let i = *i - 1;
                        if i < 0 || i >= l.len() as i64 {
                            return self.push_error(E_RANGE);
                        }

                        let mut nval = l.clone();
                        nval[i as usize] = value;
                        v_list(nval)
                    }
                    (Variant::Str(s), Variant::Int(i)) => {
                        // Adjust for 1 indexing.
                        let i = *i - 1;
                        if i < 0 || i >= s.len() as i64 {
                            return self.push_error(E_RANGE);
                        }

                        let Variant::Str(value) = value.variant() else {
                            return self.push_error(E_INVARG);
                        };

                        if value.len() != 1 {
                            return self.push_error(E_INVARG);
                        }

                        let i = i as usize;
                        let (mut head, tail) = (String::from(&s[0..i]), &s[i + 1..]);
                        head.push_str(&value[0..1]);
                        head.push_str(tail);
                        v_str(&head)
                    }
                    (_, _) => {
                        return self.push_error(E_TYPE);
                    }
                };
                self.push(&nval);
            }
            Op::MakeSingletonList => {
                let v = self.pop();
                self.push(&v_list(vec![v]))
            }
            Op::PutTemp => {
                self.top_mut().temp = self.peek_top();
            }
            Op::PushTemp => {
                let tmp = self.top().temp.clone();
                self.push(&tmp);
                self.top_mut().temp = VAR_NONE;
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
                let r = lhs.index_in(&rhs);
                if let Variant::Err(e) = r.variant() {
                    return self.push_error(*e);
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
                let v = self.peek_top().is_true();
                if !v {
                    self.jump(label)
                } else {
                    self.pop();
                }
            }
            Op::Or(label) => {
                let v = self.peek_top().is_true();
                if v {
                    self.jump(label);
                } else {
                    self.pop();
                }
            }
            Op::Not => {
                let v = !self.pop().is_true();
                self.push(&v_bool(v));
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
                match v.variant() {
                    Variant::None => return self.push_error(E_VARNF),
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
                let v = match (index.variant(), list.variant()) {
                    (Variant::Int(index), Variant::List(list)) => {
                        // MOO is 1-indexed, it's easier if we adjust in advance.
                        if *index < 1 {
                            return self.push_error(E_RANGE);
                        }
                        let index = ((*index) - 1) as usize;
                        if index >= list.len() {
                            return self.push_error(E_RANGE);
                        } else {
                            list[index].clone()
                        }
                    }
                    (_, _) => return self.push_error(E_TYPE),
                };
                self.push(&v);
            }
            Op::Ref => {
                let index = self.pop();
                let l = self.pop();
                let Variant::Int(index) = index.variant() else {
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
                match (to.variant(), from.variant()) {
                    (Variant::Int(to), Variant::Int(from)) => {
                        // MOO is 1-indexed. Adjust.
                        match base.range(*from, *to) {
                            Err(e) => return self.push_error(e),
                            Ok(v) => self.push(&v),
                        }
                    }
                    (_, _) => return self.push_error(E_TYPE),
                };
            }
            Op::RangeSet => {
                let (value, to, from, base) = (self.pop(), self.pop(), self.pop(), self.pop());
                match (to.variant(), from.variant()) {
                    (Variant::Int(to), Variant::Int(from)) => {
                        match base.rangeset(value, *from, *to) {
                            Err(e) => return self.push_error(e),
                            Ok(v) => self.push(&v),
                        }
                    }
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
                match v.variant() {
                    Variant::None => return self.push_error(E_VARNF),
                    _ => {
                        self.push(&v);
                    }
                }
            }
            Op::Length(offset) => {
                let v = self.top().valstack[offset.0 as usize].clone();
                match v.variant() {
                    Variant::Str(s) => self.push(&v_int(s.len() as i64)),
                    Variant::List(l) => self.push(&v_int(l.len() as i64)),
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
                let (propname, obj) = match (propname.variant(), obj.variant()) {
                    (Variant::Str(propname), Variant::Obj(obj)) => (propname, obj),
                    (_, _) => {
                        return self.push_error(E_TYPE);
                    }
                };

                let update_result =
                    state.update_property(*obj, propname, self.top().player_flags, &rhs);

                match update_result {
                    Ok(()) => {
                        self.push(&VAR_NONE);
                    }
                    Err(e) => match e {
                        PropertyNotFound(_, _) => {
                            return self.push_error(E_PROPNF);
                        }
                        PropertyPermissionDenied(_, _) => {
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
                let (args, verb, obj) = match (args.variant(), verb.variant(), obj.variant()) {
                    (Variant::List(l), Variant::Str(s), Variant::Obj(o)) => (l, s, o),
                    _ => {
                        return self.push_error(E_TYPE);
                    }
                };
                // TODO: check obj for validity, return E_INVIND if not

                return self.call_verb(state, *obj, verb.clone(), args, false);
            }
            Op::Return => {
                let ret_val = self.pop();
                return self.unwind_stack(FinallyReason::Return(ret_val));
            }
            Op::Return0 => {
                return self.unwind_stack(FinallyReason::Return(v_int(0)));
            }
            Op::Done => {
                return self.unwind_stack(FinallyReason::Return(VAR_NONE));
            }
            Op::FuncCall { id } => {
                // Pop arguments, should be a list.
                let args = self.pop();
                let Variant::List(args) = args.variant() else {
                    return self.push_error(E_ARGS);
                };
                if id.0 as usize >= self.bf_funcs.len() {
                    return self.push_error(E_VARNF);
                }
                let bf = self.bf_funcs[id.0 as usize].clone();
                let result = bf
                    .call(state, self.top_mut(), client_connection, args)
                    .await?;
                self.push(&result);
            }
            Op::PushLabel(label) => {
                self.push(&v_label(label));
            }
            Op::TryFinally(label) => {
                self.push(&v_finally(label));
            }
            Op::Catch => {
                self.push(&v_catch(1.into()));
            }
            Op::TryExcept(label) => {
                self.push(&v_catch(label));
            }
            Op::EndCatch(label) | Op::EndExcept(label) => {
                let is_catch = op == Op::EndCatch(label);
                let v = if is_catch { self.pop() } else { VAR_NONE };
                let marker = self.pop();
                let Variant::_Catch(marker) = marker.variant() else {
                    panic!("Stack marker is not type Catch");
                };
                for _i in 0..marker.0 {
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
                let Variant::_Finally(_marker) = v.variant() else {
                    panic!("Stack marker is not type Finally");
                };
                self.push(&v_int(0) /* fallthrough */);
                self.push(&v_int(0));
            }
            Op::Continue => {
                let why = self.pop();
                let Variant::Int(why) = why.variant() else {
                    panic!("'why' is not an integer representing a FinallyReason");
                };
                let why = FinallyReason::from_code(*why as usize);
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
                rest,
                labels,
                done,
                ..
            } => {
                let have_rest = rest <= nargs;
                let rhs = self.peek_top();
                let Variant::List(rhs_values) = rhs.variant() else {
                    self.pop();
                    return self.push_error(E_TYPE);
                };

                let len = rhs_values.len();
                if len < nreq || !have_rest && len > nargs {
                    self.pop();
                    return self.push_error(E_ARGS);
                }

                assert_eq!(nargs, labels.len());
                let mut nopt_avail = len - nreq;

                let mut jump_where = None;
                let mut args_iter = rhs_values.iter();
                for label in labels.iter() {
                    match label {
                        ScatterLabel::Rest(id) => {
                            let mut v = vec![];
                            for _ in 1..nargs {
                                let Some(rest) = args_iter.next() else {
                                    break;
                                };
                                v.push(rest.clone());
                            }
                            let rest = v_list(v);
                            self.set_env(*id, &rest);
                        }
                        ScatterLabel::Required(id) => {
                            let Some(arg) = args_iter.next() else {
                                return self.push_error(E_ARGS);
                            };

                            self.set_env(*id, arg);
                        }
                        ScatterLabel::Optional(id, jump_to) => {
                            if nopt_avail > 0 {
                                nopt_avail -= 1;
                                let Some(arg) = args_iter.next() else {
                                    return self.push_error(E_ARGS);
                                };
                                self.set_env(*id, arg);
                            } else if jump_where.is_none() && jump_to.is_some() {
                                jump_where = *jump_to;
                            }
                        }
                    }
                }
                match jump_where {
                    None => self.jump(done),
                    Some(jump_where) => self.jump(jump_where),
                }
            }
            Op::CheckListForSplice => {
                let Variant::List(_) = self.peek_top().variant() else {
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

    use std::sync::Arc;

    use anyhow::Error;
    use async_trait::async_trait;
    use tokio::sync::RwLock;
    use tracing_test::traced_test;

    use crate::compiler::codegen::compile;
    use crate::compiler::labels::Names;
    use crate::db::mock_world_state::MockWorldStateSource;
    use crate::db::state::{WorldState, WorldStateSource};

    use crate::model::objects::ObjFlag;
    use crate::model::props::PropFlag;

    use crate::model::ObjectError;
    use crate::model::ObjectError::VerbNotFound;

    use crate::tasks::Sessions;
    use crate::util::bitenum::BitEnum;
    use crate::var::error::Error::E_VERBNF;
    use crate::var::{v_err, v_int, v_list, v_obj, v_str, Objid, Var, VAR_NONE};
    use crate::vm::execute::{ExecutionResult, VM};
    use crate::vm::opcode::Op::*;
    use crate::vm::opcode::{Binary, Op};

    struct NoopClientConnection {}
    impl NoopClientConnection {
        pub fn new() -> Self {
            Self {}
        }
    }

    #[async_trait]
    impl Sessions for NoopClientConnection {
        async fn send_text(&mut self, _player: Objid, _msg: String) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
            Ok(vec![])
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

    fn call_verb(state: &mut dyn WorldState, verb_name: &str, vm: &mut VM) {
        let o = Objid(0);

        assert!(vm
            .do_method_verb(
                0,
                state,
                o,
                verb_name,
                false,
                o,
                o,
                BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer,
                o,
                &[],
            )
            .is_ok());
    }

    fn exec_vm(state: &mut dyn WorldState, vm: &mut VM) -> Var {
        tokio_test::block_on(async {
            let client_connection = Arc::new(RwLock::new(NoopClientConnection::new()));
            // Call repeatedly into exec until we ge either an error or Complete.
            loop {
                match vm.exec(state, client_connection.clone()).await {
                    Ok(ExecutionResult::More) => continue,
                    Ok(ExecutionResult::Complete(a)) => return a,
                    Err(e) => panic!("error during execution: {:?}", e),
                    Ok(ExecutionResult::Exception(e)) => {
                        panic!("MOO exception {:?}", e);
                    }
                }
            }
        })
    }

    #[test]
    fn test_verbnf() {
        let mut state_src = MockWorldStateSource::new();
        let mut state = state_src.new_world_state().unwrap();
        let mut vm = VM::new();
        let o = Objid(0);

        match vm.do_method_verb(
            0,
            state.as_mut(),
            o,
            "test",
            false,
            o,
            o,
            BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer,
            o,
            &[],
        ) {
            Err(e) => match e.downcast::<ObjectError>() {
                Ok(VerbNotFound(vo, vs)) => {
                    assert_eq!(vo, o);
                    assert_eq!(vs, "test");
                }
                _ => {
                    panic!("expected verbnf error");
                }
            },
            _ => panic!("expected verbnf error"),
        }
    }

    #[test]
    fn test_simple_vm_execute() {
        let binary = mk_binary(vec![Imm(0.into()), Pop, Done], vec![1.into()], Names::new());
        let mut state_src = MockWorldStateSource::new_with_verb("test", &binary);
        let mut state = state_src.new_world_state().unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, VAR_NONE);
    }

    #[test]
    fn test_string_value_simple_indexing() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
                vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done],
                vec![v_str("hello"), 2.into()],
                Names::new(),
            ),
        )
        .new_world_state()
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_str("e"));
    }

    #[test]
    fn test_string_value_range_indexing() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
                vec![
                    Imm(0.into()),
                    Imm(1.into()),
                    Imm(2.into()),
                    RangeRef,
                    Return,
                    Done,
                ],
                vec![v_str("hello"), 2.into(), 4.into()],
                Names::new(),
            ),
        )
        .new_world_state()
        .unwrap();
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_str("ell"));
    }

    #[test]
    fn test_list_value_simple_indexing() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
                vec![Imm(0.into()), Imm(1.into()), Ref, Return, Done],
                vec![v_list(vec![111.into(), 222.into(), 333.into()]), 2.into()],
                Names::new(),
            ),
        )
        .new_world_state()
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(222));
    }

    #[test]
    fn test_list_value_range_indexing() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
                vec![
                    Imm(0.into()),
                    Imm(1.into()),
                    Imm(2.into()),
                    RangeRef,
                    Return,
                    Done,
                ],
                vec![
                    v_list(vec![111.into(), 222.into(), 333.into()]),
                    2.into(),
                    3.into(),
                ],
                Names::new(),
            ),
        )
        .new_world_state()
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![222.into(), 333.into()]));
    }

    #[test]
    fn test_list_set_range() {
        let mut var_names = Names::new();
        let a = var_names.find_or_add_name("a");
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
                vec![
                    Imm(0.into()),
                    Put(a.0),
                    Pop,
                    Push(a.0),
                    Imm(1.into()),
                    Imm(2.into()),
                    Imm(3.into()),
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
                    v_list(vec![111.into(), 222.into(), 333.into()]),
                    2.into(),
                    3.into(),
                    v_list(vec![321.into(), 123.into()]),
                ],
                var_names,
            ),
        )
        .new_world_state()
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![111.into(), 321.into(), 123.into()]));
    }

    #[test]
    fn test_list_splice() {
        let program = "a = {1,2,3,4,5}; return {@a[2..4]};";
        let binary = compile(program).unwrap();
        let mut state = MockWorldStateSource::new_with_verb("test", &binary)
            .new_world_state()
            .unwrap();
        let mut vm = VM::new();
        let _args = binary.find_var("args");
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![2.into(), 3.into(), 4.into()]));
    }

    #[test]
    fn test_list_range_length() {
        let program = "return {{1,2,3}[2..$], {1}[$]};";
        let mut state = MockWorldStateSource::new_with_verb("test", &compile(program).unwrap())
            .new_world_state()
            .unwrap();
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(
            result,
            v_list(vec![v_list(vec![2.into(), 3.into()]), v_int(1)])
        );
    }

    #[test]
    fn test_if_or_expr() {
        let program = "if (1 || 0) return 1; else return 2; endif";
        let mut state = MockWorldStateSource::new_with_verb("test", &compile(program).unwrap())
            .new_world_state()
            .unwrap();
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(1));
    }

    #[test]
    fn test_string_set_range() {
        let mut var_names = Names::new();
        let a = var_names.find_or_add_name("a");

        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
                vec![
                    Imm(0.into()),
                    Put(a.0),
                    Pop,
                    Push(a.0),
                    Imm(1.into()),
                    Imm(2.into()),
                    Imm(3.into()),
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
                vec![v_str("mandalorian"), 4.into(), 7.into(), v_str("bozo")],
                var_names,
            ),
        )
        .new_world_state()
        .unwrap();
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_str("manbozorian"));
    }

    #[test]
    fn test_property_retrieval() {
        let mut state = MockWorldStateSource::new_with_verb(
            "test",
            &mk_binary(
                vec![Imm(0.into()), Imm(1.into()), GetProp, Return, Done],
                vec![v_obj(0), v_str("test_prop")],
                Names::new(),
            ),
        )
        .new_world_state()
        .unwrap();
        {
            state
                .add_property(
                    Objid(0),
                    "test_prop",
                    Objid(0),
                    BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                    Some(v_int(666)),
                )
                .unwrap();
        }
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(666));
    }

    #[test]
    fn test_call_verb() {
        // Prepare two, chained, test verbs in our environment, with simple operations.

        // The first merely returns the value "666" immediately.
        let return_verb_binary = mk_binary(
            vec![Imm(0.into()), Return, Done],
            vec![v_int(666)],
            Names::new(),
        );

        // The second actually calls the first verb, and returns the result.
        let call_verb_binary = mk_binary(
            vec![
                Imm(0.into()), /* obj */
                Imm(1.into()), /* verb */
                Imm(2.into()), /* args */
                CallVerb,
                Return,
                Done,
            ],
            vec![v_obj(0), v_str("test_return_verb"), v_list(vec![])],
            Names::new(),
        );
        let mut state = MockWorldStateSource::new_with_verbs(vec![
            ("test_return_verb", &return_verb_binary),
            ("test_call_verb", &call_verb_binary),
        ])
        .new_world_state()
        .unwrap();
        let mut vm = VM::new();

        // Invoke the second verb
        call_verb(state.as_mut(), "test_call_verb", &mut vm);

        let result = exec_vm(state.as_mut(), &mut vm);

        assert_eq!(result, v_int(666));
    }

    fn world_with_test_program(program: &str) -> Box<dyn WorldState> {
        let binary = compile(program).unwrap();
        let state = MockWorldStateSource::new_with_verb("test", &binary)
            .new_world_state()
            .unwrap();
        state
    }

    #[test]
    fn test_assignment_from_range() {
        let program = "x = 1; y = {1,2,3}; x = x + y[2]; return x;";
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(3));
    }

    #[test]
    fn test_while_loop() {
        let program =
            "x = 0; while (x<100) x = x + 1; if (x == 75) break; endif endwhile return x;";
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(75));
    }

    #[test]
    fn test_while_labelled_loop() {
        let program = "x = 0; while broken (1) x = x + 1; if (x == 50) break; else continue broken; endif endwhile return x;";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(50));
    }

    #[test]
    fn test_while_breaks() {
        let program = "x = 0; while (1) x = x + 1; if (x == 50) break; endif endwhile return x;";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(50));
    }

    #[test]
    fn test_for_list_loop() {
        let program = "x = {1,2,3,4}; z = 0; for i in (x) z = z + i; endfor return {i,z};";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(4), v_int(10)]));
    }

    #[test]
    fn test_for_range_loop() {
        let program = "z = 0; for i in [1..4] z = z + i; endfor return {i,z};";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(4), v_int(10)]));
    }

    #[test]
    fn test_basic_scatter_assign() {
        let program = "{a, b, c, ?d = 4} = {1, 2, 3}; return {d, c, b, a};";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(4), v_int(3), v_int(2), v_int(1)]));
    }

    #[test]
    fn test_more_scatter_assign() {
        let program = "{a, b, @c} = {1, 2, 3, 4}; {x, @y, ?z} = {5,6,7,8}; return {a,b,c,x,y,z};";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(
            result,
            v_list(vec![
                v_int(1),
                v_int(2),
                v_list(vec![v_int(3), v_int(4)]),
                v_int(5),
                v_list(vec![v_int(6), v_int(7)]),
                v_int(8),
            ])
        );
    }

    #[test]
    fn test_scatter_multi_optional() {
        let program = "{?a, ?b, ?c, ?d = a, @remain} = {1, 2, 3}; return {d, c, b, a, remain};";
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(
            result,
            v_list(vec![v_int(1), v_int(3), v_int(2), v_int(1), v_list(vec![])])
        );
    }

    #[test]
    #[traced_test]
    fn test_scatter_regression() {
        // Wherein I discovered that precedence order for scatter assign was wrong wrong wrong.
        let program = r#"
        a = {{#2, #70, #70, #-1, #-1}, #70};
        thing = a[2];
        {?who = player, ?what = thing, ?where = this:_locations(who), ?dobj, ?iobj, @other} = a[1];
        return {who, what, where, dobj, iobj, @other};
        "#;
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        // MOO has  {#2, #70, #70, #-1, #-1, {}} for this equiv in JHCore parse_parties, and does not
        // actually invoke `_locations` (where i've subbed 666) for these values.
        // So something is wonky about our scatter evaluation, looks like on the first arg.
        assert_eq!(
            result,
            v_list(vec![v_obj(2), v_obj(70), v_obj(70), v_obj(-1), v_obj(-1)])
        );
    }

    #[test]
    #[traced_test]
    fn test_scatter_precedence() {
        // Simplified case of operator precedence fix.
        let program = "{a,b,c} = {{1,2,3}}[1]; return {a,b,c};";
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(1), v_int(2), v_int(3)]));
    }

    #[test]
    fn test_conditional_expr() {
        let program = "return 1 ? 2 | 3;";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(2));
    }

    #[test]
    fn test_catch_expr() {
        let program = "return {`x ! e_varnf => 666', `321 ! e_verbnf => 123'};";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(666), v_int(321)]));
    }

    #[test]
    fn test_catch_expr_any() {
        let program = "return `raise(E_VERBNF) ! ANY';";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_err(E_VERBNF));
    }

    #[test]
    fn test_try_except_stmt() {
        let program = "try a; except e (E_VARNF) return 666; endtry return 333;";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(666));
    }

    #[test]
    fn test_try_finally_stmt() {
        let program = "try a; finally return 666; endtry return 333;";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(666));
    }

    #[test]
    fn test_if_elseif_else_chain() {
        let program = r#"
            ret = {};
            for a in ({1,2,3})
                if (a == 1)
                    ret = {1, @ret};
                elseif (a == 2)
                    ret = {2, @ret};
                else
                    ret = {3, @ret};
                endif
            endfor
            return ret;
        "#;
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_list(vec![v_int(3), v_int(2), v_int(1)]));
    }

    #[test]
    fn test_if_elseif_elseif_chains() {
        let program = r#"
            if (1 == 2)
                return 5;
            elseif (2 == 3)
                return 3;
            elseif (3 == 4)
                return 4;
            else
                return 6;
            endif
        "#;
        let mut state = world_with_test_program(program);
        let mut vm = VM::new();
        call_verb(state.as_mut(), "test", &mut vm);
        let result = exec_vm(state.as_mut(), &mut vm);
        assert_eq!(result, v_int(6));
    }

    struct MockClientConnection {
        received: Vec<String>,
    }
    impl MockClientConnection {
        pub fn new() -> Self {
            Self { received: vec![] }
        }
    }
    #[async_trait]
    impl Sessions for MockClientConnection {
        async fn send_text(&mut self, _player: Objid, msg: String) -> Result<(), Error> {
            self.received.push(msg);
            Ok(())
        }

        async fn connected_players(&self) -> Result<Vec<Objid>, Error> {
            Ok(vec![])
        }
    }

    async fn exec_vm_with_mock_client_connection(
        vm: &mut VM,
        state: &mut dyn WorldState,
        client_connection: Arc<RwLock<MockClientConnection>>,
    ) -> Var {
        // Call repeatedly into exec until we ge either an error or Complete.
        loop {
            match vm.exec(state, client_connection.clone()).await {
                Ok(ExecutionResult::More) => continue,
                Ok(ExecutionResult::Complete(a)) => return a,
                Err(e) => panic!("error during execution: {:?}", e),
                Ok(ExecutionResult::Exception(e)) => {
                    panic!("MOO exception {:?}", e);
                }
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_call_builtin() {
        let program = "return notify(#1, \"test\");";
        let mut state = world_with_test_program(program);

        let mut vm = VM::new();

        call_verb(state.as_mut(), "test", &mut vm);

        let client_connection = Arc::new(RwLock::new(MockClientConnection::new()));
        let result =
            exec_vm_with_mock_client_connection(&mut vm, state.as_mut(), client_connection.clone())
                .await;
        assert_eq!(result, VAR_NONE);

        assert_eq!(
            client_connection.read().await.received,
            vec!["test".to_string()]
        );
    }
}
