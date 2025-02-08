// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::vm::moo_frame::{CatchType, MooStackFrame, ScopeType};
use crate::vm::vm_unwind::FinallyReason;
use crate::vm::ExecutionResult;
use lazy_static::lazy_static;
use moor_compiler::{Op, ScatterLabel};
use moor_values::model::WorldState;
use std::ops::Add;
use std::time::Duration;

use crate::config::FeaturesConfig;
use moor_values::Error::{E_ARGS, E_DIV, E_INVARG, E_INVIND, E_RANGE, E_TYPE, E_VARNF};
use moor_values::{
    v_bool_int, v_empty_list, v_empty_map, v_err, v_float, v_flyweight, v_int, v_list, v_map,
    v_none, v_obj, v_str, v_sym, Error, IndexMode, Obj, Sequence, Var, Variant,
};
use moor_values::{Symbol, VarType};

lazy_static! {
    static ref DELEGATE_SYM: Symbol = Symbol::mk("delegate");
    static ref SLOTS_SYM: Symbol = Symbol::mk("slots");
}

macro_rules! binary_bool_op {
    ( $f:ident, $op:tt, $bi:expr ) => {
        let rhs = $f.pop();
        let lhs = $f.peek_top();
        let bres : bool = *lhs $op rhs;
        let result = {
            if $bi {
                Var::mk_bool(bres)
            } else {
                v_bool_int(bres)
            }
        };
        $f.poke(0, result)
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
                return ExecutionResult::PushError(err_code);
            }
        }
    };
}

/// Main VM opcode execution for MOO stack frames. The actual meat of the MOO virtual machine.
pub fn moo_frame_execute(
    tick_slice: usize,
    tick_count: &mut usize,
    permissions: Obj,
    f: &mut MooStackFrame,
    world_state: &mut dyn WorldState,
    features_config: &FeaturesConfig,
) -> ExecutionResult {
    // To avoid borrowing issues when mutating the frame elsewhere...
    let opcodes = f.program.main_vector.clone();

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
    while tick_slice_count < tick_slice {
        tick_slice_count += 1;
        *tick_count += 1;

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
                    return ExecutionResult::RaiseError(E_TYPE);
                };
                let count = *count as usize;
                let Variant::List(l) = list.variant() else {
                    f.pop();
                    f.pop();

                    f.jump(end_label);
                    return ExecutionResult::RaiseError(E_TYPE);
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

                            return ExecutionResult::RaiseError(E_TYPE);
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
                    return ExecutionResult::PushError(E_TYPE);
                }
                // TODO: quota check SVO_MAX_LIST_CONCAT -> E_QUOTA in list add and append
                let result = list.push(&tail);
                match result {
                    Ok(v) => {
                        f.poke(0, v);
                    }
                    Err(e) => {
                        f.pop();
                        return ExecutionResult::PushError(e);
                    }
                }
            }
            Op::ListAppend => {
                let (tail, list) = (f.pop(), f.peek_top_mut());

                // Don't allow strings here.
                if list.type_code() == VarType::TYPE_STR {
                    f.pop();
                    return ExecutionResult::PushError(E_TYPE);
                }

                if !tail.is_sequence() || !list.is_sequence() {
                    f.pop();
                    return ExecutionResult::PushError(E_TYPE);
                }
                let new_list = list.append(&tail);
                match new_list {
                    Ok(v) => {
                        f.poke(0, v);
                    }
                    Err(e) => {
                        f.pop();
                        return ExecutionResult::PushError(e);
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
                        return ExecutionResult::PushError(e);
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
                        return ExecutionResult::PushError(e);
                    }
                }
            }
            Op::MakeFlyweight(num_slots) => {
                // Stack should be: contents, slots, delegate
                let contents = f.pop();
                // Contents must be a list
                let Variant::List(contents) = contents.variant() else {
                    return ExecutionResult::PushError(E_TYPE);
                };
                let mut slots = Vec::with_capacity(*num_slots);
                for _ in 0..*num_slots {
                    let (k, v) = (f.pop(), f.pop());
                    let Ok(sym) = k.as_symbol() else {
                        return ExecutionResult::PushError(E_TYPE);
                    };
                    slots.push((sym, v));
                }
                let delegate = f.pop();
                let Variant::Obj(delegate) = delegate.variant() else {
                    return ExecutionResult::PushError(E_TYPE);
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
                binary_bool_op!(f, ==, features_config.use_boolean_returns);
            }
            Op::Ne => {
                binary_bool_op!(f, !=, features_config.use_boolean_returns);
            }
            Op::Gt => {
                binary_bool_op!(f, >, features_config.use_boolean_returns);
            }
            Op::Lt => {
                binary_bool_op!(f, <, features_config.use_boolean_returns);
            }
            Op::Ge => {
                binary_bool_op!(f, >=, features_config.use_boolean_returns);
            }
            Op::Le => {
                binary_bool_op!(f, <=, features_config.use_boolean_returns);
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
                        return ExecutionResult::PushError(e);
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
                    return ExecutionResult::PushError(E_DIV);
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
                    return ExecutionResult::PushError(E_DIV);
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
                let b = if features_config.use_boolean_returns {
                    Var::mk_bool(v)
                } else {
                    v_bool_int(v)
                };
                f.poke(0, b);
            }
            Op::UnaryMinus => {
                let v = f.peek_top();
                match v.negative() {
                    Err(e) => {
                        f.pop();
                        return ExecutionResult::PushError(e);
                    }
                    Ok(v) => f.poke(0, v),
                }
            }
            Op::Push(ident) => {
                let Some(v) = f.get_env(ident) else {
                    return ExecutionResult::PushError(E_VARNF);
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
                        return ExecutionResult::PushError(e);
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
                        return ExecutionResult::PushError(e);
                    }
                }
            }
            Op::RangeRef => {
                let (to, from, base) = (f.pop(), f.pop(), f.peek_top());
                let result = base.range(&from, &to, IndexMode::OneBased);
                if let Err(e) = result {
                    f.pop();
                    return ExecutionResult::PushError(e);
                }
                f.poke(0, result.unwrap());
            }
            Op::RangeSet => {
                let (value, to, from, base) = (f.pop(), f.pop(), f.pop(), f.peek_top());
                let result = base.range_set(&from, &to, &value, IndexMode::OneBased);
                if let Err(e) = result {
                    f.pop();
                    return ExecutionResult::PushError(e);
                }
                f.poke(0, result.unwrap());
            }
            Op::Length(offset) => {
                let v = f.peek_abs(offset.0 as usize);
                match v.len() {
                    Ok(l) => f.push(v_int(l as i64)),
                    Err(e) => return ExecutionResult::PushError(e),
                }
            }
            Op::GetProp => {
                let (propname, obj) = (f.pop(), f.peek_top());

                let Ok(propname) = propname.as_symbol() else {
                    return ExecutionResult::PushError(E_TYPE);
                };

                let value = get_property(world_state, &permissions, obj, propname, features_config);
                match value {
                    Ok(v) => {
                        f.poke(0, v);
                    }
                    Err(e) => {
                        return ExecutionResult::PushError(e);
                    }
                }
            }
            Op::PushGetProp => {
                let (propname, obj) = f.peek2();

                let Ok(propname) = propname.as_symbol() else {
                    return ExecutionResult::PushError(E_TYPE);
                };

                let value = get_property(world_state, &permissions, obj, propname, features_config);
                match value {
                    Ok(v) => {
                        f.push(v);
                    }
                    Err(e) => {
                        return ExecutionResult::PushError(e);
                    }
                }
            }
            Op::PutProp => {
                let (rhs, propname, obj) = (f.pop(), f.pop(), f.peek_top());

                let Variant::Obj(obj) = obj.variant() else {
                    return ExecutionResult::PushError(E_TYPE);
                };
                let Ok(propname) = propname.as_symbol() else {
                    return ExecutionResult::PushError(E_TYPE);
                };
                let update_result =
                    world_state.update_property(&permissions, obj, propname, &rhs.clone());

                match update_result {
                    Ok(()) => {
                        f.poke(0, rhs);
                    }
                    Err(e) => {
                        return ExecutionResult::PushError(e.to_error_code());
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
                        return ExecutionResult::PushError(E_TYPE);
                    }
                };

                if time < 0.0 {
                    return ExecutionResult::PushError(E_INVARG);
                }
                let delay = (time != 0.0).then(|| Duration::from_secs_f64(time));

                return ExecutionResult::TaskStartFork(delay, *id, *fv_offset);
            }
            Op::Pass => {
                let args = f.pop();
                let Variant::List(args) = args.variant() else {
                    return ExecutionResult::PushError(E_TYPE);
                };
                return ExecutionResult::DispatchVerbPass(args.clone());
            }
            Op::CallVerb => {
                let (args, verb, obj) = (f.pop(), f.pop(), f.pop());
                let Variant::List(l) = args.variant() else {
                    return ExecutionResult::PushError(E_TYPE);
                };
                let Ok(verb) = verb.as_symbol() else {
                    return ExecutionResult::PushError(E_TYPE);
                };
                return ExecutionResult::PrepareVerbDispatch {
                    this: obj,
                    verb_name: verb,
                    args: l.clone(),
                };
            }
            Op::Return => {
                let ret_val = f.pop();
                return ExecutionResult::Return(ret_val);
            }
            Op::Return0 => {
                return ExecutionResult::Return(v_int(0));
            }
            Op::Done => {
                return ExecutionResult::Return(v_none());
            }
            Op::FuncCall { id } => {
                // Pop arguments, should be a list.
                let args = f.pop();
                let Variant::List(args) = args.variant() else {
                    return ExecutionResult::PushError(E_ARGS);
                };
                return ExecutionResult::DispatchBuiltin {
                    builtin: *id,
                    arguments: args.iter().collect(),
                };
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
                    panic!("Handler is not a catch handler",);
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
                        return ExecutionResult::Unwind(why);
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
                let Some(_scope) = f.pop_scope() else {
                    panic!(
                        "EndScope without a scope @ {} ({})",
                        f.pc,
                        f.find_line_no(f.pc).unwrap_or(0)
                    );
                };
            }
            Op::ExitId(label) => {
                f.jump(label);
                continue;
            }
            Op::Exit { stack, label } => {
                return ExecutionResult::Unwind(FinallyReason::Exit {
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
                        return ExecutionResult::PushError(E_TYPE);
                    };
                    rhs_values.clone()
                };

                let len = rhs_values.len();
                if len < nreq || !have_rest && len > nargs {
                    f.pop();
                    return ExecutionResult::PushError(E_ARGS);
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
                                return ExecutionResult::PushError(E_ARGS);
                            };

                            f.set_env(id, arg.clone());
                        }
                        ScatterLabel::Optional(id, jump_to) => {
                            if nopt_avail > 0 {
                                nopt_avail -= 1;
                                let Some(arg) = args_iter.next() else {
                                    return ExecutionResult::PushError(E_ARGS);
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
                    return ExecutionResult::PushError(E_TYPE);
                }
            }

            // Execution of the comprehension is:
            //
            //  Op::BeginComprehension (enter scope)
            //      pushes empty list & scope to stack
            //  set variable to start of index
            //  push end of index to stack
            //  begin loop (set label X)
            //      Op:ComprehendRange:
            //        pop end of range index from stack
            //        get index from var
            //        if index > end of range, jmp to end label (Y)
            //        push index
            //      execute producer expr
            //      Op::ContinueRange
            //          pop result
            //          pop list from stack
            //          append result to list, push back
            //          push end of range index to stack
            //          push cur index to stack
            //      jmp X
            //  end loop / scope
            //  (set label Y)
            Op::BeginComprehension(_type, end_label, _start) => {
                f.push(v_empty_list());
                f.push_scope(ScopeType::Comprehension, 1, end_label);
            }
            Op::ComprehendRange {
                position,
                end_of_range_register,
                end_label,
            } => {
                let end_of_range = f.get_env(end_of_range_register).unwrap().clone();
                let position = f
                    .get_env(position)
                    .expect("Bad range position variable in range comprehension")
                    .clone();
                if !position.le(&end_of_range) {
                    f.jump(end_label);
                }
            }
            Op::ComprehendList {
                item_variable,
                position_register,
                list_register,
                end_label,
            } => {
                let list = f.get_env(list_register).unwrap().clone();
                let position = f.get_env(position_register).unwrap().clone();
                let Variant::Int(position) = position.variant() else {
                    return ExecutionResult::PushError(E_TYPE);
                };
                let position = *position;
                if position > list.len().unwrap() as i64 {
                    f.jump(end_label);
                } else {
                    let Ok(item) = list.index(&v_int(position), IndexMode::OneBased) else {
                        return ExecutionResult::PushError(E_RANGE);
                    };
                    f.set_env(item_variable, item);
                }
            }
            Op::ContinueComprehension(id) => {
                let result = f.pop();
                let list = f.pop();
                let position = f
                    .get_env(id)
                    .expect("Bad range position variable in range comprehension")
                    .clone();
                let Ok(new_position) = position.add(&v_int(1)) else {
                    return ExecutionResult::PushError(E_TYPE);
                };
                let Ok(new_list) = list.push(&result) else {
                    return ExecutionResult::PushError(E_TYPE);
                };
                f.set_env(id, new_position);
                f.push(new_list);
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
    propname: Symbol,
    features_config: &FeaturesConfig,
) -> Result<Var, Error> {
    match obj.variant() {
        Variant::Obj(obj) => {
            let result = world_state.retrieve_property(permissions, obj, propname);
            match result {
                Ok(v) => Ok(v),
                Err(e) => Err(e.to_error_code()),
            }
        }
        Variant::Flyweight(flyweight) => {
            // If propname is `delegate`, return the delegate object.
            // If the propname is `slots`, return the slots list.
            // Otherwise, return the value from the slots list.
            let value = if propname == *DELEGATE_SYM {
                v_obj(flyweight.delegate().clone())
            } else if propname == *SLOTS_SYM {
                let slots: Vec<_> = flyweight
                    .slots()
                    .iter()
                    .map(|(k, v)| {
                        (
                            if features_config.use_symbols_in_builtins {
                                v_sym(k.clone())
                            } else {
                                v_str(k.as_str())
                            },
                            v.clone(),
                        )
                    })
                    .collect();
                v_map(&slots)
            } else if let Some(result) = flyweight.get_slot(&propname) {
                result.clone()
            } else {
                // Now check the delegate
                let delegate = flyweight.delegate();
                let result = world_state.retrieve_property(permissions, delegate, propname);
                match result {
                    Ok(v) => v,
                    Err(e) => return Err(e.to_error_code()),
                }
            };
            Ok(value)
        }
        _ => Err(E_INVIND),
    }
}
