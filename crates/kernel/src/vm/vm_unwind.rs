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

use std::fmt::Display;

use bincode::{Decode, Encode};
use tracing::trace;

use moor_values::model::VerbFlag;
use moor_values::var::{v_err, v_int, v_list, v_none, v_objid, v_str, Var};
use moor_values::var::{v_listv, Variant};
use moor_values::var::{Error, ErrorPack};
use moor_values::NOTHING;

use crate::vm::activation::{Activation, VmStackFrame};
use crate::vm::frame::HandlerType;
use crate::vm::{ExecutionResult, VMExecState};
use moor_compiler::BUILTIN_DESCRIPTORS;
use moor_compiler::{Label, Offset};
use moor_values::model::Named;

#[derive(Clone, Eq, PartialEq, Debug, Decode, Encode)]
pub struct UncaughtException {
    pub code: Error,
    pub msg: String,
    pub value: Var,
    pub stack: Vec<Var>,
    pub backtrace: Vec<Var>,
}

impl Display for UncaughtException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Uncaught exception: {} ({})", self.msg, self.code)
    }
}

impl std::error::Error for UncaughtException {}

#[derive(Clone, Eq, PartialEq, Debug, Decode, Encode)]
pub enum FinallyReason {
    Fallthrough,
    Raise {
        code: Error,
        msg: String,
        stack: Vec<Var>,
    },
    Uncaught(UncaughtException),
    Return(Var),
    Abort,
    Exit {
        stack: Offset,
        label: Label,
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
            FinallyReason::Uncaught(UncaughtException { .. }) => FINALLY_REASON_UNCAUGHT,
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

impl VMExecState {
    /// Find the currently active catch handler for a given error code, if any.
    /// Then return the stack offset (from now) of the activation frame containing the handler.
    fn find_handler_active(&mut self, raise_code: Error) -> Option<usize> {
        // Scan activation frames and their stacks, looking for the first _Catch we can find.
        let mut activation_num = self.stack.len() - 1;
        loop {
            let activation = &self.stack.get(activation_num)?;
            // Skip non-MOO frames.
            if let VmStackFrame::Moo(ref frame) = activation.frame {
                for handler in &frame.handler_stack {
                    if let HandlerType::Catch(cnt) = handler.handler_type {
                        // Found one, now scan forwards from 'cnt' backwards in the valstack looking for either the first
                        // non-list value, or a list containing the error code.
                        // TODO check for 'cnt' being too large. not sure how to handle, tho
                        // TODO this actually i think is wrong, it needs to pull two values off the stack
                        let i = handler.valstack_pos;
                        for j in (i - cnt)..i {
                            if let Variant::List(codes) = &frame.valstack[j].variant() {
                                if !codes.contains(&v_err(raise_code)) {
                                    continue;
                                }
                            }
                            return Some(activation_num);
                        }
                    }
                }
            }
            if activation_num == 0 {
                break;
            }
            activation_num -= 1;
        }
        None
    }

    /// Compose a list of the current stack frames, starting from `start_frame_num` and working
    /// upwards.
    fn make_stack_list(&self, activations: &[Activation], start_frame_num: usize) -> Vec<Var> {
        // TODO LambdaMOO had logic in here about 'root_vector' and 'line_numbers_too' that I haven't included yet.

        let mut stack_list = vec![];
        for (i, a) in activations.iter().rev().enumerate() {
            if i < start_frame_num {
                continue;
            }
            // Produce traceback line for each activation frame and append to stack_list
            // Should include line numbers (if possible), the name of the currently running verb,
            // its definer, its location, and the current player, and 'this'.
            let line_no = match a.frame.find_line_no() {
                None => v_none(),
                Some(l) => v_int(l as i64),
            };
            // TODO: abstract this a bit further, putting its construction onto the frame itself.
            let traceback_entry = match &a.frame {
                VmStackFrame::Moo(_) => {
                    vec![
                        v_objid(a.this),
                        v_str(a.verb_info.verbdef().names().join(" ").as_str()),
                        v_objid(a.verb_definer()),
                        v_objid(a.verb_owner()),
                        v_objid(a.player),
                        line_no,
                    ]
                }
                VmStackFrame::Bf(bf_frame) => {
                    vec![
                        v_objid(a.this),
                        v_str(BUILTIN_DESCRIPTORS[bf_frame.bf_index].name.as_str()),
                        v_objid(NOTHING),
                        v_objid(NOTHING),
                        v_objid(a.player),
                        v_none(),
                    ]
                }
            };

            stack_list.push(v_listv(traceback_entry));
        }
        stack_list
    }

    /// Compose a backtrace list of strings for an error, starting from the current stack frame.
    fn error_backtrace_list(&mut self, raise_msg: &str) -> Vec<Var> {
        // Walk live activation frames and produce a written representation of a traceback for each
        // frame.
        let mut backtrace_list = vec![];
        for (i, a) in self.stack.iter().rev().enumerate() {
            let mut pieces = vec![];
            if i != 0 {
                pieces.push("... called from ".to_string());
            }
            // TODO: abstract this a bit further, putting it onto the frame itself
            match &a.frame {
                VmStackFrame::Moo(_) => {
                    pieces.push(format!("{}:{}", a.verb_definer(), a.verb_name));
                }
                VmStackFrame::Bf(bf_frame) => {
                    pieces.push(format!(
                        "builtin {}",
                        BUILTIN_DESCRIPTORS[bf_frame.bf_index].name.as_str()
                    ));
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
            // TODO builtin-function name if a builtin

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

        // Look for first active catch handler's activation frame and its (reverse) offset in the activation stack.
        let handler_activ = self.find_handler_active(p.code);

        let why = if let Some(handler_active_num) = handler_activ {
            FinallyReason::Raise {
                code: p.code,
                msg: p.msg,
                stack: self.make_stack_list(&self.stack, handler_active_num),
            }
        } else {
            FinallyReason::Uncaught(UncaughtException {
                code: p.code,
                msg: p.msg.clone(),
                value: p.value,
                stack: self.make_stack_list(&self.stack, 0),
                backtrace: self.error_backtrace_list(p.msg.as_str()),
            })
        };

        self.unwind_stack(why)
    }

    /// Push an error to the stack and raise it.
    pub(crate) fn push_error(&mut self, code: Error) -> ExecutionResult {
        trace!(?code, "push_error");
        self.set_return_value(v_err(code));
        // Check 'd' bit of running verb. If it's set, we raise the error. Otherwise nope.
        if let Some(activation) = self.stack.last() {
            if activation
                .verb_info
                .verbdef()
                .flags()
                .contains(VerbFlag::Debug)
            {
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
            if activation
                .verb_info
                .verbdef()
                .flags()
                .contains(VerbFlag::Debug)
            {
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
            if activation
                .verb_info
                .verbdef()
                .flags()
                .contains(VerbFlag::Debug)
            {
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
            match &mut a.frame {
                VmStackFrame::Moo(frame) => {
                    while frame.valstack.pop().is_some() {
                        // Check the handler stack to see if we've hit a finally or catch handler that
                        // was registered for this position in the value stack.
                        let Some(handler) = frame.pop_applicable_handler() else {
                            continue;
                        };

                        match handler.handler_type {
                            HandlerType::Finally(label) => {
                                let why_code = why.code();
                                if why_code == FinallyReason::Abort.code() {
                                    continue;
                                }
                                // Jump to the label pointed to by the finally label and then continue on
                                // executing.
                                frame.jump(&label);
                                frame.push(v_int(why_code as i64));
                                trace!(jump = ?label, ?why, "matched finally handler");
                                return ExecutionResult::More;
                            }
                            HandlerType::Catch(_) => {
                                let FinallyReason::Raise { code, .. } = &why else {
                                    continue;
                                };

                                let Some(handler) = frame.pop_applicable_handler() else {
                                    continue;
                                };
                                let HandlerType::CatchLabel(pushed_label) = &handler.handler_type
                                else {
                                    panic!("Expected CatchLabel");
                                };

                                // The value at the top of the stack could be the error codes list.
                                let v = frame.pop();
                                let found = match v.variant() {
                                    Variant::List(error_codes) => {
                                        error_codes.contains(&v_err(*code))
                                    }
                                    _ => true,
                                };
                                if found {
                                    frame.jump(pushed_label);
                                    frame.push(v_list(&[v_err(*code)]));
                                    return ExecutionResult::More;
                                }
                            }
                            HandlerType::CatchLabel(_) => {
                                unreachable!("CatchLabel where we didn't expect it...")
                            }
                        }
                    }

                    // Exit with a jump.. let's go...
                    if let FinallyReason::Exit { label, .. } = why {
                        frame.jump(&label);
                        return ExecutionResult::More;
                    }
                }
                VmStackFrame::Bf(_) => {
                    // TODO: unwind builtin function frames here in a way that takes their
                    //   `return_value` (and maybe error state/) and propagates it up the stack.
                    //   This way things like push_bf_err can be removed.
                    //   This might involve encompassing some of the stuff below, too.
                }
            }

            // If we're doing a return, and this is the last activation, we're done and just pass
            // the returned value up out of the interpreter loop.
            // Otherwise pop off this activation, and continue unwinding.
            if let FinallyReason::Return(value) = &why {
                if self.stack.len() == 1 {
                    return ExecutionResult::Complete(value.clone());
                }
            }

            if let FinallyReason::Uncaught(UncaughtException { .. }) = &why {
                return ExecutionResult::Exception(why);
            }

            self.stack.pop().expect("Stack underflow");

            if self.stack.is_empty() {
                return ExecutionResult::Complete(v_none());
            }

            // TODO builtin function unwinding stuff

            // If it was an explicit return that brought us here, set the return value explicitly.
            // (Unless we're the final activation, in which case that should have been handled
            // above)
            if let FinallyReason::Return(value) = &why {
                self.set_return_value(value.clone());
                return ExecutionResult::More;
            }
        }

        // We realistically should not get here...
        unreachable!("Unwound stack to empty, but no exit condition was hit");
    }
}
