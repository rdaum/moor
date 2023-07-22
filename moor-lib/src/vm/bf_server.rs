use std::sync::Arc;
use std::time::SystemTime;
use anyhow::anyhow;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::info;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::db::state::WorldState;
use crate::model::objects::ObjFlag;
use crate::model::ObjectError;
use crate::tasks::Sessions;
use crate::var::error::Error::{E_INVARG, E_PERM, E_TYPE};
use crate::var::{v_bool, v_err, v_int, v_list, v_objid, v_string, Var, Variant, VAR_NONE};
use crate::vm::activation::Activation;
use crate::vm::execute::{BfFunction, VM};

async fn bf_noop(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    _args: &[Var],
) -> Result<Var, anyhow::Error> {
    unimplemented!("BF is not implemented");
}
bf_declare!(noop, bf_noop);

async fn bf_notify(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let player = args[0].variant();
    let Variant::Obj(player) = player else {
        return Ok(v_err(E_TYPE));
    };
    let msg = args[1].variant();
    let Variant::Str(msg) = msg else {
        return Ok(v_err(E_TYPE));
    };

    sess.write()
        .await
        .send_text(*player, msg.clone())
        .await
        .unwrap();

    Ok(VAR_NONE)
}
bf_declare!(notify, bf_notify);

async fn bf_connected_players(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if !args.is_empty() {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_list(
        sess.read()
            .await
            .connected_players()
            .await
            .unwrap()
            .iter()
            .map(|p| v_objid(*p))
            .collect(),
    ))
}
bf_declare!(connected_players, bf_connected_players);

async fn bf_is_player(
    ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let player = args[0].variant();
    let Variant::Obj(player) = player else {
        return Ok(v_err(E_TYPE));
    };

    let is_player = match ws.flags_of(*player) {
        Ok(flags) => flags.contains(ObjFlag::User),
        Err(ObjectError::ObjectNotFound(_)) => return Ok(v_err(E_INVARG)),
        Err(e) => return Err(e.into()),
    };
    Ok(v_bool(is_player))
}
bf_declare!(is_player, bf_is_player);

async fn bf_caller_perms(
    _ws: &mut dyn WorldState,
    frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if !args.is_empty() {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_objid(frame.caller_perms))
}
bf_declare!(caller_perms, bf_caller_perms);

async fn bf_set_task_perms(
    _ws: &mut dyn WorldState,
    frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let Variant::Obj(player) = args[0].variant() else {
        return Ok(v_err(E_TYPE));
    };

    if !frame.player_flags.contains(ObjFlag::Wizard) {
        return Ok(v_err(E_PERM));
    }
    frame.caller_perms = *player;

    Ok(VAR_NONE)
}
bf_declare!(set_task_perms, bf_set_task_perms);

async fn bf_callers(
    _ws: &mut dyn WorldState,
    frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if !args.is_empty() {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_list(
        frame
            .callers
            .iter()
            .map(|c| {
                let callers = vec![
                    v_objid(c.this),
                    v_string(c.verb_name.clone()),
                    v_objid(c.programmer),
                    v_objid(c.verb_loc),
                    v_objid(c.player),
                    v_int(c.line_number as i64),
                ];
                v_list(callers)
            })
            .collect(),
    ))
}
bf_declare!(callers, bf_callers);

async fn bf_task_id(
    _ws: &mut dyn WorldState,
    frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if !args.is_empty() {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_int(frame.task_id as i64))
}
bf_declare!(task_id, bf_task_id);

async fn bf_idle_seconds(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if !args.is_empty() {
        return Ok(v_err(E_INVARG));
    }

    // TODO: This is a placeholder for now.
    Ok(v_int(0))
}
bf_declare!(idle_seconds, bf_idle_seconds);

async fn bf_time(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if !args.is_empty() {
        return Ok(v_err(E_INVARG));
    }
    Ok(v_int(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    ))
}
bf_declare!(time, bf_time);

async fn bf_raise(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    // Syntax:  raise (<code> [, str <message> [, <value>]])   => none
    //
    // Raises <code> as an error in the same way as other MOO expressions, statements, and functions do.  <Message>, which defaults to the value of `tostr(<code>)',
    // and <value>, which defaults to zero, are made available to any `try'-`except' statements that catch the error.  If the error is not caught, then <message> will
    // appear on the first line of the traceback printed to the user.
    if args.is_empty() || args.len() > 3 {
        return Ok(v_err(E_INVARG));
    }

    let Variant::Err(e) = args[0].variant() else {
        return Ok(v_err(E_INVARG));
    };

    // TODO implement message & value params, can't do that with the existing bf interface for
    // returning errors right now :-(
    // probably need to change the result type here to not use anyhow::Error, and pack in some
    // more useful stuff
    Ok(args[0].clone())
}
bf_declare!(raise, bf_raise);

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
        self.bf_funcs[offset_for_builtin("idle_seconds")] = Arc::new(Box::new(BfIdleSeconds {}));
        self.bf_funcs[offset_for_builtin("time")] = Arc::new(Box::new(BfTime {}));
        self.bf_funcs[offset_for_builtin("raise")] = Arc::new(Box::new(BfRaise {}));
        Ok(())
    }
}
