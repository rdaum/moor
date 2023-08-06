use std::sync::Arc;
use std::time::Duration;

use moor_value::var::objid::Objid;
use moor_value::var::Var;

use crate::compiler::builtins::BUILTINS;
use crate::compiler::labels::{Name, Offset};
use crate::model::permissions::PermissionsContext;
use crate::model::verbs::VerbInfo;
use crate::tasks::command_parse::ParsedCommand;
use crate::tasks::VerbCall;
use crate::vm::activation::Activation;
use crate::vm::bf_server::BfNoop;
use crate::vm::builtin::BuiltinFunction;
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
    pub(crate) stack: Vec<Activation>,
    pub(crate) builtins: Vec<Arc<Box<dyn BuiltinFunction>>>,
}

/// The set of parameters for a VM-requested fork.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ForkRequest {
    /// The player. This is in the activation as well, but it's nicer to have it up here and
    /// explicit
    pub(crate) player: Objid,
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
pub struct ResolvedVerbCall {
    /// The applicable permissions context.
    pub permissions: PermissionsContext,
    /// The resolved verb.
    pub resolved_verb: VerbInfo,
    /// The call parameters that were used to resolve the verb.
    pub call: VerbCall,
    /// The parsed user command that led to this verb dispatch, if any.
    pub command: Option<ParsedCommand>,
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
    ContinueVerb(ResolvedVerbCall),
    /// Request dispatch of a new task as a fork
    DispatchFork(ForkRequest),
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
        let mut bf_funcs: Vec<Arc<Box<dyn BuiltinFunction>>> = Vec::with_capacity(BUILTINS.len());
        for _ in 0..BUILTINS.len() {
            bf_funcs.push(Arc::new(Box::new(BfNoop {})))
        }
        let _bf_noop = Box::new(BfNoop {});
        let mut vm = Self {
            stack: vec![],
            builtins: bf_funcs,
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
