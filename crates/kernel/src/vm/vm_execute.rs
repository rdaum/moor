// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use kanal::Sender;
use std::sync::Arc;
use std::time::Duration;

use moor_compiler::{Name, Offset};

use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::sessions::Session;
use crate::tasks::task_messages::SchedulerControlMsg;
use crate::tasks::{TaskId, VerbCall};
use moor_compiler::Program;
use moor_compiler::{Op, ScatterLabel};
use moor_values::model::VerbInfo;
use moor_values::model::WorldState;
use moor_values::var::Error::{E_ARGS, E_DIV, E_INVARG, E_MAXREC, E_RANGE, E_TYPE, E_VARNF};
use moor_values::var::Objid;
use moor_values::var::Variant;
use moor_values::var::{v_bool, v_empty_list, v_err, v_int, v_list, v_none, v_obj, v_objid, Var};
use moor_values::var::{v_listv, Error};

use crate::vm::activation::{Activation, HandlerType};
use crate::vm::vm_unwind::{FinallyReason, UncaughtException};
use crate::vm::{VMExecState, VM};

/// The set of parameters for a VM-requested fork.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Fork {
    /// The player. This is in the activation as well, but it's nicer to have it up here and
    /// explicit
    pub(crate) player: Objid,
    /// The permissions context for the forked task.
    pub(crate) progr: Objid,
    /// The task ID of the task that forked us
    pub(crate) parent_task_id: usize,
    /// The time to delay before starting the forked task, if any.
    pub(crate) delay: Option<Duration>,
    /// A copy of the activation record from the task that forked us.
    pub(crate) activation: Activation,
    /// The unique fork vector offset into the fork vector for the executing binary held in the
    /// activation record.  This is copied into the main vector and execution proceeds from there,
    /// instead.
    pub(crate) fork_vector_offset: Offset,
    /// The (optional) variable label where the task ID of the new task should be stored, in both
    /// the parent activation and the new task's activation.
    pub task_id: Option<Name>,
}

/// Represents the set of parameters passed to the VM for execution.
pub struct VmExecParams {
    pub scheduler_sender: Sender<(TaskId, SchedulerControlMsg)>,
    pub max_stack_depth: usize,
}
#[derive(Eq, PartialEq, Debug, Clone)]
pub enum ExecutionResult {
    /// Execution of this call stack is complete.
    Complete(Var),
    /// All is well. The task should let the VM continue executing.
    More,
    /// An exception was raised during execution.
    Exception(FinallyReason),
    /// Request dispatch to another verb
    ContinueVerb {
        /// The applicable permissions context.
        permissions: Objid,
        /// The requested verb.
        resolved_verb: VerbInfo,
        /// The call parameters that were used to resolve the verb.
        call: VerbCall,
        /// The parsed user command that led to this verb dispatch, if any.
        command: Option<ParsedCommand>,
        /// What to set the 'trampoline' to (if anything) when the verb returns.
        /// If this is set, the builtin function that issued this ContinueVerb will be re-called
        /// and the bf_trampoline argument on its activation record will be set to this value.
        /// This is usually used to drive a state machine through a series of actions on a builtin
        /// as it calls out to verbs.
        trampoline: Option<usize>,
        /// Likewise, along with the trampoline # above, this can be set with an optional argument
        /// that can be used to pass data back to the builtin function that issued this request.
        trampoline_arg: Option<Var>,
    },
    /// Request dispatch of a new task as a fork
    DispatchFork(Fork),
    /// Request dispatch of a builtin function with the given arguments.
    ContinueBuiltin {
        bf_func_num: usize,
        arguments: Vec<Var>,
    },
    /// Request that this task be suspended for a duration of time.
    /// This leads to the task performing a commit, being suspended for a delay, and then being
    /// resumed under a new transaction.
    /// If the duration is None, then the task is suspended indefinitely, until it is killed or
    /// resumed using `resume()` or `kill_task()`.
    Suspend(Option<Duration>),
    /// Request input from the client.
    NeedInput,
    /// Request `eval` execution, which is a kind of special activation creation where we've already
    /// been given the program to execute instead of having to look it up.
    PerformEval {
        /// The permissions context for the eval.
        permissions: Objid,
        /// The player who is performing the eval.
        player: Objid,
        /// The program to execute.
        program: Program,
    },
}

macro_rules! binary_bool_op {
    ( $f:ident, $op:tt ) => {
        let rhs = $f.pop();
        let lhs = $f.peek_top();
        let result = if lhs $op &rhs { 1 } else { 0 };
        $f.poke(0, v_int(result))
    };
}

macro_rules! binary_var_op {
    ( $vm:ident, $f:ident, $state:ident, $op:tt ) => {
        let rhs = $f.pop();
        let lhs = $f.peek_top();
        let result = lhs.$op(&rhs);
        match result {
            Ok(result) => $f.poke(0, result),
            Err(err_code) => {
                $f.pop();
                return $vm.push_error($state, err_code);
            }
        }
    };
}

#[inline]
pub(crate) fn one_to_zero_index(v: &Var) -> Result<usize, Error> {
    let Variant::Int(index) = v.variant() else {
        return Err(E_TYPE);
    };
    let index = index - 1;
    if index < 0 {
        return Err(E_RANGE);
    }
    Ok(index as usize)
}

impl VM {
    /// Main VM opcode execution. The actual meat of the machine.
    pub fn exec(
        &self,
        exec_params: &VmExecParams,
        state: &mut VMExecState,
        world_state: &mut dyn WorldState,
        session: Arc<dyn Session>,
    ) -> ExecutionResult {
        // Before executing, check stack depth...
        if state.stack.len() >= exec_params.max_stack_depth {
            // Absolutely raise-unwind an error here instead of just offering it as a potential
            // return value if this is a non-d verb. At least I think this the right thing to do?
            return self.throw_error(state, E_MAXREC);
        }

        // If the current activation frame is a builtin function, we need to jump back into it,
        // but increment the trampoline counter, as it means we're returning into it after
        // executing elsewhere. It will be up to the function to interpret the counter.
        // Functions that did not set a trampoline are assumed to be complete.
        if !state.stack.is_empty() && state.top().bf_index.is_some() {
            return self.reenter_builtin_function(state, exec_params, world_state, session);
        }

        // Try to consume & execute as many opcodes as we can without returning back to the task
        // scheduler, for efficiency reasons...

        let opcodes = state.top_mut().frame.program.main_vector.clone();

        while state.tick_count < state.tick_slice {
            state.tick_count += 1;

            // Borrow the top of the activation stack for the lifetime of this execution.
            let a = state.top_mut();
            let f = &mut a.frame;

            // Otherwise, start poppin' opcodes.
            // We panic here if we run out of opcodes, as that means there's a bug in either the
            // compiler or in opcode execution.
            let op = &opcodes[f.pc];
            f.pc += 1;

            match op {
                Op::If(label) | Op::Eif(label) | Op::IfQues(label) | Op::While(label) => {
                    let cond = f.pop();
                    if !cond.is_true() {
                        f.jump(label);
                    }
                }
                Op::Jump { label } => {
                    f.jump(label);
                }
                Op::WhileId { id, end_label } => {
                    let v = f.pop();
                    let is_true = v.is_true();
                    f.set_env(id, v);
                    if !is_true {
                        f.jump(end_label);
                    }
                }
                Op::ForList { end_label, id } => {
                    // Pop the count and list off the stack. We push back later when we re-enter.

                    let (count, list) = f.peek2();
                    let Variant::Int(count) = count.variant() else {
                        f.pop();
                        f.pop();

                        // If the result of raising error was just to push the value -- that is, we
                        // didn't 'throw' and unwind the stack -- we need to get out of the loop.
                        // So we preemptively jump (here and below for List) and then raise the error.
                        f.jump(end_label);
                        return self.raise_error(state, E_TYPE);
                    };
                    let count = *count as usize;
                    let Variant::List(l) = list.variant() else {
                        f.pop();
                        f.pop();

                        f.jump(end_label);
                        return self.raise_error(state, E_TYPE);
                    };

                    // If we've exhausted the list, pop the count and list and jump out.
                    if count >= l.len() {
                        f.pop();
                        f.pop();

                        f.jump(end_label);
                        continue;
                    }

                    // Track iteration count for range; set id to current list element for the count,
                    // then increment the count, rewind the program counter to the top of the loop, and
                    // continue.
                    f.set_env(id, l[count].clone());
                    f.poke(0, v_int((count + 1) as i64));
                }
                Op::ForRange { end_label, id } => {
                    // Pull the range ends off the stack.
                    let (from, next_val) = {
                        let (to, from) = f.peek2();

                        // TODO: LambdaMOO has special handling for MAXINT/MAXOBJ
                        //   Given we're 64-bit this is highly unlikely to ever be a concern for us, but
                        //   we also don't want to *crash* on obscene values, so impl that here.

                        let next_val = match (to.variant(), from.variant()) {
                            (Variant::Int(to_i), Variant::Int(from_i)) => {
                                if from_i > to_i {
                                    f.pop();
                                    f.pop();
                                    f.jump(end_label);

                                    continue;
                                }
                                v_int(from_i + 1)
                            }
                            (Variant::Obj(to_o), Variant::Obj(from_o)) => {
                                if from_o.0 > to_o.0 {
                                    f.pop();
                                    f.pop();
                                    f.jump(end_label);

                                    continue;
                                }
                                v_obj(from_o.0 + 1)
                            }
                            (_, _) => {
                                f.pop();
                                f.pop();
                                // Make sure we've jumped out of the loop before raising the error,
                                // because in verbs that aren't `d' we could end up continuing on in
                                // the loop (with a messed up stack) otherwise.
                                f.jump(end_label);

                                return self.raise_error(state, E_TYPE);
                            }
                        };
                        (from.clone(), next_val)
                    };
                    f.poke(1, next_val);
                    f.set_env(id, from);
                }
                Op::Pop => {
                    f.pop();
                }
                Op::ImmNone => {
                    f.push(v_none());
                }
                Op::ImmBigInt(val) => {
                    f.push(v_int(*val));
                }
                Op::ImmInt(val) => {
                    f.push(v_int(*val as i64));
                }
                Op::ImmObjid(val) => {
                    f.push(v_objid(*val));
                }
                Op::ImmErr(val) => {
                    f.push(v_err(*val));
                }
                Op::Imm(slot) => {
                    // TODO: it's questionable whether this optimization actually will be of much use
                    //   on a modern CPU as it could cause branch prediction misses. We should
                    //   benchmark this. its purpose is to avoid pointless stack ops for literals
                    //   that are never used (e.g. comments).
                    //   what might be better is an "optimization pass" that removes these prior to
                    //   execution, but then we'd have to cache them, etc. etc.
                    match f.lookahead() {
                        Some(Op::Pop) => {
                            // skip
                            f.skip();
                            continue;
                        }
                        _ => {
                            let value = &f.program.literals[slot.0 as usize];
                            f.push(value.clone());
                        }
                    }
                }
                Op::ImmEmptyList => f.push(v_empty_list()),
                Op::ListAddTail => {
                    let (tail, list) = (f.pop(), f.peek_top_mut());
                    let Variant::List(ref mut list) = list.variant_mut() else {
                        f.pop();
                        return self.push_error(state, E_TYPE);
                    };

                    // TODO: quota check SVO_MAX_LIST_CONCAT -> E_QUOTA
                    let result = list.push(tail);
                    f.poke(0, result);
                }
                Op::ListAppend => {
                    let (tail, list) = (f.pop(), f.peek_top_mut());

                    let Variant::List(list) = list.variant_mut() else {
                        f.pop();

                        return self.push_error(state, E_TYPE);
                    };

                    let Variant::List(tail) = tail.take_variant() else {
                        f.pop();

                        return self.push_error(state, E_TYPE);
                    };

                    // TODO: quota check SVO_MAX_LIST_CONCAT -> E_QUOTA
                    let new_list = list.append(tail);
                    f.poke(0, new_list);
                }
                Op::IndexSet => {
                    let (rhs, index, lhs) = (f.pop(), f.pop(), f.peek_top_mut());
                    let i = match one_to_zero_index(&index) {
                        Ok(i) => i,
                        Err(e) => {
                            f.pop();
                            return self.push_error(state, e);
                        }
                    };
                    match lhs.index_set(i, rhs) {
                        Ok(v) => {
                            f.poke(0, v);
                        }
                        Err(e) => {
                            f.pop();
                            return self.push_error(state, e);
                        }
                    }
                }
                Op::MakeSingletonList => {
                    let v = f.peek_top();
                    f.poke(0, v_list(&[v.clone()]));
                }
                Op::PutTemp => {
                    f.temp = f.peek_top().clone();
                }
                Op::PushTemp => {
                    let tmp = f.temp.clone();
                    f.push(tmp);
                    f.temp = v_none();
                }
                Op::Eq => {
                    binary_bool_op!(f, ==);
                }
                Op::Ne => {
                    binary_bool_op!(f, !=);
                }
                Op::Gt => {
                    binary_bool_op!(f, >);
                }
                Op::Lt => {
                    binary_bool_op!(f, <);
                }
                Op::Ge => {
                    binary_bool_op!(f, >=);
                }
                Op::Le => {
                    binary_bool_op!(f, <=);
                }
                Op::In => {
                    let (lhs, rhs) = (f.pop(), f.peek_top());
                    let r = lhs.index_in(rhs);
                    if let Variant::Err(e) = r.variant() {
                        f.pop();
                        return self.push_error(state, *e);
                    }
                    f.poke(0, r);
                }
                Op::Mul => {
                    binary_var_op!(self, f, state, mul);
                }
                Op::Sub => {
                    binary_var_op!(self, f, state, sub);
                }
                Op::Div => {
                    // Explicit division by zero check to raise E_DIV.
                    // Note that LambdaMOO consider 1/0.0 to be E_DIV, but Rust permits it, creating
                    // `inf`. I'll follow Rust's lead here, unless it leads to problems.
                    let divargs = f.peek_range(2);
                    if let Variant::Int(0) = divargs[1].variant() {
                        return self.push_error(state, E_DIV);
                    };
                    binary_var_op!(self, f, state, div);
                }
                Op::Add => {
                    binary_var_op!(self, f, state, add);
                }
                Op::Exp => {
                    binary_var_op!(self, f, state, pow);
                }
                Op::Mod => {
                    binary_var_op!(self, f, state, modulus);
                }
                Op::And(label) => {
                    let v = f.peek_top().is_true();
                    if !v {
                        f.jump(label)
                    } else {
                        f.pop();
                    }
                }
                Op::Or(label) => {
                    let v = f.peek_top().is_true();
                    if v {
                        f.jump(label);
                    } else {
                        f.pop();
                    }
                }
                Op::Not => {
                    let v = !f.peek_top().is_true();
                    f.poke(0, v_bool(v));
                }
                Op::UnaryMinus => {
                    let v = f.peek_top();
                    match v.negative() {
                        Err(e) => {
                            f.pop();
                            return self.push_error(state, e);
                        }
                        Ok(v) => f.poke(0, v),
                    }
                }
                Op::Push(ident) => {
                    let Some(v) = f.get_env(ident) else {
                        return self.push_error(state, E_VARNF);
                    };
                    f.push(v.clone());
                }
                Op::Put(ident) => {
                    let v = f.peek_top();
                    f.set_env(ident, v.clone());
                }
                Op::PushRef => {
                    let (index, list) = f.peek2();
                    let index = match one_to_zero_index(index) {
                        Ok(i) => i,
                        Err(e) => return self.push_error(state, e),
                    };
                    match list.index(index) {
                        Err(e) => return self.push_error(state, e),
                        Ok(v) => f.push(v),
                    }
                }
                Op::Ref => {
                    let (index, l) = (f.pop(), f.peek_top());
                    let index = match one_to_zero_index(&index) {
                        Ok(i) => i,
                        Err(e) => {
                            f.pop();
                            return self.push_error(state, e);
                        }
                    };

                    match l.index(index) {
                        Err(e) => {
                            f.pop();
                            return self.push_error(state, e);
                        }
                        Ok(v) => f.poke(0, v),
                    }
                }
                Op::RangeRef => {
                    let (to, from, base) = (f.pop(), f.pop(), f.peek_top());
                    match (to.variant(), from.variant()) {
                        (Variant::Int(to), Variant::Int(from)) => match base.range(*from, *to) {
                            Err(e) => {
                                f.pop();
                                return self.push_error(state, e);
                            }
                            Ok(v) => f.poke(0, v),
                        },
                        (_, _) => return self.push_error(state, E_TYPE),
                    };
                }
                Op::RangeSet => {
                    let (value, to, from, base) = (f.pop(), f.pop(), f.pop(), f.peek_top());
                    match (to.variant(), from.variant()) {
                        (Variant::Int(to), Variant::Int(from)) => {
                            match base.rangeset(value, *from, *to) {
                                Err(e) => {
                                    f.pop();
                                    return self.push_error(state, e);
                                }
                                Ok(v) => f.poke(0, v),
                            }
                        }
                        _ => {
                            f.pop();
                            return self.push_error(state, E_TYPE);
                        }
                    }
                }
                Op::GPut { id } => {
                    f.set_env(id, f.peek_top().clone());
                }
                Op::GPush { id } => {
                    let Some(v) = f.get_env(id) else {
                        return self.push_error(state, E_VARNF);
                    };
                    f.push(v.clone());
                }
                Op::Length(offset) => {
                    let v = f.peek_abs(offset.0 as usize);
                    match v.len() {
                        Ok(l) => f.push(l),
                        Err(e) => return self.push_error(state, e),
                    }
                }
                Op::GetProp => {
                    let (propname, obj) = (f.pop(), f.peek_top());

                    match self.resolve_property(
                        a.permissions,
                        world_state,
                        propname.clone(),
                        obj.clone(),
                    ) {
                        Ok(v) => f.poke(0, v),
                        Err(e) => {
                            f.pop();
                            return self.push_error(state, e);
                        }
                    }
                }
                Op::PushGetProp => {
                    let (propname, obj) = f.peek2();
                    match self.resolve_property(
                        a.permissions,
                        world_state,
                        propname.clone(),
                        obj.clone(),
                    ) {
                        Ok(v) => f.push(v),
                        Err(e) => return self.push_error(state, e),
                    }
                }
                Op::PutProp => {
                    let (rhs, propname, obj) = (f.pop(), f.pop(), f.peek_top());
                    match self.set_property(
                        a.permissions,
                        world_state,
                        propname.clone(),
                        obj.clone(),
                        rhs.clone(),
                    ) {
                        Ok(v) => f.poke(0, v),
                        Err(e) => {
                            f.pop();
                            return self.push_error(state, e);
                        }
                    }
                }
                Op::Fork { id, fv_offset } => {
                    // Delay time should be on stack
                    let time = f.pop();
                    let Variant::Int(time) = time.variant() else {
                        return self.push_error(state, E_TYPE);
                    };

                    if *time < 0 {
                        return self.push_error(state, E_INVARG);
                    }
                    let delay = (*time != 0).then(|| Duration::from_secs(*time as u64));
                    let new_activation = a.clone();
                    let fork = Fork {
                        player: a.player,
                        progr: a.permissions,
                        parent_task_id: state.task_id,
                        delay,
                        activation: new_activation,
                        fork_vector_offset: *fv_offset,
                        task_id: *id,
                    };
                    return ExecutionResult::DispatchFork(fork);
                }
                Op::Pass => {
                    let args = f.pop();
                    let Variant::List(args) = args.variant() else {
                        return self.push_error(state, E_TYPE);
                    };
                    return self.prepare_pass_verb(state, world_state, &args[..]);
                }
                Op::CallVerb => {
                    let (args, verb, obj) = (f.pop(), f.pop(), f.pop());
                    let (args, verb, obj) = match (args.variant(), verb.variant(), obj.variant()) {
                        (Variant::List(l), Variant::Str(s), Variant::Obj(o)) => (l, s, o),
                        _ => {
                            return self.push_error(state, E_TYPE);
                        }
                    };
                    return self.prepare_call_verb(
                        state,
                        world_state,
                        *obj,
                        verb.as_str(),
                        &args[..],
                    );
                }
                Op::Return => {
                    let ret_val = f.pop();
                    return self.unwind_stack(state, FinallyReason::Return(ret_val));
                }
                Op::Return0 => {
                    return self.unwind_stack(state, FinallyReason::Return(v_int(0)));
                }
                Op::Done => {
                    return self.unwind_stack(state, FinallyReason::Return(v_none()));
                }
                Op::FuncCall { id } => {
                    // Pop arguments, should be a list.
                    let args = f.pop();
                    let Variant::List(args) = args.variant() else {
                        return self.push_error(state, E_ARGS);
                    };
                    return self.call_builtin_function(
                        state,
                        id.0 as usize,
                        &args[..],
                        exec_params,
                        world_state,
                        session,
                    );
                }
                Op::PushLabel(label) => {
                    f.push_handler_label(HandlerType::CatchLabel(*label));
                }
                Op::TryFinally(label) => {
                    f.push_handler_label(HandlerType::Finally(*label));
                }
                Op::Catch(_) => {
                    f.push_handler_label(HandlerType::Catch(1));
                }
                Op::TryExcept { num_excepts } => {
                    f.push_handler_label(HandlerType::Catch(*num_excepts));
                }
                Op::EndCatch(label) | Op::EndExcept(label) => {
                    let is_catch = matches!(op, Op::EndCatch(_));
                    let v = if is_catch { f.pop() } else { v_none() };

                    let handler = f
                        .pop_applicable_handler()
                        .expect("Missing handler for try/catch/except");
                    let HandlerType::Catch(num_excepts) = handler.handler_type else {
                        panic!("Handler is not a catch handler");
                    };

                    for _i in 0..num_excepts {
                        f.pop(); /* code list */
                        f.handler_stack.pop();
                    }
                    if is_catch {
                        f.push(v);
                    }
                    f.jump(label);
                }
                Op::EndFinally => {
                    let Some(finally_handler) = f.pop_applicable_handler() else {
                        panic!("Missing handler for try/finally")
                    };
                    let HandlerType::Finally(_) = finally_handler.handler_type else {
                        panic!("Handler is not a finally handler")
                    };
                    f.push(v_int(0) /* fallthrough */);
                    f.push(v_int(0));
                }
                Op::Continue => {
                    let why = f.pop();
                    let Variant::Int(why) = why.variant() else {
                        panic!("'why' is not an integer representing a FinallyReason");
                    };
                    let why = FinallyReason::from_code(*why as usize);
                    match why {
                        FinallyReason::Fallthrough => {
                            // Do nothing, normal case.
                            continue;
                        }
                        FinallyReason::Raise { .. }
                        | FinallyReason::Uncaught(UncaughtException { .. })
                        | FinallyReason::Return(_)
                        | FinallyReason::Exit { .. } => {
                            return self.unwind_stack(state, why);
                        }
                        FinallyReason::Abort => {
                            panic!("Unexpected FINALLY_ABORT in Continue")
                        }
                    }
                }
                Op::ExitId(label) => {
                    f.jump(label);
                    continue;
                }
                Op::Exit { stack, label } => {
                    return self.unwind_stack(
                        state,
                        FinallyReason::Exit {
                            stack: *stack,
                            label: *label,
                        },
                    );
                }
                Op::Scatter(sa) => {
                    let have_rest = sa.rest <= sa.nargs;
                    let rhs_values = {
                        let rhs = f.peek_top();
                        let Variant::List(rhs_values) = rhs.variant() else {
                            f.pop();
                            return self.push_error(state, E_TYPE);
                        };
                        rhs_values.clone()
                    };

                    let len = rhs_values.len();
                    if len < sa.nreq || !have_rest && len > sa.nargs {
                        f.pop();
                        return self.push_error(state, E_ARGS);
                    }

                    assert_eq!(sa.nargs, sa.labels.len());
                    let mut nopt_avail = len - sa.nreq;
                    let nrest = if have_rest && len >= sa.nargs {
                        len - sa.nargs + 1
                    } else {
                        0
                    };
                    let mut jump_where = None;
                    let mut args_iter = rhs_values.iter();

                    for label in sa.labels.iter() {
                        match label {
                            ScatterLabel::Rest(id) => {
                                let mut v = vec![];
                                for _ in 0..nrest {
                                    let Some(rest) = args_iter.next() else {
                                        break;
                                    };
                                    v.push(rest.clone());
                                }
                                let rest = v_listv(v);
                                f.set_env(id, rest);
                            }
                            ScatterLabel::Required(id) => {
                                let Some(arg) = args_iter.next() else {
                                    return self.push_error(state, E_ARGS);
                                };

                                f.set_env(id, arg.clone());
                            }
                            ScatterLabel::Optional(id, jump_to) => {
                                if nopt_avail > 0 {
                                    nopt_avail -= 1;
                                    let Some(arg) = args_iter.next() else {
                                        return self.push_error(state, E_ARGS);
                                    };
                                    f.set_env(id, arg.clone());
                                } else if jump_where.is_none() && jump_to.is_some() {
                                    jump_where = *jump_to;
                                }
                            }
                        }
                    }
                    match &jump_where {
                        None => f.jump(&sa.done),
                        Some(jump_where) => f.jump(jump_where),
                    }
                }
                Op::CheckListForSplice => {
                    let Variant::List(_) = f.peek_top().variant() else {
                        f.pop();
                        return self.push_error(state, E_TYPE);
                    };
                }
            }
        }
        // We don't usually get here because most execution paths return before we hit the end of
        // the loop. But if we do, we need to return More so the scheduler knows to keep feeding
        // us.
        ExecutionResult::More
    }
}
