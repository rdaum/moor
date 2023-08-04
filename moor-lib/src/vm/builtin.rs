use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use moor_value::var::Var;

use crate::model::permissions::PermissionsContext;
use crate::model::world_state::WorldState;
use crate::tasks::Sessions;
use crate::vm::VM;

/// The arguments and other state passed to a built-in function.
pub(crate) struct BfCallState<'a> {
    pub(crate) vm: &'a VM,
    pub(crate) name: &'a str,
    pub(crate) world_state: &'a mut dyn WorldState,
    pub(crate) sessions: Arc<RwLock<dyn Sessions>>,
    pub(crate) args: Vec<Var>,
}

impl<'a> BfCallState<'a> {
    pub fn perms(&self) -> PermissionsContext {
        self.vm.top().permissions.clone()
    }
}

#[async_trait]
pub(crate) trait BuiltinFunction: Sync + Send {
    fn name(&self) -> &str;
    async fn call<'a>(&self, bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error>;
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
                async fn call<'a>(
                    &self,
                    bf_args: &mut BfCallState<'a>
                ) -> Result<Var, anyhow::Error> {
                    $action(bf_args).await
                }
            }
        }
    };
}
