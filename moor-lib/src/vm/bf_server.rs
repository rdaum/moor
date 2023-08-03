use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use tracing::warn;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::model::objects::ObjFlag;
use crate::model::ObjectError;
use crate::values::error::Error::{E_INVARG, E_TYPE};
use crate::values::var::{v_bool, v_err, v_int, v_list, v_none, v_objid, v_string, Var};
use crate::values::variant::Variant;
use crate::vm::builtin::{BfCallState, BuiltinFunction};
use crate::vm::VM;

async fn bf_noop<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    // TODO after some time, this should get flipped to a runtime error (E_INVIND or something)
    // instead. right now it just panics so we can find all the places that need to be updated.
    unimplemented!("BF is not implemented: {}", bf_args.name);
}
bf_declare!(noop, bf_noop);

async fn bf_notify<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let player = bf_args.args[0].variant();
    let Variant::Obj(player) = player else {
        return Ok(v_err(E_TYPE));
    };
    let msg = bf_args.args[1].variant();
    let Variant::Str(msg) = msg else {
        return Ok(v_err(E_TYPE));
    };

    // If player is not the calling task perms, or a caller is not a wizard, raise E_PERM.
    bf_args
        .perms()
        .task_perms()
        .check_obj_owner_perms(*player)?;

    if let Err(send_error) = bf_args
        .sessions
        .write()
        .await
        .send_text(*player, msg.as_str())
        .await
    {
        warn!(
            "Unable to send message to player: #{}: {}",
            player.0, send_error
        );
    }

    Ok(v_none())
}
bf_declare!(notify, bf_notify);

async fn bf_connected_players<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_list(
        bf_args
            .sessions
            .read()
            .await
            .connected_players()
            .unwrap()
            .iter()
            .map(|p| v_objid(*p))
            .collect(),
    ))
}
bf_declare!(connected_players, bf_connected_players);

async fn bf_is_player<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let player = bf_args.args[0].variant();
    let Variant::Obj(player) = player else {
        return Ok(v_err(E_TYPE));
    };

    let is_player = match bf_args.world_state.flags_of(*player) {
        Ok(flags) => flags.contains(ObjFlag::User),
        Err(ObjectError::ObjectNotFound(_)) => return Ok(v_err(E_INVARG)),
        Err(e) => return Err(e.into()),
    };
    Ok(v_bool(is_player))
}
bf_declare!(is_player, bf_is_player);

async fn bf_caller_perms<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_objid(bf_args.frame.permissions.caller_perms().obj))
}
bf_declare!(caller_perms, bf_caller_perms);

async fn bf_set_task_perms<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let Variant::Obj(perms_for) = bf_args.args[0].variant() else {
        return Ok(v_err(E_TYPE));
    };

    bf_args.perms().task_perms().check_wizard()?;
    bf_args
        .frame
        .permissions
        .set_task_perms(*perms_for, bf_args.world_state.flags_of(*perms_for)?);

    Ok(v_none())
}
bf_declare!(set_task_perms, bf_set_task_perms);

async fn bf_callers<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_list(
        bf_args
            .frame
            .callers
            .iter()
            .map(|c| {
                let callers = vec![
                    v_objid(c.this),
                    v_string(c.verb_name.clone()),
                    v_objid(c.perms.task_perms().obj),
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

async fn bf_task_id<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_int(bf_args.frame.task_id as i64))
}
bf_declare!(task_id, bf_task_id);

async fn bf_idle_seconds<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let Variant::Obj(who) = bf_args.args[0].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let sessions = bf_args.sessions.read().await;
    let idle_seconds = sessions.idle_seconds(*who)?;

    Ok(v_int(idle_seconds as i64))
}
bf_declare!(idle_seconds, bf_idle_seconds);

async fn bf_connected_seconds<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let Variant::Obj(who) = bf_args.args[0].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let sessions = bf_args.sessions.read().await;
    let connected_seconds = sessions.connected_seconds(*who)?;

    Ok(v_int(connected_seconds as i64))
}
bf_declare!(connected_seconds, bf_connected_seconds);

async fn bf_time<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if !bf_args.args.is_empty() {
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

async fn bf_raise<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    // Syntax:  raise (<code> [, str <message> [, <value>]])   => none
    //
    // Raises <code> as an error in the same way as other MOO expressions, statements, and functions do.  <Message>, which defaults to the value of `tostr(<code>)',
    // and <value>, which defaults to zero, are made available to any `try'-`except' statements that catch the error.  If the error is not caught, then <message> will
    // appear on the first line of the traceback printed to the user.
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Ok(v_err(E_INVARG));
    }

    let Variant::Err(_) = bf_args.args[0].variant() else {
        return Ok(v_err(E_INVARG));
    };

    // TODO implement message & value params, can't do that with the existing bf interface for
    // returning errors right now :-(
    // probably need to change the result type here to not use anyhow::Error, and pack in some
    // more useful stuff
    Ok(bf_args.args[0].clone())
}
bf_declare!(raise, bf_raise);

async fn bf_server_version<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(v_err(E_INVARG));
    }
    // TODO: This is a placeholder for now, should be set by the server on startup. But right now
    // there isn't a good place to stash this other than WorldState. I intend on refactoring the
    // signature for BF invocations, and when I do this, I'll get additional metadata on there.
    Ok(v_string("0.0.1".to_string()))
}
bf_declare!(server_version, bf_server_version);

impl VM {
    pub(crate) fn register_bf_server(&mut self) -> Result<(), anyhow::Error> {
        self.builtins[offset_for_builtin("notify")] = Arc::new(Box::new(BfNotify {}));
        self.builtins[offset_for_builtin("connected_players")] =
            Arc::new(Box::new(BfConnectedPlayers {}));
        self.builtins[offset_for_builtin("is_player")] = Arc::new(Box::new(BfIsPlayer {}));
        self.builtins[offset_for_builtin("caller_perms")] = Arc::new(Box::new(BfCallerPerms {}));
        self.builtins[offset_for_builtin("set_task_perms")] = Arc::new(Box::new(BfSetTaskPerms {}));
        self.builtins[offset_for_builtin("callers")] = Arc::new(Box::new(BfCallers {}));
        self.builtins[offset_for_builtin("task_id")] = Arc::new(Box::new(BfTaskId {}));
        self.builtins[offset_for_builtin("idle_seconds")] = Arc::new(Box::new(BfIdleSeconds {}));
        self.builtins[offset_for_builtin("connected_seconds")] =
            Arc::new(Box::new(BfConnectedSeconds {}));
        self.builtins[offset_for_builtin("time")] = Arc::new(Box::new(BfTime {}));
        self.builtins[offset_for_builtin("raise")] = Arc::new(Box::new(BfRaise {}));
        self.builtins[offset_for_builtin("server_version")] =
            Arc::new(Box::new(BfServerVersion {}));
        Ok(())
    }
}
