use std::sync::Arc;

use anyhow::Error;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::compiler::builtins::offset_for_builtin;
use crate::db::state::WorldState;
use crate::model::var::Error::{E_INVARG, E_NONE, E_TYPE};
use crate::model::var::Var;
use crate::server::ClientConnection;
use crate::vm::execute::{BfFunction, VM};

pub struct BfNoop {}
#[async_trait]
impl BfFunction for BfNoop {
    fn name(&self) -> String {
        "noop".to_string()
    }

    async fn call(
        &self,
        _world_state: &mut dyn WorldState,
        _client_connection: Arc<Mutex<dyn ClientConnection + Send + Sync>>,
        _args: Vec<Var>,
    ) -> Result<Var, anyhow::Error> {
        Ok(Var::Err(E_NONE))
    }
}

pub struct BfNotify {}
#[async_trait]
impl BfFunction for BfNotify {
    fn name(&self) -> String {
        "notify".to_string()
    }

    async fn call(
        &self,
        _world_state: &mut dyn WorldState,
        client_connection: Arc<Mutex<dyn ClientConnection + Send + Sync>>,
        args: Vec<Var>,
    ) -> Result<Var, Error> {
        if args.len() != 2 {
            return Ok(Var::Err(E_INVARG));
        }
        let player = args[0].clone();
        let Var::Obj(player) = player else {
            return Ok(Var::Err(E_TYPE));
        };
        let msg = args[1].clone();
        let Var::Str(msg) = msg else {
            return Ok(Var::Err(E_TYPE));
        };

        client_connection
            .lock()
            .await
            .send_text(player, msg)
            .await
            .unwrap();

        Ok(Var::None)
    }
}

impl VM {
    pub(crate) fn register_bf_server(&mut self) -> Result<(), anyhow::Error> {
        let notify_idx = offset_for_builtin("notify").unwrap();
        self.bf_funcs[notify_idx] = Box::new(BfNotify {});
        Ok(())
    }
}
