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

use std::ops::Add;
use std::sync::Arc;
use std::time::Duration;

use crate::tasks::sessions::Session;
use crate::vm::activation::Frame;
use crate::vm::moo_frame::{CatchType, ScopeType};
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::{ExecutionResult, Fork, VMExecState, VmExecParams};
use moor_compiler::{to_literal, Op, ScatterLabel};
use moor_values::model::WorldState;

use moor_values::Error::{E_ARGS, E_DIV, E_INVARG, E_INVIND, E_TYPE, E_VARNF};
use moor_values::{
    v_bool, v_empty_list, v_empty_map, v_err, v_float, v_flyweight, v_int, v_list, v_map, v_none,
    v_obj, v_str, Error, IndexMode, Obj, Sequence, Str, Var, Variant,
};
use moor_values::{Symbol, VarType};

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
                return $state.push_error(err_code);
            }
        }
    };
}

/// Main VM opcode execution for MOO stack frames. The actual meat of the MOO virtual machine.
pub fn moo_frame_execute(
    exec_params: &VmExecParams,
    state: &mut VMExecState,
    world_state: &mut dyn WorldState,
    session: Arc<dyn Session>,
) -> ExecutionResult {
    let opcodes = {
        // Check the frame type to verify it's MOO, before doing anything else
        let a = state.top_mut();
        let Frame::Moo(ref mut f) = a.frame else {
            panic!("Unsupported VM stack frame type");
        };

        // We clone the main vector here to avoid borrowing issues with the frame later, as we
        // need to modify the program counter.
        f.program.main_vector.clone()
    };

    // Special case for empty opcodes set, just return v_none() immediately.
    if opcodes.is_empty() {
        return ExecutionResult::Complete(v_none());
    }

    // The per-execution slice count. This is used to limit the amount of work we do in a single
    // execution slice for this task.
    // We should not execute more than `tick_slice` in a single VM instruction fetch/execute
    // run. This is to allow us to be responsive to the task scheduler.
    // Note this is not the same as the total amount of ticks aportioned to the task -- that's
    // `max_ticks` on the task itself.
    // For clarity, to avoid regressions again:
    // `tick_count` tracks the total task execution, `task_slice` the maximum current _slice_
    //   and the variable `tick_slice_count` that slice's progress.
    //  `max_ticks` on the task is the total limit which is checked above us, outside this loop.
    let mut tick_slice_count = 0;
    while tick_slice_count < state.tick_slice {
        tick_slice_count += 1;
        state.tick_count += 1;

        // Borrow the top of the activation stack for the lifetime of this execution.
        let a = state.top_mut();
        let Frame::Moo(ref mut f) = a.frame else {
            panic!("Unsupported VM stack frame type");
        };

        // Otherwise, start poppin' opcodes.
        // We panic here if we run out of opcodes, as that means there's a bug in either the
        // compiler or in opcode execution.
        let op = &opcodes[f.pc];
        f.pc += 1;

        match op {
            Op::If(label, environment_width)
            | Op::Eif(label, environment_width)
            | Op::While {
                jump_label: label,
                environment_width,
            } => {
                let scope_type = match op {
                    Op::If(..) | Op::Eif(..) => ScopeType::If,
                    Op::While { .. } => ScopeType::While,
                    _ => unreachable!(),
                };
                f.push_scope(scope_type, *environment_width, label);
                let cond = f.pop();
                if !cond.is_true() {
                    f.jump(label);
                }
            }
            Op::IfQues(label) => {
                let cond = f.pop();
                if !cond.is_true() {
                    f.jump(label);
                }
            }
            Op::Jump { label } => {
                f.jump(label);
            }
            Op::WhileId {
                id,
                end_label,
                environment_width,
            } => {
                f.push_scope(ScopeType::While, *environment_width, end_label);
                let v = f.pop();
                let is_true = v.is_true();
                f.set_env(id, v);
                if !is_true {
                    f.jump(end_label);
                }
            }
            Op::ForList {
                end_label,
                id,
                environment_width,
            } => {
                f.push_scope(ScopeType::For, *environment_width, end_label);

                // Pop the count and list off the stack. We push back later when we re-enter.

                let (count, list) = f.peek2();
                let Variant::Int(count) = count.variant() else {
                    f.pop();
                    f.pop();

                    // If the result of raising error was just to push the value -- that is, we
                    // didn't 'throw' and unwind the stack -- we need to get out of the loop.
                    // So we preemptively jump (here and below for List) and then raise the error.
                    f.jump(end_label);
                    return state.raise_error(E_TYPE);
                };
                let count = *count as usize;
                let Variant::List(l) = list.variant() else {
                    f.pop();
                    f.pop();

                    f.jump(end_label);
                    return state.raise_error(E_TYPE);
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
                f.set_env(id, l.index(count).unwrap().clone());
                f.poke(0, v_int((count + 1) as i64));
            }
            Op::ForRange {
                end_label,
                id,
                environment_width,
            } => {
                f.push_scope(ScopeType::For, *environment_width, end_label);

                // Pull the range ends off the stack.
                let (from, next_val) = {
                    let (to, from) = f.peek2();

                    // TODO: Handling for MAXINT/MAXOBJ in various opcodes
                    //   Given we're 64-bit this is highly unlikely to ever be a concern for us, but
                    //   we also don't want to *crash* on obscene common, so impl that here.

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
                            if from_o > to_o {
                                f.pop();
                                f.pop();
                                f.jump(end_label);

                                continue;
                            }
                            v_obj(from_o.clone().add(Obj::mk_id(1)))
                        }
                        (_, _) => {
                            f.pop();
                            f.pop();
                            // Make sure we've jumped out of the loop before raising the error,
                            // because in verbs that aren't `d' we could end up continuing on in
                            // the loop (with a messed up stack) otherwise.
                            f.jump(end_label);

                            return state.raise_error(E_TYPE);
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
            Op::ImmFloat(val) => {
                f.push(v_float(*val));
            }
            Op::ImmInt(val) => {
                f.push(v_int(*val as i64));
            }
            Op::ImmObjid(val) => {
                f.push(v_obj(val.clone()));
            }
            Op::ImmErr(val) => {
                f.push(v_err(*val));
            }
            Op::Imm(slot) => {
                // it's questionable whether this optimization actually will be of much use
                // on a modern CPU as it could cause branch prediction misses. We should
                // benchmark this. its purpose is to avoid pointless stack ops for literals
                // that are never used (e.g. comments).
                // what might be better is an "optimization pass" that removes these prior to
                // execution, but then we'd have to cache them, etc. etc.
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
                if !list.is_sequence() || list.type_code() == VarType::TYPE_STR {
                    f.pop();
                    return state.push_error(E_TYPE);
                }
                // TODO: quota check SVO_MAX_LIST_CONCAT -> E_QUOTA in list add and append
                let result = list.push(&tail);
                match result {
                    Ok(v) => {
                        f.poke(0, v);
                    }
                    Err(e) => {
                        f.pop();
                        return state.push_error(e);
                    }
                }
            }
            Op::ListAppend => {
                let (tail, list) = (f.pop(), f.peek_top_mut());

                // Don't allow strings here.
                if list.type_code() == VarType::TYPE_STR {
                    f.pop();
                    return state.push_error(E_TYPE);
                }

                if !tail.is_sequence() || !list.is_sequence() {
                    f.pop();
                    return state.push_error(E_TYPE);
                }
                let new_list = list.append(&tail);
                match new_list {
                    Ok(v) => {
                        f.poke(0, v);
                    }
                    Err(e) => {
                        f.pop();
                        return state.push_error(e);
                    }
                }
            }
            Op::IndexSet => {
                let (rhs, index, lhs) = (f.pop(), f.pop(), f.peek_top_mut());
                let result = lhs.index_set(&index, &rhs, IndexMode::OneBased);
                match result {
                    Ok(v) => {
                        f.poke(0, v);
                    }
                    Err(e) => {
                        f.pop();
                        return state.push_error(e);
                    }
                }
            }
            Op::MakeSingletonList => {
                let v = f.peek_top();
                f.poke(0, v_list(&[v.clone()]));
            }
            Op::MakeMap => {
                f.push(v_empty_map());
            }
            Op::MapInsert => {
                let (value, key, map) = (f.pop(), f.pop(), f.peek_top_mut());
                let result = map.index_set(&key, &value, IndexMode::OneBased);
                match result {
                    Ok(v) => {
                        f.poke(0, v);
                    }
                    Err(e) => {
                        f.pop();
                        return state.push_error(e);
                    }
                }
            }
            Op::MakeFlyweight(num_slots) => {
                // Stack should be: contents, slots, delegate
                let contents = f.pop();
                // Contents must be a list
                let Variant::List(contents) = contents.variant() else {
                    return state.push_error(E_TYPE);
                };
                let mut slots = Vec::with_capacity(*num_slots);
                for _ in 0..*num_slots {
                    let (k, v) = (f.pop(), f.pop());
                    let Variant::Str(k) = k.variant() else {
                        return state.push_error(E_TYPE);
                    };
                    let sym = Symbol::mk_case_insensitive(k.as_string());
                    slots.push((sym, v));
                }
                let delegate = f.pop();
                let Variant::Obj(delegate) = delegate.variant() else {
                    return state.push_error(E_TYPE);
                };
                // Slots should be v_str -> value, num_slots times

                let flyweight = v_flyweight(delegate.clone(), &slots, contents.clone(), None);
                f.push(flyweight);
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
                let r = lhs.index_in(rhs, false, IndexMode::OneBased);
                match r {
                    Ok(v) => {
                        f.poke(0, v);
                    }
                    Err(e) => {
                        f.pop();
                        return state.push_error(e);
                    }
                }
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
                // `inf`.
                let divargs = f.peek_range(2);
                if matches!(divargs[1].variant(), Variant::Int(0) | Variant::Float(0.0)) {
                    return state.push_error(E_DIV);
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
                let divargs = f.peek_range(2);
                if matches!(divargs[1].variant(), Variant::Int(0) | Variant::Float(0.0)) {
                    return state.push_error(E_DIV);
                };
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
                        return state.push_error(e);
                    }
                    Ok(v) => f.poke(0, v),
                }
            }
            Op::Push(ident) => {
                let Some(v) = f.get_env(ident) else {
                    return state.push_error(E_VARNF);
                };
                f.push(v.clone());
            }
            Op::Put(ident) => {
                let v = f.peek_top();
                f.set_env(ident, v.clone());
            }
            Op::PushRef => {
                let (index, value) = f.peek2();
                let result = value.index(index, IndexMode::OneBased);
                match result {
                    Ok(v) => f.push(v),
                    Err(e) => {
                        f.pop();
                        return state.push_error(e);
                    }
                }
            }
            Op::Ref => {
                let (index, value) = (f.pop(), f.peek_top());

                let result = value.index(&index, IndexMode::OneBased);
                match result {
                    Ok(v) => f.poke(0, v),
                    Err(e) => {
                        f.pop();
                        return state.push_error(e);
                    }
                }
            }
            Op::RangeRef => {
                let (to, from, base) = (f.pop(), f.pop(), f.peek_top());
                let result = base.range(&from, &to, IndexMode::OneBased);
                if let Err(e) = result {
                    f.pop();
                    return state.push_error(e);
                }
                f.poke(0, result.unwrap());
            }
            Op::RangeSet => {
                let (value, to, from, base) = (f.pop(), f.pop(), f.pop(), f.peek_top());
                let result = base.range_set(&from, &to, &value, IndexMode::OneBased);
                if let Err(e) = result {
                    f.pop();
                    return state.push_error(e);
                }
                f.poke(0, result.unwrap());
            }
            Op::Length(offset) => {
                let v = f.peek_abs(offset.0 as usize);
                match v.len() {
                    Ok(l) => f.push(v_int(l as i64)),
                    Err(e) => return state.push_error(e),
                }
            }
            Op::GetProp => {
                let (propname, obj) = (f.pop(), f.peek_top());

                let Variant::Str(propname) = propname.variant() else {
                    return state.push_error(E_TYPE);
                };

                let value = get_property(world_state, &a.permissions, obj, propname);
                match value {
                    Ok(v) => {
                        f.poke(0, v);
                    }
                    Err(e) => {
                        return state.push_error(e);
                    }
                }
            }
            Op::PushGetProp => {
                let (propname, obj) = f.peek2();

                let Variant::Str(propname) = propname.variant() else {
                    return state.push_error(E_TYPE);
                };

                let value = get_property(world_state, &a.permissions, obj, propname);
                match value {
                    Ok(v) => {
                        f.push(v);
                    }
                    Err(e) => {
                        return state.push_error(e);
                    }
                }
            }
            Op::PutProp => {
                let (rhs, propname, obj) = (f.pop(), f.pop(), f.peek_top());

                let (propname, obj) = match (propname.variant(), obj.variant()) {
                    (Variant::Str(propname), Variant::Obj(obj)) => (propname, obj),
                    (_, _) => {
                        return state.push_error(E_TYPE);
                    }
                };

                let propname = Symbol::mk_case_insensitive(propname.as_string());
                let update_result =
                    world_state.update_property(&a.permissions, obj, propname, &rhs.clone());

                match update_result {
                    Ok(()) => {
                        f.poke(0, rhs);
                    }
                    Err(e) => {
                        return state.push_error(e.to_error_code());
                    }
                }
            }
            Op::Fork { id, fv_offset } => {
                // Delay time should be on stack
                let time = f.pop();

                let time = match time.variant() {
                    Variant::Int(time) => *time as f64,
                    Variant::Float(time) => *time,
                    _ => {
                        return state.push_error(E_TYPE);
                    }
                };

                if time < 0.0 {
                    return state.push_error(E_INVARG);
                }
                let delay = (time != 0.0).then(|| Duration::from_secs_f64(time));
                let new_activation = a.clone();
                let fork = Fork {
                    player: a.player.clone(),
                    progr: a.permissions.clone(),
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
                    return state.push_error(E_TYPE);
                };
                return state.prepare_pass_verb(world_state, args);
            }
            Op::CallVerb => {
                let (args, verb, obj) = (f.pop(), f.pop(), f.pop());
                let (Variant::List(l), Variant::Str(s)) = (args.variant(), verb.variant()) else {
                    return state.push_error(E_TYPE);
                };
                let verb = Symbol::mk_case_insensitive(s.as_string());
                let result = state.verb_dispatch(exec_params, world_state, obj, verb, l.clone());
                match result {
                    Ok(r) => return r,
                    Err(e) => {
                        return state.push_error(e);
                    }
                }
            }
            Op::Return => {
                let ret_val = f.pop();
                return state.unwind_stack(FinallyReason::Return(ret_val));
            }
            Op::Return0 => {
                return state.unwind_stack(FinallyReason::Return(v_int(0)));
            }
            Op::Done => {
                return state.unwind_stack(FinallyReason::Return(v_none()));
            }
            Op::FuncCall { id } => {
                // Pop arguments, should be a list.
                let args = f.pop();
                let Variant::List(args) = args.variant() else {
                    return state.push_error(E_ARGS);
                };
                return state.call_builtin_function(
                    *id,
                    args.iter().collect(),
                    exec_params,
                    world_state,
                    session,
                );
            }
            Op::PushCatchLabel(label) => {
                // Get the error codes, which is either a list of error codes or Any.
                let error_codes = f.pop().clone();

                // The scope above us has to be a TryCatch, and we need to push into that scope
                // the code list that we're going to execute.
                match error_codes.variant() {
                    Variant::List(error_codes) => {
                        let error_codes = error_codes.iter().map(|v| {
                            let Variant::Err(e) = v.variant() else {
                                panic!("Error codes list contains non-error code");
                            };
                            *e
                        });
                        f.catch_stack
                            .push((CatchType::Errors(error_codes.into_iter().collect()), *label));
                    }
                    Variant::Int(0) => {
                        f.catch_stack.push((CatchType::Any, *label));
                    }
                    _ => {
                        panic!("Invalid error codes list");
                    }
                }
            }
            Op::TryFinally {
                end_label,
                environment_width,
            } => {
                // Next opcode must be BeginScope, to define the variable scoping.
                f.push_scope(
                    ScopeType::TryFinally(*end_label),
                    *environment_width,
                    end_label,
                );
            }
            Op::TryCatch { end_label, .. } => {
                let catches = std::mem::take(&mut f.catch_stack);
                f.push_scope(ScopeType::TryCatch(catches), 0, end_label);
            }
            Op::TryExcept {
                environment_width,
                end_label,
                ..
            } => {
                let catches = std::mem::take(&mut f.catch_stack);
                f.push_scope(ScopeType::TryCatch(catches), *environment_width, end_label);
            }
            Op::EndCatch(label) | Op::EndExcept(label) => {
                let is_catch = matches!(op, Op::EndCatch(_));
                let v = if is_catch { f.pop() } else { v_none() };

                let handler = f.pop_scope().expect("Missing handler for try/catch/except");
                let ScopeType::TryCatch(..) = handler.scope_type else {
                    panic!(
                        "Handler is not a catch handler; {}:{} line {}",
                        to_literal(&a.this),
                        a.verb_name,
                        f.find_line_no(f.pc - 1).unwrap()
                    );
                };

                if is_catch {
                    f.push(v);
                }
                f.jump(label);
            }
            Op::EndFinally => {
                // Execution of the block completed successfully, so we can just continue with
                // fall-through into the FinallyContinue block
                f.finally_stack.push(FinallyReason::Fallthrough);
            }
            //
            Op::FinallyContinue => {
                let why = f.finally_stack.pop().expect("Missing finally reason");
                match why {
                    FinallyReason::Fallthrough => continue,
                    FinallyReason::Abort => {
                        panic!("Unexpected FINALLY_ABORT in FinallyContinue")
                    }
                    FinallyReason::Raise(_)
                    | FinallyReason::Return(_)
                    | FinallyReason::Exit { .. } => {
                        return state.unwind_stack(why);
                    }
                }
            }
            Op::BeginScope {
                num_bindings,
                end_label,
            } => {
                f.push_scope(ScopeType::Block, *num_bindings, end_label);
            }
            Op::EndScope { num_bindings: _ } => {
                f.pop_scope().expect("Missing scope");
            }
            Op::ExitId(label) => {
                f.jump(label);
                continue;
            }
            Op::Exit { stack, label } => {
                return state.unwind_stack(FinallyReason::Exit {
                    stack: *stack,
                    label: *label,
                });
            }
            Op::Scatter(sa) => {
                // TODO: this could do with some attention. a lot of the complexity here has to
                //   do with translating fairly directly from the lambdamoo sources.
                let (nargs, rest, nreq) = {
                    let mut nargs = 0;
                    let mut rest = 0;
                    let mut nreq = 0;
                    for label in sa.labels.iter() {
                        match label {
                            ScatterLabel::Rest(_) => rest += 1,
                            ScatterLabel::Required(_) => nreq += 1,
                            ScatterLabel::Optional(_, _) => {}
                        }
                        nargs += 1;
                    }
                    (nargs, rest, nreq)
                };
                // TODO: ?
                let have_rest = rest <= nargs;
                let rhs_values = {
                    let rhs = f.peek_top();
                    let Variant::List(rhs_values) = rhs.variant() else {
                        f.pop();
                        return state.push_error(E_TYPE);
                    };
                    rhs_values.clone()
                };

                let len = rhs_values.len();
                if len < nreq || !have_rest && len > nargs {
                    f.pop();
                    return state.push_error(E_ARGS);
                }
                let mut nopt_avail = len - nreq;
                let nrest = if have_rest && len >= nargs {
                    len - nargs + 1
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
                            let rest = v_list(&v);
                            f.set_env(id, rest);
                        }
                        ScatterLabel::Required(id) => {
                            let Some(arg) = args_iter.next() else {
                                return state.push_error(E_ARGS);
                            };

                            f.set_env(id, arg.clone());
                        }
                        ScatterLabel::Optional(id, jump_to) => {
                            if nopt_avail > 0 {
                                nopt_avail -= 1;
                                let Some(arg) = args_iter.next() else {
                                    return state.push_error(E_ARGS);
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
                if !f.peek_top().is_sequence() {
                    f.pop();
                    return state.push_error(E_TYPE);
                }
            }
        }
    }
    // We don't usually get here because most execution paths return before we hit the end of
    // the loop. But if we do, we need to return More so the scheduler knows to keep feeding
    // us.
    ExecutionResult::More
}

fn get_property(
    world_state: &mut dyn WorldState,
    permissions: &Obj,
    obj: &Var,
    propname: &Str,
) -> Result<Var, Error> {
    match obj.variant() {
        Variant::Obj(obj) => {
            let propname = Symbol::mk_case_insensitive(propname.as_string());
            let result = world_state.retrieve_property(permissions, obj, propname);
            match result {
                Ok(v) => Ok(v),
                Err(e) => Err(e.to_error_code()),
            }
        }
        Variant::Flyweight(flyweight) => {
            let propname = Symbol::mk_case_insensitive(propname.as_string());

            // If propname is `delegate`, return the delegate object.
            // If the propname is `slots`, return the slots list.
            // Otherwise, return the value from the slots list.
            let value = match propname.as_str() {
                "delegate" => v_obj(flyweight.delegate().clone()),
                "slots" => {
                    let slots: Vec<_> = flyweight
                        .slots()
                        .iter()
                        .map(|(k, v)| (v_str(k.as_str()), v.clone()))
                        .collect();

                    v_map(&slots)
                }
                _ => {
                    if let Some(result) = flyweight.get_slot(&propname) {
                        result.clone()
                    } else {
                        // Now check the delegate
                        let delegate = flyweight.delegate();
                        let result = world_state.retrieve_property(permissions, delegate, propname);
                        match result {
                            Ok(v) => v,
                            Err(e) => return Err(e.to_error_code()),
                        }
                    }
                }
            };
            Ok(value)
        }
        _ => Err(E_INVIND),
    }
}
