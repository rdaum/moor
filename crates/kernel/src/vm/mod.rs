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

//! A LambdaMOO 1.8.x compatibl(ish) virtual machine.
//! Executes opcodes which are essentially 1:1 with LambdaMOO's.
//! Aims to be semantically identical, so as to be able to run existing LambdaMOO compatible cores
//! without blocking issues.

use std::sync::Arc;

use moor_compiler::BUILTIN_DESCRIPTORS;

use crate::builtins::bf_server::BfNoop;
use crate::builtins::BuiltinFunction;

pub(crate) mod activation;
pub(crate) mod exec_state;
pub(crate) mod vm_call;
pub(crate) mod vm_execute;
pub(crate) mod vm_unwind;

// Exports to the rest of the kernel
pub use exec_state::VMExecState;
pub use vm_call::VerbExecutionRequest;
pub use vm_execute::{ExecutionResult, Fork, VmExecParams};
pub use vm_unwind::{FinallyReason, UncaughtException};

mod frame;
#[cfg(test)]
mod vm_test;

pub struct VM {
    /// The set of built-in functions, indexed by their Name offset in the variable stack.
    pub(crate) builtins: Vec<Arc<dyn BuiltinFunction>>,
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    pub fn new() -> Self {
        let mut builtins: Vec<Arc<dyn BuiltinFunction>> =
            Vec::with_capacity(BUILTIN_DESCRIPTORS.len());
        for _ in 0..BUILTIN_DESCRIPTORS.len() {
            builtins.push(Arc::new(BfNoop {}))
        }
        let mut vm = Self { builtins };

        vm.register_bf_server();
        vm.register_bf_num();
        vm.register_bf_values();
        vm.register_bf_strings();
        vm.register_bf_list_sets();
        vm.register_bf_objects();
        vm.register_bf_verbs();
        vm.register_bf_properties();

        vm
    }
}
