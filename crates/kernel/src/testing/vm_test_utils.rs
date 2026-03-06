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

//! VM testing utilities for executing verbs, eval, and forks in test environments

use std::{cmp::max, sync::Arc, time::Duration};

use moor_common::matching::ParsedCommand;
use moor_common::model::{DispatchFlagsSource, ObjFlag, VerbDispatch, VerbLookup, WorldState};
use moor_common::util::BitEnum;
use moor_compiler::Program;
use moor_var::{
    List, Obj, SYSTEM_OBJECT, Symbol, Var, program::names::GlobalName, v_empty_str, v_obj,
    v_symbol_str,
};
use strum::EnumCount;

use crate::{
    config::FeaturesConfig,
    task_context::TaskGuard,
    tasks::{task_program_cache::TaskProgramCache, task_scheduler_client::TaskSchedulerClient},
    vm::{VMHostResponse, builtins::BuiltinRegistry, vm_host::VmHost},
};

use moor_common::tasks::{Exception, Session};

pub type ExecResult = Result<Var, Exception>;

/// Setup test task context with proper task scheduler client
pub fn setup_task_context(world_state: Box<dyn WorldState>) -> TaskGuard {
    let (scs_tx, _scs_rx) = flume::unbounded();
    let task_scheduler_client = TaskSchedulerClient::new(0, scs_tx);
    let session = std::sync::Arc::new(moor_common::tasks::NoopClientSession::new());
    TaskGuard::new(
        world_state,
        task_scheduler_client,
        0,
        moor_var::NOTHING,
        session,
    )
}

fn execute_fork(
    session: Arc<dyn Session>,
    builtins: &BuiltinRegistry,
    fork_request: crate::vm::Fork,
    task_id: usize,
) -> ExecResult {
    // For testing, forks execute in the same transaction context as the parent

    let mut vm_host = VmHost::new(task_id, 20, 90_000, Duration::from_secs(5));

    vm_host.start_fork(task_id, &fork_request, false);

    let config = Arc::new(FeaturesConfig::default());
    let mut program_cache = TaskProgramCache::default();

    // Execute the forked task until completion
    loop {
        let exec_result = vm_host.exec_interpreter(
            task_id,
            session.as_ref(),
            builtins,
            config.as_ref(),
            &mut program_cache,
        );
        match exec_result {
            VMHostResponse::ContinueOk => {
                continue;
            }
            VMHostResponse::DispatchFork(nested_fork) => {
                // Execute nested fork - if it fails, propagate the error
                let nested_result =
                    execute_fork(session.clone(), builtins, *nested_fork, task_id + 1);
                nested_result?;
                continue;
            }
            VMHostResponse::AbortLimit(a) => {
                panic!("Fork task aborted: {a:?}");
            }
            VMHostResponse::CompleteException(e) => {
                return Err(e.as_ref().clone());
            }
            VMHostResponse::CompleteSuccess(v) => {
                return Ok(v);
            }
            VMHostResponse::CompleteAbort => {
                panic!("Fork task aborted");
            }
            VMHostResponse::Suspend(_) => {
                panic!("Fork task suspended");
            }
            VMHostResponse::SuspendNeedInput(_) => {
                panic!("Fork task needs input");
            }
            VMHostResponse::RollbackRetry => {
                panic!("Fork task rollback retry");
            }
            VMHostResponse::CompleteRollback(_) => {
                panic!("Fork task rollback");
            }
        }
    }
}

fn execute<F>(
    world_state: Box<dyn WorldState>,
    session: Arc<dyn Session>,
    builtins: BuiltinRegistry,
    fun: F,
) -> ExecResult
where
    F: FnOnce(&mut VmHost),
{
    let mut vm_host = VmHost::new(0, 20, 90_000, Duration::from_secs(5));

    let _tx_guard = setup_task_context(world_state);

    fun(&mut vm_host);

    let config = Arc::new(FeaturesConfig::default());
    let mut program_cache = TaskProgramCache::default();

    // Call repeatedly into exec until we ge either an error or Complete.
    loop {
        let exec_result = vm_host.exec_interpreter(
            0,
            session.as_ref(),
            &builtins,
            config.as_ref(),
            &mut program_cache,
        );
        match exec_result {
            VMHostResponse::ContinueOk => {
                continue;
            }
            VMHostResponse::DispatchFork(f) => {
                // For testing, execute the fork separately (sequentially)
                // If the fork fails, propagate the error to terminate main execution
                let fork_result = execute_fork(session.clone(), &builtins, *f, 1);
                fork_result?;
                // Continue main execution after successful fork dispatch
                continue;
            }
            VMHostResponse::AbortLimit(a) => {
                panic!("Unexpected abort: {a:?}");
            }
            VMHostResponse::CompleteException(e) => {
                return Err(e.as_ref().clone());
            }
            VMHostResponse::CompleteSuccess(v) => {
                return Ok(v);
            }
            VMHostResponse::CompleteAbort => {
                panic!("Unexpected abort");
            }
            VMHostResponse::Suspend(_) => {
                panic!("Unexpected suspend");
            }
            VMHostResponse::SuspendNeedInput(_) => {
                panic!("Unexpected suspend need input");
            }
            VMHostResponse::RollbackRetry => {
                panic!("Unexpected rollback retry");
            }
            VMHostResponse::CompleteRollback(_) => {
                panic!("Unexpected rollback");
            }
        }
    }
}

pub fn call_verb(
    world_state: Box<dyn WorldState>,
    session: Arc<dyn Session>,
    builtins: BuiltinRegistry,
    verb_name: &str,
    args: List,
) -> ExecResult {
    // Set up the verb call before starting transaction context
    let verb_name = Symbol::mk(verb_name);
    let verb_result = world_state
        .dispatch_verb(
            &SYSTEM_OBJECT,
            VerbDispatch::new(
                VerbLookup::method(&SYSTEM_OBJECT, verb_name),
                DispatchFlagsSource::Permissions,
            ),
        )
        .unwrap()
        .unwrap();
    let program = world_state
        .retrieve_verb(
            &SYSTEM_OBJECT,
            &verb_result.program_key.verb_definer,
            verb_result.program_key.verb_uuid,
        )
        .unwrap()
        .0;
    let verbdef = verb_result.verbdef;

    // Use wizard + programmer flags for testing
    let permissions_flags = BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer;
    execute(world_state, session, builtins, |vm_host| {
        vm_host.start_call_method_verb(
            0,
            verbdef,
            verb_name,
            v_obj(SYSTEM_OBJECT),
            SYSTEM_OBJECT,
            args,
            v_obj(SYSTEM_OBJECT),
            v_empty_str(),
            permissions_flags,
            program,
        );
    })
}

pub fn call_eval_builtin(
    world_state: Box<dyn WorldState>,
    session: Arc<dyn Session>,
    builtins: BuiltinRegistry,
    player: Obj,
    program: Program,
) -> ExecResult {
    execute(world_state, session, builtins, |vm_host| {
        vm_host.start_eval(0, &player, program, None);
    })
}

pub fn call_eval_builtin_with_env(
    world_state: Box<dyn WorldState>,
    session: Arc<dyn Session>,
    builtins: BuiltinRegistry,
    player: Obj,
    program: Program,
    initial_env: &[(Symbol, Var)],
) -> ExecResult {
    execute(world_state, session, builtins, |vm_host| {
        vm_host.start_eval(0, &player, program, Some(initial_env));
    })
}

pub fn call_fork(
    session: Arc<dyn Session>,
    builtins: BuiltinRegistry,
    fork_request: crate::vm::Fork,
) -> ExecResult {
    execute_fork(session, &builtins, fork_request, 0)
}

/// Opaque wrapper for benchmarking Activation creation without exposing internals.
/// Holds the result of creating an activation frame.
pub struct ActivationBenchResult {
    inner: crate::vm::activation::Activation,
}

impl ActivationBenchResult {
    /// Get a reference to the inner activation for use as a parent frame.
    pub(crate) fn as_ref(&self) -> &crate::vm::activation::Activation {
        &self.inner
    }
}

/// Opaque wrapper for benchmarking MooStackFrame construction without exposing internals.
/// Holds the result of creating a raw MOO frame.
pub struct MooFrameBenchResult {
    inner: crate::vm::moo_frame::MooStackFrame,
}

impl MooFrameBenchResult {
    pub(crate) fn as_ref(&self) -> &crate::vm::moo_frame::MooStackFrame {
        &self.inner
    }
}

/// Opaque state for directly benchmarking activation assembly around a prebuilt MOO frame.
/// This reuses owned values across iterations to avoid measuring clone/setup overhead.
pub struct ActivationAssemblyBenchState {
    frame: Option<crate::vm::moo_frame::MooStackFrame>,
    this: Option<Var>,
    args: Option<List>,
    verbdef: Option<moor_common::model::ResolvedVerb>,
    player: Obj,
    verb_name: Symbol,
    permissions_flags: BitEnum<ObjFlag>,
}

/// Opaque wrapper for benchmarking Environment construction without exposing internals.
/// Holds the result of creating a raw environment for a frame.
#[allow(dead_code)]
pub struct EnvironmentBenchResult {
    inner: crate::vm::environment::Environment,
}

/// Create a top-level MooStackFrame for benchmarking purposes.
/// This isolates `MooStackFrame::new_with_all_globals`.
#[allow(clippy::too_many_arguments)]
#[allow(irrefutable_let_patterns)]
pub fn create_top_level_moo_frame_for_bench(
    verb_name: Symbol,
    this: Var,
    player: Obj,
    args: List,
    caller: Var,
    argstr: Var,
    program: moor_var::program::ProgramType,
) -> MooFrameBenchResult {
    let moor_var::program::ProgramType::MooR(program) = program else {
        unimplemented!("Only MOO programs are supported");
    };

    let frame = crate::vm::moo_frame::MooStackFrame::new_with_all_globals(
        program,
        v_obj(player),
        this,
        caller,
        v_symbol_str(verb_name),
        args.into(),
        argstr,
    );
    MooFrameBenchResult { inner: frame }
}

/// Create a nested MooStackFrame for benchmarking purposes.
/// This isolates `MooStackFrame::new_with_globals_from_source`.
#[allow(clippy::too_many_arguments)]
#[allow(irrefutable_let_patterns)]
pub fn create_nested_moo_frame_for_bench(
    verb_name: Symbol,
    this: Var,
    player: Obj,
    args: List,
    caller: Var,
    source: &MooFrameBenchResult,
    program: moor_var::program::ProgramType,
) -> MooFrameBenchResult {
    let moor_var::program::ProgramType::MooR(program) = program else {
        unimplemented!("Only MOO programs are supported");
    };

    let frame = crate::vm::moo_frame::MooStackFrame::new_with_globals_from_source(
        program,
        v_obj(player),
        this,
        caller,
        v_symbol_str(verb_name),
        args.into(),
        source.as_ref(),
    );
    MooFrameBenchResult { inner: frame }
}

/// Prepare reusable state for direct activation assembly benchmarking from a prebuilt frame.
#[allow(clippy::too_many_arguments)]
pub fn create_activation_assembly_state_for_bench(
    verbdef: moor_common::model::ResolvedVerb,
    verb_name: Symbol,
    this: Var,
    player: Obj,
    args: List,
    frame: MooFrameBenchResult,
) -> ActivationAssemblyBenchState {
    ActivationAssemblyBenchState {
        frame: Some(frame.inner),
        this: Some(this),
        args: Some(args),
        verbdef: Some(verbdef),
        player,
        verb_name,
        permissions_flags: BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer,
    }
}

/// Execute one direct activation assembly cycle using prebuilt state.
/// This isolates struct assembly/disassembly with a prebuilt `MooStackFrame`.
pub fn run_activation_assembly_cycle_for_bench(state: &mut ActivationAssemblyBenchState) {
    let frame = state
        .frame
        .take()
        .expect("activation assembly bench state missing frame");
    let this = state
        .this
        .take()
        .expect("activation assembly bench state missing this");
    let args = state
        .args
        .take()
        .expect("activation assembly bench state missing args");
    let verbdef = state
        .verbdef
        .take()
        .expect("activation assembly bench state missing verbdef");

    let activation = crate::vm::activation::Activation {
        frame: crate::vm::activation::Frame::Moo(frame),
        this,
        player: state.player,
        args,
        verb_name: state.verb_name,
        permissions: verbdef.owner(),
        verbdef,
        permissions_flags: state.permissions_flags,
    };
    let activation = std::hint::black_box(activation);

    let crate::vm::activation::Frame::Moo(frame) = activation.frame else {
        unreachable!("activation assembly bench uses only MOO frames")
    };
    state.frame = Some(frame);
    state.this = Some(activation.this);
    state.args = Some(activation.args);
    state.verbdef = Some(activation.verbdef);
}

/// Execute one cycle of activation assembly state plumbing without constructing an Activation.
/// This isolates benchmark harness overhead for corrected assembly attribution.
pub fn run_activation_assembly_cycle_overhead_for_bench(state: &mut ActivationAssemblyBenchState) {
    let frame = state
        .frame
        .take()
        .expect("activation assembly bench state missing frame");
    let this = state
        .this
        .take()
        .expect("activation assembly bench state missing this");
    let args = state
        .args
        .take()
        .expect("activation assembly bench state missing args");
    let verbdef = state
        .verbdef
        .take()
        .expect("activation assembly bench state missing verbdef");

    let (frame, this, args, verbdef) = std::hint::black_box((frame, this, args, verbdef));

    state.frame = Some(frame);
    state.this = Some(this);
    state.args = Some(args);
    state.verbdef = Some(verbdef);
}

/// Create a top-level Environment for benchmarking purposes.
/// This isolates top-level call global environment construction.
#[allow(clippy::too_many_arguments)]
#[allow(irrefutable_let_patterns)]
pub fn create_top_level_environment_for_bench(
    verb_name: Symbol,
    this: Var,
    player: Obj,
    args: List,
    caller: Var,
    argstr: Var,
    program: moor_var::program::ProgramType,
) -> EnvironmentBenchResult {
    let moor_var::program::ProgramType::MooR(program) = program else {
        unimplemented!("Only MOO programs are supported");
    };
    let width = max(program.var_names().global_width(), GlobalName::COUNT);

    let env = crate::vm::environment::Environment::with_call_globals(
        v_obj(player),
        this,
        caller,
        v_symbol_str(verb_name),
        args.into(),
        argstr,
        width,
    );

    EnvironmentBenchResult { inner: env }
}

/// Create a nested Environment for benchmarking purposes.
/// This isolates nested call global construction with parsing-global copy.
#[allow(clippy::too_many_arguments)]
#[allow(irrefutable_let_patterns)]
pub fn create_nested_environment_for_bench(
    verb_name: Symbol,
    this: Var,
    player: Obj,
    args: List,
    caller: Var,
    source: &MooFrameBenchResult,
    program: moor_var::program::ProgramType,
) -> EnvironmentBenchResult {
    let moor_var::program::ProgramType::MooR(program) = program else {
        unimplemented!("Only MOO programs are supported");
    };
    let width = max(program.var_names().global_width(), GlobalName::COUNT);

    let env = crate::vm::environment::Environment::with_call_globals_copy_parsing(
        v_obj(player),
        this,
        caller,
        v_symbol_str(verb_name),
        args.into(),
        &source.as_ref().environment,
        width,
    );

    EnvironmentBenchResult { inner: env }
}

/// Create an Activation for benchmarking purposes.
/// This exposes `Activation::for_call` for micro-benchmarking frame creation costs.
#[allow(clippy::too_many_arguments)]
pub fn create_activation_for_bench(
    _permissions: Obj,
    verbdef: moor_common::model::ResolvedVerb,
    verb_name: Symbol,
    this: Var,
    player: Obj,
    args: List,
    caller: Var,
    argstr: Var,
    program: moor_var::program::ProgramType,
) -> ActivationBenchResult {
    // Use wizard + programmer flags for benchmarking
    let permissions_flags = BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer;
    let activation = crate::vm::activation::Activation::for_call(
        verbdef,
        permissions_flags,
        verb_name,
        this,
        player,
        args,
        caller,
        argstr,
        None, // No parent activation for top-level calls
        crate::vm::vm_call::CallProgram::Materialized(program),
    );
    ActivationBenchResult { inner: activation }
}

/// Create a command activation for benchmarking purposes.
/// Mirrors `exec_command_request` setup: args/argstr come from `ParsedCommand`.
#[allow(clippy::too_many_arguments)]
pub fn create_command_activation_for_bench(
    _permissions: Obj,
    verbdef: moor_common::model::ResolvedVerb,
    verb_name: Symbol,
    this: Var,
    player: Obj,
    caller: Var,
    mut command: ParsedCommand,
    program: moor_var::program::ProgramType,
) -> ActivationBenchResult {
    let permissions_flags = BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer;
    let args: List = std::mem::take(&mut command.args).into_iter().collect();
    let argstr = moor_var::v_string(std::mem::take(&mut command.argstr));

    let mut activation = crate::vm::activation::Activation::for_call(
        verbdef,
        permissions_flags,
        verb_name,
        this,
        player,
        args,
        caller,
        argstr,
        None,
        crate::vm::vm_call::CallProgram::Materialized(program),
    );

    activation.frame.set_global_variable(
        GlobalName::dobj,
        v_obj(command.dobj.unwrap_or(moor_var::NOTHING)),
    );
    activation.frame.set_global_variable(
        GlobalName::dobjstr,
        command
            .dobjstr
            .take()
            .map_or_else(v_empty_str, moor_var::v_string),
    );
    activation.frame.set_global_variable(
        GlobalName::prepstr,
        command
            .prepstr
            .take()
            .map_or_else(v_empty_str, moor_var::v_string),
    );
    activation.frame.set_global_variable(
        GlobalName::iobj,
        v_obj(command.iobj.unwrap_or(moor_var::NOTHING)),
    );
    activation.frame.set_global_variable(
        GlobalName::iobjstr,
        command
            .iobjstr
            .take()
            .map_or_else(v_empty_str, moor_var::v_string),
    );

    ActivationBenchResult { inner: activation }
}

/// Create an Activation with a parent frame for benchmarking nested verb calls.
/// This exercises the `with_globals_from_source` path which copies parsing globals.
#[allow(clippy::too_many_arguments)]
pub fn create_nested_activation_for_bench(
    _permissions: Obj,
    verbdef: moor_common::model::ResolvedVerb,
    verb_name: Symbol,
    this: Var,
    player: Obj,
    args: List,
    caller: Var,
    argstr: Var,
    parent: &ActivationBenchResult,
    program: moor_var::program::ProgramType,
) -> ActivationBenchResult {
    // Use wizard + programmer flags for benchmarking
    let permissions_flags = BitEnum::new_with(ObjFlag::Wizard) | ObjFlag::Programmer;
    let activation = crate::vm::activation::Activation::for_call(
        verbdef,
        permissions_flags,
        verb_name,
        this,
        player,
        args,
        caller,
        argstr,
        Some(parent.as_ref()),
        crate::vm::vm_call::CallProgram::Materialized(program),
    );
    ActivationBenchResult { inner: activation }
}
