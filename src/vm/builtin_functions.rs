use crate::compiler::builtins::offset_for_builtin;
use crate::db::state::WorldState;
use crate::model::var::Error::E_NONE;
use crate::model::var::Var;
use crate::vm::execute::{BfFunction, VM};
use crate::ClientConnection;
use anyhow::{Error};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct BfNoop {}
impl BfFunction for BfNoop {
    fn name(&self) -> String {
        "noop".to_string()
    }

    fn call(
        &self,
        _world_state: &mut dyn WorldState,
        _client_connection: Arc<Mutex<dyn ClientConnection + Send + Sync>>,
        _args: Vec<Var>,
    ) -> Result<Var, anyhow::Error> {
        Ok(Var::Err(E_NONE))
    }
}

pub struct BfNotify {}
impl BfFunction for BfNotify {
    fn name(&self) -> String {
        "notify".to_string()
    }

    fn call(
        &self,
        _world_state: &mut dyn WorldState,
        _client_connection: Arc<Mutex<dyn ClientConnection + Send + Sync>>,
        _args: Vec<Var>,
    ) -> Result<Var, Error> {
        Ok(Var::None)
    }
}

impl VM {
    fn register_bf_server(&mut self) -> Result<(), anyhow::Error> {
        let notify_idx = offset_for_builtin("notify").unwrap();
        self.bf_funcs[notify_idx] = Box::new(BfNotify {});
        Ok(())
    }
}
