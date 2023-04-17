use std::sync::Arc;

use tokio::sync::Mutex;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::db::state::WorldState;
use crate::model::var::Error::{E_INVARG, E_TYPE};
use crate::model::var::Var;
use crate::server::Sessions;
use crate::vm::execute::{BfFunction, VM};
use async_trait::async_trait;

async fn bf_noop(
    _ws: &mut dyn WorldState,
    _sess: Arc<Mutex<dyn Sessions>>,
    _args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    return Ok(Var::None);
}
bf_declare!(noop, bf_noop);

async fn bf_notify(
    _ws: &mut dyn WorldState,
    sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
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

    sess.lock().await.send_text(player, msg).await.unwrap();

    Ok(Var::None)
}
bf_declare!(notify, bf_notify);

async fn bf_connected_players(
    _ws: &mut dyn WorldState,
    sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if !args.is_empty() {
        return Ok(Var::Err(E_INVARG));
    }

    Ok(Var::List(
        sess.lock()
            .await
            .connected_players()
            .await
            .unwrap()
            .iter()
            .map(|p| Var::Obj(*p))
            .collect(),
    ))
}
bf_declare!(connected_players, bf_connected_players);

impl VM {
    pub(crate) fn register_bf_server(&mut self) -> Result<(), anyhow::Error> {
        self.bf_funcs[offset_for_builtin("notify")] = Box::new(BfNotify {});
        self.bf_funcs[offset_for_builtin("connected_players")] = Box::new(BfConnectedPlayers {});
        Ok(())
    }
}
