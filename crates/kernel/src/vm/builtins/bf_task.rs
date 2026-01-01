// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

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
    E_ARGS, E_INVARG, E_PERM, E_TYPE, Sequence, Symbol, Variant, v_arc_str, v_int, v_list,
    v_list_iter, v_obj, v_str, v_string, v_sym,
};
use std::time::{Duration, SystemTime};
use tracing::warn;

/// Usage: `any suspend([num seconds])`
/// Suspends the current task for the given number of seconds. If no argument,
/// suspends indefinitely until resumed. Returns the value passed to resume().
fn bf_suspend(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(E_ARGS.msg("suspend() requires 0 or 1 arguments")));
    }

    let suspend_condition = if bf_args.args.is_empty() {
        TaskSuspend::Never
    } else {
        let seconds = match bf_args.args[0].variant() {
            Variant::Float(seconds) => seconds,
            Variant::Int(seconds) => seconds as f64,
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

/// Usage: `bool suspend_if_needed([num threshold])`
/// If remaining ticks are below threshold (default 4000), commits and immediately resumes
/// in a new transaction, returning true. Otherwise returns false without suspending.
fn bf_suspend_if_needed(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(
            E_ARGS.msg("suspend_if_needed() requires 0 or 1 arguments"),
        ));
    }

    let threshold = if bf_args.args.is_empty() {
        4000
    } else {
        match bf_args.args[0].variant() {
            Variant::Float(threshold) => threshold as i64,
            Variant::Int(threshold) => threshold,
            _ => {
                return Err(ErrValue(E_TYPE.msg(
                    "suspend_if_needed() requires a number as the first argument",
                )));
            }
        }
    };

    if threshold < 0 {
        return Err(ErrValue(E_INVARG.msg(
            "suspend_if_needed() requires a non-negative number as the first argument",
        )));
    }

    // Calculate remaining ticks
    let ticks_left = bf_args
        .exec_state
        .max_ticks
        .saturating_sub(bf_args.exec_state.tick_count);

    // If we're within the threshold, suspend with commit (immediate commit/resume)
    if ticks_left < threshold as usize {
        Ok(VmInstr(ExecutionResult::TaskSuspend(TaskSuspend::Commit(
            bf_args.v_bool(true),
        ))))
    } else {
        // Otherwise, just return without suspending
        Ok(Ret(bf_args.v_bool(false)))
    }
}

/// Usage: `any commit([any value])`
/// Commits the current transaction and immediately resumes in a new transaction.
/// Returns the provided value (or false if omitted).
fn bf_commit(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(E_ARGS.msg("commit() takes 0 or 1 arguments")));
    }

    let return_value = if bf_args.args.is_empty() {
        bf_args.v_bool(false)
    } else {
        bf_args.args[0].clone()
    };

    Ok(VmInstr(ExecutionResult::TaskSuspend(TaskSuspend::Commit(
        return_value,
    ))))
}

/// Usage: `none rollback([bool output_session])`
/// Aborts the current transaction, discarding all changes. If output_session is true,
/// the session output buffer is preserved. Wizard-only.
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

/// Usage: `any wait_task(int task_id)`
/// Suspends the current task until the specified task completes. Returns the result
/// of the waited-for task.
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

/// Usage: `str read([obj player [, map metadata]])`
/// Suspends until the player enters a line of input. Optional metadata provides UI hints
/// (input_type, prompt, choices, min/max) for rich clients. Player must be current player.
fn bf_read(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 2 {
        return Err(ErrValue(E_ARGS.msg("read() requires 0 to 2 arguments")));
    }

    // We don't actually support reading from arbitrary connections that aren't the current player,
    // so we'll raise E_INVARG for anything else, because we don't support LambdaMOO's
    // network listener model.
    if !bf_args.args.is_empty() {
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

    // Parse optional metadata (similar to bf_notify)
    let metadata = if bf_args.args.len() == 2 {
        let metadata_arg = &bf_args.args[1];
        let mut metadata_vec = Vec::new();

        match metadata_arg.variant() {
            Variant::Map(m) => {
                for (key, value) in m.iter() {
                    let key_sym = key.as_symbol().map_err(ErrValue)?;
                    metadata_vec.push((key_sym, value));
                }
            }
            Variant::List(l) => {
                for item in l.iter() {
                    match item.variant() {
                        Variant::List(pair) => {
                            if pair.len() != 2 {
                                return Err(ErrValue(E_ARGS.msg(
                                    "read() metadata alist must contain {key, value} pairs",
                                )));
                            }
                            let key_sym = pair[0].as_symbol().map_err(ErrValue)?;
                            metadata_vec.push((key_sym, pair[1].clone()));
                        }
                        _ => {
                            return Err(ErrValue(
                                E_TYPE.msg("read() metadata alist must contain {key, value} pairs"),
                            ));
                        }
                    }
                }
            }
            _ => {
                return Err(ErrValue(
                    E_TYPE.msg("read() metadata must be a map or alist"),
                ));
            }
        }

        Some(metadata_vec)
    } else {
        None
    };

    Ok(VmInstr(ExecutionResult::TaskNeedInput(metadata)))
}

/// Usage: `list queued_tasks()`
/// Returns all suspended tasks. Each entry is `{task_id, start_time, 0, 0, programmer,
/// verb_loc, verb_name, line, this}`.
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
            None => bf_args.v_bool(false),
            Some(start_time) => {
                let time = start_time.duration_since(SystemTime::UNIX_EPOCH).unwrap();
                v_int(time.as_secs() as i64)
            }
        };
        let x = v_int(0);
        let y = v_int(0);
        let programmer = v_obj(task.permissions);
        let verb_loc = v_obj(task.verb_definer);
        let verb_name = v_arc_str(task.verb_name.as_arc_str());
        let line = v_int(task.line_number as i64);
        let this = task.this.clone();
        v_list(&[
            task_id, start_time, x, y, programmer, verb_loc, verb_name, line, this,
        ])
    });

    Ok(Ret(v_list_iter(tasks)))
}

/// Usage: `list active_tasks()`
/// Returns running (not suspended) tasks. Wizards see all tasks, others see only their own.
/// Each entry is `{task_id, player, start_info}` where start_info varies by task type.
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
            v_arc_str(s.as_arc_str())
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
                argstr.clone(),
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
            TaskStart::StartEval { player, .. } => {
                v_list(&[sym_or_str(Symbol::mk("eval")), v_obj(*player)])
            }
            TaskStart::StartExceptionHandler { .. } => {
                // Exception handlers run inline, so they never appear in tasks() output
                panic!("Exception handler should not appear in tasks() listing");
            }
        };
        let entry = v_list(&[task_id, player_id, task_start]);
        output.push(entry);
    }

    Ok(Ret(v_list_iter(output)))
}

/// Usage: `list|int queue_info([obj player])`
/// Without argument (wizard-only): returns list of players with queued tasks.
/// With player argument: returns count of queued tasks for that player.
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

/// Usage: `none kill_task(int task_id)`
/// Terminates the specified task (suspended or running). The caller must own the task
/// or be a wizard.
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

/// Usage: `none resume(int task_id [, any value])`
/// Resumes a suspended task. The optional value becomes the return value of suspend()
/// in the resumed task (defaults to 0).
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
        bf_args.v_bool(false)
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

/// Usage: `int ticks_left()`
/// Returns the number of ticks remaining before the task is forcibly suspended.
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

/// Usage: `int seconds_left()`
/// Returns the number of seconds remaining before the task is forcibly aborted.
/// Returns -1 if there is no time limit.
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

/// Usage: `list callers()`
/// Returns the call stack (excluding current frame). Each entry is
/// `{this, verb_name, programmer, verb_loc, player, line_number}`.
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

/// Usage: `int task_id()`
/// Returns the unique identifier of the currently executing task.
fn bf_task_id(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("task_id() does not take any arguments"),
        ));
    }

    Ok(Ret(v_int(bf_args.exec_state.task_id as i64)))
}

/// Usage: `bool valid_task(int task_id)`
/// Returns true if task_id refers to a currently running or suspended task, false otherwise.
fn bf_valid_task(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(E_ARGS.msg("valid_task() requires 1 argument")));
    }

    let Some(task_id) = bf_args.args[0].as_integer() else {
        return Err(ErrValue(
            E_TYPE.msg("valid_task() requires an integer as the first argument"),
        ));
    };

    let task_id = task_id as TaskId;

    if current_task_scheduler_client()
        .task_list()
        .iter()
        .any(|task| task.task_id == task_id)
    {
        return Ok(Ret(bf_args.v_bool(true)));
    }

    let active_tasks = match current_task_scheduler_client().active_tasks() {
        Ok(tasks) => tasks,
        Err(e) => {
            return Err(ErrValue(e));
        }
    };

    let is_active = active_tasks.iter().any(|(id, _, _)| *id == task_id);
    Ok(Ret(bf_args.v_bool(is_active)))
}

pub(crate) fn register_bf_task(builtins: &mut [BuiltinFunction]) {
    builtins[offset_for_builtin("suspend")] = bf_suspend;
    builtins[offset_for_builtin("suspend_if_needed")] = bf_suspend_if_needed;
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
    builtins[offset_for_builtin("valid_task")] = bf_valid_task;
}
