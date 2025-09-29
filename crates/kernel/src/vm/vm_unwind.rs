// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use crate::vm::{
    activation::{Activation, Frame},
    exec_state::VMExecState,
    moo_frame::{CatchType, ScopeType},
    vm_host::ExecutionResult,
};

#[cfg(feature = "trace_events")]
use crate::{trace_builtin_end, trace_stack_unwind, trace_verb_end};
use bincode::{Decode, Encode};
use moor_common::{
    model::{Named, VerbFlag},
    tasks::Exception,
};
use moor_compiler::{BUILTINS, Label, Offset, to_literal};
use moor_var::{
    Error, NOTHING, Var, v_arc_string, v_bool, v_err, v_error, v_int, v_list, v_none, v_obj, v_str,
    v_string,
};
use tracing::warn;

#[derive(Clone, Eq, PartialEq, Debug, Decode, Encode)]
pub enum FinallyReason {
    Fallthrough,
    Raise(Box<Exception>),
    Return(Var),
    Abort,
    Exit { stack: Offset, label: Label },
}

impl VMExecState {
    /// Compose a list of the current stack frames, starting from `start_frame_num` and working
    /// upwards.
    pub(crate) fn make_stack_list(activations: &[Activation]) -> Vec<Var> {
        let mut stack_list = vec![];
        for a in activations.iter().rev() {
            // Produce traceback line for each activation frame and append to stack_list
            // Should include line numbers (if possible), the name of the currently running verb,
            // its definer, its location, and the current player, and 'this'.
            let line_no = match a.frame.find_line_no() {
                None => v_none(),
                Some(l) => v_int(l as i64),
            };
            // TODO: abstract this a bit further, putting its construction onto the activation/frame
            let traceback_entry = match &a.frame {
                Frame::Moo(_) => {
                    vec![
                        a.this.clone(),
                        v_str(
                            a.verbdef
                                .names()
                                .iter()
                                .map(|s| s.as_string())
                                .collect::<Vec<_>>()
                                .join(" ")
                                .as_str(),
                        ),
                        v_obj(a.permissions),
                        v_obj(a.verb_definer()),
                        v_obj(a.player),
                        line_no,
                    ]
                }
                Frame::Bf(bf_frame) => {
                    let bf_name = BUILTINS.name_of(bf_frame.bf_id).unwrap();
                    vec![
                        a.this.clone(),
                        v_arc_string(bf_name.as_arc_string()),
                        v_obj(a.permissions),
                        v_obj(NOTHING),
                        v_obj(a.player),
                        v_none(),
                    ]
                }
            };

            stack_list.push(v_list(&traceback_entry));
        }
        stack_list
    }

    /// Compose a backtrace list of strings for an error, starting from the current stack frame.
    pub(crate) fn make_backtrace(activations: &[Activation], error: &Error) -> Vec<Var> {
        // Walk live activation frames and produce a written representation of a traceback for each
        // frame.
        let mut backtrace_list = vec![];
        for (i, a) in activations.iter().rev().enumerate() {
            let mut pieces = vec![];
            if i != 0 {
                pieces.push("... called from ".to_string());
            }
            // TODO: abstract this a bit further, putting it onto the frame itself
            match &a.frame {
                Frame::Moo(_) => {
                    pieces.push(format!("{}:{}", a.verb_definer(), a.verb_name));
                }
                Frame::Bf(bf_frame) => {
                    let bf_name = BUILTINS.name_of(bf_frame.bf_id).unwrap();
                    pieces.push(format!("builtin {bf_name}",));
                }
            }
            if v_obj(a.verb_definer()) != a.this {
                pieces.push(format!(" (this == {})", to_literal(&a.this)));
            }
            if let Some(line_num) = a.frame.find_line_no() {
                pieces.push(format!(" (line {line_num})"));
            }
            if i == 0 {
                let raise_msg = format!("{} ({})", error.err_type, error.message());
                pieces.push(format!(": {raise_msg}"));
            }
            let piece = pieces.join("");
            backtrace_list.push(v_str(&piece))
        }
        backtrace_list.push(v_str("(End of traceback)"));
        backtrace_list
    }

    /// Explicitly raise an error.
    /// Finds the catch handler for the given error if there is one, and unwinds the stack to it.
    /// If there is no handler, creates an 'Uncaught' reason with backtrace, and unwinds with that.
    pub fn throw_error(&mut self, error: Error) -> ExecutionResult {
        let stack = Self::make_stack_list(&self.stack);
        let backtrace = Self::make_backtrace(&self.stack, &error);
        let exception = Box::new(Exception {
            error,
            stack,
            backtrace,
        });
        self.unwind_stack(FinallyReason::Raise(exception))
    }

    /// Push an error up the activation stack (set returned value), and raise it depending on the `d` flag
    pub(crate) fn push_error(&mut self, error: Error) -> ExecutionResult {
        self.set_return_value(v_error(error.clone()));
        // Check 'd' bit of running verb. If it's set, we raise the error. Otherwise nope.
        let verb_frame = self.stack.iter().rev().find(|a| !a.is_builtin_frame());
        if let Some(activation) = verb_frame
            && activation.verbdef.flags().contains(VerbFlag::Debug)
        {
            return self.throw_error(error);
        }
        let verb_this_name = verb_frame.map(|a| {
            format!(
                "{}:{} line: {:?}",
                to_literal(&a.this),
                a.verb_name.as_arc_string(),
                a.frame.find_line_no()
            )
        });

        // TODO: remove at some point 1.0, this is mainly here for diagnostics on uncooperative
        //   dbs.
        warn!(error = ?error, verb = ?verb_this_name, "Pushing error from !d verb");
        ExecutionResult::More
    }

    /// Only raise an error if the 'd' bit is set on the running verb. Most times this is what we
    /// want.
    pub(crate) fn raise_error(&mut self, error: Error) -> ExecutionResult {
        // Check 'd' bit of running verb. If it's set, we raise the error. Otherwise nope.
        // Filter out frames for builtin invocations
        let verb_frame = self.stack.iter().rev().find(|a| !a.is_builtin_frame());
        if let Some(activation) = verb_frame
            && activation.verbdef.flags().contains(VerbFlag::Debug)
        {
            return self.throw_error(error);
        }
        ExecutionResult::More
    }

    /// Unwind the stack with the given reason and return an execution result back to the VM loop
    /// which makes its way back up to the scheduler.
    /// Contains all the logic for handling the various reasons for exiting a verb execution:
    ///     * Error raises of various kinds
    ///     * Return common
    pub(crate) fn unwind_stack(&mut self, why: FinallyReason) -> ExecutionResult {
        // Emit stack unwind trace event
        #[cfg(feature = "trace_events")]
        {
            let reason = match &why {
                FinallyReason::Fallthrough => "Fallthrough".to_string(),
                FinallyReason::Raise(exception) => format!("Raise({})", exception.error.err_type),
                FinallyReason::Return(value) => format!("Return({})", to_literal(value)),
                FinallyReason::Abort => "Abort".to_string(),
                FinallyReason::Exit { stack, label } => {
                    format!("Exit(stack: {stack:?}, label: {label:?})")
                }
            };
            trace_stack_unwind!(self.task_id, &reason);
        }
        // Walk activation stack from bottom to top, tossing frames as we go.
        while let Some(a) = self.stack.last_mut() {
            // If this is an error or exit attempt to find a handler for it.
            match &mut a.frame {
                Frame::Moo(frame) => {
                    // Exit with a jump.. let's go...
                    if let FinallyReason::Exit { label, .. } = why {
                        frame.jump(&label);
                        return ExecutionResult::More;
                    }

                    loop {
                        // Check the scope stack to see if we've hit a finally or catch handler that
                        // was registered for this position in the value stack.
                        let Some(scope) = frame.pop_scope() else {
                            break;
                        };

                        match scope.scope_type {
                            ScopeType::TryFinally(finally_label) => {
                                // Jump to the label pointed to by the finally label and then continue on
                                // executing.
                                frame.jump(&finally_label);
                                frame.finally_stack.push(why.clone());
                                return ExecutionResult::More;
                            }
                            ScopeType::TryCatch(catches) => {
                                if let FinallyReason::Raise(e) = &why {
                                    for catch in catches {
                                        let found = match catch.0 {
                                            CatchType::Any => true,
                                            CatchType::Errors(errs) => errs.contains(&e.error),
                                        };
                                        if found {
                                            let value = e
                                                .error
                                                .value
                                                .as_deref()
                                                .cloned()
                                                .unwrap_or(v_none());
                                            frame.jump(&catch.1);
                                            frame.push(v_list(&[
                                                v_err(e.error.err_type),
                                                v_string(e.error.message()),
                                                value,
                                                v_list(&e.stack),
                                            ]));
                                            return ExecutionResult::More;
                                        }
                                    }
                                }
                            }
                            _ => {
                                // This is a lexical scope, so we just let it pop off the stack and
                                // continue on.
                            }
                        }
                    }
                }
                Frame::Bf(_) => {
                    // TODO: unwind builtin function frames here in a way that takes their
                    //   `return_value` (and maybe error state/) and propagates it up the stack.
                    //   This way things like push_bf_err can be removed.
                    //   This might involve encompassing some of the stuff below, too.
                }
            }

            // No match in the frame, so we pop it.
            #[cfg(feature = "trace_events")]
            let popped_frame = self.stack.pop().expect("Stack underflow");
            #[cfg(not(feature = "trace_events"))]
            self.stack.pop();

            // Emit verb end trace event for each popped activation record
            #[cfg(feature = "trace_events")]
            match &popped_frame.frame {
                Frame::Moo(_) => {
                    trace_verb_end!(self.task_id, &popped_frame.verb_name.as_string());
                }
                Frame::Bf(bf_frame) => {
                    let bf_name = BUILTINS.name_of(bf_frame.bf_id).unwrap();
                    trace_builtin_end!(self.task_id, bf_name);
                }
            }

            // No more frames to unwind, so break out and handle final exit.
            if self.stack.is_empty() {
                break;
            }

            // If it was an explicit return that brought us here, set the return value explicitly.
            // (Unless we're the final activation, in which case that should have been handled
            // above)
            if let FinallyReason::Return(value) = &why {
                self.set_return_value(value.clone());
                return ExecutionResult::More;
            }
        }

        match why {
            FinallyReason::Return(r) => ExecutionResult::Complete(r),
            FinallyReason::Fallthrough => ExecutionResult::Complete(v_bool(false)),
            _ => ExecutionResult::Exception(why),
        }
    }
}
