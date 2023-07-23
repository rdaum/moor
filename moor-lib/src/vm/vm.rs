use anyhow::bail;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::trace;

use crate::compiler::builtins::BUILTINS;
use crate::compiler::labels::{Label, Offset};
use crate::db::state::WorldState;
use crate::model::objects::ObjFlag;
use crate::model::verbs::VerbInfo;
use crate::model::ObjectError::{PropertyNotFound, PropertyPermissionDenied, VerbNotFound};
use crate::tasks::scheduler::TaskId;
use crate::tasks::Sessions;
use crate::util::bitenum::BitEnum;
use crate::var::error::Error::{E_INVIND, E_PERM, E_PROPNF, E_TYPE, E_VERBNF};
use crate::var::error::{Error, ErrorPack};
use crate::var::{v_err, v_int, v_list, v_objid, v_str, Objid, Var, Variant, NOTHING, VAR_NONE};
use crate::vm::activation::{Activation, Caller};
use crate::vm::bf_server::BfNoop;
use crate::vm::opcode::Op;

#[derive(Clone, Eq, PartialEq, Debug)]
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

#[async_trait]
pub(crate) trait BfFunction: Sync + Send {
    fn name(&self) -> &str;
    async fn call(
        &self,
        world_state: &mut dyn WorldState,
        frame: &mut Activation,
        sessions: Arc<RwLock<dyn Sessions>>,
        args: &[Var],
    ) -> Result<Var, anyhow::Error>;
}

pub struct VM {
    // Activation stack.
    stack: Vec<Activation>,
    pub(crate) bf_funcs: Vec<Arc<Box<dyn BfFunction>>>,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum ExecutionResult {
    Complete(Var),
    More,
    Exception(FinallyReason),
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    pub fn new() -> Self {
        let mut bf_funcs: Vec<Arc<Box<dyn BfFunction>>> = Vec::with_capacity(BUILTINS.len());
        for _ in 0..BUILTINS.len() {
            bf_funcs.push(Arc::new(Box::new(BfNoop {})))
        }
        let _bf_noop = Box::new(BfNoop {});
        let mut vm = Self {
            stack: vec![],
            bf_funcs,
        };

        vm.register_bf_server().unwrap();
        vm.register_bf_num().unwrap();
        vm.register_bf_values().unwrap();
        vm.register_bf_strings().unwrap();
        vm.register_bf_list_sets().unwrap();
        vm.register_bf_objects().unwrap();

        vm
    }

    fn find_handler_active(&mut self, raise_code: Error) -> Option<(usize, &Activation)> {
        // Scan activation frames and their stacks, looking for the first _Catch we can find.
        for a in self.stack.iter().rev() {
            let mut i = a.valstack.len();
            while i > 0 {
                if let Variant::_Catch(cnt) = a.valstack[i - 1].variant() {
                    // Found one, now scan forwards from 'cnt' backwards looking for either the first
                    // non-list value, or a list containing the error code.
                    // TODO check for 'cnt' being too large. not sure how to handle, tho
                    // TODO this actually i think is wrong, it needs to pull two values off the stack
                    for j in (i - cnt.0 as usize)..i {
                        if let Variant::List(codes) = &a.valstack[j].variant() {
                            if codes.contains(&v_err(raise_code)) {
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
                v_objid(a.this),
                v_str(a.verb_name()),
                v_objid(a.verb_definer()),
                v_objid(a.verb_owner()),
                v_objid(a.player),
                // TODO: find_line_number and add here.
            ];

            stack_list.push(v_list(traceback_entry));
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
            pieces.push(format!("{}:{}", a.verb_definer(), a.verb_name()));
            if a.verb_definer() != a.this {
                pieces.push(format!(" (this == {})", a.this.0));
            }
            // TODO line number
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

    pub(crate) fn push_error(&mut self, code: Error) -> Result<ExecutionResult, anyhow::Error> {
        trace!("push_error: {:?}", code);
        self.push(&v_err(code));
        self.raise_error_pack(code.make_error_pack(None))
    }

    fn push_error_msg(
        &mut self,
        code: Error,
        msg: String,
    ) -> Result<ExecutionResult, anyhow::Error> {
        trace!("push_error_msg: {:?} {:?}", code, msg);
        self.push(&v_err(code));
        self.raise_error_pack(code.make_error_pack(Some(msg)))
    }

    pub(crate) fn raise_error(&mut self, code: Error) -> Result<ExecutionResult, anyhow::Error> {
        trace!("raise_error: {:?}", code);
        self.raise_error_pack(code.make_error_pack(None))
    }

    pub(crate) fn unwind_stack(
        &mut self,
        why: FinallyReason,
    ) -> Result<ExecutionResult, anyhow::Error> {
        trace!("unwind_stack: {:?}", why);
        // Walk activation stack from bottom to top, tossing frames as we go.
        while let Some(a) = self.stack.last_mut() {
            // Pop the value stack seeking finally/catch handler values.
            while let Some(v) = a.valstack.pop() {
                match v.variant() {
                    Variant::_Finally(label) => {
                        /* FINALLY handler */
                        let why_num = why.code();
                        if why_num == FinallyReason::Abort.code() {
                            continue;
                        }
                        a.jump(*label);
                        a.push(v_int(why_num as i64));
                        return Ok(ExecutionResult::More);
                    }
                    Variant::_Catch(_label) => {
                        /* TRY-EXCEPT or `expr ! ...' handler */
                        let FinallyReason::Raise{code, value, ..} = &why else {
                            continue
                        };
                        // Jump further back the value stack looking for a list of errors + labels
                        // we will match on.
                        let mut found = false;
                        if a.valstack.len() >= 2 {
                            if let (Some(pushed_label), Some(error_codes)) =
                                (a.valstack.pop(), a.valstack.pop())
                            {
                                if let Variant::_Label(pushed_label) = pushed_label.variant() {
                                    if let Variant::List(error_codes) = error_codes.variant() {
                                        if error_codes.contains(&v_err(*code)) {
                                            a.jump(*pushed_label);
                                            found = true;
                                        }
                                    } else {
                                        a.jump(*pushed_label);
                                        found = true;
                                    }
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

            if let FinallyReason::Uncaught {
                code: _,
                msg: _,
                value: _,
                stack: _,
                backtrace: _,
            } = &why
            {
                return Ok(ExecutionResult::Exception(why));
            }

            self.stack.pop().expect("Stack underflow");

            if self.stack.is_empty() {
                return Ok(ExecutionResult::Complete(VAR_NONE));
            }
            // TODO builtin function unwinding stuff

            // If it was a return that brought us here, stick it onto the end of the next
            // activation's value stack.
            // (Unless we're the final activation, in which case that should have been handled
            // above)
            if let FinallyReason::Return(value) = why {
                self.push(&value);
                trace!(
                    "Unwinding stack, pushing return value: {} back to verb {}",
                    value,
                    self.top().verb_name()
                );
                return Ok(ExecutionResult::More);
            }
        }

        // We realistically should not get here...
        panic!("Unwound stack to empty, but no exit condition was hit");
    }

    pub(crate) fn top_mut(&mut self) -> &mut Activation {
        self.stack.last_mut().expect("activation stack underflow")
    }

    pub(crate) fn top(&self) -> &Activation {
        self.stack.last().expect("activation stack underflow")
    }

    pub(crate) fn pop(&mut self) -> Var {
        self.top_mut()
            .pop()
            .unwrap_or_else(|| panic!("stack underflow, activation depth: {}", self.stack.len()))
    }

    pub(crate) fn push(&mut self, v: &Var) {
        self.top_mut().push(v.clone())
    }

    pub(crate) fn next_op(&mut self) -> Option<Op> {
        self.top_mut().next_op()
    }

    pub(crate) fn jump(&mut self, label: Label) {
        self.top_mut().jump(label)
    }

    pub(crate) fn get_env(&mut self, id: Label) -> Var {
        self.top().environment[id.0 as usize].clone()
    }

    pub(crate) fn set_env(&mut self, id: Label, v: &Var) {
        self.top_mut().environment[id.0 as usize] = v.clone();
    }

    pub(crate) fn peek(&self, amt: usize) -> Vec<Var> {
        self.top().peek(amt)
    }

    pub(crate) fn peek_top(&self) -> Var {
        self.top().peek_top().expect("stack underflow")
    }

    pub(crate) fn get_prop(
        &mut self,
        state: &mut dyn WorldState,
        player_flags: BitEnum<ObjFlag>,
        propname: Var,
        obj: Var,
    ) -> Result<ExecutionResult, anyhow::Error> {
        let Variant::Str(propname) = propname.variant() else {
            return self.push_error(E_TYPE);
        };

        let Variant::Obj(obj) = obj.variant() else {
            return self.push_error(E_INVIND);
        };

        let result = state.retrieve_property(*obj, propname.as_str(), player_flags);
        let v = match result {
            Ok(v) => v,
            Err(e) => match e {
                PropertyPermissionDenied(_, _) => return self.push_error(E_PERM),
                PropertyNotFound(_, _) => return self.push_error(E_PROPNF),
                _ => {
                    panic!("Unexpected error in property retrieval: {:?}", e);
                }
            },
        };
        self.push(&v);
        Ok(ExecutionResult::More)
    }

    pub(crate) fn call_verb(
        &mut self,
        state: &mut dyn WorldState,
        this: Objid,
        verb: String,
        args: &[Var],
        do_pass: bool,
    ) -> Result<ExecutionResult, anyhow::Error> {
        let this = if do_pass {
            let valid_definer = state.valid(self.top().verb_definer())?;

            if !valid_definer {
                return self.push_error(E_INVIND);
            }
            state.parent_of(this)?
        } else {
            this
        };

        let self_valid = state.valid(this)?;
        if !self_valid {
            return self.push_error(E_INVIND);
        }
        // find callable verb
        let Ok(verbinfo) = state.find_method_verb_on(this, verb.as_str()) else {
            return self.push_error_msg(E_VERBNF, format!("Verb \"{}\" not found", verb));
        };
        let Some(binary) = verbinfo.attrs.program.clone() else {
            return self.push_error_msg(
                E_VERBNF,
                format!("Verb \"{}\" is not a program", verb),
            );
        };

        let caller = self.top().this;

        let top = self.top();
        let mut callers = top.callers.to_vec();
        let task_id = top.task_id;

        callers.push(Caller {
            this,
            verb_name: top.verb_name().to_string(),
            programmer: top.verb_owner(),
            verb_loc: top.verb_definer(),
            player: top.player,
            line_number: 0,
        });

        let a = Activation::new_for_method(
            task_id,
            binary,
            caller,
            this,
            top.player,
            top.player_flags,
            verbinfo,
            args,
            callers,
        )?;

        self.stack.push(a);
        trace!("call_verb: {}:{}({:?})", this, verb, args);
        Ok(ExecutionResult::More)
    }

    pub fn do_method_cmd(
        &mut self,
        task_id: TaskId,
        vi: VerbInfo,
        obj: Objid,
        verb_name: &str,
        this: Objid,
        player: Objid,
        player_flags: BitEnum<ObjFlag>,
        args: &[Var],
    ) -> Result<(), anyhow::Error> {
        let Some(binary) = vi.attrs.program.clone() else {
            bail!(VerbNotFound(obj, verb_name.to_string()))
        };

        let a = Activation::new_for_method(
            task_id,
            binary,
            NOTHING,
            this,
            player,
            player_flags,
            vi,
            args,
            vec![],
        )?;

        self.stack.push(a);
        trace!("do_method_cmd: {}:{}({:?})", this, verb_name, args);

        Ok(())
    }

    pub fn do_method_verb(
        &mut self,
        task_id: TaskId,
        state: &mut dyn WorldState,
        obj: Objid,
        verb_name: &str,
        this: Objid,
        player: Objid,
        player_flags: BitEnum<ObjFlag>,
        args: &[Var],
    ) -> Result<(), anyhow::Error> {
        let vi = state.find_method_verb_on(obj, verb_name)?;

        let Some(binary) = vi.attrs.program.clone() else {
            bail!(VerbNotFound(obj, verb_name.to_string()))
        };

        let a = Activation::new_for_method(
            task_id,
            binary,
            NOTHING,
            this,
            player,
            player_flags,
            vi,
            args,
            vec![],
        )?;

        self.stack.push(a);

        trace!("do_method_verb: {}:{}({:?})", this, verb_name, args);

        Ok(())
    }
}
