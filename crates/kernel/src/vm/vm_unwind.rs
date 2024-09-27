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

use bincode::{Decode, Encode};
use moor_compiler::{Label, Offset, BUILTINS};
use moor_values::model::Named;
use moor_values::model::VerbFlag;
use moor_values::tasks::Exception;
use moor_values::NOTHING;
use moor_values::{v_err, v_int, v_list, v_none, v_objid, v_str, Var};
use moor_values::{Error, ErrorPack};
use tracing::trace;

use crate::vm::activation::{Activation, Frame};
use crate::vm::moo_frame::{CatchType, ScopeType};
use crate::vm::{ExecutionResult, VMExecState};

#[derive(Clone, Eq, PartialEq, Debug, Decode, Encode)]
pub enum FinallyReason {
    Fallthrough,
    Raise(Exception),
    Return(Var),
    Abort,
    Exit { stack: Offset, label: Label },
}

impl VMExecState {
    /// Compose a list of the current stack frames, starting from `start_frame_num` and working
    /// upwards.
    fn make_stack_list(activations: &[Activation]) -> Vec<Var> {
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
                        v_objid(a.this),
                        v_str(a.verbdef.names().join(" ").as_str()),
                        v_objid(a.verb_definer()),
                        v_objid(a.verb_owner()),
                        v_objid(a.player),
                        line_no,
                    ]
                }
                Frame::Bf(bf_frame) => {
                    let bf_name = BUILTINS.name_of(bf_frame.bf_id).unwrap();
                    vec![
                        v_objid(a.this),
                        v_str(bf_name.as_str()),
                        v_objid(NOTHING),
                        v_objid(NOTHING),
                        v_objid(a.player),
                        v_none(),
                    ]
                }
            };

            stack_list.push(v_list(&traceback_entry));
        }
        stack_list
    }

    /// Compose a backtrace list of strings for an error, starting from the current stack frame.
    fn make_backtrace(activations: &[Activation], raise_msg: &str) -> Vec<Var> {
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
            if a.verb_definer() != a.this {
                pieces.push(format!(" (this == #{})", a.this.0));
            }
            if let Some(line_num) = a.frame.find_line_no() {
                pieces.push(format!(" (line {})", line_num));
            }
            if i == 0 {
                pieces.push(format!(": {}", raise_msg));
            }
            let piece = pieces.join("");
            backtrace_list.push(v_str(&piece))
        }
        backtrace_list.push(v_str("(End of traceback)"));
        backtrace_list
    }

    /// Raise an error.
    /// Finds the catch handler for the given error if there is one, and unwinds the stack to it.
    /// If there is no handler, creates an 'Uncaught' reason with backtrace, and unwinds with that.
    fn raise_error_pack(&mut self, p: ErrorPack) -> ExecutionResult {
        trace!(error = ?p, "raising error");

        let stack = Self::make_stack_list(&self.stack);
        let backtrace = Self::make_backtrace(&self.stack, &p.msg);
        let exception = Exception {
            code: p.code,
            msg: p.msg,
            value: p.value,
            stack,
            backtrace,
        };
        self.unwind_stack(FinallyReason::Raise(exception))
    }

    /// Push an error to the stack and raise it.
    pub(crate) fn push_error(&mut self, code: Error) -> ExecutionResult {
        trace!(?code, "push_error");
        self.set_return_value(v_err(code));
        // Check 'd' bit of running verb. If it's set, we raise the error. Otherwise nope.
        if let Some(activation) = self.stack.last() {
            if activation.verbdef.flags().contains(VerbFlag::Debug) {
                return self.raise_error_pack(code.make_error_pack(None, None));
            }
        }
        ExecutionResult::More
    }

    /// Same as push_error, but for returns from builtin functions.
    pub(crate) fn push_bf_error(
        &mut self,
        code: Error,
        msg: Option<String>,
        value: Option<Var>,
    ) -> ExecutionResult {
        // TODO: revisit this now that Bf frames are a thing...
        //   We should be able to come up with a way to propagate and unwind for any kind of frame...
        //   And not have a special case here

        trace!(?code, "push_bf_error");
        // No matter what, the error value has to be on the stack of the *calling* verb, not on this
        // frame; as we are incapable of doing anything with it, we'll never pop it, being a builtin
        // function.
        self.parent_activation_mut()
            .frame
            .set_return_value(v_err(code));

        // Check 'd' bit of running verb. If it's set, we raise the error. Otherwise nope.
        // Filter out frames for builtin invocations
        let verb_frame = self.stack.iter().rev().find(|a| !a.is_builtin_frame());
        if let Some(activation) = verb_frame {
            if activation.verbdef.flags().contains(VerbFlag::Debug) {
                return self.raise_error_pack(code.make_error_pack(msg, value));
            }
        }
        // If we're not unwinding, we need to pop the builtin function's activation frame.
        self.stack.pop();
        ExecutionResult::More
    }

    /// Push an error to the stack with a description and raise it.
    pub(crate) fn push_error_msg(&mut self, code: Error, msg: String) -> ExecutionResult {
        trace!(?code, msg, "push_error_msg");
        self.set_return_value(v_err(code));
        self.raise_error(code)
    }

    /// Only raise an error if the 'd' bit is set on the running verb. Most times this is what we
    /// want.
    pub(crate) fn raise_error(&mut self, code: Error) -> ExecutionResult {
        trace!(?code, "maybe_raise_error");

        // Check 'd' bit of running verb. If it's set, we raise the error. Otherwise nope.
        // Filter out frames for builtin invocations
        let verb_frame = self.stack.iter().rev().find(|a| !a.is_builtin_frame());
        if let Some(activation) = verb_frame {
            if activation.verbdef.flags().contains(VerbFlag::Debug) {
                return self.raise_error_pack(code.make_error_pack(None, None));
            }
        }
        ExecutionResult::More
    }

    /// Explicitly raise an error, regardless of the 'd' bit.
    pub(crate) fn throw_error(&mut self, code: Error) -> ExecutionResult {
        trace!(?code, "raise_error");
        self.raise_error_pack(code.make_error_pack(None, None))
    }

    /// Unwind the stack with the given reason and return an execution result back to the VM loop
    /// which makes its way back up to the scheduler.
    /// Contains all the logic for handling the various reasons for exiting a verb execution:
    ///     * Error raises of various kinds
    ///     * Return values
    pub(crate) fn unwind_stack(&mut self, why: FinallyReason) -> ExecutionResult {
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
                                if let FinallyReason::Raise(Exception { code, .. }) = &why {
                                    for catch in catches {
                                        let found = match catch.0 {
                                            CatchType::Any => true,
                                            CatchType::Errors(e) => e.contains(code),
                                        };
                                        if found {
                                            frame.jump(&catch.1);
                                            frame.push(v_list(&[v_err(*code)]));
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
            self.stack.pop().expect("Stack underflow");

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
            FinallyReason::Fallthrough => ExecutionResult::Complete(v_none()),
            _ => ExecutionResult::Exception(why),
        }
    }
}
