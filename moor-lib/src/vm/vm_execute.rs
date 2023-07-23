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
