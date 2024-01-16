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

mod bf_list_sets;
mod bf_num;
mod bf_objects;
mod bf_properties;
pub mod bf_server;
mod bf_strings;
mod bf_values;
mod bf_verbs;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

use moor_values::model::Perms;
use moor_values::model::WorldState;
use moor_values::model::WorldStateError;
use moor_values::var::Error;
use moor_values::var::Objid;
use moor_values::var::Var;

use crate::tasks::sessions::Session;
use crate::tasks::task_messages::SchedulerControlMsg;
use crate::tasks::TaskId;
use crate::vm::{ExecutionResult, VMExecState};

/// The arguments and other state passed to a built-in function.
pub struct BfCallState<'a> {
    /// The name of the invoked function.
    pub(crate) name: String,
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
    pub(crate) scheduler_sender: UnboundedSender<(TaskId, SchedulerControlMsg)>,
    /// How many ticks are left in the current task.
    pub(crate) ticks_left: usize,
    /// How much time is left in the current task.
    pub(crate) time_left: Option<Duration>,
}

impl BfCallState<'_> {
    pub fn caller_perms(&self) -> Objid {
        self.exec_state.caller_perms()
    }

    pub fn task_perms_who(&self) -> Objid {
        self.exec_state.task_perms()
    }
    pub async fn task_perms(&self) -> Result<Perms, WorldStateError> {
        let who = self.task_perms_who();
        let flags = self.world_state.flags_of(who).await?;
        Ok(Perms { who, flags })
    }
}

#[async_trait]
pub trait BuiltinFunction: Sync + Send {
    fn name(&self) -> &str;
    async fn call<'a>(&self, bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error>;
}

/// Return possibilities from a built-in function.
pub enum BfRet {
    /// Successful return, with a value to be pushed to the value stack.
    Ret(Var),
    /// BF wants to return control back to the VM, with specific instructions to things like
    /// `suspend` or dispatch to a verb call or execute eval.
    VmInstr(ExecutionResult),
}

#[macro_export]
macro_rules! bf_declare {
    ( $name:ident, $action:expr ) => {
        paste::item! {
            pub struct [<Bf $name:camel >] {}
            #[async_trait]
            impl BuiltinFunction for [<Bf $name:camel >] {
                fn name(&self) -> &str {
                    return stringify!($name)
                }
                // TODO use the descriptor in BUILTIN_DESCRIPTORS to check the arguments
                // instead of doing it manually in each BF?
                async fn call<'a>(
                    &self,
                    bf_args: &mut BfCallState<'a>
                ) -> Result<BfRet, Error> {
                    $action(bf_args).await
                }
            }
        }
    };
}
