use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::db::state::WorldState;
use crate::model::objects::ObjFlag;
use crate::model::var::Error::{E_INVARG, E_PERM, E_TYPE};
use crate::model::var::Var;

use crate::model::ObjectError;
use crate::server::Sessions;
use crate::vm::activation::Activation;
use crate::vm::execute::{BfFunction, VM};

async fn bf_noop(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    _args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    unimplemented!("BF is not implemented");
}
bf_declare!(noop, bf_noop);

async fn bf_notify(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
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
    _frame: &mut Activation,
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

async fn bf_is_player(
    ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(Var::Err(E_INVARG));
    }
    let player = args[0].clone();
    let Var::Obj(player) = player else {
        return Ok(Var::Err(E_TYPE));
    };

    let is_player = match ws.flags_of(player) {
        Ok(flags) => flags.contains(ObjFlag::User),
        Err(ObjectError::ObjectNotFound(_)) => return Ok(Var::Err(E_INVARG)),
        Err(e) => return Err(e.into()),
    };
    Ok(Var::Int(if is_player { 1 } else { 0 }))
}
bf_declare!(is_player, bf_is_player);

async fn bf_caller_perms(
    _ws: &mut dyn WorldState,
    frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if !args.is_empty() {
        return Ok(Var::Err(E_INVARG));
    }

    Ok(Var::Obj(frame.caller_perms))
}
bf_declare!(caller_perms, bf_caller_perms);

async fn bf_set_task_perms(
    _ws: &mut dyn WorldState,
    frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(Var::Err(E_INVARG));
    }
    let Var::Obj(player) = args[0] else {
        return Ok(Var::Err(E_TYPE));
    };

    if !frame.player_flags.contains(ObjFlag::Wizard) {
        return Ok(Var::Err(E_PERM));
    }
    frame.caller_perms = player;

    Ok(Var::None)
}
bf_declare!(set_task_perms, bf_set_task_perms);

async fn bf_callers(
    _ws: &mut dyn WorldState,
    frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if !args.is_empty() {
        return Ok(Var::Err(E_INVARG));
    }

    Ok(Var::List(
        frame
            .callers
            .iter()
            .map(|c| {
                let callers = vec![
                    Var::Obj(c.this),
                    Var::Str(c.verb_name.clone()),
                    Var::Obj(c.programmer),
                    Var::Obj(c.verb_loc),
                    Var::Obj(c.player),
                    Var::Int(c.line_number as i64),
                ];
                Var::List(callers)
            })
            .collect(),
    ))
}
bf_declare!(callers, bf_callers);

async fn bf_task_id(
    _ws: &mut dyn WorldState,
    frame: &mut Activation,
    _sess: Arc<Mutex<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if !args.is_empty() {
        return Ok(Var::Err(E_INVARG));
    }

    Ok(Var::Int(frame.task_id as i64))
}
bf_declare!(task_id, bf_task_id);

impl VM {
    pub(crate) fn register_bf_server(&mut self) -> Result<(), anyhow::Error> {
        self.bf_funcs[offset_for_builtin("notify")] = Arc::new(Box::new(BfNotify {}));
        self.bf_funcs[offset_for_builtin("connected_players")] =
            Arc::new(Box::new(BfConnectedPlayers {}));
        self.bf_funcs[offset_for_builtin("is_player")] = Arc::new(Box::new(BfIsPlayer {}));
        self.bf_funcs[offset_for_builtin("caller_perms")] = Arc::new(Box::new(BfCallerPerms {}));
        self.bf_funcs[offset_for_builtin("set_task_perms")] = Arc::new(Box::new(BfSetTaskPerms {}));
        self.bf_funcs[offset_for_builtin("callers")] = Arc::new(Box::new(BfCallers {}));
        self.bf_funcs[offset_for_builtin("task_id")] = Arc::new(Box::new(BfTaskId {}));

        Ok(())
    }
}
