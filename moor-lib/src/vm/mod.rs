use std::sync::Arc;

use crate::compiler::builtins::BUILTINS;
use crate::values::var::Var;
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

        vm
    }
}
