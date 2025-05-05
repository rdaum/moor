// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use std::io::Read;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, TimeZone};
use chrono_tz::{OffsetName, Tz};
use iana_time_zone::get_timezone;
use tracing::{error, info, warn};

use crate::bf_declare;
use crate::builtins::BfErr::Code;
use crate::builtins::BfRet::{Ret, VmInstr};
use crate::builtins::{
    BfCallState, BfErr, BfRet, BuiltinFunction, bf_perf_counters, world_state_bf_err,
};
use crate::tasks::{TaskStart, sched_counters};
use crate::vm::exec_state::vm_counters;
use crate::vm::{ExecutionResult, TaskSuspend};
use moor_common::build::{PKG_VERSION, SHORT_COMMIT};
use moor_common::model::{Named, ObjFlag, WorldStateError};
use moor_common::tasks::Event::{Present, Unpresent};
use moor_common::tasks::TaskId;
use moor_common::tasks::{NarrativeEvent, Presentation};
use moor_common::util::PerfCounter;
use moor_compiler::compile;
use moor_compiler::{ArgCount, ArgType, BUILTINS, Builtin, offset_for_builtin};
use moor_var::Error::{E_ARGS, E_INVARG, E_INVIND, E_PERM, E_TYPE};
use moor_var::VarType::TYPE_STR;
use moor_var::{Error, Symbol, v_list_iter};
use moor_var::{Sequence, v_map};
use moor_var::{Var, v_float, v_int, v_list, v_none, v_obj, v_str, v_string};
use moor_var::{Variant, v_sym};

fn bf_noop(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    error!(
        "Builtin function {} is not implemented, called with arguments: ({:?})",
        bf_args.name, bf_args.args
    );
    Err(BfErr::Raise(
        E_INVIND,
        Some(format!("Builtin {} is not implemented", bf_args.name)),
        Some(v_str(bf_args.name.as_str())),
    ))
}
bf_declare!(noop, bf_noop);

fn bf_notify(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // If in non rich-mode `notify` can only send text.
    // Otherwise, it can send any value, and it's up to the host/client to interpret it.
    if !bf_args.config.rich_notify {
        if bf_args.args.len() != 2 {
            return Err(BfErr::Code(E_ARGS));
        }
        if bf_args.args[1].type_code() != TYPE_STR {
            return Err(BfErr::Code(E_TYPE));
        }
    }

    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }

    let player = bf_args.args[0].variant();
    let Variant::Obj(player) = player else {
        return Err(BfErr::Code(E_TYPE));
    };

    // If player is not the calling task perms, or a caller is not a wizard, raise E_PERM.
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    task_perms
        .check_obj_owner_perms(player)
        .map_err(world_state_bf_err)?;

    let content_type = if bf_args.config.rich_notify && bf_args.args.len() == 3 {
        let content_type = bf_args.args[2].as_symbol().map_err(Code)?;
        Some(content_type)
    } else {
        None
    };
    let event = NarrativeEvent::notify(
        bf_args.exec_state.this(),
        bf_args.args[1].clone(),
        content_type,
    );
    bf_args.task_scheduler_client.notify(player.clone(), event);

    // MOO docs say this should return none, but in reality it returns 1?
    Ok(Ret(v_int(1)))
}
bf_declare!(notify, bf_notify);

/// presentation(player, id : string, [content_type : string, target : string, content: string, [ attributes : list / map]])
/// Emits a presentation event to the client. The client should interpret this as a request to present
/// the content provided as a pop-up, panel, or other client-specific UI element (depending on 'target')
///
/// If only the first two arguments are provided, the client should "unpresent" the presentation with that ID.
fn bf_present(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.config.rich_notify {
        return Err(BfErr::Code(E_PERM));
    };

    if bf_args.args.len() < 2 || bf_args.args.len() > 6 {
        return Err(BfErr::Code(E_ARGS));
    }

    let player = bf_args.args[0].variant();
    let Variant::Obj(player) = player else {
        return Err(BfErr::Code(E_TYPE));
    };

    // If player is not the calling task perms, or a caller is not a wizard, raise E_PERM.
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    task_perms
        .check_obj_owner_perms(player)
        .map_err(world_state_bf_err)?;

    let id = match bf_args.args[1].variant() {
        Variant::Str(id) => id,
        _ => return Err(BfErr::Code(E_TYPE)),
    };

    // This is unpresent
    if bf_args.args.len() == 2 {
        let event = Unpresent(id.as_str().to_string());
        let event = NarrativeEvent {
            timestamp: SystemTime::now(),
            author: bf_args.exec_state.this(),
            event,
        };
        bf_args.task_scheduler_client.notify(player.clone(), event);

        return Ok(Ret(v_int(1)));
    }

    if bf_args.args.len() < 5 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Str(content_type) = bf_args.args[2].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let Variant::Str(target) = bf_args.args[3].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let Variant::Str(content) = bf_args.args[4].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let mut attributes = vec![];
    if bf_args.args.len() == 6 {
        // must be either a list of { string, string } pairs, or a map of string -> string values.
        match bf_args.args[5].variant() {
            Variant::List(l) => {
                for item in l.iter() {
                    match item.variant() {
                        Variant::List(l) => {
                            if l.len() != 2 {
                                return Err(BfErr::Code(E_ARGS));
                            }
                            let key = match l[0].variant() {
                                Variant::Str(s) => s,
                                _ => return Err(BfErr::Code(E_TYPE)),
                            };
                            let value = match l[1].variant() {
                                Variant::Str(s) => s,
                                _ => return Err(BfErr::Code(E_TYPE)),
                            };
                            attributes.push((key.as_str().to_string(), value.as_str().to_string()));
                        }
                        _ => {
                            return Err(BfErr::Code(E_TYPE));
                        }
                    }
                }
            }
            Variant::Map(m) => {
                for (key, value) in m.iter() {
                    let key = match key.variant() {
                        Variant::Str(s) => s,
                        _ => return Err(BfErr::Code(E_TYPE)),
                    };
                    let value = match value.variant() {
                        Variant::Str(s) => s,
                        _ => return Err(BfErr::Code(E_TYPE)),
                    };
                    attributes.push((key.as_str().to_string(), value.as_str().to_string()));
                }
            }
            _ => {
                return Err(BfErr::Code(E_TYPE));
            }
        }
    }

    let event = Presentation {
        id: id.as_str().to_string(),
        content_type: content_type.as_str().to_string(),
        content: content.as_str().to_string(),
        target: target.as_str().to_string(),
        attributes,
    };

    let event = NarrativeEvent {
        timestamp: SystemTime::now(),
        author: bf_args.exec_state.this(),
        event: Present(event),
    };

    bf_args.task_scheduler_client.notify(player.clone(), event);

    Ok(Ret(v_none()))
}
bf_declare!(present, bf_present);

fn bf_connected_players(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let include_all = if bf_args.args.len() == 1 {
        let Variant::Int(include_all) = bf_args.args[0].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        *include_all == 1
    } else {
        false
    };

    let connected_player_set = bf_args
        .session
        .connected_players()
        .expect("Connected players should always be available");
    let map = connected_player_set.iter().filter_map(|p| {
        if p.id().0 < 0 && !include_all {
            return None;
        }
        Some(v_obj(p.clone()))
    });
    Ok(Ret(v_list_iter(map)))
}
bf_declare!(connected_players, bf_connected_players);

fn bf_is_player(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let player = bf_args.args[0].variant();
    let Variant::Obj(player) = player else {
        return Err(BfErr::Code(E_TYPE));
    };

    let is_player = match bf_args.world_state.flags_of(player) {
        Ok(flags) => flags.contains(ObjFlag::User),
        Err(WorldStateError::ObjectNotFound(_)) => return Err(BfErr::Code(E_ARGS)),
        Err(e) => return Err(world_state_bf_err(e)),
    };
    Ok(Ret(bf_args.v_bool(is_player)))
}
bf_declare!(is_player, bf_is_player);

fn bf_caller_perms(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    Ok(Ret(v_obj(bf_args.caller_perms())))
}
bf_declare!(caller_perms, bf_caller_perms);

fn bf_set_task_perms(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(perms_for) = bf_args.args[0].variant().clone() else {
        return Err(BfErr::Code(E_TYPE));
    };

    // If the caller is not a wizard, perms_for must be the caller
    let perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    if !perms.check_is_wizard().map_err(world_state_bf_err)? && perms_for != perms.who {
        return Err(BfErr::Code(E_PERM));
    }
    bf_args.exec_state.set_task_perms(perms_for);

    Ok(Ret(v_none()))
}
bf_declare!(set_task_perms, bf_set_task_perms);

fn bf_callers(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
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
            v_obj(c.programmer.clone()),
            // verb location
            v_obj(c.definer.clone()),
            // player
            v_obj(c.player.clone()),
            // line number
            v_int(c.line_number as i64),
        ];
        v_list(&callers)
    }))))
}
bf_declare!(callers, bf_callers);

fn bf_task_id(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    Ok(Ret(v_int(bf_args.exec_state.task_id as i64)))
}
bf_declare!(task_id, bf_task_id);

fn bf_idle_seconds(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(who) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Ok(idle_seconds) = bf_args.session.idle_seconds(who.clone()) else {
        return Err(BfErr::Code(E_INVARG));
    };

    Ok(Ret(v_int(idle_seconds as i64)))
}
bf_declare!(idle_seconds, bf_idle_seconds);

fn bf_connected_seconds(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(who) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Ok(connected_seconds) = bf_args.session.connected_seconds(who.clone()) else {
        return Err(BfErr::Code(E_INVARG));
    };

    Ok(Ret(v_int(connected_seconds as i64)))
}
bf_declare!(connected_seconds, bf_connected_seconds);

/*
Syntax:  connection_name (obj <player>)   => str

Returns a network-specific string identifying the connection being used by the given player.  If the programmer is not a wizard and not
<player>, then `E_PERM' is raised.  If <player> is not currently connected, then `E_INVARG' is raised.

 */
fn bf_connection_name(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Obj(player) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let caller = bf_args.caller_perms();
    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_is_wizard()
        .map_err(world_state_bf_err)?
        && caller != *player
    {
        return Err(BfErr::Code(E_PERM));
    }

    let Ok(connection_name) = bf_args.session.connection_name(player.clone()) else {
        return Err(BfErr::Code(E_ARGS));
    };

    Ok(Ret(v_string(connection_name)))
}
bf_declare!(connection_name, bf_connection_name);

fn bf_shutdown(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let msg = if bf_args.args.is_empty() {
        None
    } else {
        let Variant::Str(msg) = bf_args.args[0].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        Some(msg.as_str().to_string())
    };

    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    bf_args.task_scheduler_client.shutdown(msg);

    Ok(Ret(v_none()))
}
bf_declare!(shutdown, bf_shutdown);

fn bf_time(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }
    Ok(Ret(v_int(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    )))
}
bf_declare!(time, bf_time);

fn bf_ftime(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    // If argument is provided and equals 1, return uptime
    if bf_args.args.len() == 1 {
        let Variant::Int(arg) = bf_args.args[0].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };

        if *arg == 1 {
            // Use Instant::now() to get the current monotonic time
            // We need to use a static to track the start time
            use std::sync::OnceLock;
            static START_TIME: OnceLock<Instant> = OnceLock::new();

            // Initialize on first call
            let start = START_TIME.get_or_init(Instant::now);
            let uptime = start.elapsed().as_secs_f64();

            return Ok(Ret(v_float(uptime)));
        } else if *arg == 0 {
            // ftime(0) behaves the same as ftime()
            // Fall through to the default case
        } else {
            return Err(BfErr::Code(E_INVARG));
        }
    }

    // Default: return time since Unix epoch as a float
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let seconds = duration.as_secs() as f64;
    let nanos = duration.subsec_nanos() as f64 / 1_000_000_000.0;

    Ok(Ret(v_float(seconds + nanos)))
}
bf_declare!(ftime, bf_ftime);

fn bf_ctime(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let time = if bf_args.args.is_empty() {
        SystemTime::now()
    } else {
        let Variant::Int(time) = bf_args.args[0].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        if *time < 0 {
            SystemTime::UNIX_EPOCH - Duration::from_secs(time.unsigned_abs())
        } else {
            SystemTime::UNIX_EPOCH + Duration::from_secs(time.unsigned_abs())
        }
    };

    let date_time: DateTime<Local> = chrono::DateTime::from(time);
    let tz_str = get_timezone().unwrap();
    let tz: Tz = tz_str.parse().unwrap();
    let offset = tz.offset_from_local_date(&date_time.date_naive()).unwrap();
    let abbreviation = offset.abbreviation().unwrap_or("??");
    let datetime_str = format!(
        "{} {}",
        date_time.format("%a %b %d %H:%M:%S %Y"),
        abbreviation
    );

    Ok(Ret(v_string(datetime_str.to_string())))
}
bf_declare!(ctime, bf_ctime);
fn bf_raise(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  raise (<code> [, str <message> [, <value>]])   => none
    //
    // Raises <code> as an error in the same way as other MOO expressions, statements, and functions do.  <Message>, which defaults to the value of `tostr(<code>)',
    // and <value>, which defaults to zero, are made available to any `try'-`except' statements that catch the error.  If the error is not caught, then <message> will
    // appear on the first line of the traceback printed to the user.
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Err(err) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_ARGS));
    };

    let msg = if bf_args.args.len() > 1 {
        let Variant::Str(msg) = bf_args.args[1].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        Some(msg.as_str().to_string())
    } else {
        None
    };

    let value = if bf_args.args.len() > 2 {
        Some(bf_args.args[2].clone())
    } else {
        None
    };

    Err(BfErr::Raise(*err, msg, value))
}

bf_declare!(raise, bf_raise);

fn bf_server_version(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }
    let version_string = format!("{}+{}", PKG_VERSION, SHORT_COMMIT);
    Ok(Ret(v_string(version_string)))
}
bf_declare!(server_version, bf_server_version);

fn bf_suspend(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let suspend_condition = if bf_args.args.is_empty() {
        TaskSuspend::Never
    } else {
        let seconds = match bf_args.args[0].variant() {
            Variant::Float(seconds) => *seconds,
            Variant::Int(seconds) => *seconds as f64,
            _ => return Err(BfErr::Code(E_TYPE)),
        };
        if seconds < 0.0 {
            return Err(BfErr::Code(E_INVARG));
        }
        TaskSuspend::Timed(Duration::from_secs_f64(seconds))
    };

    Ok(VmInstr(ExecutionResult::TaskSuspend(suspend_condition)))
}
bf_declare!(suspend, bf_suspend);

fn bf_commit(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    Ok(VmInstr(ExecutionResult::TaskSuspend(TaskSuspend::Commit)))
}
bf_declare!(commit, bf_commit);

fn bf_rollback(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Rollback is wizard only
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.len() > 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let output_session = !bf_args.args.is_empty() && bf_args.args[0].is_true();

    Ok(VmInstr(ExecutionResult::TaskRollback(output_session)))
}
bf_declare!(rollback, bf_rollback);

fn bf_wait_task(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Int(task_id) = bf_args.args[0].variant().clone() else {
        return Err(BfErr::Code(E_TYPE));
    };

    Ok(VmInstr(ExecutionResult::TaskSuspend(
        TaskSuspend::WaitTask(task_id as TaskId),
    )))
}
bf_declare!(wait_task, bf_wait_task);

fn bf_read(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    // We don't actually support reading from arbitrary connections that aren't the current player,
    // so we'll raise E_INVARG for anything else, because we don't support LambdaMOO's
    // network listener model.
    if bf_args.args.len() == 1 {
        let Variant::Obj(requested_player) = bf_args.args[0].variant() else {
            return Err(BfErr::Code(E_ARGS));
        };
        let player = &bf_args.exec_state.top().player;
        if requested_player != player {
            // We log this because we'd like to know if cores are trying to do this.
            warn!(
                requested_player = ?requested_player,
                caller = ?bf_args.exec_state.caller(),
                ?player,
                "read() called with non-current player");
            return Err(BfErr::Code(E_ARGS));
        }
    }

    Ok(VmInstr(ExecutionResult::TaskNeedInput))
}
bf_declare!(read, bf_read);

fn bf_queued_tasks(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    // Ask the scheduler (through its mailbox) to describe all the tasks.
    let tasks = bf_args.task_scheduler_client.task_list();

    // return in form:
    //     {<task-id>, <start-time>, <x>, <y>,
    //      <programmer>, <verb-loc>, <verb-name>, <line>, <this>}
    let tasks = tasks.iter().map(|task| {
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
        let programmer = v_obj(task.permissions.clone());
        let verb_loc = v_obj(task.verb_definer.clone());
        let verb_name = v_str(task.verb_name.as_str());
        let line = v_int(task.line_number as i64);
        let this = task.this.clone();
        v_list(&[
            task_id, start_time, x, y, programmer, verb_loc, verb_name, line, this,
        ])
    });

    Ok(Ret(v_list_iter(tasks)))
}
bf_declare!(queued_tasks, bf_queued_tasks);

/// Function: active_tasks()
/// Returns the list of active running (not suspended/queued) running tasks.
/// If the player is a wizard, it returns the list of all active tasks, otherwise it returns the list of
/// tasks only for the player themselves.
/// The information returned differs from queued_tasks and provides only:
/// { task_id, player_id, task_start } where task_start is a description of how the task was started,
/// and varies depending on the type of task:
/// - { 'command, #handler, #player, "command" }
/// - { 'do_command, #handler, #player, "command" } - when $do_command has been invoked
/// - { 'verb, #player, #verb_location, 'verb, { args }, "argstr" }
/// - { 'eval, #player }
/// - { 'fork, #player, #perms, parent task, verb_location, verb_name, args }
fn bf_active_tasks(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let tasks = match bf_args.task_scheduler_client.active_tasks() {
        Ok(tasks) => tasks,
        Err(e) => {
            return Err(BfErr::Code(e));
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
            v_obj(player_id.clone()) == player
        }
    });

    let sym_or_str = |s| {
        if bf_args.config.symbol_type {
            v_sym(Symbol::mk(s))
        } else {
            v_str(s)
        }
    };

    let mut output = vec![];
    for r in results {
        let task_id = v_int(r.0 as i64);
        let player_id = v_obj(r.1.clone());
        let task_start = match &r.2 {
            TaskStart::StartCommandVerb {
                handler_object,
                player,
                command,
            } => v_list(&[
                sym_or_str("command"),
                v_obj(handler_object.clone()),
                v_obj(player.clone()),
                v_str(command),
            ]),
            TaskStart::StartDoCommand {
                handler_object,
                player,
                command,
            } => v_list(&[
                sym_or_str("do_command"),
                v_obj(handler_object.clone()),
                v_obj(player.clone()),
                v_str(command),
            ]),
            TaskStart::StartVerb {
                player,
                vloc,
                verb,
                args,
                argstr,
            } => v_list(&[
                sym_or_str("verb"),
                v_obj(player.clone()),
                vloc.clone(),
                sym_or_str(verb.as_str()),
                v_list_iter(args.iter()),
                v_str(argstr),
            ]),
            TaskStart::StartFork {
                fork_request,
                suspended: _,
            } => {
                let player = v_obj(fork_request.player.clone());
                let parent_task = v_int(fork_request.parent_task_id as i64);
                let perms = v_obj(fork_request.progr.clone());
                let verb_loc = v_obj(fork_request.activation.verbdef.location().clone());
                let verb_name = v_str(fork_request.activation.verbdef.names().join(" ").as_str());
                let args = v_list_iter(fork_request.activation.args.iter());
                v_list(&[
                    sym_or_str("fork"),
                    player,
                    perms,
                    parent_task,
                    verb_loc,
                    verb_name,
                    args,
                ])
            }
            TaskStart::StartEval { player, program: _ } => {
                v_list(&[sym_or_str("eval"), v_obj(player.clone())])
            }
        };
        let entry = v_list(&[task_id, player_id, task_start]);
        output.push(entry);
    }

    Ok(Ret(v_list_iter(output)))
}
bf_declare!(active_tasks, bf_active_tasks);

/// Function: list queue_info ([obj player])
/// If player is omitted, returns a list of object numbers naming all players that currently have active task
/// queues inside the server. If player is provided, returns the number of background tasks currently queued for that user.
/// It is guaranteed that queue_info(X) will return zero for any X not in the result of queue_info().
fn bf_queue_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let player = if bf_args.args.is_empty() {
        None
    } else {
        let Variant::Obj(player) = bf_args.args[0].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        Some(player)
    };

    let tasks = bf_args.task_scheduler_client.task_list();
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
                .map(|task| task.permissions.clone())
                .collect::<Vec<_>>();

            Ok(Ret(v_list_iter(players.iter().map(|p| v_obj(p.clone())))))
        }
        Some(p) => {
            // Player must be either a wizard, or the player themselves.
            let perms = bf_args.task_perms().map_err(world_state_bf_err)?;
            if !perms.check_is_wizard().map_err(world_state_bf_err)? && !p.eq(&perms.who) {
                return Err(BfErr::Code(E_PERM));
            }
            let queued_tasks = tasks.iter().filter(|t| &t.permissions == p).count();
            Ok(Ret(v_int(queued_tasks as i64)))
        }
    }
}
bf_declare!(queue_info, bf_queue_info);

fn bf_kill_task(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  kill_task(<task-id>)   => none
    //
    // Kills the task with the given <task-id>.
    // The task can be suspended / queued or running.
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Int(victim_task_id) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    // If the task ID is itself, that means returning an Complete execution result, which will cascade
    // back to the task loop and it will terminate itself.
    // Not sure this is *exactly* what MOO does, but it's close enough for now.
    let victim_task_id = *victim_task_id as TaskId;

    if victim_task_id == bf_args.exec_state.task_id {
        return Ok(VmInstr(ExecutionResult::Complete(v_none())));
    }

    let result = bf_args.task_scheduler_client.kill_task(
        victim_task_id,
        bf_args.task_perms().map_err(world_state_bf_err)?,
    );
    if let Variant::Err(err) = result.variant() {
        return Err(BfErr::Code(*err));
    }
    Ok(Ret(result))
}
bf_declare!(kill_task, bf_kill_task);

fn bf_resume(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax: resume(INT task-ID[, ANY value])
    // Resumes a previously suspended task, optionally with a value to pass back to the suspend() call
    if bf_args.args.len() > 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Int(resume_task_id) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    // Optional 2nd argument is the value to return from suspend() in the resumed task.
    let return_value = if bf_args.args.len() == 2 {
        bf_args.args[1].clone()
    } else {
        v_none()
    };

    let task_id = *resume_task_id as TaskId;

    // Resuming ourselves makes no sense, it's not suspended. E_INVARG.
    if task_id == bf_args.exec_state.task_id {
        return Err(BfErr::Code(E_ARGS));
    }

    let result = bf_args.task_scheduler_client.resume_task(
        task_id,
        bf_args.task_perms().map_err(world_state_bf_err)?,
        return_value.clone(),
    );
    if let Variant::Err(err) = result.variant() {
        return Err(BfErr::Code(*err));
    }
    Ok(Ret(result))
}
bf_declare!(resume, bf_resume);

fn bf_ticks_left(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  ticks_left()   => int
    //
    // Returns the number of ticks left in the current time slice.
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    let ticks_left = bf_args
        .exec_state
        .max_ticks
        .saturating_sub(bf_args.exec_state.tick_count);

    Ok(Ret(v_int(ticks_left as i64)))
}
bf_declare!(ticks_left, bf_ticks_left);

fn bf_seconds_left(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  seconds_left()   => int
    //
    // Returns the number of seconds left in the current time slice.
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    let seconds_left = match bf_args.exec_state.time_left() {
        None => v_none(),
        Some(d) => v_int(d.as_secs() as i64),
    };

    Ok(Ret(seconds_left))
}
bf_declare!(seconds_left, bf_seconds_left);

fn bf_boot_player(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  boot_player(<player>)   => none
    //
    // Disconnects the player with the given object number.
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Obj(player) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    if task_perms.who != *player && !task_perms.check_is_wizard().map_err(world_state_bf_err)? {
        return Err(BfErr::Code(E_PERM));
    }

    bf_args.task_scheduler_client.boot_player(player.clone());

    Ok(Ret(v_none()))
}
bf_declare!(boot_player, bf_boot_player);

fn bf_call_function(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  call_function(<func>, <arg1>, <arg2>, ...)   => value
    //
    // Calls the given function with the given arguments and returns the result.
    if bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    let func_name = bf_args.args[0].as_symbol().map_err(Code)?;

    // Arguments are everything left, if any.
    let (_, args) = bf_args.args.pop_front().map_err(|_| BfErr::Code(E_ARGS))?;
    let Variant::List(arguments) = args.variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    // Find the function id for the given function name.

    let builtin = BUILTINS
        .find_builtin(func_name)
        .ok_or(BfErr::Code(E_INVARG))?;

    // Then ask the scheduler to run the function as a continuation of what we're doing now.
    Ok(VmInstr(ExecutionResult::DispatchBuiltin {
        builtin,
        arguments: arguments.clone(),
    }))
}
bf_declare!(call_function, bf_call_function);

/*Syntax:  server_log (str <message> [, <is-error>])   => none

The text in <message> is sent to the server log with a distinctive prefix (so that it can be distinguished from server-generated messages).  If the programmer
is not a wizard, then `E_PERM' is raised.  If <is-error> is provided and true, then <message> is marked in the server log as an error.

*/
fn bf_server_log(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Str(message) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let is_error = if bf_args.args.len() == 2 {
        let Variant::Int(is_error) = bf_args.args[1].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };
        *is_error == 1
    } else {
        false
    };

    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_is_wizard()
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_PERM));
    }

    if is_error {
        error!(
            "SERVER_LOG {}: {}",
            bf_args.exec_state.top().player,
            message
        );
    } else {
        info!(
            "SERVER_LOG {}: {}",
            bf_args.exec_state.top().player,
            message
        );
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
    let types = bf.types.iter().map(|t| match t {
        ArgType::Typed(ty) => v_int(*ty as i64),
        ArgType::Any => v_int(-1),
        ArgType::AnyNum => v_int(-2),
    });

    v_list(&[
        v_str(bf.name.as_str()),
        min_args,
        max_args,
        v_list_iter(types),
    ])
}

fn bf_function_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    if bf_args.args.len() == 1 {
        let func_name = bf_args.args[0].as_symbol().map_err(BfErr::Code)?;
        let bf = BUILTINS
            .find_builtin(func_name)
            .ok_or(BfErr::Code(E_ARGS))?;
        let Some(desc) = BUILTINS.description_for(bf) else {
            return Err(BfErr::Code(E_ARGS));
        };
        let desc = bf_function_info_to_list(desc);
        return Ok(Ret(desc));
    }

    let bf_list = BUILTINS
        .descriptions()
        .filter(|&bf| bf.implemented)
        .map(bf_function_info_to_list);
    Ok(Ret(v_list_iter(bf_list)))
}
bf_declare!(function_info, bf_function_info);

/// Function: value listen (obj object, point [, print-messages], [host-type])
/// Start listening for connections on the given port.
/// `object` is the object to call when a connection is established, in lieux of #0 (the system object)
/// if `print-messages` is true, then the server will print messages like ** Connected ** etc to the connection when it establishes
/// if `host-type` is provided, it should be a string, and it will be used to determine the type of host that will be expected to listen.
///   this defaults to "tcp", but other common can include "websocket"
fn bf_listen(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Requires wizard permissions.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Obj(object) = bf_args.args[0].variant().clone() else {
        return Err(BfErr::Code(E_TYPE));
    };

    // point is a protocol specific value, but for now we'll just assume it's an integer for port
    let Variant::Int(point) = bf_args.args[1].variant().clone() else {
        return Err(BfErr::Code(E_TYPE));
    };

    if point < 0 || point > (u16::MAX as i64) {
        return Err(BfErr::Code(E_INVARG));
    }

    let port = point as u16;

    let print_messages = if bf_args.args.len() >= 3 {
        let Variant::Int(print_messages) = bf_args.args[2].variant().clone() else {
            return Err(BfErr::Code(E_TYPE));
        };
        print_messages == 1
    } else {
        false
    };

    let host_type = if bf_args.args.len() == 4 {
        let Variant::Str(host_type) = bf_args.args[3].variant().clone() else {
            return Err(BfErr::Code(E_TYPE));
        };
        host_type.as_str().to_string()
    } else {
        "tcp".to_string()
    };

    // Ask the scheduler to broadcast a listen request out to all the hosts.
    if let Some(error) =
        bf_args
            .task_scheduler_client
            .listen(object, host_type, port, print_messages)
    {
        return Err(BfErr::Code(error));
    }

    // "Listen() returns canon, a `canonicalized' version of point, with any configuration-specific defaulting or aliasing accounted for. "
    // Uh, ok for now we'll just return the port.
    Ok(Ret(v_int(port as i64)))
}

bf_declare!(listen, bf_listen);

fn bf_listeners(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Requires wizard permissions.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    let listeners = bf_args.task_scheduler_client.listeners();
    let listeners = listeners.iter().map(|listener| {
        let print_messages = if listener.3 { v_int(1) } else { v_int(0) };
        v_list(&[
            v_obj(listener.0.clone()),
            v_int(listener.2 as i64),
            print_messages,
        ])
    });

    let listeners = v_list_iter(listeners);

    Ok(Ret(listeners))
}
bf_declare!(listeners, bf_listeners);

// unlisten(port, [host-type])
fn bf_unlisten(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Requires wizard permissions.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    // point is a protocol specific value, but for now we'll just assume it's an integer for port
    let Variant::Int(point) = bf_args.args[0].variant().clone() else {
        return Err(BfErr::Code(E_TYPE));
    };

    if point < 0 || point > (u16::MAX as i64) {
        return Err(BfErr::Code(E_INVARG));
    }

    let port = point as u16;
    let host_type = if bf_args.args.len() == 4 {
        let Variant::Str(host_type) = bf_args.args[3].variant().clone() else {
            return Err(BfErr::Code(E_TYPE));
        };
        host_type.as_str().to_string()
    } else {
        "tcp".to_string()
    };

    if let Some(err) = bf_args.task_scheduler_client.unlisten(host_type, port) {
        return Err(BfErr::Code(err));
    }

    Ok(Ret(v_none()))
}
bf_declare!(unlisten, bf_unlisten);

pub const BF_SERVER_EVAL_TRAMPOLINE_START_INITIALIZE: usize = 0;
pub const BF_SERVER_EVAL_TRAMPOLINE_RESUME: usize = 1;

fn bf_eval(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_programmer()
        .map_err(world_state_bf_err)?;
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Str(program_code) = bf_args.args[0].variant().clone() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let tramp = bf_args
        .bf_frame_mut()
        .bf_trampoline
        .take()
        .unwrap_or(BF_SERVER_EVAL_TRAMPOLINE_START_INITIALIZE);

    match tramp {
        BF_SERVER_EVAL_TRAMPOLINE_START_INITIALIZE => {
            let program_code = program_code.as_str().to_string();
            let program = match compile(&program_code, bf_args.config.compile_options()) {
                Ok(program) => program,
                Err(e) => return Ok(Ret(v_list(&[v_int(0), v_string(e.to_string())]))),
            };
            let bf_frame = bf_args.bf_frame_mut();
            bf_frame.bf_trampoline = Some(BF_SERVER_EVAL_TRAMPOLINE_RESUME);
            // Now we have to construct things to set up for eval. Which means tramping through with a
            // setup-for-eval result here.
            Ok(VmInstr(ExecutionResult::DispatchEval {
                permissions: bf_args.task_perms_who(),
                player: bf_args.exec_state.top().player.clone(),
                program,
            }))
        }
        BF_SERVER_EVAL_TRAMPOLINE_RESUME => {
            // Value must be on in our activation's "return value"
            let value = bf_args.exec_state.top().frame.return_value();
            Ok(Ret(v_list(&[bf_args.v_bool(true), value])))
        }
        _ => {
            panic!("Invalid trampoline value for bf_eval: {}", tramp);
        }
    }
}
bf_declare!(eval, bf_eval);

fn bf_dump_database(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    bf_args.task_scheduler_client.checkpoint();

    Ok(Ret(bf_args.v_bool(true)))
}
bf_declare!(dump_database, bf_dump_database);

fn bf_memory_usage(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    // Must be wizard.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    // Get system page size
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page_size == -1 {
        return Err(BfErr::Code(Error::E_QUOTA));
    }

    // Then read /proc/self/statm
    let mut statm = String::new();
    std::fs::File::open("/proc/self/statm")
        .map_err(|_| BfErr::Code(Error::E_QUOTA))?
        .read_to_string(&mut statm)
        .map_err(|_| BfErr::Code(Error::E_QUOTA))?;

    // Split on whitespace -- then we have VmSize and VmRSS in pages
    let mut statm = statm.split_whitespace();
    let vm_size = statm
        .next()
        .ok_or(BfErr::Code(Error::E_QUOTA))?
        .parse::<i64>()
        .map_err(|_| BfErr::Code(Error::E_QUOTA))?;
    let vm_rss = statm
        .next()
        .ok_or(BfErr::Code(Error::E_QUOTA))?
        .parse::<i64>()
        .map_err(|_| BfErr::Code(Error::E_QUOTA))?;

    // Return format for memory_usage is:
    // {block-size, nused, nfree}
    //
    // "where block-size is the size in bytes of a particular class of memory
    // fragments, nused is the number of such fragments currently in use in the server,
    // and nfree is the number of such fragments that have been reserved for use but are
    // currently free.
    // So for our purposes, block-size = our page size, nfree is vm_size - vm_rss, and nused is vm_rss.
    let block_size = v_int(page_size);
    let nused = v_int(vm_rss);
    let nfree = v_int(vm_size - vm_rss);

    Ok(Ret(v_list(&[block_size, nused, nfree])))
}
bf_declare!(memory_usage, bf_memory_usage);

fn db_disk_size(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  db_disk_size()   => int
    //
    // Returns the number of bytes currently occupied by the database on disk.
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    // Must be wizard.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    let disk_size = bf_args.world_state.db_usage().map_err(world_state_bf_err)?;

    Ok(Ret(v_int(disk_size as i64)))
}
bf_declare!(db_disk_size, db_disk_size);

/* Function: none load_server_options ()

   This causes the server to consult the current common of properties on $server_options, updating
   the corresponding server option settings (see section Server Options Set in the Database)
   accordingly. If the programmer is not a wizard, then E_PERM is raised.
*/
fn load_server_options(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }

    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    bf_args.task_scheduler_client.refresh_server_options();

    Ok(Ret(v_none()))
}
bf_declare!(load_server_options, load_server_options);

fn counter_map(counters: &[&PerfCounter], use_symbols: bool) -> Var {
    let mut result = vec![];
    for c in counters {
        let op_name = if use_symbols {
            v_sym(c.operation)
        } else {
            v_str(c.operation.as_str())
        };

        result.push((
            op_name,
            v_list(&[
                v_int(c.invocations.sum() as i64),
                v_int(c.cumulative_duration_nanos.sum() as i64),
            ]),
        ));
    }

    v_map(&result)
}

fn bf_bf_counters(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    let counters = bf_perf_counters();
    Ok(Ret(counter_map(
        &counters.all_counters(),
        bf_args.config.use_symbols_in_builtins && bf_args.config.symbol_type,
    )))
}
bf_declare!(bf_counters, bf_bf_counters);

fn bf_db_counters(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    let counters = bf_args.world_state.perf_counters();
    Ok(Ret(counter_map(
        &counters.all_counters(),
        bf_args.config.use_symbols_in_builtins && bf_args.config.symbol_type,
    )))
}
bf_declare!(db_counters, bf_db_counters);

fn bf_vm_counters(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    let counters = vm_counters();
    Ok(Ret(counter_map(
        &counters.all_counters(),
        bf_args.config.use_symbols_in_builtins && bf_args.config.symbol_type,
    )))
}
bf_declare!(vm_counters, bf_vm_counters);

fn bf_sched_counters(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    let counters = sched_counters();
    Ok(Ret(counter_map(
        &counters.all_counters(),
        bf_args.config.use_symbols_in_builtins && bf_args.config.symbol_type,
    )))
}
bf_declare!(sched_counters, bf_sched_counters);

fn bf_force_input(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    /*Syntax:  force_input (obj <conn>, str <line> [, <at-front>])   => none
     */
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }

    // We always ignore 3rd argument ("at_front"), it makes no sense in mooR.
    let Variant::Obj(conn) = bf_args.args[0].variant().clone() else {
        return Err(BfErr::Code(E_TYPE));
    };

    // Must be either player or wizard
    let perms = bf_args.task_perms().map_err(world_state_bf_err)?;

    if perms.who != conn && !perms.check_is_wizard().map_err(world_state_bf_err)? {
        return Err(BfErr::Code(E_PERM));
    }

    let Variant::Str(line) = bf_args.args[1].variant().clone() else {
        return Err(BfErr::Code(E_TYPE));
    };

    match bf_args
        .task_scheduler_client
        .force_input(conn, line.as_str().to_string())
    {
        Ok(task_id) => Ok(Ret(v_int(task_id as i64))),
        Err(e) => Err(Code(e)),
    }
}
bf_declare!(force_input, bf_force_input);

/// worker_request(worker_type, args, ...)
/// Sends a request to a worker (e.g. outbound HTTP, files, etc.) to perform some action.
/// Task then goes into suspension until the request is completed or times out.
/// Wizard only.
fn bf_worker_request(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.len() < 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let worker_type = bf_args.args[0].as_symbol().map_err(BfErr::Code)?;

    // The args are everything after the first argument
    let args = bf_args.args.iter().skip(1).collect();
    Ok(VmInstr(ExecutionResult::TaskSuspend(
        TaskSuspend::WorkerRequest(worker_type, args),
    )))
}
bf_declare!(worker_request, bf_worker_request);

pub(crate) fn register_bf_server(builtins: &mut [Box<dyn BuiltinFunction>]) {
    builtins[offset_for_builtin("notify")] = Box::new(BfNotify {});
    builtins[offset_for_builtin("connected_players")] = Box::new(BfConnectedPlayers {});
    builtins[offset_for_builtin("is_player")] = Box::new(BfIsPlayer {});
    builtins[offset_for_builtin("caller_perms")] = Box::new(BfCallerPerms {});
    builtins[offset_for_builtin("set_task_perms")] = Box::new(BfSetTaskPerms {});
    builtins[offset_for_builtin("callers")] = Box::new(BfCallers {});
    builtins[offset_for_builtin("task_id")] = Box::new(BfTaskId {});
    builtins[offset_for_builtin("idle_seconds")] = Box::new(BfIdleSeconds {});
    builtins[offset_for_builtin("connected_seconds")] = Box::new(BfConnectedSeconds {});
    builtins[offset_for_builtin("connection_name")] = Box::new(BfConnectionName {});
    builtins[offset_for_builtin("time")] = Box::new(BfTime {});
    builtins[offset_for_builtin("ftime")] = Box::new(BfFtime {});
    builtins[offset_for_builtin("ctime")] = Box::new(BfCtime {});
    builtins[offset_for_builtin("raise")] = Box::new(BfRaise {});
    builtins[offset_for_builtin("server_version")] = Box::new(BfServerVersion {});
    builtins[offset_for_builtin("shutdown")] = Box::new(BfShutdown {});
    builtins[offset_for_builtin("suspend")] = Box::new(BfSuspend {});
    builtins[offset_for_builtin("queued_tasks")] = Box::new(BfQueuedTasks {});
    builtins[offset_for_builtin("active_tasks")] = Box::new(BfActiveTasks {});
    builtins[offset_for_builtin("queue_info")] = Box::new(BfQueueInfo {});
    builtins[offset_for_builtin("kill_task")] = Box::new(BfKillTask {});
    builtins[offset_for_builtin("resume")] = Box::new(BfResume {});
    builtins[offset_for_builtin("ticks_left")] = Box::new(BfTicksLeft {});
    builtins[offset_for_builtin("seconds_left")] = Box::new(BfSecondsLeft {});
    builtins[offset_for_builtin("boot_player")] = Box::new(BfBootPlayer {});
    builtins[offset_for_builtin("call_function")] = Box::new(BfCallFunction {});
    builtins[offset_for_builtin("server_log")] = Box::new(BfServerLog {});
    builtins[offset_for_builtin("function_info")] = Box::new(BfFunctionInfo {});
    builtins[offset_for_builtin("listeners")] = Box::new(BfListeners {});
    builtins[offset_for_builtin("listen")] = Box::new(BfListen {});
    builtins[offset_for_builtin("unlisten")] = Box::new(BfUnlisten {});
    builtins[offset_for_builtin("eval")] = Box::new(BfEval {});
    builtins[offset_for_builtin("read")] = Box::new(BfRead {});
    builtins[offset_for_builtin("dump_database")] = Box::new(BfDumpDatabase {});
    builtins[offset_for_builtin("memory_usage")] = Box::new(BfMemoryUsage {});
    builtins[offset_for_builtin("db_disk_size")] = Box::new(BfDbDiskSize {});
    builtins[offset_for_builtin("load_server_options")] = Box::new(BfLoadServerOptions {});
    builtins[offset_for_builtin("bf_counters")] = Box::new(BfBfCounters {});
    builtins[offset_for_builtin("db_counters")] = Box::new(BfDbCounters {});
    builtins[offset_for_builtin("vm_counters")] = Box::new(BfVmCounters {});
    builtins[offset_for_builtin("sched_counters")] = Box::new(BfSchedCounters {});
    builtins[offset_for_builtin("force_input")] = Box::new(BfForceInput {});
    builtins[offset_for_builtin("wait_task")] = Box::new(BfWaitTask {});
    builtins[offset_for_builtin("commit")] = Box::new(BfCommit {});
    builtins[offset_for_builtin("rollback")] = Box::new(BfRollback {});
    builtins[offset_for_builtin("present")] = Box::new(BfPresent {});
    builtins[offset_for_builtin("worker_request")] = Box::new(BfWorkerRequest {});
}
