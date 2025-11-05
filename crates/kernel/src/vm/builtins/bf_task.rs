use crate::task_context::current_task_scheduler_client;
use crate::tasks::TaskStart;
use crate::vm::TaskSuspend;
use crate::vm::builtins::BfErr::ErrValue;
use crate::vm::builtins::BfRet::{Ret, VmInstr};
use crate::vm::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction, world_state_bf_err};
use crate::vm::vm_host::ExecutionResult;
use moor_common::builtins::offset_for_builtin;
use moor_common::model::Named;
use moor_common::tasks::TaskId;
use moor_var::{
    E_ARGS, E_INVARG, E_PERM, E_TYPE, Sequence, Symbol, Variant, v_arc_string, v_bool_int, v_int,
    v_list, v_list_iter, v_obj, v_str, v_string, v_sym,
};
use std::time::{Duration, SystemTime};
use tracing::warn;

/// Suspends the current task for the given number of seconds.
/// MOO: `none suspend([num seconds])`
fn bf_suspend(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(E_ARGS.msg("suspend() requires 0 or 1 arguments")));
    }

    let suspend_condition = if bf_args.args.is_empty() {
        TaskSuspend::Never
    } else {
        let seconds = match bf_args.args[0].variant() {
            Variant::Float(seconds) => *seconds,
            Variant::Int(seconds) => *seconds as f64,
            _ => {
                return Err(ErrValue(
                    E_TYPE.msg("suspend() requires a number as the first argument"),
                ));
            }
        };
        if seconds < 0.0 {
            return Err(ErrValue(
                E_INVARG.msg("suspend() requires a positive number as the first argument"),
            ));
        }
        TaskSuspend::Timed(Duration::from_secs_f64(seconds))
    };

    Ok(VmInstr(ExecutionResult::TaskSuspend(suspend_condition)))
}

/// Commits the current transaction and suspends the task, optionally returning a value.
/// MOO: `any commit([any value])`
fn bf_commit(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(E_ARGS.msg("commit() takes 0 or 1 arguments")));
    }

    let return_value = if bf_args.args.is_empty() {
        v_bool_int(false)
    } else {
        bf_args.args[0].clone()
    };

    Ok(VmInstr(ExecutionResult::TaskSuspend(TaskSuspend::Commit(
        return_value,
    ))))
}

/// Rolls back the current transaction. Wizard-only.
/// MOO: `none rollback([bool output_session])`
fn bf_rollback(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Rollback is wizard only
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.len() > 1 {
        return Err(ErrValue(E_ARGS.msg("rollback() requires 0 or 1 arguments")));
    }

    let output_session = !bf_args.args.is_empty() && bf_args.args[0].is_true();

    Ok(VmInstr(ExecutionResult::TaskRollback(output_session)))
}

/// Suspends the current task until the specified task completes.
/// MOO: `none wait_task(int task_id)`
fn bf_wait_task(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(E_ARGS.msg("wait_task() requires 1 argument")));
    }

    let Some(task_id) = bf_args.args[0].as_integer() else {
        return Err(ErrValue(
            E_TYPE.msg("wait_task() requires an integer as the first argument"),
        ));
    };

    Ok(VmInstr(ExecutionResult::TaskSuspend(
        TaskSuspend::WaitTask(task_id as TaskId),
    )))
}

/// Suspends the current task to wait for input from a player.
/// MOO: `str read([obj player])`
fn bf_read(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(E_ARGS.msg("read() requires 0 or 1 arguments")));
    }

    // We don't actually support reading from arbitrary connections that aren't the current player,
    // so we'll raise E_INVARG for anything else, because we don't support LambdaMOO's
    // network listener model.
    if bf_args.args.len() == 1 {
        let Some(requested_player) = bf_args.args[0].as_object() else {
            return Err(ErrValue(
                E_ARGS.msg("read() requires an object as the first argument"),
            ));
        };
        let player = &bf_args.exec_state.top().player;
        if requested_player != *player {
            // We log this because we'd like to know if cores are trying to do this.
            warn!(
                requested_player = ?requested_player,
                caller = ?bf_args.exec_state.caller(),
                ?player,
                "read() called with non-current player");
            return Err(ErrValue(
                E_ARGS.msg("read() requires the current player as the first argument"),
            ));
        }
    }

    Ok(VmInstr(ExecutionResult::TaskNeedInput))
}

/// Returns a list of all queued (suspended) tasks.
/// MOO: `list queued_tasks()`
fn bf_queued_tasks(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("queued_tasks() does not take any arguments"),
        ));
    }

    // Ask the scheduler (through its mailbox) to describe all the tasks.
    let tasks = current_task_scheduler_client().task_list();

    // return in form:
    //     {<task-id>, <start-time>, <x>, <y>,
    //      <programmer>, <verb-loc>, <verb-name>, <line>, <this>}
    let tasks = tasks.iter().map(|task| {
        let task_id = v_int(task.task_id as i64);
        let start_time = match task.start_time {
            None => v_bool_int(false),
            Some(start_time) => {
                let time = start_time.duration_since(SystemTime::UNIX_EPOCH).unwrap();
                v_int(time.as_secs() as i64)
            }
        };
        let x = v_bool_int(false);
        let y = v_bool_int(false);
        let programmer = v_obj(task.permissions);
        let verb_loc = v_obj(task.verb_definer);
        let verb_name = v_arc_string(task.verb_name.as_arc_string());
        let line = v_int(task.line_number as i64);
        let this = task.this.clone();
        v_list(&[
            task_id, start_time, x, y, programmer, verb_loc, verb_name, line, this,
        ])
    });

    Ok(Ret(v_list_iter(tasks)))
}

/// Returns the list of active running (not suspended/queued) running tasks.
/// If the player is a wizard, it returns the list of all active tasks, otherwise it returns the list of
/// tasks only for the player themselves.
/// MOO: `list active_tasks()`
fn bf_active_tasks(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let tasks = match current_task_scheduler_client().active_tasks() {
        Ok(tasks) => tasks,
        Err(e) => {
            return Err(ErrValue(e));
        }
    };

    let player = bf_args.exec_state.caller();
    let is_wizard = bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_is_wizard()
        .map_err(world_state_bf_err)?;

    let results = tasks.iter().filter(|(_, player_id, _)| {
        if is_wizard {
            true
        } else {
            v_obj(*player_id) == player
        }
    });

    let sym_or_str = |s: Symbol| {
        if bf_args.config.symbol_type {
            v_sym(s)
        } else {
            v_arc_string(s.as_arc_string())
        }
    };

    let mut output = vec![];
    for r in results {
        let task_id = v_int(r.0 as i64);
        let player_id = v_obj(r.1);
        let task_start = match &r.2 {
            TaskStart::StartCommandVerb {
                handler_object,
                player,
                command,
            } => v_list(&[
                sym_or_str(Symbol::mk("command")),
                v_obj(*handler_object),
                v_obj(*player),
                v_str(command),
            ]),
            TaskStart::StartDoCommand {
                handler_object,
                player,
                command,
            } => v_list(&[
                sym_or_str(Symbol::mk("do_command")),
                v_obj(*handler_object),
                v_obj(*player),
                v_str(command),
            ]),
            TaskStart::StartVerb {
                player,
                vloc,
                verb,
                args,
                argstr,
            } => v_list(&[
                sym_or_str(Symbol::mk("verb")),
                v_obj(*player),
                vloc.clone(),
                sym_or_str(*verb),
                v_list_iter(args.iter()),
                v_str(argstr),
            ]),
            TaskStart::StartFork {
                fork_request,
                suspended: _,
            } => {
                let player = v_obj(fork_request.player);
                let parent_task = v_int(fork_request.parent_task_id as i64);
                let perms = v_obj(fork_request.progr);
                let verb_loc = v_obj(fork_request.activation.verbdef.location());
                let verb_name = v_str(
                    fork_request
                        .activation
                        .verbdef
                        .names()
                        .iter()
                        .map(|s| s.as_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                        .as_str(),
                );
                let args = v_list_iter(fork_request.activation.args.iter());
                v_list(&[
                    sym_or_str(Symbol::mk("fork")),
                    player,
                    perms,
                    parent_task,
                    verb_loc,
                    verb_name,
                    args,
                ])
            }
            TaskStart::StartEval { player, program: _ } => {
                v_list(&[sym_or_str(Symbol::mk("eval")), v_obj(*player)])
            }
        };
        let entry = v_list(&[task_id, player_id, task_start]);
        output.push(entry);
    }

    Ok(Ret(v_list_iter(output)))
}

/// If player is omitted, returns a list of object numbers naming all players that currently have active task
/// queues inside the server. If player is provided, returns the number of background tasks currently queued for that user.
/// MOO: `list queue_info([obj player])`
fn bf_queue_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(
            E_ARGS.msg("queue_info() requires 0 or 1 arguments"),
        ));
    }

    let player = if bf_args.args.is_empty() {
        None
    } else {
        let Some(player) = bf_args.args[0].as_object() else {
            return Err(ErrValue(
                E_TYPE.msg("queue_info() requires an object as the first argument"),
            ));
        };
        Some(player)
    };

    let tasks = current_task_scheduler_client().task_list();
    // Two modes: if player is None, we return a list of all players with queued tasks, but we
    // expect wiz perms.
    // If player is set, we return the number of tasks queued for that player.
    match player {
        None => {
            // Check wiz perms
            bf_args
                .task_perms()
                .map_err(world_state_bf_err)?
                .check_wizard()
                .map_err(world_state_bf_err)?;

            // Now we can get the list of players with queued tasks.
            let players = tasks
                .iter()
                .map(|task| task.permissions)
                .collect::<Vec<_>>();

            Ok(Ret(v_list_iter(players.iter().map(|p| v_obj(*p)))))
        }
        Some(p) => {
            // Player must be either a wizard, or the player themselves.
            let perms = bf_args.task_perms().map_err(world_state_bf_err)?;
            if !perms.check_is_wizard().map_err(world_state_bf_err)? && !p.eq(&perms.who) {
                return Err(ErrValue(E_PERM.msg(
                    "queue_info() requires the caller to be a wizard or the caller itself",
                )));
            }
            let queued_tasks = tasks.iter().filter(|t| t.permissions == p).count();
            Ok(Ret(v_int(queued_tasks as i64)))
        }
    }
}

/// Kills the task with the given <task-id>.
/// The task can be suspended / queued or running.
/// MOO: `none kill_task(int task_id)`
fn bf_kill_task(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(E_ARGS.msg("kill_task() requires 1 argument")));
    }

    let Some(victim_task_id) = bf_args.args[0].as_integer() else {
        return Err(ErrValue(
            E_TYPE.msg("kill_task() requires an integer as the first argument"),
        ));
    };

    // If the task ID is itself, that means returning an Complete execution result, which will cascade
    // back to the task loop and it will terminate itself.
    // Not sure this is *exactly* what MOO does, but it's close enough for now.
    let victim_task_id = victim_task_id as TaskId;

    if victim_task_id == bf_args.exec_state.task_id {
        return Ok(VmInstr(ExecutionResult::Complete(v_int(0))));
    }

    let result = current_task_scheduler_client().kill_task(
        victim_task_id,
        bf_args.task_perms().map_err(world_state_bf_err)?,
    );
    if let Some(err) = result.as_error() {
        return Err(ErrValue(err.clone()));
    }
    Ok(Ret(result))
}

/// Resumes a previously suspended task, optionally with a value to pass back to the suspend() call.
/// MOO: `none resume(int task_id [, any value])`
fn bf_resume(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 2 {
        return Err(ErrValue(E_ARGS.msg("resume() requires 1 or 2 arguments")));
    }

    let Some(resume_task_id) = bf_args.args[0].as_integer() else {
        return Err(ErrValue(
            E_TYPE.msg("resume() requires an integer as the first argument"),
        ));
    };

    // Optional 2nd argument is the value to return from suspend() in the resumed task.
    let return_value = if bf_args.args.len() == 2 {
        bf_args.args[1].clone()
    } else {
        v_bool_int(false)
    };

    let task_id = resume_task_id as TaskId;

    // Resuming ourselves makes no sense, it's not suspended. E_INVARG.
    if task_id == bf_args.exec_state.task_id {
        return Err(ErrValue(
            E_ARGS.msg("resume() cannot resume the current task"),
        ));
    }

    let result = current_task_scheduler_client().resume_task(
        task_id,
        bf_args.task_perms().map_err(world_state_bf_err)?,
        return_value.clone(),
    );
    if let Some(err) = result.as_error() {
        return Err(ErrValue(err.clone()));
    }
    Ok(Ret(result))
}

/// Returns the number of ticks left in the current time slice.
/// MOO: `int ticks_left()`
fn bf_ticks_left(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("ticks_left() does not take any arguments"),
        ));
    }

    let ticks_left = bf_args
        .exec_state
        .max_ticks
        .saturating_sub(bf_args.exec_state.tick_count);

    Ok(Ret(v_int(ticks_left as i64)))
}

/// Returns the number of seconds left in the current time slice.
/// MOO: `int seconds_left()`
fn bf_seconds_left(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("seconds_left() does not take any arguments"),
        ));
    }

    let seconds_left = match bf_args.exec_state.time_left() {
        None => v_int(-1),
        Some(d) => v_int(d.as_secs() as i64),
    };

    Ok(Ret(seconds_left))
}

/// Returns information about the current calling stack.
/// MOO: `list callers()`
fn bf_callers(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("callers() does not take any arguments"),
        ));
    }

    // We have to exempt ourselves from the callers list.
    let callers = bf_args.exec_state.callers()[1..].to_vec();
    Ok(Ret(v_list_iter(callers.iter().map(|c| {
        let callers = vec![
            // this
            c.this.clone(),
            // verb name
            v_string(c.verb_name.to_string()),
            // 'programmer'
            v_obj(c.programmer),
            // verb location
            v_obj(c.definer),
            // player
            v_obj(c.player),
            // line number
            v_int(c.line_number as i64),
        ];
        v_list(&callers)
    }))))
}

/// Returns the unique identifier of the current task.
/// MOO: `int task_id()`
fn bf_task_id(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("task_id() does not take any arguments"),
        ));
    }

    Ok(Ret(v_int(bf_args.exec_state.task_id as i64)))
}

pub(crate) fn register_bf_task(builtins: &mut [BuiltinFunction]) {
    builtins[offset_for_builtin("suspend")] = bf_suspend;
    builtins[offset_for_builtin("queued_tasks")] = bf_queued_tasks;
    builtins[offset_for_builtin("active_tasks")] = bf_active_tasks;
    builtins[offset_for_builtin("queue_info")] = bf_queue_info;
    builtins[offset_for_builtin("kill_task")] = bf_kill_task;
    builtins[offset_for_builtin("resume")] = bf_resume;
    builtins[offset_for_builtin("ticks_left")] = bf_ticks_left;
    builtins[offset_for_builtin("seconds_left")] = bf_seconds_left;
    builtins[offset_for_builtin("read")] = bf_read;
    builtins[offset_for_builtin("wait_task")] = bf_wait_task;
    builtins[offset_for_builtin("commit")] = bf_commit;
    builtins[offset_for_builtin("rollback")] = bf_rollback;
    builtins[offset_for_builtin("callers")] = bf_callers;
    builtins[offset_for_builtin("task_id")] = bf_task_id;
}
