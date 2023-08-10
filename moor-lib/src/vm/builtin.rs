use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;

use moor_value::var::error::Error;
use moor_value::var::Var;

use crate::tasks::scheduler::SchedulerControlMsg;
use crate::tasks::Sessions;
use crate::vm::{ExecutionResult, VM};
use moor_value::model::permissions::PermissionsContext;
use moor_value::model::world_state::WorldState;

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
    pub(crate) sessions: Arc<RwLock<dyn Sessions>>,
    /// For sending messages up to the scheduler
    pub(crate) scheduler_sender: UnboundedSender<SchedulerControlMsg>,
    /// How many ticks are left in the current task.
    pub(crate) ticks_left: usize,
    /// How much time is left in the current task.
    pub(crate) time_left: Option<Duration>,
}

impl<'a> BfCallState<'a> {
    pub fn perms_mut(&mut self) -> &mut PermissionsContext {
        &mut self.vm.top_mut().permissions
    }
    pub fn perms(&self) -> &PermissionsContext {
        &self.vm.top().permissions
    }
}

#[async_trait]
pub(crate) trait BuiltinFunction: Sync + Send {
    fn name(&self) -> &str;
    async fn call<'a>(&self, bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error>;
}

/// Return possibilities from a built-in function.
pub(crate) enum BfRet {
    /// Successful return, with a value to be pushed to the value stack.
    Ret(Var),
    /// An error occurred, which should be raised.
    Error(Error),
    /// VM wants to call another builtin.
    /// BF wants to return control back to the VM, with specific instructions to things like
    /// `suspend` or dispatch to a verb call.
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
                ) -> Result<BfRet, anyhow::Error> {
                    $action(bf_args).await
                }
            }
        }
    };
}
