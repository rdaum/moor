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

use crate::tasks::{TaskStart, sched_counters};
use crate::vm::TaskSuspend;
use crate::vm::builtins::BfErr::{Code, ErrValue};
use crate::vm::builtins::BfRet::{Ret, RetNil, VmInstr};
use crate::vm::builtins::{
    BfCallState, BfErr, BfRet, BuiltinFunction, bf_perf_counters, world_state_bf_err,
};
use crate::vm::vm_host::ExecutionResult;
use moor_common::build::{PKG_VERSION, SHORT_COMMIT};
use moor_common::model::{Named, ObjFlag, WorldStateError};
use moor_common::tasks::Event::{Present, Unpresent};
use moor_common::tasks::TaskId;
use moor_common::tasks::{NarrativeEvent, Presentation, SessionError};
use moor_common::util::PerfCounter;
use moor_compiler::compile;
use moor_compiler::{ArgCount, ArgType, BUILTINS, Builtin, offset_for_builtin};
use moor_db::db_counters;
use moor_var::VarType::TYPE_STR;
use moor_var::{
    E_ARGS, E_INVARG, E_INVIND, E_PERM, E_QUOTA, E_TYPE, Error, Symbol, v_arc_string, v_bool_int,
    v_list_iter,
};
use moor_var::{Sequence, v_map};
use moor_var::{Var, v_float, v_int, v_list, v_obj, v_str, v_string};
use moor_var::{Variant, v_sym};

pub(crate) fn bf_noop(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    error!(
        "Builtin function {} is not implemented, called with arguments: ({:?})",
        bf_args.name, bf_args.args
    );
    Err(BfErr::Raise(E_INVIND.with_msg_and_value(
        || format!("Builtin {} is not implemented", bf_args.name),
        v_arc_string(bf_args.name.as_arc_string()),
    )))
}

fn bf_notify(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // If in non rich-mode `notify` can only send text.
    // Otherwise, it can send any value, and it's up to the host/client to interpret it.
    if !bf_args.config.rich_notify {
        if bf_args.args.len() != 2 {
            return Err(ErrValue(E_ARGS.msg("notify() requires 2 arguments")));
        }
        if bf_args.args[1].type_code() != TYPE_STR {
            return Err(ErrValue(
                E_TYPE.msg("notify() requires a string as the second argument"),
            ));
        }
    }

    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(ErrValue(E_ARGS.msg("notify() requires 2 or 3 arguments")));
    }

    let Some(player) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("notify() requires an object as the first argument"),
        ));
    };

    // If player is not the calling task perms, or a caller is not a wizard, raise E_PERM.
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    task_perms
        .check_obj_owner_perms(&player)
        .map_err(world_state_bf_err)?;

    let content_type = if bf_args.config.rich_notify && bf_args.args.len() == 3 {
        let content_type = bf_args.args[2].as_symbol().map_err(ErrValue)?;
        Some(content_type)
    } else {
        None
    };
    let event = NarrativeEvent::notify(
        bf_args.exec_state.this(),
        bf_args.args[1].clone(),
        content_type,
    );
    bf_args
        .task_scheduler_client
        .notify(player, Box::new(event));

    // MOO docs say this should return none, but in reality it returns 1?
    Ok(Ret(v_int(1)))
}

/// presentation(player, id : string, [content_type : string, target : string, content: string, [ attributes : list / map]])
/// Emits a presentation event to the client. The client should interpret this as a request to present
/// the content provided as a pop-up, panel, or other client-specific UI element (depending on 'target')
///
/// If only the first two arguments are provided, the client should "unpresent" the presentation with that ID.
fn bf_present(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.config.rich_notify {
        return Err(ErrValue(E_PERM.msg("present() is not available")));
    };

    if bf_args.args.len() < 2 || bf_args.args.len() > 6 {
        return Err(ErrValue(E_ARGS.msg("present() requires 2 to 6 arguments")));
    }

    let Some(player) = bf_args.args[0].as_object() else {
        return Err(ErrValue(E_TYPE.with_msg(|| {
            format!(
                "present() requires an object as the first argument, got {:?}",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    // If player is not the calling task perms, or a caller is not a wizard, raise E_PERM.
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    task_perms
        .check_obj_owner_perms(&player)
        .map_err(world_state_bf_err)?;

    let id = match bf_args.args[1].variant() {
        Variant::Str(id) => id,
        _ => {
            return Err(ErrValue(
                E_TYPE.msg("present() requires a string as the second argument"),
            ));
        }
    };

    // This is unpresent
    if bf_args.args.len() == 2 {
        let event = Unpresent(id.as_str().to_string());
        let event = NarrativeEvent {
            event_id: uuid::Uuid::now_v7(),
            timestamp: SystemTime::now(),
            author: bf_args.exec_state.this(),
            event,
        };
        bf_args
            .task_scheduler_client
            .notify(player, Box::new(event));

        return Ok(Ret(v_int(1)));
    }

    if bf_args.args.len() < 5 {
        return Err(ErrValue(E_ARGS.with_msg(|| {
            format!(
                "present() requires at least 5 arguments, got {}",
                bf_args.args.len()
            )
        })));
    }

    let Some(content_type) = bf_args.args[2].as_string() else {
        return Err(ErrValue(E_TYPE.with_msg(|| {
            format!(
                "present() requires a string as the third argument, got {:?}",
                bf_args.args[2].type_code().to_literal()
            )
        })));
    };

    let Some(target) = bf_args.args[3].as_string() else {
        return Err(ErrValue(E_TYPE.with_msg(|| {
            format!(
                "present() requires a string as the fourth argument, got {:?}",
                bf_args.args[3].type_code().to_literal()
            )
        })));
    };

    let Some(content) = bf_args.args[4].as_string() else {
        return Err(ErrValue(E_TYPE.with_msg(|| {
            format!(
                "present() requires a string as the fifth argument, got {:?}",
                bf_args.args[4].type_code().to_literal()
            )
        })));
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
                                return Err(ErrValue(E_ARGS.msg(
                                    "present() requires a list of { string, string } pairs as the sixth argument",
                                )));
                            }
                            let key = match l[0].variant() {
                                Variant::Str(s) => s,
                                _ => return Err(ErrValue(E_TYPE.msg(
                                    "present() requires a list of { string, string } pairs as the sixth argument",
                                ))),
                            };
                            let value = match l[1].variant() {
                                Variant::Str(s) => s,
                                _ => return Err(ErrValue(E_TYPE.msg(
                                    "present() requires a list of { string, string } pairs as the sixth argument",
                                ))),
                            };
                            attributes.push((key.as_str().to_string(), value.as_str().to_string()));
                        }
                        _ => {
                            return Err(ErrValue(E_TYPE.msg(
                                "present() requires a list of { string, string } pairs as the sixth argument",
                            )));
                        }
                    }
                }
            }
            Variant::Map(m) => {
                for (key, value) in m.iter() {
                    let key = match key.variant() {
                        Variant::Str(s) => s,
                        _ => return Err(ErrValue(E_TYPE.msg(
                            "present() requires a map of string -> string pairs as the sixth argument",
                        ))),
                    };
                    let value = match value.variant() {
                        Variant::Str(s) => s,
                        _ => return Err(ErrValue(E_TYPE.msg(
                            "present() requires a map of string -> string pairs as the sixth argument",
                        ))),
                    };
                    attributes.push((key.as_str().to_string(), value.as_str().to_string()));
                }
            }
            _ => {
                return Err(ErrValue(E_TYPE.msg(
                    "present() requires a list of { string, string } pairs or a map of string -> string pairs as the sixth argument",
                )));
            }
        }
    }

    let event = Presentation {
        id: id.to_string(),
        content_type: content_type.to_string(),
        content: content.to_string(),
        target: target.to_string(),
        attributes,
    };

    let event = NarrativeEvent {
        event_id: uuid::Uuid::now_v7(),
        timestamp: SystemTime::now(),
        author: bf_args.exec_state.this(),
        event: Present(event),
    };

    bf_args
        .task_scheduler_client
        .notify(player, Box::new(event));

    Ok(RetNil)
}

fn bf_connected_players(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let include_all = if bf_args.args.len() == 1 {
        let Some(include_all) = bf_args.args[0].as_integer() else {
            return Err(ErrValue(E_TYPE.msg(
                "connected_players() requires an integer as the first argument",
            )));
        };
        include_all == 1
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
        Some(v_obj(*p))
    });
    Ok(Ret(v_list_iter(map)))
}

fn bf_is_player(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(E_ARGS.msg("is_player() requires 1 argument")));
    }
    let Some(player) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("is_player() requires an object as the first argument"),
        ));
    };

    let is_player = match bf_args.world_state.flags_of(&player) {
        Ok(flags) => flags.contains(ObjFlag::User),
        Err(WorldStateError::ObjectNotFound(_)) => {
            return Err(ErrValue(
                E_ARGS.msg("is_player() requires a valid object as the first argument"),
            ));
        }
        Err(e) => return Err(world_state_bf_err(e)),
    };
    Ok(Ret(bf_args.v_bool(is_player)))
}

fn bf_caller_perms(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("caller_perms() does not take any arguments"),
        ));
    }

    Ok(Ret(v_obj(bf_args.caller_perms())))
}

fn bf_set_task_perms(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(E_ARGS.msg("set_task_perms() requires 1 argument")));
    }
    let Some(perms_for) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("set_task_perms() requires an object as the first argument"),
        ));
    };

    // If the caller is not a wizard, perms_for must be the caller
    let perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    if !perms.check_is_wizard().map_err(world_state_bf_err)? && perms_for != perms.who {
        return Err(ErrValue(E_PERM.msg(
            "set_task_perms() requires the caller to be a wizard or the caller itself",
        )));
    }
    bf_args.exec_state.set_task_perms(perms_for);

    Ok(RetNil)
}

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

fn bf_task_id(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("task_id() does not take any arguments"),
        ));
    }

    Ok(Ret(v_int(bf_args.exec_state.task_id as i64)))
}

fn bf_idle_seconds(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(E_ARGS.msg("idle_seconds() requires 1 argument")));
    }
    let Some(who) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("idle_seconds() requires an object as the first argument"),
        ));
    };
    let Ok(idle_seconds) = bf_args.session.idle_seconds(who) else {
        return Err(ErrValue(E_INVARG.msg(
            "idle_seconds() requires a valid object as the first argument",
        )));
    };

    Ok(Ret(v_int(idle_seconds as i64)))
}

fn bf_connected_seconds(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(
            E_ARGS.msg("connected_seconds() requires 1 argument"),
        ));
    }
    let Some(who) = bf_args.args[0].as_object() else {
        return Err(ErrValue(E_TYPE.msg(
            "connected_seconds() requires an object as the first argument",
        )));
    };
    let Ok(connected_seconds) = bf_args.session.connected_seconds(who) else {
        return Err(ErrValue(E_INVARG.msg(
            "connected_seconds() requires a valid object as the first argument",
        )));
    };

    Ok(Ret(v_int(connected_seconds as i64)))
}

/*
Syntax:  connection_name (obj <player>)   => str

Returns a network-specific string identifying the connection being used by the given player.  If the programmer is not a wizard and not
<player>, then `E_PERM' is raised.  If <player> is not currently connected, then `E_INVARG' is raised.

 */
fn bf_connection_name(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(
            E_ARGS.msg("connection_name() requires 1 argument"),
        ));
    }

    let Some(player) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("connection_name() requires an object as the first argument"),
        ));
    };

    let caller = bf_args.caller_perms();
    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_is_wizard()
        .map_err(world_state_bf_err)?
        && caller != player
    {
        return Err(ErrValue(E_PERM.msg(
            "connection_name() requires the caller to be a wizard or the caller itself",
        )));
    }

    let Ok(connection_name) = bf_args.session.connection_name(player) else {
        return Err(ErrValue(E_ARGS.msg(
            "connection_name() requires a valid object as the first argument",
        )));
    };

    Ok(Ret(v_string(connection_name)))
}

fn bf_shutdown(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(E_ARGS.msg("shutdown() requires 0 or 1 arguments")));
    }
    let msg = if bf_args.args.is_empty() {
        None
    } else {
        let Some(msg) = bf_args.args[0].as_string() else {
            return Err(ErrValue(
                E_TYPE.msg("shutdown() requires a string as the first argument"),
            ));
        };
        Some(msg.to_string())
    };

    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    bf_args.task_scheduler_client.shutdown(msg);

    Ok(RetNil)
}

fn bf_time(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(E_ARGS.msg("time() does not take any arguments")));
    }
    Ok(Ret(v_int(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    )))
}

fn bf_ftime(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(E_ARGS.msg("ftime() requires 0 or 1 arguments")));
    }

    // If argument is provided and equals 1, return uptime
    if bf_args.args.len() == 1 {
        let Some(arg) = bf_args.args[0].as_integer() else {
            return Err(ErrValue(
                E_TYPE.msg("ftime() requires an integer as the first argument"),
            ));
        };

        if arg == 1 {
            // Use Instant::now() to get the current monotonic time
            // We need to use a static to track the start time
            use std::sync::OnceLock;
            static START_TIME: OnceLock<Instant> = OnceLock::new();

            // Initialize on first call
            let start = START_TIME.get_or_init(Instant::now);
            let uptime = start.elapsed().as_secs_f64();

            return Ok(Ret(v_float(uptime)));
        } else if arg == 0 {
            // ftime(0) behaves the same as ftime()
            // Fall through to the default case
        } else {
            return Err(ErrValue(
                E_INVARG.msg("ftime() requires 0 or 1 as the first argument"),
            ));
        }
    }

    // Default: return time since Unix epoch as a float
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let seconds = duration.as_secs() as f64;
    let nanos = duration.subsec_nanos() as f64 / 1_000_000_000.0;

    Ok(Ret(v_float(seconds + nanos)))
}

fn bf_ctime(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(E_ARGS.msg("ctime() requires 0 or 1 arguments")));
    }
    let time = if bf_args.args.is_empty() {
        SystemTime::now()
    } else {
        let Some(time) = bf_args.args[0].as_integer() else {
            return Err(ErrValue(
                E_TYPE.msg("ctime() requires an integer as the first argument"),
            ));
        };
        if time < 0 {
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
fn bf_raise(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  raise (<code> [, str <message> [, <value>]])   => none
    //
    // Raises <code> as an error in the same way as other MOO expressions, statements, and functions do.  <Message>, which defaults to the value of `tostr(<code>)',
    // and <value>, which defaults to zero, are made available to any `try'-`except' statements that catch the error.  If the error is not caught, then <message> will
    // appear on the first line of the traceback printed to the user.
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Err(ErrValue(E_ARGS.msg("raise() requires 1 to 3 arguments")));
    }

    let Some(err) = bf_args.args[0].as_error() else {
        return Err(ErrValue(
            E_ARGS.msg("raise() requires an error as the first argument"),
        ));
    };

    let msg = if bf_args.args.len() > 1 {
        let Some(msg) = bf_args.args[1].as_string() else {
            return Err(ErrValue(
                E_TYPE.msg("raise() requires a string as the second argument"),
            ));
        };
        Some(msg.to_string())
    } else {
        None
    };

    let value = if bf_args.args.len() > 2 {
        Some(bf_args.args[2].clone())
    } else {
        None
    };

    Err(BfErr::Raise(Error::new(err.err_type, msg, value)))
}

fn bf_server_version(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("server_version() does not take any arguments"),
        ));
    }
    let version_string = format!("{}+{}", PKG_VERSION, SHORT_COMMIT);
    Ok(Ret(v_string(version_string)))
}

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

fn bf_commit(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(E_ARGS.msg("commit() does not take any arguments")));
    }

    Ok(VmInstr(ExecutionResult::TaskSuspend(TaskSuspend::Commit)))
}

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

fn bf_queued_tasks(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("queued_tasks() does not take any arguments"),
        ));
    }

    // Ask the scheduler (through its mailbox) to describe all the tasks.
    let tasks = bf_args.task_scheduler_client.task_list();

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

/// Function: list queue_info ([obj player])
/// If player is omitted, returns a list of object numbers naming all players that currently have active task
/// queues inside the server. If player is provided, returns the number of background tasks currently queued for that user.
/// It is guaranteed that queue_info(X) will return zero for any X not in the result of queue_info().
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

fn bf_kill_task(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  kill_task(<task-id>)   => none
    //
    // Kills the task with the given <task-id>.
    // The task can be suspended / queued or running.
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

    let result = bf_args.task_scheduler_client.kill_task(
        victim_task_id,
        bf_args.task_perms().map_err(world_state_bf_err)?,
    );
    if let Some(err) = result.as_error() {
        return Err(ErrValue(err.clone()));
    }
    Ok(Ret(result))
}

fn bf_resume(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax: resume(INT task-ID[, ANY value])
    // Resumes a previously suspended task, optionally with a value to pass back to the suspend() call
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

    let result = bf_args.task_scheduler_client.resume_task(
        task_id,
        bf_args.task_perms().map_err(world_state_bf_err)?,
        return_value.clone(),
    );
    if let Some(err) = result.as_error() {
        return Err(ErrValue(err.clone()));
    }
    Ok(Ret(result))
}

fn bf_ticks_left(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  ticks_left()   => int
    //
    // Returns the number of ticks left in the current time slice.
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

fn bf_seconds_left(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  seconds_left()   => int
    //
    // Returns the number of seconds left in the current time slice.
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

fn bf_boot_player(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  boot_player(<player>)   => none
    //
    // Disconnects the player with the given object number.
    if bf_args.args.len() != 1 {
        return Err(ErrValue(E_ARGS.msg("boot_player() requires 1 argument")));
    }

    let Some(player) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("boot_player() requires an object as the first argument"),
        ));
    };

    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    if task_perms.who != player && !task_perms.check_is_wizard().map_err(world_state_bf_err)? {
        return Err(ErrValue(E_PERM.msg(
            "boot_player() requires the caller to be a wizard or the caller itself",
        )));
    }

    bf_args.task_scheduler_client.boot_player(player);

    Ok(RetNil)
}

fn bf_call_function(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  call_function(<func>, <arg1>, <arg2>, ...)   => value
    //
    // Calls the given function with the given arguments and returns the result.
    if bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("call_function() requires at least 1 argument"),
        ));
    }

    let func_name = bf_args.args[0].as_symbol().map_err(ErrValue)?;

    // Arguments are everything left, if any.
    let (_, args) = bf_args
        .args
        .pop_front()
        .map_err(|_| ErrValue(E_ARGS.msg("call_function() requires at least 1 argument")))?;
    let Some(arguments) = args.as_list() else {
        return Err(ErrValue(
            E_TYPE.msg("call_function() requires a list as the second argument"),
        ));
    };

    // Find the function id for the given function name.

    let builtin = BUILTINS.find_builtin(func_name).ok_or_else(|| {
        ErrValue(
            E_INVARG.msg("call_function() requires a valid function name as the first argument"),
        )
    })?;

    // Then ask the scheduler to run the function as a continuation of what we're doing now.
    Ok(VmInstr(ExecutionResult::DispatchBuiltin {
        builtin,
        arguments: arguments.clone(),
    }))
}

/*Syntax:  server_log (str <message> [, <is-error>])   => none

The text in <message> is sent to the server log with a distinctive prefix (so that it can be distinguished from server-generated messages).  If the programmer
is not a wizard, then `E_PERM' is raised.  If <is-error> is provided and true, then <message> is marked in the server log as an error.

*/
fn bf_server_log(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(ErrValue(
            E_ARGS.msg("server_log() requires 1 or 2 arguments"),
        ));
    }

    let Some(message) = bf_args.args[0].as_string() else {
        return Err(ErrValue(
            E_TYPE.msg("server_log() requires a string as the first argument"),
        ));
    };

    let is_error = if bf_args.args.len() == 2 {
        let Some(is_error) = bf_args.args[1].as_integer() else {
            return Err(ErrValue(
                E_TYPE.msg("server_log() requires an integer as the second argument"),
            ));
        };
        is_error == 1
    } else {
        false
    };

    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_is_wizard()
        .map_err(world_state_bf_err)?
    {
        return Err(ErrValue(
            E_PERM.msg("server_log() requires the caller to be a wizard"),
        ));
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

    Ok(RetNil)
}

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
        v_arc_string(bf.name.as_arc_string()),
        min_args,
        max_args,
        v_list_iter(types),
    ])
}

fn bf_function_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(
            E_ARGS.msg("function_info() requires 0 or 1 arguments"),
        ));
    }

    if bf_args.args.len() == 1 {
        let func_name = bf_args.args[0].as_symbol().map_err(ErrValue)?;
        let bf = BUILTINS.find_builtin(func_name).ok_or_else(|| {
            ErrValue(
                E_ARGS.msg("function_info() requires a valid function name as the first argument"),
            )
        })?;
        let Some(desc) = BUILTINS.description_for(bf) else {
            return Err(ErrValue(E_ARGS.msg(
                "function_info() requires a valid function name as the first argument",
            )));
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
        return Err(ErrValue(E_ARGS.msg("listen() requires 2 to 4 arguments")));
    }

    let Some(object) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("listen() requires an object as the first argument"),
        ));
    };

    // point is a protocol specific value, but for now we'll just assume it's an integer for port
    let Some(point) = bf_args.args[1].as_integer() else {
        return Err(ErrValue(
            E_TYPE.msg("listen() requires an integer as the second argument"),
        ));
    };

    if point < 0 || point > (u16::MAX as i64) {
        return Err(ErrValue(E_INVARG.msg(
            "listen() requires a positive integer as the second argument",
        )));
    }

    let port = point as u16;

    let print_messages = if bf_args.args.len() >= 3 {
        let Some(print_messages) = bf_args.args[2].as_integer() else {
            return Err(ErrValue(
                E_TYPE.msg("listen() requires an integer as the third argument"),
            ));
        };
        print_messages == 1
    } else {
        false
    };

    let host_type = if bf_args.args.len() == 4 {
        let Some(host_type) = bf_args.args[3].as_string() else {
            return Err(ErrValue(
                E_TYPE.msg("listen() requires a string as the fourth argument"),
            ));
        };
        host_type.to_string()
    } else {
        "tcp".to_string()
    };

    // Ask the scheduler to broadcast a listen request out to all the hosts.
    if let Some(error) =
        bf_args
            .task_scheduler_client
            .listen(object, host_type, port, print_messages)
    {
        return Err(ErrValue(error));
    }

    // "Listen() returns canon, a `canonicalized' version of point, with any configuration-specific defaulting or aliasing accounted for. "
    // Uh, ok for now we'll just return the port.
    Ok(Ret(v_int(port as i64)))
}

fn bf_listeners(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Requires wizard permissions.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("listeners() does not take any arguments"),
        ));
    }

    let listeners = bf_args.task_scheduler_client.listeners();
    let listeners = listeners.iter().map(|listener| {
        let print_messages = if listener.3 { v_int(1) } else { v_int(0) };
        v_list(&[v_obj(listener.0), v_int(listener.2 as i64), print_messages])
    });

    let listeners = v_list_iter(listeners);

    Ok(Ret(listeners))
}

// unlisten(port, [host-type])
fn bf_unlisten(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Requires wizard permissions.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(ErrValue(E_ARGS.msg("unlisten() requires 1 or 2 arguments")));
    }

    // point is a protocol specific value, but for now we'll just assume it's an integer for port
    let Some(point) = bf_args.args[0].as_integer() else {
        return Err(ErrValue(
            E_TYPE.msg("unlisten() requires an integer as the first argument"),
        ));
    };

    if point < 0 || point > (u16::MAX as i64) {
        return Err(ErrValue(E_INVARG.msg(
            "unlisten() requires a positive integer as the first argument",
        )));
    }

    let port = point as u16;
    let host_type = if bf_args.args.len() == 4 {
        let Some(host_type) = bf_args.args[3].as_string() else {
            return Err(ErrValue(
                E_TYPE.msg("unlisten() requires a string as the second argument"),
            ));
        };
        host_type.to_string()
    } else {
        "tcp".to_string()
    };

    if let Some(err) = bf_args.task_scheduler_client.unlisten(host_type, port) {
        return Err(ErrValue(err));
    }

    Ok(RetNil)
}

pub const BF_SERVER_EVAL_TRAMPOLINE_START_INITIALIZE: usize = 0;
pub const BF_SERVER_EVAL_TRAMPOLINE_RESUME: usize = 1;

fn bf_eval(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_programmer()
        .map_err(world_state_bf_err)?;
    if bf_args.args.len() != 1 {
        return Err(ErrValue(E_ARGS.msg("bf_eval() requires 1 argument")));
    }
    let Some(program_code) = bf_args.args[0].as_string() else {
        return Err(ErrValue(
            E_TYPE.msg("bf_eval() requires a string as the first argument"),
        ));
    };

    // Clone the program code before we borrow bf_args mutably
    let program_code_string = program_code.to_string();

    let tramp = bf_args
        .bf_frame_mut()
        .bf_trampoline
        .take()
        .unwrap_or(BF_SERVER_EVAL_TRAMPOLINE_START_INITIALIZE);

    match tramp {
        BF_SERVER_EVAL_TRAMPOLINE_START_INITIALIZE => {
            let program = match compile(&program_code_string, bf_args.config.compile_options()) {
                Ok(program) => program,
                Err(e) => return Ok(Ret(v_list(&[v_int(0), v_string(e.to_string())]))),
            };
            let bf_frame = bf_args.bf_frame_mut();
            bf_frame.bf_trampoline = Some(BF_SERVER_EVAL_TRAMPOLINE_RESUME);
            // Now we have to construct things to set up for eval. Which means tramping through with a
            // setup-for-eval result here.
            Ok(VmInstr(ExecutionResult::DispatchEval {
                permissions: bf_args.task_perms_who(),
                player: bf_args.exec_state.top().player,
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

fn bf_dump_database(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    bf_args.task_scheduler_client.checkpoint();

    Ok(Ret(bf_args.v_bool(true)))
}

fn bf_memory_usage(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("memory_usage() does not take any arguments"),
        ));
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
        return Err(Code(E_QUOTA));
    }

    // Then read /proc/self/statm
    let mut statm = String::new();
    std::fs::File::open("/proc/self/statm")
        .map_err(|_| Code(E_QUOTA))?
        .read_to_string(&mut statm)
        .map_err(|_| Code(E_QUOTA))?;

    // Split on whitespace -- then we have VmSize and VmRSS in pages
    let mut statm = statm.split_whitespace();
    let vm_size = statm
        .next()
        .ok_or_else(|| Code(E_QUOTA))?
        .parse::<i64>()
        .map_err(|_| Code(E_QUOTA))?;
    let vm_rss = statm
        .next()
        .ok_or_else(|| Code(E_QUOTA))?
        .parse::<i64>()
        .map_err(|_| Code(E_QUOTA))?;

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

fn db_disk_size(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Syntax:  db_disk_size()   => int
    //
    // Returns the number of bytes currently occupied by the database on disk.
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("db_disk_size() does not take any arguments"),
        ));
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

/* Function: none load_server_options ()

   This causes the server to consult the current common of properties on $server_options, updating
   the corresponding server option settings (see section Server Options Set in the Database)
   accordingly. If the programmer is not a wizard, then E_PERM is raised.
*/
fn load_server_options(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("load_server_options() does not take any arguments"),
        ));
    }

    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    bf_args.task_scheduler_client.refresh_server_options();

    Ok(RetNil)
}

fn counter_map(counters: &[&PerfCounter], use_symbols: bool) -> Var {
    let mut result = vec![];
    for c in counters {
        let op_name = if use_symbols {
            v_sym(c.operation)
        } else {
            v_arc_string(c.operation.as_arc_string())
        };

        result.push((
            op_name,
            v_list(&[
                v_int(c.invocations().sum() as i64),
                v_int(c.cumulative_duration_nanos().sum() as i64),
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

fn bf_db_counters(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    Ok(Ret(counter_map(
        &db_counters().all_counters(),
        bf_args.config.use_symbols_in_builtins && bf_args.config.symbol_type,
    )))
}

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

fn bf_force_input(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    /*Syntax:  force_input (obj <conn>, str <line> [, <at-front>])   => none
     */
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(ErrValue(
            E_ARGS.msg("force_input() requires 2 or 3 arguments"),
        ));
    }

    // We always ignore 3rd argument ("at_front"), it makes no sense in mooR.
    let Some(conn) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("force_input() requires an object as the first argument"),
        ));
    };

    // Must be either player or wizard
    let perms = bf_args.task_perms().map_err(world_state_bf_err)?;

    if perms.who != conn && !perms.check_is_wizard().map_err(world_state_bf_err)? {
        return Err(Code(E_PERM));
    }

    let Some(line) = bf_args.args[1].as_string() else {
        return Err(Code(E_TYPE));
    };

    match bf_args
        .task_scheduler_client
        .force_input(conn, line.to_string())
    {
        Ok(task_id) => Ok(Ret(v_int(task_id as i64))),
        Err(e) => Err(ErrValue(e)),
    }
}

/// worker_request(worker_type, args, ... [, timeout])
/// Sends a request to a worker (e.g. outbound HTTP, files, etc.) to perform some action.
/// Task then goes into suspension until the request is completed or times out.
/// Wizard only.
fn bf_worker_request(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(Code(E_ARGS));
    }

    let worker_type = bf_args.args[0].as_symbol().map_err(ErrValue)?;
    let request_params = match bf_args.args[1].variant() {
        Variant::List(l) => l.iter().collect(),
        _ => {
            return Err(ErrValue(E_TYPE.msg(
                "worker_request: second argument must be a list of request parameters",
            )));
        }
    };

    let mut timeout = None;
    if bf_args.args.len() == 3 {
        match bf_args.args[2].variant() {
            Variant::Map(m) => {
                for (k, v) in m.iter() {
                    let key = k.as_symbol().map_err(ErrValue)?;
                    if key.as_string() == "timeout_seconds" {
                        if let Some(secs) = v.as_float() {
                            timeout = Some(Duration::from_secs_f64(secs));
                        }
                    }
                }
            }
            Variant::List(l) => {
                for pair in l.iter() {
                    match pair.variant() {
                        Variant::List(pair_list) if pair_list.len() == 2 => {
                            let key = pair_list[0].as_symbol().map_err(ErrValue)?;
                            if key.as_string() == "timeout_seconds" {
                                if let Some(secs) = pair_list[1].as_float() {
                                    timeout = Some(Duration::from_secs_f64(secs));
                                }
                            }
                        }
                        _ => continue,
                    }
                }
            }
            _ => {
                return Err(ErrValue(E_TYPE.msg(
                    "worker_request: third argument must be a map or alist of worker arguments",
                )));
            }
        }
    }
    Ok(VmInstr(ExecutionResult::TaskSuspend(
        TaskSuspend::WorkerRequest(worker_type, request_params, timeout),
    )))
}

fn bf_connections(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(
            E_ARGS.msg("connections() requires 0 or 1 arguments"),
        ));
    }

    let player = if bf_args.args.is_empty() {
        None
    } else {
        let Some(player) = bf_args.args[0].as_object() else {
            return Err(ErrValue(
                E_TYPE.msg("connections() requires an object as the first argument"),
            ));
        };
        Some(player)
    };

    // Permission check: if requesting for another player, must be wizard or same player
    if let Some(target_player) = player {
        let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
        if target_player != task_perms.who
            && !task_perms.check_is_wizard().map_err(world_state_bf_err)?
        {
            return Err(ErrValue(E_PERM.msg(
                "connections() requires the caller to be a wizard or requesting their own connections",
            )));
        }
    }

    let connection_details = match bf_args.session.connection_details(player) {
        Ok(result) => result,
        Err(SessionError::NoConnectionForPlayer(_)) => {
            return Err(ErrValue(E_INVARG.msg("No connection found for player")));
        }
        Err(_) => {
            return Err(ErrValue(
                E_INVARG.msg("Unable to get connection information"),
            ));
        }
    };

    // Convert connection details to a list of lists with connection info
    let mut connections_list = Vec::new();
    for detail in connection_details {
        // Convert acceptable content types to a list of symbols or strings
        let content_types = if bf_args.config.symbol_type {
            v_list_iter(detail.acceptable_content_types.iter().map(|ct| v_sym(*ct)))
        } else {
            v_list_iter(
                detail
                    .acceptable_content_types
                    .iter()
                    .map(|ct| v_arc_string(ct.as_arc_string())),
            )
        };

        let connection_list = v_list(&[
            v_obj(detail.connection_obj),
            v_str(&detail.peer_addr),
            v_float(detail.idle_seconds),
            content_types,
        ]);
        connections_list.push(connection_list);
    }

    Ok(Ret(v_list_iter(connections_list)))
}

pub(crate) fn register_bf_server(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("notify")] = Box::new(bf_notify);
    builtins[offset_for_builtin("connected_players")] = Box::new(bf_connected_players);
    builtins[offset_for_builtin("is_player")] = Box::new(bf_is_player);
    builtins[offset_for_builtin("caller_perms")] = Box::new(bf_caller_perms);
    builtins[offset_for_builtin("set_task_perms")] = Box::new(bf_set_task_perms);
    builtins[offset_for_builtin("callers")] = Box::new(bf_callers);
    builtins[offset_for_builtin("task_id")] = Box::new(bf_task_id);
    builtins[offset_for_builtin("idle_seconds")] = Box::new(bf_idle_seconds);
    builtins[offset_for_builtin("connected_seconds")] = Box::new(bf_connected_seconds);
    builtins[offset_for_builtin("connection_name")] = Box::new(bf_connection_name);
    builtins[offset_for_builtin("time")] = Box::new(bf_time);
    builtins[offset_for_builtin("ftime")] = Box::new(bf_ftime);
    builtins[offset_for_builtin("ctime")] = Box::new(bf_ctime);
    builtins[offset_for_builtin("raise")] = Box::new(bf_raise);
    builtins[offset_for_builtin("server_version")] = Box::new(bf_server_version);
    builtins[offset_for_builtin("shutdown")] = Box::new(bf_shutdown);
    builtins[offset_for_builtin("suspend")] = Box::new(bf_suspend);
    builtins[offset_for_builtin("queued_tasks")] = Box::new(bf_queued_tasks);
    builtins[offset_for_builtin("active_tasks")] = Box::new(bf_active_tasks);
    builtins[offset_for_builtin("queue_info")] = Box::new(bf_queue_info);
    builtins[offset_for_builtin("kill_task")] = Box::new(bf_kill_task);
    builtins[offset_for_builtin("resume")] = Box::new(bf_resume);
    builtins[offset_for_builtin("ticks_left")] = Box::new(bf_ticks_left);
    builtins[offset_for_builtin("seconds_left")] = Box::new(bf_seconds_left);
    builtins[offset_for_builtin("boot_player")] = Box::new(bf_boot_player);
    builtins[offset_for_builtin("call_function")] = Box::new(bf_call_function);
    builtins[offset_for_builtin("server_log")] = Box::new(bf_server_log);
    builtins[offset_for_builtin("function_info")] = Box::new(bf_function_info);
    builtins[offset_for_builtin("listeners")] = Box::new(bf_listeners);
    builtins[offset_for_builtin("listen")] = Box::new(bf_listen);
    builtins[offset_for_builtin("unlisten")] = Box::new(bf_unlisten);
    builtins[offset_for_builtin("eval")] = Box::new(bf_eval);
    builtins[offset_for_builtin("read")] = Box::new(bf_read);
    builtins[offset_for_builtin("dump_database")] = Box::new(bf_dump_database);
    builtins[offset_for_builtin("memory_usage")] = Box::new(bf_memory_usage);
    builtins[offset_for_builtin("db_disk_size")] = Box::new(db_disk_size);
    builtins[offset_for_builtin("load_server_options")] = Box::new(load_server_options);
    builtins[offset_for_builtin("bf_counters")] = Box::new(bf_bf_counters);
    builtins[offset_for_builtin("db_counters")] = Box::new(bf_db_counters);
    builtins[offset_for_builtin("sched_counters")] = Box::new(bf_sched_counters);
    builtins[offset_for_builtin("force_input")] = Box::new(bf_force_input);
    builtins[offset_for_builtin("wait_task")] = Box::new(bf_wait_task);
    builtins[offset_for_builtin("commit")] = Box::new(bf_commit);
    builtins[offset_for_builtin("rollback")] = Box::new(bf_rollback);
    builtins[offset_for_builtin("present")] = Box::new(bf_present);
    builtins[offset_for_builtin("worker_request")] = Box::new(bf_worker_request);
    builtins[offset_for_builtin("connections")] = Box::new(bf_connections);
}
