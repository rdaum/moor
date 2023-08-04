use std::sync::Arc;

use moor_value::var::objid::Objid;
use moor_value::var::Var;

use crate::compiler::builtins::BUILTINS;
use crate::model::permissions::PermissionsContext;
use crate::model::verbs::VerbInfo;
use crate::tasks::command_parse::ParsedCommand;
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
    // Activation stack.
    pub(crate) stack: Vec<Activation>,
    pub(crate) builtins: Vec<Arc<Box<dyn BuiltinFunction>>>,
}

/// The minimum set of information needed to make a *resolution* call for a verb.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerbCall {
    pub verb_name: String,
    pub location: Objid,
    pub this: Objid,
    pub player: Objid,
    pub args: Vec<Var>,
    pub caller: Objid,
}

/// The set of arguments for a VM-requested *resolved* verb method dispatch.
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
