/// A LambdaMOO 1.8.x compatibl(ish) virtual machine.
/// Executes opcodes which are essentially 1:1 with LambdaMOO's.
/// Aims to be semantically identical, so as to be able to run existing LambdaMOO compatible cores
/// without blocking issues.
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use moor_value::model::verb_info::VerbInfo;
use moor_value::var::objid::Objid;
use moor_value::var::Var;

use crate::compiler::builtins::BUILTIN_DESCRIPTORS;
use crate::compiler::labels::{Name, Offset};
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::VerbCall;
use crate::vm::activation::Activation;
use crate::vm::bf_server::BfNoop;
use crate::vm::builtin::BuiltinFunction;
use crate::vm::opcode::Program;
use crate::vm::vm_unwind::FinallyReason;

pub(crate) mod opcode;
pub(crate) mod vm_call;
pub(crate) mod vm_execute;
pub(crate) mod vm_unwind;
pub(crate) mod vm_util;

mod activation;

mod bf_list_sets;
mod bf_num;
mod bf_objects;
mod bf_properties;
mod bf_server;
mod bf_strings;
mod bf_values;
mod bf_verbs;

mod builtin;
#[cfg(test)]
mod vm_test;

pub struct VM {
    /// The stack of activation records / stack frames.
    pub(crate) stack: Vec<Activation>,
    /// The set of built-in functions, indexed by their Name offset in the variable stack.
    pub(crate) builtins: Vec<Arc<Box<dyn BuiltinFunction>>>,
    /// The number of ticks that have been executed so far.
    pub(crate) tick_count: usize,
    /// The time at which the VM was started.
    pub(crate) start_time: Option<SystemTime>,
}

/// The set of parameters for a VM-requested fork.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ForkRequest {
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

/// The set of parameters for a VM-requested *resolved* verb method dispatch.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerbExecutionRequest {
    /// The applicable permissions.
    pub permissions: Objid,
    /// The resolved verb.
    pub resolved_verb: VerbInfo,
    /// The call parameters that were used to resolve the verb.
    pub call: VerbCall,
    /// The parsed user command that led to this verb dispatch, if any.
    pub command: Option<ParsedCommand>,
    /// The decoded MOO Binary that contains the verb to be executed.
    pub program: Program,
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
    DispatchFork(ForkRequest),
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
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    #[tracing::instrument()]
    pub fn new() -> Self {
        let mut bf_funcs: Vec<Arc<Box<dyn BuiltinFunction>>> =
            Vec::with_capacity(BUILTIN_DESCRIPTORS.len());
        for _ in 0..BUILTIN_DESCRIPTORS.len() {
            bf_funcs.push(Arc::new(Box::new(BfNoop {})))
        }
        let _bf_noop = Box::new(BfNoop {});
        let mut vm = Self {
            stack: vec![],
            builtins: bf_funcs,
            tick_count: 0,
            start_time: None,
        };

        vm.register_bf_server().unwrap();
        vm.register_bf_num().unwrap();
        vm.register_bf_values().unwrap();
        vm.register_bf_strings().unwrap();
        vm.register_bf_list_sets().unwrap();
        vm.register_bf_objects().unwrap();
        vm.register_bf_verbs().unwrap();
        vm.register_bf_properties().unwrap();

        vm
    }
}
