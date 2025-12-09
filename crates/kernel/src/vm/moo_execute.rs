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

use crate::{
    config::FeaturesConfig,
    task_context::with_current_transaction_mut,
    vm::{
        moo_frame::{CatchType, MooStackFrame, ScopeType},
        scatter_assign::scatter_assign,
        vm_host::ExecutionResult,
        vm_unwind::FinallyReason,
    },
};
use lazy_static::lazy_static;
use moor_compiler::{Op, to_literal};
use moor_var::{
    E_ARGS, E_DIV, E_INVARG, E_INVIND, E_RANGE, E_TYPE, E_VARNF, Error, IndexMode, Obj, Symbol,
    TypeClass, Var, VarType, program::names::Name, v_arc_string, v_bool, v_bool_int, v_empty_list,
    v_empty_map, v_err, v_error, v_float, v_flyweight, v_int, v_list, v_map, v_none, v_obj, v_sym,
};
use std::time::Duration;

lazy_static! {
    static ref DELEGATE_SYM: Symbol = Symbol::mk("delegate");
    static ref SLOTS_SYM: Symbol = Symbol::mk("slots");
}

/// Build a captured environment from a list of captured variables
/// This recreates the environment structure needed by lambda execution
fn build_captured_environment(
    captured_vars: &[(moor_var::program::names::Name, Var)],
    lambda_program: &moor_compiler::Program,
) -> Vec<Vec<Var>> {
    if captured_vars.is_empty() {
        return vec![];
    }

    // Organize variables by scope depth using a Vec (scope depths are sequential from 0)
    let max_scope_depth = captured_vars
        .iter()
        .map(|(name, _)| name.1 as usize)
        .max()
        .unwrap_or(0);

    let mut scope_vars: Vec<Vec<(u16, Var)>> = vec![Vec::new(); max_scope_depth + 1];

    for &(name, ref value) in captured_vars {
        let scope_depth = name.1 as usize;
        let var_offset = name.0;
        scope_vars[scope_depth].push((var_offset, value.clone()));
    }

    // Build environment with proper scope structure
    let mut captured_env = Vec::new();

    for (scope_idx, vars_in_scope) in scope_vars.iter().enumerate() {
        // For scope 0 (global), use global width. For others, use a reasonable default.
        let expected_var_count = if scope_idx == 0 {
            lambda_program.var_names().global_width()
        } else {
            // For non-global scopes, start with a minimum size and expand as needed
            16
        };
        let mut scope_env = vec![moor_var::v_none(); expected_var_count];

        if !vars_in_scope.is_empty() {
            // Find the maximum offset to ensure we have enough space
            let max_offset = vars_in_scope
                .iter()
                .map(|(offset, _)| *offset as usize)
                .max()
                .unwrap_or(0);
            if max_offset >= scope_env.len() {
                scope_env.resize(max_offset + 1, moor_var::v_none());
            }

            for &(var_offset, ref value) in vars_in_scope {
                scope_env[var_offset as usize] = value.clone();
            }
        }

        captured_env.push(scope_env);
    }

    captured_env
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
    features_config: &FeaturesConfig,
) -> ExecutionResult {
    // Special case for empty opcodes set, just return immediately.
    if f.opcodes().is_empty() {
        return ExecutionResult::Complete(v_bool(false));
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
        // compiler or in opcode execution, and we'd dearly like to know about it, not hide it.
        let pc = f.pc;
        f.pc += 1;
        let op = &f.opcodes()[pc];

        match op {
            Op::If(label, environment_width)
            | Op::Eif(label, environment_width)
            | Op::While {
                jump_label: label,
                environment_width,
            } => {
                let scope_type = match op {
                    Op::If(..) => ScopeType::If,
                    Op::Eif(..) => ScopeType::Eif,
                    Op::While { .. } => ScopeType::While,
                    _ => unreachable!(),
                };
                let (environment_width, label) = (*environment_width, *label);
                f.push_scope(scope_type, environment_width, &label);
                let cond = f.pop();
                if !cond.is_true() {
                    f.jump(&label);
                }
            }
            Op::IfQues(label) => {
                let label = *label;
                let cond = f.pop();
                if !cond.is_true() {
                    f.jump(&label);
                }
            }
            Op::Jump { label } => {
                let label = *label;
                f.jump(&label);
            }
            Op::WhileId {
                id,
                end_label,
                environment_width,
            } => {
                let (id, environment_width, end_label) = (*id, *environment_width, *end_label);
                f.push_scope(ScopeType::While, environment_width, &end_label);
                let v = f.pop();
                let is_true = v.is_true();
                f.set_variable(&id, v);
                if !is_true {
                    f.jump(&end_label);
                }
            }
            Op::BeginForSequence { operand } => {
                let operand_offset = *operand;
                let operand = f.program.for_sequence_operand(operand_offset).clone();

                // Pop sequence from stack
                let sequence = f.pop();

                // Validate sequence
                if (!sequence.is_sequence() && !sequence.is_associative())
                    || sequence.type_code() == VarType::TYPE_STR
                {
                    f.jump(&operand.end_label);
                    return ExecutionResult::RaiseError(
                        E_TYPE.msg("invalid sequence type in for loop"),
                    );
                }

                let Ok(list_len) = sequence.len() else {
                    return ExecutionResult::RaiseError(
                        E_TYPE.msg("invalid sequence length in for loop"),
                    );
                };

                // If sequence is empty, jump to end immediately
                if list_len == 0 {
                    f.jump(&operand.end_label);
                    continue;
                }

                // Create ForSequence scope with initial state
                f.push_for_sequence_scope(
                    sequence,
                    operand.value_bind,
                    operand.key_bind,
                    &operand.end_label,
                    operand.environment_width,
                );
            }
            Op::IterateForSequence => {
                // Get ForSequence scope or error early
                let Some(ScopeType::ForSequence {
                    sequence,
                    current_index,
                    current_key,
                    value_bind,
                    key_bind,
                    end_label,
                }) = f.get_for_sequence_scope_mut()
                else {
                    return ExecutionResult::RaiseError(
                        E_ARGS.msg("IterateForSequence without ForSequence scope"),
                    );
                };

                // Get current element - check bounds first to avoid expensive error construction
                let tc = sequence.type_class();
                let len = match &tc {
                    TypeClass::Sequence(s) => s.len(),
                    TypeClass::Associative(a) => a.len(),
                    TypeClass::Scalar => {
                        return ExecutionResult::RaiseError(
                            E_TYPE.msg("invalid sequence type in for loop"),
                        );
                    }
                };

                // Bounds check before iteration (avoids E_RANGE error construction)
                if *current_index >= len {
                    let end_lbl = *end_label;
                    f.jump(&end_lbl);
                    continue;
                }

                let next = match tc {
                    TypeClass::Sequence(s) => s
                        .index(*current_index)
                        .map(|v| (v_int(*current_index as i64 + 1), v.clone())),
                    TypeClass::Associative(a) => match current_key {
                        Some(current_key) => a.next_after(current_key, false),
                        None => a.first(),
                    },
                    TypeClass::Scalar => unreachable!(),
                };

                let k_v = match next {
                    Ok(k_v) => k_v,
                    Err(e) => return ExecutionResult::RaiseError(e),
                };

                // Extract values we need for variable setting
                let value_bind = *value_bind;
                let key_bind = *key_bind;

                // Increment index for next iteration
                *current_index += 1;
                if let TypeClass::Associative(_) = tc {
                    *current_key = Some(k_v.0.clone());
                }

                // Set loop variables (separate borrow)
                f.set_variable(&value_bind, k_v.1);
                if let Some(key_bind) = key_bind {
                    f.set_variable(&key_bind, k_v.0);
                }
            }
            Op::BeginForRange { operand } => {
                let operand_offset = *operand;
                let operand = f.program.for_range_operand(operand_offset).clone();

                // Pop end_value and start_value from stack (stack: [from, to])
                let end_val = f.pop();
                let start_val = f.pop();

                // Validate range values are integers, floats, or objects
                if !start_val.same_numeric_type(&end_val) {
                    return ExecutionResult::RaiseError(E_TYPE.msg(
                        "for-range requires matching types (both INT, both FLOAT, or both OBJ)",
                    ));
                }

                // For object ranges, only numeric OIDs can be iterated (not UUIDs or anonymous)
                if let (Some(start_obj), Some(end_obj)) =
                    (start_val.as_object(), end_val.as_object())
                    && (!start_obj.is_oid() || !end_obj.is_oid()) {
                        return ExecutionResult::RaiseError(
                            E_TYPE.msg("for-range requires numeric object IDs, not UUIDs"),
                        );
                    }

                // If start > end, jump to end immediately (empty range)
                if start_val > end_val {
                    f.jump(&operand.end_label);
                    continue;
                }

                // Create ForRange scope with initial state
                f.push_for_range_scope(
                    &start_val,
                    &end_val,
                    operand.loop_variable,
                    &operand.end_label,
                    operand.environment_width,
                );
            }
            Op::IterateForRange => {
                let Some(ScopeType::ForRange {
                    current_value,
                    end_value,
                    loop_variable,
                    end_label,
                }) = f.get_for_range_scope_mut()
                else {
                    return ExecutionResult::RaiseError(
                        E_INVARG.msg("IterateForRange without ForRange scope"),
                    );
                };

                // Check bounds and handle end condition
                if *current_value > *end_value {
                    let end_lbl = *end_label;
                    f.jump(&end_lbl);
                    continue;
                }

                // Extract values we need for variable setting
                let current_val = current_value.clone();
                let loop_var = *loop_variable;

                // Increment for next iteration with type-specific logic and overflow protection
                // Use direct accessors to avoid variant() overhead on the hot path
                let next_value = if let Some(i) = current_val.as_integer() {
                    // Integer case (most common)
                    if i == i64::MAX {
                        // Decrement end_value instead to avoid overflow
                        if let Some(e) = end_value.as_integer()
                            && e > i64::MIN {
                                *end_value = v_int(e - 1);
                            }
                        current_val.clone()
                    } else {
                        v_int(i + 1)
                    }
                } else if let Some(f_val) = current_val.as_float() {
                    v_float(f_val + 1.0)
                } else if let Some(o) = current_val.as_object() {
                    // Only numeric object IDs can be iterated - not UUIDs or anonymous
                    if !o.is_oid() {
                        return ExecutionResult::RaiseError(
                            E_TYPE.msg("cannot iterate over non-numeric object IDs"),
                        );
                    }
                    let obj_id = o.id().0;
                    if obj_id == i32::MAX {
                        // Decrement end_value instead to avoid overflow
                        if let Some(e) = end_value.as_object()
                            && e.is_oid() && e.id().0 > i32::MIN {
                                *end_value = v_obj(Obj::mk_id(e.id().0 - 1));
                            }
                        current_val.clone()
                    } else {
                        v_obj(Obj::mk_id(obj_id + 1))
                    }
                } else {
                    // This shouldn't happen due to validation in BeginForRange
                    return ExecutionResult::RaiseError(
                        E_TYPE.msg("invalid type in for-range iteration"),
                    );
                };
                *current_value = next_value;

                f.set_variable(&loop_var, current_val);
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
                f.push(v_obj(*val));
            }
            Op::ImmSymbol(val) => {
                f.push(v_sym(*val));
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
                        let value = f.program.find_literal(slot).expect("literal not found");
                        f.push(value.clone());
                    }
                }
            }
            Op::ImmType(vt) => {
                let value = *vt as u8;
                f.push(v_int(value as i64));
            }
            Op::ImmEmptyList => f.push(v_empty_list()),
            Op::ListAddTail => {
                let (tail, list) = (f.pop(), f.peek_top_mut());
                if !list.is_sequence() || list.type_code() == VarType::TYPE_STR {
                    f.pop();
                    return ExecutionResult::PushError(E_TYPE.msg("invalid value in list append"));
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
                    return ExecutionResult::PushError(E_TYPE.msg("invalid value in list append"));
                }

                if !tail.is_sequence() || !list.is_sequence() {
                    f.pop();
                    return ExecutionResult::PushError(E_TYPE.msg("invalid value in list append"));
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
                let result = lhs.set(&index, &rhs, IndexMode::OneBased);
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
            Op::MakeError(offset) => {
                let code = *f.program.error_operand(*offset);

                // Expect an argument on stack (otherwise we would have used ImmErr)
                let err_msg = f.pop();
                let Some(err_msg) = err_msg.as_string() else {
                    return ExecutionResult::PushError(
                        E_TYPE.msg("invalid value for error message"),
                    );
                };
                f.push(v_error(code.msg(err_msg)));
            }
            Op::MakeSingletonList => {
                let v = f.peek_top();
                f.poke(0, v_list(std::slice::from_ref(v)));
            }
            Op::MakeMap => {
                f.push(v_empty_map());
            }
            Op::MapInsert => {
                let (value, key, map) = (f.pop(), f.pop(), f.peek_top_mut());
                match map.type_class() {
                    TypeClass::Associative(a) => {
                        let result = a.set(&key, &value);
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
                    _ => {
                        f.pop();
                        return ExecutionResult::PushError(
                            E_TYPE.msg("invalid value in map insert"),
                        );
                    }
                }
            }
            Op::MakeFlyweight(num_slots) => {
                let num_slots = *num_slots;
                // Stack should be: contents, slots, delegate
                let contents = f.pop();
                // Contents must be a list
                let Some(contents) = contents.as_list() else {
                    return ExecutionResult::PushError(
                        E_TYPE.msg("invalid value for flyweight contents, must be list"),
                    );
                };
                let mut slots = Vec::with_capacity(num_slots);
                for _ in 0..num_slots {
                    let (k, v) = (f.pop(), f.pop());
                    let Ok(sym) = k.as_symbol() else {
                        return ExecutionResult::PushError(
                            E_TYPE.msg("invalid value for flyweight slot, must be a valid symbol"),
                        );
                    };
                    slots.push((sym, v));
                }
                let delegate = f.pop();
                let Some(delegate) = delegate.as_object() else {
                    return ExecutionResult::PushError(
                        E_TYPE.msg("invalid value for flyweight delegate, must be object"),
                    );
                };
                // Slots should be v_str -> value, num_slots times

                let flyweight = v_flyweight(delegate, &slots, contents.clone());
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
                if divargs[1].is_zero() {
                    return ExecutionResult::PushError(E_DIV.msg("division by zero"));
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
                if divargs[1].is_zero() {
                    return ExecutionResult::PushError(E_DIV.msg("division by zero"));
                };
                binary_var_op!(self, f, state, modulus);
            }
            Op::And(label) => {
                let label = *label;
                let v = f.peek_top().is_true();
                if !v {
                    f.jump(&label)
                } else {
                    f.pop();
                }
            }
            Op::Or(label) => {
                let label = *label;
                let v = f.peek_top().is_true();
                if v {
                    f.jump(&label);
                } else {
                    f.pop();
                }
            }
            Op::BitAnd => {
                binary_var_op!(self, f, state, bitand);
            }
            Op::BitOr => {
                binary_var_op!(self, f, state, bitor);
            }
            Op::BitXor => {
                binary_var_op!(self, f, state, bitxor);
            }
            Op::BitShl => {
                binary_var_op!(self, f, state, bitshl);
            }
            Op::BitShr => {
                binary_var_op!(self, f, state, bitshr);
            }
            Op::BitLShr => {
                binary_var_op!(self, f, state, bitlshr);
            }
            Op::BitNot => {
                let v = f.peek_top();
                match v.bitnot() {
                    Err(e) => {
                        f.pop();
                        return ExecutionResult::PushError(e);
                    }
                    Ok(result) => {
                        f.poke(0, result);
                    }
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
                    if let Some(var_name) = f.program.var_names().ident_for_name(ident) {
                        return ExecutionResult::PushError(
                            E_VARNF.with_msg(|| format!("Variable `{var_name}` not found")),
                        );
                    } else {
                        return ExecutionResult::PushError(E_VARNF.msg("Variable not found"));
                    }
                };
                f.push(v.clone());
            }
            Op::Put(ident) => {
                let ident = *ident;
                let v = f.peek_top();
                f.set_variable(&ident, v.clone());
            }
            Op::PushRef => {
                let (key_or_index, value) = f.peek2();
                let result = value.get(key_or_index, IndexMode::OneBased);
                match result {
                    Ok(v) => f.push(v),
                    Err(e) => {
                        f.pop();
                        return ExecutionResult::PushError(e);
                    }
                }
            }
            Op::Ref => {
                let (key_or_index, value) = (f.pop(), f.peek_top());

                let result = value.get(&key_or_index, IndexMode::OneBased);
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
                    return ExecutionResult::PushError(
                        E_TYPE.with_msg(|| {
                            format!("Invalid property name: {}", to_literal(&propname))
                        }),
                    );
                };

                let value = get_property(&permissions, obj, propname, features_config);
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
                    return ExecutionResult::PushError(
                        E_TYPE.with_msg(|| {
                            format!("Invalid property name: {}", to_literal(propname))
                        }),
                    );
                };

                let value = get_property(&permissions, obj, propname, features_config);
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

                let Some(obj) = obj.as_object() else {
                    return ExecutionResult::PushError(E_TYPE.with_msg(|| {
                        format!("Invalid value for property access: {}", to_literal(obj))
                    }));
                };
                let Ok(propname) = propname.as_symbol() else {
                    return ExecutionResult::PushError(
                        E_TYPE.with_msg(|| {
                            format!("Invalid property name: {}", to_literal(&propname))
                        }),
                    );
                };
                let update_result = with_current_transaction_mut(|world_state| {
                    world_state.update_property(&permissions, &obj, propname, &rhs.clone())
                });

                match update_result {
                    Ok(()) => {
                        f.poke(0, rhs);
                    }
                    Err(e) => {
                        return ExecutionResult::PushError(e.to_error());
                    }
                }
            }
            Op::Fork { id, fv_offset } => {
                let (id, fv_offset) = (*id, *fv_offset);
                // Delay time should be on stack
                let time = f.pop();

                let time = if let Some(i) = time.as_integer() {
                    i as f64
                } else if let Some(f) = time.as_float() {
                    f
                } else {
                    return ExecutionResult::PushError(
                        E_TYPE.msg("invalid value for delay time in fork"),
                    );
                };

                if time < 0.0 {
                    return ExecutionResult::PushError(
                        E_INVARG.msg("invalid value for delay time in fork"),
                    );
                }
                let delay = (time != 0.0).then(|| Duration::from_secs_f64(time));

                return ExecutionResult::TaskStartFork(delay, id, fv_offset);
            }
            Op::Pass => {
                let args = f.pop();
                let Some(args) = args.as_list() else {
                    return ExecutionResult::PushError(E_TYPE.with_msg(|| {
                        format!("Invalid target for verb dispatch: {}", to_literal(&args))
                    }));
                };
                return ExecutionResult::DispatchVerbPass(args.clone());
            }
            Op::CallVerb => {
                let (args, verb, obj) = (f.pop(), f.pop(), f.pop());
                let Some(l) = args.as_list() else {
                    return ExecutionResult::PushError(E_TYPE.with_msg(|| {
                        format!("Invalid target for verb dispatch: {}", to_literal(&args))
                    }));
                };
                let Ok(verb) = verb.as_symbol() else {
                    return ExecutionResult::PushError(
                        E_TYPE.with_msg(|| format!("Invalid verb name: {}", to_literal(&verb))),
                    );
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
                return ExecutionResult::Return(v_bool(false));
            }
            Op::FuncCall { id } => {
                let builtin = *id;
                // Pop arguments, should be a list.
                let args = f.pop();
                let Some(args) = args.as_list() else {
                    return ExecutionResult::PushError(
                        E_ARGS.msg("invalid value for function call"),
                    );
                };
                return ExecutionResult::DispatchBuiltin {
                    builtin,
                    arguments: args.iter().collect(),
                };
            }
            Op::PushCatchLabel(label) => {
                let label = *label;
                // Get the error codes, which is either a list of error codes or Any.
                let error_codes = f.pop().clone();

                // The scope above us has to be a TryCatch, and we need to push into that scope
                // the code list that we're going to execute.
                if let Some(error_codes) = error_codes.as_list() {
                    let error_codes = error_codes.iter().map(|v| {
                        let Some(e) = v.as_error() else {
                            panic!("Error codes list contains non-error code");
                        };
                        e.clone()
                    });
                    f.catch_stack
                        .push((CatchType::Errors(error_codes.into_iter().collect()), label));
                } else if error_codes.as_integer() == Some(0) {
                    f.catch_stack.push((CatchType::Any, label));
                } else {
                    panic!("Invalid error codes list");
                }
            }
            Op::TryFinally {
                end_label,
                environment_width,
            } => {
                let (environment_width, end_label) = (*environment_width, *end_label);
                f.push_scope(
                    ScopeType::TryFinally(end_label),
                    environment_width,
                    &end_label,
                );
            }
            Op::TryCatch { end_label, .. } => {
                let end_label = *end_label;
                let catches = std::mem::take(&mut f.catch_stack);
                f.push_non_var_scope(ScopeType::TryCatch(catches), &end_label);
            }
            Op::TryExcept {
                environment_width,
                end_label,
                ..
            } => {
                let (environment_width, end_label) = (*environment_width, *end_label);
                let catches = std::mem::take(&mut f.catch_stack);
                f.push_scope(ScopeType::TryCatch(catches), environment_width, &end_label);
            }
            Op::EndExcept(label) => {
                let label = *label;
                let handler = f.pop_scope().expect("Missing handler for try/catch/except");
                let ScopeType::TryCatch(..) = handler.scope_type else {
                    panic!("Handler is not a catch handler",);
                };
                f.jump(&label);
            }
            Op::EndCatch(label) => {
                let label = *label;

                let stack_top = f.pop();
                let handler = f.pop_scope().expect("Missing handler for try/catch/except");
                let ScopeType::TryCatch(_) = handler.scope_type else {
                    panic!("Handler is not a catch handler",);
                };
                f.jump(&label);
                f.push(stack_top);
            }
            Op::EndFinally => {
                // Execution of the block completed successfully, so we can just continue with
                // fall-through into the FinallyContinue block
                // Pop the scope that was pushed by TryFinally
                let scope = f.pop_scope().expect("Missing scope for try/finally");
                if !matches!(scope.scope_type, ScopeType::TryFinally(_)) {
                    panic!("EndFinally without TryFinally scope");
                }
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
                let (num_bindings, end_label) = (*num_bindings, *end_label);
                f.push_scope(ScopeType::Block, num_bindings, &end_label);
            }
            Op::EndScope { .. } => {
                let Some(..) = f.pop_scope() else {
                    panic!(
                        "EndScope without a scope @ {} ({})",
                        f.pc,
                        f.find_line_no(f.pc).unwrap_or(0)
                    );
                };
            }
            Op::ExitId(label) => {
                let label = *label;
                f.jump(&label);
                continue;
            }
            Op::Exit { stack, label } => {
                return ExecutionResult::Unwind(FinallyReason::Exit {
                    stack: *stack,
                    label: *label,
                });
            }
            Op::Scatter(sa) => {
                // Get the scatter table and the values to assign
                let sa = *sa;
                let table = &f.program.scatter_table(sa).clone();
                let rhs_values = {
                    let rhs = f.peek_top();
                    let Some(rhs_values) = rhs.as_list() else {
                        let scatter_err = E_TYPE
                            .with_msg(|| format!("Invalid value for scatter: {}", to_literal(rhs)));
                        f.pop();
                        return ExecutionResult::PushError(scatter_err);
                    };
                    rhs_values.clone()
                };

                // Use the shared scatter assignment logic
                let result = scatter_assign(
                    table,
                    &rhs_values.iter().collect::<Vec<_>>(),
                    |name, value| {
                        f.set_variable(name, value);
                    },
                );

                match result.result {
                    Err(e) => {
                        f.pop();
                        return ExecutionResult::PushError(e);
                    }
                    Ok(()) => {
                        // Jump to appropriate location based on whether defaults are needed
                        let jump_label = result.first_default_label.unwrap_or(table.done);
                        f.jump(&jump_label);
                    }
                }
            }
            Op::CheckListForSplice => {
                if !f.peek_top().is_sequence() {
                    f.pop();
                    return ExecutionResult::PushError(E_TYPE.msg("invalid value in list splice"));
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
            Op::BeginComprehension(_, end_label, _) => {
                let end_label = *end_label;
                f.push(v_empty_list());
                f.push_scope(ScopeType::Comprehension, 1, &end_label);
            }
            Op::ComprehendRange(offset) => {
                let offset = *offset;
                let range_comprehension = f.program.range_comprehension(offset).clone();
                let end_of_range = f
                    .get_env(&range_comprehension.end_of_range_register)
                    .unwrap()
                    .clone();
                let position = f
                    .get_env(&range_comprehension.position)
                    .expect("Bad range position variable in range comprehension")
                    .clone();
                if !position.le(&end_of_range) {
                    f.jump(&range_comprehension.end_label);
                }
            }
            Op::ComprehendList(offset) => {
                let offset = *offset;
                let list_comprehension = f.program.list_comprehension(offset).clone();
                let list = f
                    .get_env(&list_comprehension.list_register)
                    .unwrap()
                    .clone();
                let position = f
                    .get_env(&list_comprehension.position_register)
                    .unwrap()
                    .clone();
                let Some(position) = position.as_integer() else {
                    return ExecutionResult::PushError(
                        E_TYPE.msg("invalid value in list comprehension"),
                    );
                };
                if position > list.len().unwrap() as i64 {
                    f.jump(&list_comprehension.end_label);
                } else {
                    let Ok(item) = list.index(&v_int(position), IndexMode::OneBased) else {
                        return ExecutionResult::PushError(
                            E_RANGE.msg("invalid index in list comprehension"),
                        );
                    };
                    f.set_variable(&list_comprehension.item_variable, item);
                }
            }
            Op::ContinueComprehension(id) => {
                let id = *id;
                let result = f.pop();
                let list = f.pop();
                let position = f
                    .get_env(&id)
                    .expect("Bad range position variable in range comprehension")
                    .clone();
                let Ok(new_position) = position.add(&v_int(1)) else {
                    return ExecutionResult::PushError(
                        E_TYPE.msg("invalid value in list comprehension"),
                    );
                };
                let Ok(new_list) = list.push(&result) else {
                    return ExecutionResult::PushError(
                        E_TYPE.msg("invalid value in list comprehension"),
                    );
                };
                f.set_variable(&id, new_position);
                f.push(new_list);
            }
            Op::Capture(var_name) => {
                let var_name = *var_name;
                // Capture a variable from the current environment for lambda closure
                if let Some(value) = f.get_env(&var_name) {
                    f.capture_stack.push((var_name, value.clone()));
                } else {
                    // Variable not found - capture None/v_none
                    f.capture_stack.push((var_name, moor_var::v_none()));
                }
            }
            Op::MakeLambda {
                scatter_offset,
                program_offset,
                self_var,
                num_captured,
            } => {
                let (scatter_offset, program_offset, self_var, num_captured) =
                    (*scatter_offset, *program_offset, *self_var, *num_captured);
                // Retrieve the scatter specification for lambda parameters
                let scatter_spec = f.program.scatter_table(scatter_offset).clone();

                // Retrieve the pre-compiled Program for the lambda body
                let lambda_program = f.program.lambda_program(program_offset).clone();

                // Build captured environment from the capture stack
                let captured_env = if num_captured == 0 {
                    vec![]
                } else {
                    // Take the last num_captured items from the capture stack
                    let stack_len = f.capture_stack.len();
                    if stack_len < num_captured as usize {
                        return ExecutionResult::PushError(
                            E_ARGS.msg("insufficient captured variables on stack"),
                        );
                    }

                    // Extract captured variables and convert to environment format
                    let captured_vars: Vec<(Name, Var)> = f
                        .capture_stack
                        .drain(stack_len - num_captured as usize..)
                        .collect();

                    build_captured_environment(&captured_vars, &lambda_program)
                };

                // Create the lambda value with self-reference information
                // Self-reference will be handled during lambda activation
                let lambda_var =
                    Var::mk_lambda(scatter_spec, lambda_program, captured_env, self_var);

                // Push lambda value onto the stack
                f.push(lambda_var);
            }
            Op::CallLambda => {
                // Pop arguments list and lambda value from stack
                let args_list = f.pop();
                let lambda_var = f.pop();

                // Verify we have a lambda value
                let Some(lambda) = lambda_var.as_lambda() else {
                    return ExecutionResult::PushError(E_TYPE.msg("expected lambda value"));
                };

                // Convert args list to List type for dispatch
                let Some(args) = args_list.as_list() else {
                    return ExecutionResult::PushError(E_ARGS.msg("expected argument list"));
                };
                let args = args.clone();

                // Request lambda dispatch - this will create a new activation
                return ExecutionResult::DispatchLambda {
                    lambda: lambda.clone(),
                    arguments: args,
                };
            }
        }
    }
    // We don't usually get here because most execution paths return before we hit the end of
    // the loop. But if we do, we need to return More so the scheduler knows to keep feeding
    // us.
    ExecutionResult::More
}

fn get_property(
    permissions: &Obj,
    obj: &Var,
    propname: Symbol,
    features_config: &FeaturesConfig,
) -> Result<Var, Error> {
    // Fast path: Obj is by far the most common case for property access
    if let Some(obj_ref) = obj.as_object() {
        let result = with_current_transaction_mut(|world_state| {
            world_state.retrieve_property(permissions, &obj_ref, propname)
        });
        return match result {
            Ok(v) => Ok(v),
            Err(e) => Err(e.to_error()),
        };
    }

    // Flyweight case
    if let Some(flyweight) = obj.as_flyweight() {
        // If propname is `delegate`, return the delegate object.
        // If the propname is `slots`, return the slots list.
        // Otherwise, return the value from the slots list.
        let value = if propname == *DELEGATE_SYM {
            v_obj(*flyweight.delegate())
        } else if propname == *SLOTS_SYM {
            let slots: Vec<_> = flyweight
                .slots()
                .iter()
                .map(|(k, v)| {
                    (
                        if features_config.use_symbols_in_builtins {
                            v_sym(*k)
                        } else {
                            v_arc_string(k.as_arc_string())
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
            let result = with_current_transaction_mut(|world_state| {
                world_state.retrieve_property(permissions, delegate, propname)
            });
            match result {
                Ok(v) => v,
                Err(e) => return Err(e.to_error()),
            }
        };
        return Ok(value);
    }

    // Invalid target for property access
    Err(E_INVIND.with_msg(|| format!("Invalid value for property access: {}", to_literal(obj))))
}
