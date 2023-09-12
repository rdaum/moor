use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use chrono::{DateTime, Local};
use metrics_macros::increment_counter;
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

use moor_value::model::objects::ObjFlag;
use moor_value::model::{NarrativeEvent, WorldStateError};
use moor_value::var::error::Error::{E_INVARG, E_PERM, E_TYPE};
use moor_value::var::variant::Variant;
use moor_value::var::{v_bool, v_int, v_list, v_none, v_objid, v_str, v_string, Var};

use crate::bf_declare;
use crate::compiler::builtins::{
    offset_for_builtin, ArgCount, ArgType, Builtin, BUILTIN_DESCRIPTORS,
};
use crate::compiler::codegen::compile;
use crate::tasks::scheduler::SchedulerControlMsg;
use crate::tasks::TaskId;
use crate::vm::builtin::BfRet::{Error, Ret, VmInstr};
use crate::vm::builtin::{BfCallState, BfRet, BuiltinFunction};
use crate::vm::{ExecutionResult, VM};

async fn bf_noop<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    increment_counter!("vm.bf_noop.calls");
    // TODO after some time, this should get flipped to a runtime error (E_INVIND or something)
    // instead. right now it just panics so we can find all the places that need to be updated.
    unimplemented!("BF is not implemented: {}", bf_args.name);
}
bf_declare!(noop, bf_noop);

async fn bf_notify<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let player = bf_args.args[0].variant();
    let Variant::Obj(player) = player else {
        return Ok(Error(E_TYPE));
    };
    let msg = bf_args.args[1].variant();
    let Variant::Str(msg) = msg else {
        return Ok(Error(E_TYPE));
    };

    // If player is not the calling task perms, or a caller is not a wizard, raise E_PERM.
    bf_args.task_perms().await?.check_obj_owner_perms(*player)?;

    let event = NarrativeEvent::new_durable(bf_args.vm.caller(), msg.to_string());
    if let Err(send_error) = bf_args.session.send_event(*player, event).await {
        warn!(
            "Unable to send message to player: #{}: {}",
            player.0, send_error
        );
    }

    // MOO docs say this should return none, but in reality it returns 1?
    Ok(Ret(v_int(1)))
}
bf_declare!(notify, bf_notify);

async fn bf_connected_players<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(Error(E_INVARG));
    }

    Ok(Ret(v_list(
        bf_args
            .session
            .connected_players()
            .await
            .unwrap()
            .iter()
            .map(|p| v_objid(*p))
            .collect(),
    )))
}
bf_declare!(connected_players, bf_connected_players);

async fn bf_is_player<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }
    let player = bf_args.args[0].variant();
    let Variant::Obj(player) = player else {
        return Ok(Error(E_TYPE));
    };

    let is_player = match bf_args.world_state.flags_of(*player).await {
        Ok(flags) => flags.contains(ObjFlag::User),
        Err(WorldStateError::ObjectNotFound(_)) => return Ok(Error(E_INVARG)),
        Err(e) => return Err(e.into()),
    };
    Ok(Ret(v_bool(is_player)))
}
bf_declare!(is_player, bf_is_player);

async fn bf_caller_perms<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(Error(E_INVARG));
    }

    Ok(Ret(v_objid(bf_args.caller_perms())))
}
bf_declare!(caller_perms, bf_caller_perms);

async fn bf_set_task_perms<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(perms_for) = bf_args.args[0].variant().clone() else {
        return Ok(Error(E_TYPE));
    };

    // If the caller is not a wizard, perms_for must be the caller
    let perms = bf_args.task_perms().await?;
    if !perms.check_is_wizard()? && perms_for != perms.who {
        return Ok(Error(E_PERM));
    }
    bf_args.vm.set_task_perms(perms_for);

    Ok(Ret(v_none()))
}
bf_declare!(set_task_perms, bf_set_task_perms);

async fn bf_callers<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(Error(E_INVARG));
    }

    // We have to exempt ourselves from the callers list.
    let callers = bf_args.vm.callers()[1..].to_vec();
    Ok(Ret(v_list(
        callers
            .iter()
            .map(|c| {
                let callers = vec![
                    // this
                    v_objid(c.this),
                    // verb name
                    v_string(c.verb_name.clone()),
                    // 'programmer'
                    v_objid(c.programmer),
                    // verb location
                    v_objid(c.definer),
                    // player
                    v_objid(c.player),
                    // line number
                    v_int(c.line_number as i64),
                ];
                v_list(callers)
            })
            .collect(),
    )))
}
bf_declare!(callers, bf_callers);

async fn bf_task_id<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(Error(E_INVARG));
    }

    Ok(Ret(v_int(bf_args.vm.top().task_id as i64)))
}
bf_declare!(task_id, bf_task_id);

async fn bf_idle_seconds<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(who) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Ok(idle_seconds) = bf_args.session.idle_seconds(*who).await else {
        return Ok(Error(E_INVARG));
    };

    Ok(Ret(v_int(idle_seconds as i64)))
}
bf_declare!(idle_seconds, bf_idle_seconds);

async fn bf_connected_seconds<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(who) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Ok(connected_seconds) = bf_args.session.connected_seconds(*who).await else {
        return Ok(Error(E_INVARG));
    };

    Ok(Ret(v_int(connected_seconds as i64)))
}
bf_declare!(connected_seconds, bf_connected_seconds);

/*
Syntax:  connection_name (obj <player>)   => str

Returns a network-specific string identifying the connection being used by the given player.  If the programmer is not a wizard and not
<player>, then `E_PERM' is raised.  If <player> is not currently connected, then `E_INVARG' is raised.

 */
async fn bf_connection_name<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }

    let Variant::Obj(player) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };

    let caller = bf_args.caller_perms();
    if !bf_args.task_perms().await?.check_is_wizard()? && caller != *player {
        return Ok(Error(E_PERM));
    }

    let Ok(connection_name) = bf_args.session.connection_name(*player).await else {
        return Ok(Error(E_INVARG));
    };

    Ok(Ret(v_string(connection_name)))
}
bf_declare!(connection_name, bf_connection_name);

async fn bf_shutdown<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() > 1 {
        return Ok(Error(E_INVARG));
    }
    let msg = if bf_args.args.is_empty() {
        None
    } else {
        let Variant::Str(msg) = bf_args.args[0].variant() else {
            return Ok(Error(E_TYPE));
        };
        Some(msg.as_str().to_string())
    };

    bf_args.task_perms().await?.check_wizard()?;
    bf_args.session.shutdown(msg).await.unwrap();

    Ok(Ret(v_none()))
}
bf_declare!(shutdown, bf_shutdown);

async fn bf_time<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(Error(E_INVARG));
    }
    Ok(Ret(v_int(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    )))
}
bf_declare!(time, bf_time);

async fn bf_ctime<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() > 1 {
        return Ok(Error(E_INVARG));
    }
    let time = if bf_args.args.is_empty() {
        SystemTime::now()
    } else {
        let Variant::Int(time) = bf_args.args[0].variant() else {
            return Ok(Error(E_TYPE));
        };
        SystemTime::UNIX_EPOCH + Duration::from_secs(*time as u64)
    };

    let date_time: DateTime<Local> = chrono::DateTime::from(time);

    Ok(Ret(v_string(date_time.to_rfc2822())))
}
bf_declare!(ctime, bf_ctime);
async fn bf_raise<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    // Syntax:  raise (<code> [, str <message> [, <value>]])   => none
    //
    // Raises <code> as an error in the same way as other MOO expressions, statements, and functions do.  <Message>, which defaults to the value of `tostr(<code>)',
    // and <value>, which defaults to zero, are made available to any `try'-`except' statements that catch the error.  If the error is not caught, then <message> will
    // appear on the first line of the traceback printed to the user.
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Ok(Error(E_INVARG));
    }

    let Variant::Err(err) = bf_args.args[0].variant() else {
        return Ok(Error(E_INVARG));
    };

    // TODO implement message & value params, can't do that with the existing bf interface for
    // returning errors right now :-(
    Ok(Error(*err))
}
bf_declare!(raise, bf_raise);

async fn bf_server_version<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(Error(E_INVARG));
    }
    // TODO: This is a placeholder for now, should be set by the server on startup. But right now
    // there isn't a good place to stash this other than WorldState. I intend on refactoring the
    // signature for BF invocations, and when I do this, I'll get additional metadata on there.
    Ok(Ret(v_string("0.0.1".to_string())))
}
bf_declare!(server_version, bf_server_version);

async fn bf_suspend<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    // Syntax:  suspend(<seconds>)   => none
    //
    // Suspends the current task for <seconds> seconds.  If <seconds> is not specified, the task is suspended indefinitely.  The task may be resumed early by
    // calling `resume' on it.
    if bf_args.args.len() > 1 {
        return Ok(Error(E_INVARG));
    }

    let seconds = if bf_args.args.is_empty() {
        None
    } else {
        let Variant::Int(seconds) = bf_args.args[0].variant() else {
            return Ok(Error(E_TYPE));
        };
        Some(Duration::from_secs(*seconds as u64))
    };

    Ok(VmInstr(ExecutionResult::Suspend(seconds)))
}
bf_declare!(suspend, bf_suspend);

async fn bf_queued_tasks<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(Error(E_INVARG));
    }

    // Ask the scheduler (through its mailbox) to describe all the queued tasks.
    let (send, receive) = oneshot::channel();
    debug!("sending DescribeOtherTasks to scheduler");
    bf_args
        .scheduler_sender
        .send(SchedulerControlMsg::DescribeOtherTasks(send))
        .expect("scheduler is not listening");
    debug!("waiting for response from scheduler");
    let tasks = receive.await?;
    debug!("got response from scheduler");

    // return in form:
    //     {<task-id>, <start-time>, <x>, <y>,
    //      <programmer>, <verb-loc>, <verb-name>, <line>, <this>}
    let tasks = tasks
        .iter()
        .map(|task| {
            let task_id = v_int(task.task_id as i64);
            let start_time = match task.start_time {
                None => v_none(),
                Some(start_time) => {
                    let time = start_time.duration_since(SystemTime::UNIX_EPOCH).unwrap();
                    v_int(time.as_secs() as i64)
                }
            };
            let x = v_none();
            let y = v_none();
            let programmer = v_objid(task.permissions);
            let verb_loc = v_objid(task.verb_definer);
            let verb_name = v_string(task.verb_name.clone());
            let line = v_int(task.line_number as i64);
            let this = v_objid(task.this);
            v_list(vec![
                task_id, start_time, x, y, programmer, verb_loc, verb_name, line, this,
            ])
        })
        .collect();

    Ok(Ret(v_list(tasks)))
}
bf_declare!(queued_tasks, bf_queued_tasks);

async fn bf_kill_task<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    // Syntax:  kill_task(<task-id>)   => none
    //
    // Kills the task with the given <task-id>.  The task must be queued or suspended, and the current task must be the owner of the task being killed.
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }

    let Variant::Int(victim_task_id) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };

    // If the task ID is itself, that means returning an Complete execution result, which will cascade
    // back to the task loop and it will terminate itself.
    // Not sure this is *exactly* what MOO does, but it's close enough for now.
    let victim_task_id = *victim_task_id as TaskId;

    if victim_task_id == bf_args.vm.top().task_id {
        return Ok(VmInstr(ExecutionResult::Complete(v_none())));
    }

    let (send, receive) = oneshot::channel();
    bf_args
        .scheduler_sender
        .send(SchedulerControlMsg::KillTask {
            victim_task_id,
            sender_permissions: bf_args.task_perms().await?,
            result_sender: send,
        })
        .expect("scheduler is not listening");

    let result = receive.await?;
    if let Variant::Err(err) = result.variant() {
        return Ok(Error(*err));
    }
    Ok(Ret(result))
}
bf_declare!(kill_task, bf_kill_task);

async fn bf_resume<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() < 2 {
        return Ok(Error(E_INVARG));
    }

    let Variant::Int(resume_task_id) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };

    // Optional 2nd argument is the value to return from suspend() in the resumed task.
    let return_value = if bf_args.args.len() == 2 {
        bf_args.args[1].clone()
    } else {
        v_none()
    };

    let task_id = *resume_task_id as TaskId;

    // Resuming ourselves makes no sense, it's not suspended. E_INVARG.
    if task_id == bf_args.vm.top().task_id {
        return Ok(Error(E_INVARG));
    }

    let (send, receive) = oneshot::channel();
    bf_args
        .scheduler_sender
        .send(SchedulerControlMsg::ResumeTask {
            queued_task_id: task_id,
            sender_permissions: bf_args.task_perms().await?,
            return_value,
            result_sender: send,
        })
        .expect("scheduler is not listening");

    let result = receive.await?;
    if let Variant::Err(err) = result.variant() {
        return Ok(Error(*err));
    }
    Ok(Ret(result))
}
bf_declare!(resume, bf_resume);

async fn bf_ticks_left<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    // Syntax:  ticks_left()   => int
    //
    // Returns the number of ticks left in the current time slice.
    if !bf_args.args.is_empty() {
        return Ok(Error(E_INVARG));
    }

    let ticks_left = bf_args.ticks_left;

    Ok(Ret(v_int(ticks_left as i64)))
}
bf_declare!(ticks_left, bf_ticks_left);

async fn bf_seconds_left<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    // Syntax:  seconds_left()   => int
    //
    // Returns the number of seconds left in the current time slice.
    if !bf_args.args.is_empty() {
        return Ok(Error(E_INVARG));
    }

    let seconds_left = match bf_args.time_left {
        None => v_none(),
        Some(d) => v_int(d.as_secs() as i64),
    };

    Ok(Ret(seconds_left))
}
bf_declare!(seconds_left, bf_seconds_left);

async fn bf_boot_player<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    // Syntax:  boot_player(<player>)   => none
    //
    // Disconnects the player with the given object number.
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }

    let Variant::Obj(player) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };

    let task_perms = bf_args.task_perms().await?;
    if task_perms.who != *player && !task_perms.check_is_wizard()? {
        return Ok(Error(E_PERM));
    }

    bf_args
        .scheduler_sender
        .send(SchedulerControlMsg::BootPlayer {
            player: *player,
            sender_permissions: task_perms,
        })
        .expect("scheduler is not listening");

    Ok(Ret(v_none()))
}
bf_declare!(boot_player, bf_boot_player);

async fn bf_call_function<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    // Syntax:  call_function(<func>, <arg1>, <arg2>, ...)   => value
    //
    // Calls the given function with the given arguments and returns the result.
    if bf_args.args.is_empty() {
        return Ok(Error(E_INVARG));
    }

    let Variant::Str(func_name) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };

    // Arguments are everything left, if any.
    let args = &bf_args.args[1..];

    // Find the function id for the given function name.
    let func_name: &str = func_name.as_str();
    let Some(func_offset) = BUILTIN_DESCRIPTORS
        .iter()
        .position(|bf| bf.name == func_name)
    else {
        return Ok(Error(E_INVARG));
    };

    // Then ask the scheduler to run the function as a continuation of what we're doing now.
    Ok(VmInstr(ExecutionResult::ContinueBuiltin {
        bf_func_num: func_offset,
        arguments: args[..].to_vec(),
    }))
}
bf_declare!(call_function, bf_call_function);

/*Syntax:  server_log (str <message> [, <is-error>])   => none

The text in <message> is sent to the server log with a distinctive prefix (so that it can be distinguished from server-generated messages).  If the programmer
is not a wizard, then `E_PERM' is raised.  If <is-error> is provided and true, then <message> is marked in the server log as an error.

*/
async fn bf_server_log<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Ok(Error(E_INVARG));
    }

    let Variant::Str(message) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };

    let is_error = if bf_args.args.len() == 2 {
        let Variant::Int(is_error) = bf_args.args[1].variant() else {
            return Ok(Error(E_TYPE));
        };
        *is_error == 1
    } else {
        false
    };

    if !bf_args.task_perms().await?.check_is_wizard()? {
        return Ok(Error(E_PERM));
    }

    if is_error {
        error!("SERVER_LOG {}: {}", bf_args.vm.top().player, message);
    } else {
        info!("SERVER_LOG {}: {}", bf_args.vm.top().player, message);
    }

    Ok(Ret(v_none()))
}
bf_declare!(server_log, bf_server_log);

fn bf_function_info_to_list(bf: &Builtin) -> Var {
    let min_args = match bf.min_args {
        ArgCount::Q(q) => v_int(q as i64),
        ArgCount::U => v_int(-1),
    };
    let max_args = match bf.max_args {
        ArgCount::Q(q) => v_int(q as i64),
        ArgCount::U => v_int(-1),
    };
    let types = bf
        .types
        .iter()
        .map(|t| match t {
            ArgType::Typed(ty) => v_int(*ty as i64),
            ArgType::Any => v_int(-1),
            ArgType::AnyNum => v_int(-2),
        })
        .collect::<Vec<_>>();

    v_list(vec![
        v_str(bf.name.as_str()),
        min_args,
        max_args,
        v_list(types),
    ])
}

async fn bf_function_info<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() > 1 {
        return Ok(Error(E_INVARG));
    }

    if bf_args.args.len() == 1 {
        let Variant::Str(func_name) = bf_args.args[0].variant() else {
            return Ok(Error(E_TYPE));
        };
        let bf = BUILTIN_DESCRIPTORS
            .iter()
            .find(|bf| bf.name == func_name.as_str())
            .map(bf_function_info_to_list);
        let Some(desc) = bf else {
            return Ok(Error(E_INVARG));
        };
        return Ok(Ret(desc));
    }

    let bf_list = BUILTIN_DESCRIPTORS
        .iter()
        .filter_map(|bf| bf.implemented.then(|| bf_function_info_to_list(bf)))
        .collect();
    Ok(Ret(v_list(bf_list)))
}
bf_declare!(function_info, bf_function_info);

async fn bf_listeners<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if !bf_args.args.is_empty() {
        return Ok(Error(E_INVARG));
    }

    // TODO this function is hardcoded to just return {{#0, 7777, 1}}
    // this is on account that existing cores expect this to be the case
    // but we have no intend of supporting other network listener magic at this point
    let listeners = v_list(vec![v_list(vec![v_int(0), v_int(7777), v_int(1)])]);

    Ok(Ret(listeners))
}
bf_declare!(listeners, bf_listeners);

pub const BF_SERVER_EVAL_TRAMPOLINE_START_INITIALIZE: usize = 0;
pub const BF_SERVER_EVAL_TRAMPOLINE_RESUME: usize = 1;

async fn bf_eval<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Str(program_code) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };

    let tramp = bf_args
        .vm
        .top()
        .bf_trampoline
        .unwrap_or(BF_SERVER_EVAL_TRAMPOLINE_START_INITIALIZE);

    match tramp {
        BF_SERVER_EVAL_TRAMPOLINE_START_INITIALIZE => {
            let program_code = program_code.as_str();
            let program = match compile(program_code) {
                Ok(program) => program,
                Err(e) => return Ok(Ret(v_list(vec![v_int(0), v_string(e.to_string())]))),
            };

            // Now we have to construct things to set up for eval. Which means tramping through with a
            // setup-for-eval result here.
            return Ok(VmInstr(ExecutionResult::PerformEval {
                permissions: bf_args.task_perms_who(),
                player: bf_args.vm.top().player,
                program,
            }));
        }
        BF_SERVER_EVAL_TRAMPOLINE_RESUME => {
            // Value must be on stack,  and we then wrap that up in the {success, value} tuple.
            let value = bf_args.vm.pop();
            Ok(Ret(v_list(vec![v_bool(true), value])))
        }
        _ => {
            panic!("Invalid trampoline value for bf_eval: {}", tramp);
        }
    }
}
bf_declare!(eval, bf_eval);

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
        self.builtins[offset_for_builtin("connection_name")] =
            Arc::new(Box::new(BfConnectionName {}));
        self.builtins[offset_for_builtin("time")] = Arc::new(Box::new(BfTime {}));
        self.builtins[offset_for_builtin("ctime")] = Arc::new(Box::new(BfCtime {}));
        self.builtins[offset_for_builtin("raise")] = Arc::new(Box::new(BfRaise {}));
        self.builtins[offset_for_builtin("server_version")] =
            Arc::new(Box::new(BfServerVersion {}));
        self.builtins[offset_for_builtin("shutdown")] = Arc::new(Box::new(BfShutdown {}));
        self.builtins[offset_for_builtin("suspend")] = Arc::new(Box::new(BfSuspend {}));
        self.builtins[offset_for_builtin("queued_tasks")] = Arc::new(Box::new(BfQueuedTasks {}));
        self.builtins[offset_for_builtin("kill_task")] = Arc::new(Box::new(BfKillTask {}));
        self.builtins[offset_for_builtin("resume")] = Arc::new(Box::new(BfResume {}));
        self.builtins[offset_for_builtin("ticks_left")] = Arc::new(Box::new(BfTicksLeft {}));
        self.builtins[offset_for_builtin("seconds_left")] = Arc::new(Box::new(BfSecondsLeft {}));
        self.builtins[offset_for_builtin("boot_player")] = Arc::new(Box::new(BfBootPlayer {}));
        self.builtins[offset_for_builtin("call_function")] = Arc::new(Box::new(BfCallFunction {}));
        self.builtins[offset_for_builtin("server_log")] = Arc::new(Box::new(BfServerLog {}));
        self.builtins[offset_for_builtin("function_info")] = Arc::new(Box::new(BfFunctionInfo {}));
        self.builtins[offset_for_builtin("listeners")] = Arc::new(Box::new(BfListeners {}));
        self.builtins[offset_for_builtin("eval")] = Arc::new(Box::new(BfEval {}));

        Ok(())
    }
}
