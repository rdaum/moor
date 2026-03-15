// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use std::sync::LazyLock;
use std::time::{Duration, SystemTime};

use tracing::warn;

use moor_common::matching::ParsedCommand;
use moor_common::model::{
    DispatchFlagsSource, ObjFlag, ResolvedVerb, VerbDispatch, VerbFlag, VerbLookup, WorldStateError,
};
use moor_common::tasks::{Exception, TaskId};
use moor_common::util::BitEnum;
use moor_common::util::Instant;
use moor_compiler::{BUILTINS, to_literal};
use moor_var::{
    E_INVIND, E_PERM, E_TYPE, E_VERBNF, Error, List, NOTHING, Obj, SYSTEM_OBJECT, Sequence, Symbol,
    Var, Variant, program::names::GlobalName, v_arc_str, v_bool, v_empty_str, v_err, v_error,
    v_int, v_list, v_none, v_obj, v_str, v_string,
};

use crate::activation::CallProgram;
use crate::moo_execute::{ExecutionResult, Fork, VerbExecutionRequest};
use crate::{Activation, CatchType, FinallyReason, Frame, PhantomUnsync, ScopeType, VmHost};

static LIST_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("list_proto"));
static MAP_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("map_proto"));
static STRING_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("str_proto"));
static INTEGER_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("int_proto"));
static FLOAT_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("float_proto"));
static ERROR_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("err_proto"));
static BOOL_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("bool_proto"));
static SYM_PROTO_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("sym_proto"));

/// Per-task snapshot of program cache performance counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProgramCacheLocalSnapshot {
    pub hits: i64,
    pub misses: i64,
    pub inserts: i64,
    pub reclaimed: i64,
}

// {this, verb-name, programmer, verb-loc, player, line-number}
#[derive(Clone)]
pub struct Caller {
    pub this: Var,
    pub verb_name: Symbol,
    pub programmer: Obj,
    pub definer: Obj,
    pub player: Obj,
    pub line_number: usize,
}

/// Represents the state of VM execution for a given task.
#[derive(Debug)]
pub struct ExecState {
    /// The task ID of the task that for current stack of activations.
    pub task_id: TaskId,
    /// The stack of activation records / stack frames.
    pub stack: Vec<Activation>,
    /// The tick slice for the current/next execution.
    pub tick_slice: usize,
    /// The total number of ticks that the task is allowed to run.
    pub max_ticks: usize,
    /// The number of ticks that have been executed so far.
    pub tick_count: usize,
    /// The time at which the task was started.
    pub start_time: Option<SystemTime>,
    /// Monotonic start time used for runtime limit checks and time_left calculations.
    pub start_instant: Option<Instant>,
    /// The amount of time the task is allowed to run.
    pub maximum_time: Option<Duration>,
    /// Pending error to raise when execution resumes
    pub pending_raise_error: Option<moor_var::Error>,
    /// Program-cache stats for the currently running task.
    pub program_cache_stats: ProgramCacheLocalSnapshot,
    /// Current cache occupancy (task-local cache).
    pub program_cache_total_slots: usize,
    pub program_cache_live_slots: usize,
    pub program_cache_key_count: usize,

    pub unsync: PhantomUnsync,
}

impl ExecState {
    pub fn new(task_id: TaskId, max_ticks: usize) -> Self {
        Self {
            task_id,
            stack: Vec::with_capacity(32),
            tick_count: 0,
            start_time: None,
            start_instant: None,
            max_ticks,
            tick_slice: 0,
            maximum_time: None,
            pending_raise_error: None,
            program_cache_stats: ProgramCacheLocalSnapshot::default(),
            program_cache_total_slots: 0,
            program_cache_live_slots: 0,
            program_cache_key_count: 0,
            unsync: Default::default(),
        }
    }

    /// Return the callers stack, in the format expected by the `callers` built-in function.
    pub fn callers(&self) -> Vec<Caller> {
        let mut callers_iter = self.stack.iter().rev();
        callers_iter.next(); // skip the top activation, that's our current frame

        let mut callers = vec![];
        for activation in callers_iter {
            let verb_name = activation.verb_name;
            let definer = activation.verb_definer();
            let player = activation.player;
            let line_number = activation.frame.find_line_no().unwrap_or(0);
            let this = activation.this.clone();
            let perms = activation.permissions;
            let programmer = match activation.frame {
                Frame::Bf(_) => NOTHING,
                _ => perms,
            };
            callers.push(Caller {
                verb_name,
                definer,
                player,
                line_number,
                this,
                programmer,
            });
        }
        callers
    }

    pub fn top_mut(&mut self) -> &mut Activation {
        self.stack.last_mut().expect("activation stack underflow")
    }

    pub fn top(&self) -> &Activation {
        self.stack.last().expect("activation stack underflow")
    }

    /// Try to get the top activation without panicking. Returns None if stack is empty.
    /// Used for diagnostics and scheduler queries where stack emptiness is not a bug.
    pub fn try_top(&self) -> Option<&Activation> {
        self.stack.last()
    }

    /// Return the object that called the current activation.
    pub fn caller(&self) -> Var {
        let stack_iter = self.stack.iter().rev();

        // Skip builtin-frames (for now?)
        for activation in stack_iter {
            if let Frame::Bf(_) = activation.frame {
                continue;
            }
            return activation.this.clone();
        }
        v_obj(NOTHING)
    }

    /// Return the permissions of the caller of the current activation.
    pub fn caller_perms(&self) -> Obj {
        // Walk the stack backwards, skipping builtin frames (checking them for overrides)
        // and skipping the first non-builtin frame (current), returning the second non-builtin
        // frame's permissions (the caller).
        let stack_iter = self.stack.iter().rev();
        let mut non_builtin_count = 0;

        for activation in stack_iter {
            // Check if this is a builtin frame with an override
            if let Frame::Bf(bf_frame) = &activation.frame {
                if let Some(override_perms) = bf_frame.caller_perms_override {
                    return override_perms;
                }
                // Regular builtin frame without override - skip it
                continue;
            }

            // This is a non-builtin frame
            non_builtin_count += 1;

            // Skip the first non-builtin (current frame)
            if non_builtin_count == 1 {
                continue;
            }

            // Return the second non-builtin (caller frame)
            return activation.permissions;
        }

        // No caller found
        NOTHING
    }

    /// Return the permissions of the current task, which is the "starting"
    /// permissions of the current task, but note that this can be modified by
    /// the `set_task_perms` built-in function.
    pub fn task_perms(&self) -> Obj {
        let stack_top = self.stack.iter().rev().find(|a| !a.is_builtin_frame());
        stack_top.map(|a| a.permissions).unwrap_or(NOTHING)
    }

    /// Return the cached flags for the task permissions object.
    pub fn task_perms_flags(&self) -> BitEnum<ObjFlag> {
        let stack_top = self.stack.iter().rev().find(|a| !a.is_builtin_frame());
        stack_top
            .map(|a| a.permissions_flags)
            .unwrap_or_else(BitEnum::new)
    }

    pub fn this(&self) -> Var {
        let stack_top = self.stack.iter().rev().find(|a| !a.is_builtin_frame());
        stack_top.map(|a| a.this.clone()).unwrap_or(v_obj(NOTHING))
    }

    /// Update the permissions of the current task, as called by the `set_task_perms`
    /// built-in.
    pub fn set_task_perms(&mut self, host: &mut impl VmHost, perms: Obj) {
        // Look up the flags for the new perms object
        let perms_flags = host.flags_of(&perms).unwrap_or_default();

        // Copy the stack perms up to the last non-builtin. That is, make sure builtin-frames
        // get the permissions update, and the first non-builtin, too.
        for activation in self.stack.iter_mut().rev() {
            activation.permissions = perms;
            activation.permissions_flags = perms_flags;
            if !activation.is_builtin_frame() {
                break;
            }
        }
    }

    /// Push a value onto the value stack
    pub fn set_return_value(&mut self, v: Var) {
        self.top_mut().frame.set_return_value(v);
    }

    #[inline]
    pub fn mark_started_now(&mut self) {
        self.start_time = Some(SystemTime::now());
        self.start_instant = Some(Instant::now());
    }

    #[inline]
    pub fn elapsed_runtime(&self) -> Option<Duration> {
        if let Some(start) = self.start_instant {
            return Some(start.elapsed());
        }
        let start_time = self.start_time?;
        let now = SystemTime::now();
        now.duration_since(start_time).ok()
    }

    pub fn time_left(&self) -> Option<Duration> {
        let max_time = self.maximum_time?;
        let elapsed = self.elapsed_runtime()?;
        max_time.checked_sub(elapsed)
    }

    pub fn materialize_frame_programs(&mut self) {
        for activation in &mut self.stack {
            if let Frame::Moo(frame) = &mut activation.frame {
                frame.materialize_program_from_slot();
            }
        }
    }
}

impl ExecState {
    /// Compose a list of the current stack frames, starting from `start_frame_num` and working
    /// upwards.
    pub fn make_stack_list(activations: &[Activation]) -> Vec<Var> {
        let mut stack_list = Vec::with_capacity(activations.len());
        for a in activations.iter().rev() {
            // Produce traceback line for each activation frame and append to stack_list
            // Should include line numbers (if possible), the name of the currently running verb,
            // its definer, its location, and the current player, and 'this'.
            let line_no = match a.frame.find_line_no() {
                None => v_none(),
                Some(l) => v_int(l as i64),
            };
            match &a.frame {
                Frame::Moo(_) => stack_list.push(v_list(&[
                    a.this.clone(),
                    v_str(&a.verb_name.as_string()),
                    v_obj(a.permissions),
                    v_obj(a.verb_definer()),
                    v_obj(a.player),
                    line_no,
                ])),
                Frame::Bf(bf_frame) => {
                    let bf_name = BUILTINS.name_of(bf_frame.bf_id).unwrap();
                    stack_list.push(v_list(&[
                        a.this.clone(),
                        v_arc_str(bf_name.as_arc_str()),
                        v_obj(a.permissions),
                        v_obj(NOTHING),
                        v_obj(a.player),
                        v_int(0),
                    ]));
                }
            }
        }
        stack_list
    }

    /// Compose a backtrace list of strings for an error, starting from the current stack frame.
    pub fn make_backtrace(activations: &[Activation], error: &Error) -> Vec<Var> {
        // Walk live activation frames and produce a written representation of a traceback for each
        // frame.
        let mut backtrace_list = Vec::with_capacity(activations.len() + 1);
        for (i, a) in activations.iter().rev().enumerate() {
            let mut piece = String::new();
            if i != 0 {
                piece.push_str("... called from ");
            }
            match &a.frame {
                Frame::Moo(_) => {
                    piece.push_str(&format!("{}:{}", a.verb_definer(), a.verb_name));
                }
                Frame::Bf(bf_frame) => {
                    let bf_name = BUILTINS.name_of(bf_frame.bf_id).unwrap();
                    piece.push_str(&format!("builtin {bf_name}"));
                }
            }
            if v_obj(a.verb_definer()) != a.this {
                piece.push_str(&format!(" (this == {})", to_literal(&a.this)));
            }
            if let Some(line_num) = a.frame.find_line_no() {
                piece.push_str(&format!(" (line {line_num})"));
            }
            if i == 0 {
                let raise_msg = format!("{} ({})", error.err_type, error.message());
                piece.push_str(&format!(": {raise_msg}"));
            }
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
    pub fn push_error(&mut self, error: Error) -> ExecutionResult {
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
                a.verb_name.as_arc_str(),
                a.frame.find_line_no()
            )
        });

        warn!(error = ?error, verb = ?verb_this_name, "Pushing error from !d verb");
        ExecutionResult::More
    }

    /// Only raise an error if the 'd' bit is set on the running verb. Most times this is what we
    /// want.
    pub fn raise_error(&mut self, error: Error) -> ExecutionResult {
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
    pub fn unwind_stack(&mut self, why: FinallyReason) -> ExecutionResult {
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
                                frame.finally_stack.push(why);
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
                                                .unwrap_or(v_int(0));
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
            self.stack.pop();

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

impl ExecState {
    /// Entry point from scheduler for actually beginning the dispatch of a method execution
    /// (verb-to-verb call) in this VM.
    /// Actually creates the activation record and puts it on the stack.
    #[allow(clippy::too_many_arguments)]
    pub fn exec_call_request(
        &mut self,
        permissions_flags: BitEnum<ObjFlag>,
        resolved_verb: ResolvedVerb,
        verb_name: Symbol,
        this: Var,
        player: Obj,
        args: List,
        caller: Var,
        argstr: Var,
        program: CallProgram,
    ) {
        // Get current activation to inherit global variables from, if any.
        let current_activation = self.stack.last();

        let a = Activation::for_call(
            resolved_verb,
            permissions_flags,
            verb_name,
            this,
            player,
            args,
            caller,
            argstr,
            current_activation,
            program,
        );
        self.stack.push(a);
    }

    /// Entry point from scheduler for beginning the dispatch of an initial command verb execution.
    /// This sets up the initial activation with parsing variables from the parsed command.
    #[allow(clippy::too_many_arguments)]
    pub fn exec_command_request(
        &mut self,
        permissions_flags: BitEnum<ObjFlag>,
        resolved_verb: ResolvedVerb,
        verb_name: Symbol,
        this: Var,
        player: Obj,
        caller: Var,
        mut command: ParsedCommand,
        program: CallProgram,
    ) {
        let args: List = std::mem::take(&mut command.args).into_iter().collect();
        let argstr = v_string(std::mem::take(&mut command.argstr));

        // Initial command activation - no parent to inherit from
        let mut a = Activation::for_call(
            resolved_verb,
            permissions_flags,
            verb_name,
            this,
            player,
            args,
            caller,
            argstr,
            None,
            program,
        );

        // Set parsing variables from the parsed command
        a.frame
            .set_global_variable(GlobalName::dobj, v_obj(command.dobj.unwrap_or(NOTHING)));
        a.frame.set_global_variable(
            GlobalName::dobjstr,
            command.dobjstr.take().map_or_else(v_empty_str, v_string),
        );
        a.frame.set_global_variable(
            GlobalName::prepstr,
            command.prepstr.take().map_or_else(v_empty_str, v_string),
        );
        a.frame
            .set_global_variable(GlobalName::iobj, v_obj(command.iobj.unwrap_or(NOTHING)));
        a.frame.set_global_variable(
            GlobalName::iobjstr,
            command.iobjstr.take().map_or_else(v_empty_str, v_string),
        );

        self.stack.push(a);
    }

    /// Prepare a new stack & call hierarchy for invocation of a forked task.
    /// Called (ultimately) from the scheduler as the result of a fork() call.
    /// We get an activation record which is a copy of where it was forked from, and a new Program
    /// which is the new task's code, derived from a fork vector in the original task.
    pub fn exec_fork_vector(&mut self, fork_request: Fork) {
        // Set the activation up with the new task ID, and the new code.
        let mut a = fork_request.activation;

        // This makes sense only for a MOO stack frame, and could only be initiated from there,
        // so anything else is a legit panic, we shouldn't have gotten here.
        let Frame::Moo(ref mut frame) = a.frame else {
            panic!("Attempt to fork a non-MOO frame");
        };

        frame.switch_to_fork_vector(fork_request.fork_vector_offset);
        if let Some(task_id_name) = fork_request.task_id {
            frame.set_variable(&task_id_name, v_int(self.task_id as i64));
        }

        // TODO how to set the task_id in the parent activation, as we no longer have a reference
        //  to it?
        self.stack = vec![a];
    }

    /// Execute a lambda call by creating a new lambda activation
    pub fn exec_lambda_request(
        &mut self,
        lambda: moor_var::Lambda,
        args: List,
    ) -> Result<(), Error> {
        // Get current activation before borrowing self immutably
        let current_activation = self.top();
        let a = Activation::for_lambda_call(&lambda, current_activation, args.into_vec())?;
        self.stack.push(a);
        Ok(())
    }
}

/// Verb dispatch methods. These take a `VmHost` host to access world state
/// without requiring kernel-level TLS.
impl ExecState {
    /// Entry point for dispatching a verb (method) call.
    /// Called from the VM execution loop for CallVerb opcodes.
    pub fn verb_dispatch(
        &mut self,
        host: &mut impl VmHost,
        type_dispatch: bool,
        target: Var,
        verb: Symbol,
        args: List,
    ) -> Result<ExecutionResult, Error> {
        // Fast path: Obj is by far the most common case for verb dispatch
        if let Some(o) = target.as_object() {
            return Ok(self.prepare_call_verb(host, o, target, verb, args));
        }

        // Flyweight dispatches to its delegate
        if let Some(f) = target.as_flyweight() {
            return Ok(self.prepare_call_verb(host, *f.delegate(), target, verb, args));
        }

        // Primitive dispatch (int, string, float are most common)
        if !type_dispatch {
            return Err(E_TYPE.with_msg(|| {
                format!("Invalid target {:?} for verb dispatch", target.type_code())
            }));
        }

        // For primitives, look at type and dispatch to corresponding sysprop
        // e.g. "blah":reverse() becomes $string:reverse("blah")
        // Check common types first with direct accessors
        let sysprop_sym = if target.is_int() {
            *INTEGER_PROTO_SYM
        } else if target.is_string() {
            *STRING_PROTO_SYM
        } else if target.is_float() {
            *FLOAT_PROTO_SYM
        } else if target.is_list() {
            *LIST_PROTO_SYM
        } else {
            // Less common types - use variant()
            match target.variant() {
                Variant::Map(_) => *MAP_PROTO_SYM,
                Variant::Err(_) => *ERROR_PROTO_SYM,
                Variant::Sym(_) => *SYM_PROTO_SYM,
                Variant::Bool(_) => *BOOL_PROTO_SYM,
                _ => {
                    return Err(E_TYPE.with_msg(|| {
                        format!(
                            "Invalid target for verb dispatch: {}",
                            target.type_code().to_literal()
                        )
                    }));
                }
            }
        };
        let perms = self.top().permissions;
        let prop_val = host
            .retrieve_property(&perms, &SYSTEM_OBJECT, sysprop_sym)
            .map_err(|e| e.to_error())?;
        let Some(prop_val) = prop_val.as_object() else {
            return Err(E_TYPE.with_msg(|| {
                format!(
                    "Invalid target for verb dispatch: {}",
                    prop_val.type_code().to_literal()
                )
            }));
        };
        let arguments = args
            .insert(0, &target)
            .expect("Failed to insert object for dispatch");
        let Some(arguments) = arguments.as_list() else {
            return Err(E_TYPE.with_msg(|| {
                format!(
                    "Invalid arguments for verb dispatch: {}",
                    arguments.type_code().to_literal()
                )
            }));
        };
        Ok(self.prepare_call_verb(host, prop_val, v_obj(prop_val), verb, arguments.clone()))
    }

    pub fn prepare_call_verb(
        &mut self,
        host: &mut impl VmHost,
        location: Obj,
        this: Var,
        verb_name: Symbol,
        args: List,
    ) -> ExecutionResult {
        let caller = self.caller();

        // Only wizards can propagate a modified player value to called verbs.
        let activation_player = self.top().player;
        let player = if let Frame::Moo(frame) = &self.top().frame {
            frame
                .get_gvar(GlobalName::player)
                .and_then(|v| v.as_object())
                .filter(|fp| fp != &activation_player)
                .map_or(activation_player, |fp| {
                    let is_wiz = self.task_perms_flags().contains(ObjFlag::Wizard);
                    if is_wiz { fp } else { activation_player }
                })
        } else {
            activation_player
        };

        if !host.valid(&location).unwrap_or_default() {
            return self.push_error(
                E_INVIND.with_msg(|| format!("Invalid object ({location}) for verb dispatch")),
            );
        }

        let verb_result = host.dispatch_verb(
            &self.top().permissions,
            VerbDispatch::new(
                VerbLookup::method(&location, verb_name),
                DispatchFlagsSource::VerbOwner,
            ),
        );

        let (program_key, resolved_verb, permissions_flags) = match verb_result {
            Ok(Some(vi)) => (vi.program_key, vi.verbdef, vi.permissions_flags),
            Ok(None) => {
                return self.push_error(E_VERBNF.with_msg(|| {
                    format!(
                        "Verb {}:{} not found",
                        to_literal(&v_obj(location)),
                        verb_name,
                    )
                }));
            }
            Err(WorldStateError::ObjectPermissionDenied) => {
                return self.push_error(E_PERM.into());
            }
            Err(WorldStateError::RollbackRetry) => {
                return ExecutionResult::TaskRollbackRestart;
            }
            Err(WorldStateError::VerbPermissionDenied) => {
                return self.push_error(E_PERM.into());
            }
            Err(WorldStateError::VerbNotFound(_, _)) => {
                panic!("dispatch_verb() should return Ok(None), not VerbNotFound");
            }
            Err(e) => {
                panic!("Unexpected error from dispatch_verb: {e:?}")
            }
        };

        // Defer program materialization/slot resolution to VmHost so it can source programs
        // from the task-owned cache.
        ExecutionResult::DispatchVerb(Box::new(VerbExecutionRequest {
            permissions: self.top().permissions,
            permissions_flags,
            resolved_verb,
            verb_name,
            this,
            player,
            args,
            caller,
            argstr: v_empty_str(),
            program_key,
        }))
    }

    /// Setup the VM to execute the verb of the same current name, but using the parent's
    /// version.
    /// TODO this should be done up in task.rs instead. let's add a new ExecutionResult for it.
    pub fn prepare_pass_verb(&mut self, host: &mut impl VmHost, args: &List) -> ExecutionResult {
        // get parent of verb definer object & current verb name.
        let definer = self.top().verb_definer();
        let permissions = self.top().permissions;
        let verb = self.top().verb_name;

        let parent = match host.parent_of(&permissions, &definer) {
            Ok(p) => p,
            Err(WorldStateError::RollbackRetry) => {
                return ExecutionResult::TaskRollbackRestart;
            }
            Err(e) => return self.raise_error(e.to_error()),
        };

        if !host.valid(&parent).unwrap_or_default() {
            return self.push_error(E_INVIND.msg("Invalid object for pass() verb dispatch"));
        }

        let verb_result = host.dispatch_verb(
            &permissions,
            VerbDispatch::new(
                VerbLookup::method(&parent, verb),
                DispatchFlagsSource::Permissions,
            ),
        );

        let (program_key, resolved_verb, permissions_flags) = match verb_result {
            Ok(Some(vi)) => (vi.program_key, vi.verbdef, vi.permissions_flags),
            Ok(None) => {
                return self.push_error(E_VERBNF.msg("Verb not found for pass() dispatch"));
            }
            Err(WorldStateError::RollbackRetry) => {
                return ExecutionResult::TaskRollbackRestart;
            }
            Err(e) => return self.raise_error(e.to_error()),
        };

        let caller = self.caller();
        let this = self.top().this.clone();
        let player = self.top().player;
        let args_list = args.clone();
        ExecutionResult::DispatchVerb(Box::new(VerbExecutionRequest {
            permissions,
            permissions_flags,
            resolved_verb,
            verb_name: verb,
            this,
            player,
            args: args_list,
            caller,
            argstr: v_empty_str(),
            program_key,
        }))
    }

    pub fn exec_eval_request(
        &mut self,
        host: &mut impl VmHost,
        permissions: &Obj,
        player: &Obj,
        program: moor_compiler::Program,
        initial_env: Option<&[(Symbol, Var)]>,
    ) {
        let permissions_flags = host.flags_of(permissions).unwrap_or_default();
        let a = Activation::for_eval(
            *permissions,
            permissions_flags,
            player,
            program,
            initial_env,
        );
        self.stack.push(a);
    }

    /// If a bf_<xxx> wrapper function is present on #0, invoke that instead.
    pub fn maybe_invoke_bf_proxy(
        &mut self,
        host: &mut impl VmHost,
        bf_override_name: Symbol,
        args: &List,
    ) -> Option<ExecutionResult> {
        // Reject invocations of maybe-wrapper functions if the caller is #0.
        // This prevents recursion through them.
        if self.caller() == v_obj(SYSTEM_OBJECT) {
            return None;
        }

        // Look for it...
        let verb_result = host
            .dispatch_verb(
                &self.top().permissions,
                VerbDispatch::new(
                    VerbLookup::method(&SYSTEM_OBJECT, bf_override_name),
                    DispatchFlagsSource::Permissions,
                ),
            )
            .ok()?;
        let verb_result = verb_result?;
        let program_key = verb_result.program_key;
        let resolved_verb = verb_result.verbdef;
        let permissions_flags = verb_result.permissions_flags;

        let player = self.top().player;
        let caller = self.caller();
        let args_list = args.clone();
        let permissions = self.top().permissions;
        Some(ExecutionResult::DispatchVerb(Box::new(
            VerbExecutionRequest {
                permissions,
                permissions_flags,
                resolved_verb,
                verb_name: bf_override_name,
                this: v_obj(SYSTEM_OBJECT),
                player,
                args: args_list,
                caller,
                argstr: v_empty_str(),
                program_key,
            },
        )))
    }
}

// Manual Clone implementation because we need to create a new arena
impl Clone for ExecState {
    fn clone(&self) -> Self {
        Self {
            task_id: self.task_id,
            stack: self.stack.clone(),
            tick_slice: self.tick_slice,
            max_ticks: self.max_ticks,
            tick_count: self.tick_count,
            start_time: self.start_time,
            start_instant: self.start_instant,
            maximum_time: self.maximum_time,
            pending_raise_error: self.pending_raise_error.clone(),
            program_cache_stats: self.program_cache_stats,
            program_cache_total_slots: self.program_cache_total_slots,
            program_cache_live_slots: self.program_cache_live_slots,
            program_cache_key_count: self.program_cache_key_count,
            unsync: Default::default(),
        }
    }
}
