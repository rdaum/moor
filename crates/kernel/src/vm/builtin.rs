use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

use moor_values::model::permissions::Perms;
use moor_values::model::world_state::WorldState;
use moor_values::model::WorldStateError;
use moor_values::var::error::Error;
use moor_values::var::objid::Objid;
use moor_values::var::Var;

use crate::tasks::sessions::Session;
use crate::tasks::task_messages::SchedulerControlMsg;
use crate::vm::{ExecutionResult, VM};

/// The arguments and other state passed to a built-in function.
pub(crate) struct BfCallState<'a> {
    /// The name of the invoked function.
    pub(crate) name: String,
    /// Arguments passed to the function.
    pub(crate) args: Vec<Var>,
    /// Reference back to the VM, to be able to retrieve stack frames and other state.
    pub(crate) vm: &'a mut VM,
    /// Handle to the current database transaction.
    pub(crate) world_state: &'a mut dyn WorldState,
    /// For connection / message management.
    pub(crate) session: Arc<dyn Session>,
    /// For sending messages up to the scheduler
    pub(crate) scheduler_sender: UnboundedSender<SchedulerControlMsg>,
    /// How many ticks are left in the current task.
    pub(crate) ticks_left: usize,
    /// How much time is left in the current task.
    pub(crate) time_left: Option<Duration>,
}

impl BfCallState<'_> {
    pub fn caller_perms(&self) -> Objid {
        self.vm.caller_perms()
    }

    pub fn task_perms_who(&self) -> Objid {
        self.vm.task_perms()
    }
    pub async fn task_perms(&self) -> Result<Perms, WorldStateError> {
        let who = self.task_perms_who();
        let flags = self.world_state.flags_of(who).await?;
        Ok(Perms { who, flags })
    }
}

#[async_trait]
pub(crate) trait BuiltinFunction: Sync + Send {
    fn name(&self) -> &str;
    async fn call<'a>(&self, bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error>;
}

/// Return possibilities from a built-in function.
pub(crate) enum BfRet {
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
