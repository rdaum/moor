use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::db::state::WorldState;
use crate::model::var::Error::{E_INVARG, E_TYPE};
use crate::model::var::Var;
use crate::server::Sessions;
use crate::vm::execute::{BfFunction, VM};
use async_trait::async_trait;
use decorum::R64;
use std::sync::Arc;
use tokio::sync::Mutex;

async fn bf_abs(
    _ws: &mut dyn WorldState,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(Var::Err(E_INVARG));
    }

    match args[0] {
        Var::Int(i) => Ok(Var::Int(i.abs())),
        Var::Float(f) => Ok(Var::Float(f.abs())),
        _ => Ok(Var::Err(E_TYPE)),
    }
}
bf_declare!(abs, bf_abs);

async fn bf_min(
    _ws: &mut dyn WorldState,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 2 {
        return Ok(Var::Err(E_INVARG));
    }

    match (&args[0], &args[1]) {
        (Var::Int(a), Var::Int(b)) => Ok(Var::Int(*a.max(b))),
        (Var::Float(a), Var::Float(b)) => {
            let m = R64::from(*a).min(R64::from(*b));
            Ok(Var::Float(m.into()))
        }
        _ => Ok(Var::Err(E_TYPE)),
    }
}
bf_declare!(min, bf_min);

async fn bf_max(
    _ws: &mut dyn WorldState,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 2 {
        return Ok(Var::Err(E_INVARG));
    }

    match (&args[0], &args[1]) {
        (Var::Int(a), Var::Int(b)) => Ok(Var::Int(*a.max(b))),
        (Var::Float(a), Var::Float(b)) => {
            let m = R64::from(*a).max(R64::from(*b));
            Ok(Var::Float(m.into()))
        }
        _ => Ok(Var::Err(E_TYPE)),
    }
}

bf_declare!(max, bf_max);

impl VM {
    pub(crate) fn register_bf_num(&mut self) -> Result<(), anyhow::Error> {
        self.bf_funcs[offset_for_builtin("abs")] = Box::new(BfAbs {});
        self.bf_funcs[offset_for_builtin("min")] = Box::new(BfMin {});
        self.bf_funcs[offset_for_builtin("max")] = Box::new(BfMax {});
        Ok(())
    }
}
