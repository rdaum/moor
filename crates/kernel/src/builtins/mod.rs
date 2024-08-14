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

use std::sync::Arc;

use thiserror::Error;

use moor_compiler::{BuiltinId, BUILTINS};
use moor_values::model::Perms;
use moor_values::model::WorldState;
use moor_values::model::WorldStateError;
use moor_values::var::Error;
use moor_values::var::Objid;
use moor_values::var::Symbol;
use moor_values::var::Var;

use crate::builtins::bf_list_sets::register_bf_list_sets;
use crate::builtins::bf_maps::register_bf_maps;
use crate::builtins::bf_num::register_bf_num;
use crate::builtins::bf_objects::register_bf_objects;
use crate::builtins::bf_properties::register_bf_properties;
use crate::builtins::bf_server::{register_bf_server, BfNoop};
use crate::builtins::bf_strings::register_bf_strings;
use crate::builtins::bf_values::register_bf_values;
use crate::builtins::bf_verbs::register_bf_verbs;
use crate::config::Config;
use crate::tasks::sessions::Session;
use crate::tasks::task_scheduler_client::TaskSchedulerClient;
use crate::vm::activation::{BfFrame, Frame};
use crate::vm::{ExecutionResult, VMExecState};

mod bf_list_sets;
mod bf_maps;
mod bf_num;
mod bf_objects;
mod bf_properties;
pub mod bf_server;
mod bf_strings;
mod bf_values;
mod bf_verbs;

/// The bundle of builtins are stored here, and passed around globally.
pub struct BuiltinRegistry {
    // The set of built-in functions, indexed by their Name offset in the variable stack.
    pub(crate) builtins: Arc<Vec<Box<dyn BuiltinFunction>>>,
}

impl Default for BuiltinRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinRegistry {
    pub fn new() -> Self {
        let mut builtins: Vec<Box<dyn BuiltinFunction>> = Vec::with_capacity(BUILTINS.len());
        for _ in 0..BUILTINS.len() {
            builtins.push(Box::new(BfNoop {}))
        }
        register_bf_server(&mut builtins);
        register_bf_num(&mut builtins);
        register_bf_values(&mut builtins);
        register_bf_strings(&mut builtins);
        register_bf_list_sets(&mut builtins);
        register_bf_maps(&mut builtins);
        register_bf_objects(&mut builtins);
        register_bf_verbs(&mut builtins);
        register_bf_properties(&mut builtins);

        BuiltinRegistry {
            builtins: Arc::new(builtins),
        }
    }

    pub fn builtin_for(&self, id: &BuiltinId) -> &dyn BuiltinFunction {
        &*self.builtins[id.0 as usize]
    }
}
/// The arguments and other state passed to a built-in function.
pub struct BfCallState<'a> {
    /// The name of the invoked function.
    pub(crate) name: Symbol,
    /// Arguments passed to the function.
    pub(crate) args: Vec<Var>,
    /// The current execution state of this task in this VM, including the stack
    /// so that BFs can inspect and manipulate it.
    pub(crate) exec_state: &'a mut VMExecState,
    /// Handle to the current database transaction.
    pub(crate) world_state: &'a mut dyn WorldState,
    /// For connection / message management.
    pub(crate) session: Arc<dyn Session>,
    /// For sending messages up to the scheduler
    pub(crate) task_scheduler_client: TaskSchedulerClient,
    /// Config
    pub(crate) config: Arc<Config>,
}

impl BfCallState<'_> {
    pub fn caller_perms(&self) -> Objid {
        self.exec_state.caller_perms()
    }

    pub fn task_perms_who(&self) -> Objid {
        self.exec_state.task_perms()
    }
    pub fn task_perms(&self) -> Result<Perms, WorldStateError> {
        let who = self.task_perms_who();
        let flags = self.world_state.flags_of(who)?;
        Ok(Perms { who, flags })
    }

    pub fn bf_frame(&self) -> &BfFrame {
        let Frame::Bf(frame) = &self.exec_state.top().frame else {
            panic!("Expected a BF frame at the top of the stack");
        };

        frame
    }

    pub fn bf_frame_mut(&mut self) -> &mut BfFrame {
        let Frame::Bf(frame) = &mut self.exec_state.top_mut().frame else {
            panic!("Expected a BF frame at the top of the stack");
        };

        frame
    }
}

pub trait BuiltinFunction: Sync + Send {
    fn name(&self) -> &str;
    fn call(&self, bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr>;
}

/// Return possibilities from a built-in function.
pub enum BfRet {
    /// Successful return, with a value to be pushed to the value stack.
    Ret(Var),
    /// BF wants to return control back to the VM, with specific instructions to things like
    /// `suspend` or dispatch to a verb call or execute eval.
    VmInstr(ExecutionResult),
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum BfErr {
    #[error("Error in built-in function: {0}")]
    Code(Error),
    #[error("Raised error: {0:?} {1:?} {2:?}")]
    Raise(Error, Option<String>, Option<Var>),
    #[error("Transaction rollback-retry")]
    Rollback,
}

#[macro_export]
macro_rules! bf_declare {
    ( $name:ident, $action:expr ) => {
        paste::item! {
            pub struct [<Bf $name:camel >] {}
            impl BuiltinFunction for [<Bf $name:camel >] {
                fn name(&self) -> &str {
                    return stringify!($name)
                }
                // TODO use the descriptor in BUILTIN_DESCRIPTORS to check the arguments
                // instead of doing it manually in each BF?
                fn call(
                    &self,
                    bf_args: &mut BfCallState<'_>
                ) -> Result<BfRet, BfErr> {
                    $action(bf_args)
                }
            }
        }
    };
}

pub(crate) fn world_state_bf_err(err: WorldStateError) -> BfErr {
    match err {
        WorldStateError::RollbackRetry => BfErr::Rollback,
        _ => BfErr::Code(err.into()),
    }
}
