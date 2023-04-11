
use crate::db::state::WorldState;
use crate::model::var::Error::E_NONE;
use crate::model::var::Var;

use crate::vm::execute::{BfFunction, VM};


pub struct BfNoop {}
impl BfFunction for BfNoop {
    fn name(&self) -> String {
        "noop".to_string()
    }

    fn call(&self, _act: &mut dyn WorldState, _args: Vec<Var>) -> Result<Var, anyhow::Error> {
        Ok(Var::Err(E_NONE))
    }
}

impl VM {
    fn register_bf_server(&mut self) {}
}
