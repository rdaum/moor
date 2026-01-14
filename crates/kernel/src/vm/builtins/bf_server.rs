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

//! Built-in functions for server management, networking, tasks, and system operations.

use std::{
    io::Read,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Local, TimeZone};
use chrono_tz::{OffsetName, Tz};
use iana_time_zone::get_timezone;
use tracing::{error, info};

use crate::{
    task_context::{
        current_task_scheduler_client, with_current_transaction, with_current_transaction_mut,
    },
    tasks::{sched_counters, task_scheduler_client::TaskControlMsg},
    vm::{
        builtins::{
            BfCallState, BfErr,
            BfErr::{Code, ErrValue},
            BfRet,
            BfRet::{Ret, RetNil, VmInstr},
            BuiltinFunction, DiagnosticOutput, bf_perf_counters, parse_diagnostic_options,
            world_state_bf_err,
        },
        vm_host::ExecutionResult,
    },
};
use moor_common::{
    build,
    model::{ObjFlag, WorldStateError},
    util::PerfCounter,
};
use moor_compiler::{
    ArgCount, ArgType, BUILTINS, Builtin, compile, compile_error_to_map, format_compile_error,
    offset_for_builtin,
};
use moor_db::{
    db_counters,
    prop_cache::{ANCESTRY_CACHE_STATS, PROP_CACHE_STATS, VERB_CACHE_STATS},
};
use moor_var::{
    Associative, E_ARGS, E_INVARG, E_INVIND, E_PERM, E_QUOTA, E_TYPE, Error, Sequence, Var,
    VarType::TYPE_NONE, v_arc_str, v_float, v_int, v_list, v_list_iter, v_map, v_none, v_obj,
    v_str, v_string, v_sym,
};

/// Placeholder function for unimplemented builtins.
pub(crate) fn bf_noop(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    error!(
        "Builtin function {} is not implemented, called with arguments: ({:?})",
        bf_args.name, bf_args.args
    );
    Err(BfErr::Raise(E_INVIND.with_msg_and_value(
        || format!("Builtin {} is not implemented", bf_args.name),
        v_arc_str(bf_args.name.as_arc_str()),
    )))
}

/// Usage: `bool is_player(obj object)`
/// Returns true if the object has the player flag set. Raises E_INVARG if not valid.
fn bf_is_player(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(E_ARGS.msg("is_player() requires 1 argument")));
    }
    let Some(player) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("is_player() requires an object as the first argument"),
        ));
    };

    let is_player = match with_current_transaction(|world_state| world_state.flags_of(&player)) {
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

/// Usage: `obj caller_perms()`
/// Returns the object whose permissions are being used by the current verb call.
/// Initially this is the programmer of the verb, but can be changed by set_task_perms.
fn bf_caller_perms(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("caller_perms() does not take any arguments"),
        ));
    }

    Ok(Ret(v_obj(bf_args.caller_perms())))
}

/// Usage: `none set_task_perms(obj perms)`
/// Changes the permissions of the current task to those of the given object.
/// Raises E_PERM if caller is not a wizard and perms is not the caller.
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

/// Usage: `none shutdown([str message])`
/// Shuts down the server, optionally with a message. Wizard-only.
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

    current_task_scheduler_client().shutdown(msg);

    Ok(RetNil)
}

/// Usage: `int time()`
/// Returns the current time as seconds since Unix epoch (1970-01-01 00:00:00 UTC).
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

/// Usage: `float ftime([int mode])`
/// Returns the current time as a float with sub-second precision since Unix epoch.
/// If mode is 1/true, returns server uptime in seconds instead.
fn bf_ftime(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(E_ARGS.msg("ftime() requires 0 or 1 arguments")));
    }

    // If argument is provided and equals 1/true, return uptime, otherwise, epoch time in float
    if bf_args.args.len() == 1 && bf_args.args[0].is_true() {
        // Use Instant::now() to get the current monotonic time
        // We need to use a static to track the start time
        use std::sync::OnceLock;
        static START_TIME: OnceLock<Instant> = OnceLock::new();

        // Initialize on first call
        let start = START_TIME.get_or_init(Instant::now);
        let uptime = start.elapsed().as_secs_f64();

        return Ok(Ret(v_float(uptime)));
    }

    // Default: return time since Unix epoch as a float
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let seconds = duration.as_secs() as f64;
    let nanos = duration.subsec_nanos() as f64 / 1_000_000_000.0;

    Ok(Ret(v_float(seconds + nanos)))
}

/// Usage: `str ctime([int time])`
/// Converts a Unix timestamp to a human-readable string like "Mon Jan 1 12:00:00 2024 EST".
/// If no argument, uses current time.
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
/// Usage: `none raise(err code [, str message [, any value]])`
/// Raises an error that can be caught by try-except. Message defaults to the error's
/// default message, value defaults to the error's default value or 0.
fn bf_raise(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
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
        err.msg.as_deref().cloned()
    };

    let value = if bf_args.args.len() > 2 {
        Some(bf_args.args[2].clone())
    } else {
        err.value.as_deref().cloned()
    };

    Err(BfErr::Raise(Error::new(err.err_type, msg, value)))
}

/// Usage: `str server_version()`
/// Returns the version string of the server (e.g., "0.9.0-alpha+abc1234").
fn bf_server_version(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("server_version() does not take any arguments"),
        ));
    }
    let version_string = format!("{}+{}", build::PKG_VERSION, build::short_commit());
    Ok(Ret(v_string(version_string)))
}

/// Usage: `none boot_player(obj player)`
/// Disconnects the player from the server. Caller must be the player or a wizard.
fn bf_boot_player(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
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

    current_task_scheduler_client().boot_player(player);

    Ok(RetNil)
}

/// Usage: `any call_function(str func, ...)`
/// Calls a builtin function by name with the remaining arguments. Raises E_INVARG
/// if the function doesn't exist.
fn bf_call_function(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
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

/// Usage: `none server_log(str message [, int is_error])`
/// Writes a message to the server log. If is_error is true, logs at error level.
/// Wizard-only.
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

/// Usage: `none log_cache_stats()`
/// Logs property, verb, and ancestry cache statistics to the server log. Wizard-only.
/// The format mirrors LambdaMOO-style cache counters and hit rates.
fn bf_log_cache_stats(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("log_cache_stats() does not take any arguments"),
        ));
    }

    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_is_wizard()
        .map_err(world_state_bf_err)?
    {
        return Err(ErrValue(
            E_PERM.msg("Only wizards may call log_cache_stats()"),
        ));
    }

    // Log cache statistics in a format similar to LambdaMOO
    info!(
        "Property cache: {} hits, {} misses, {} flushes, {} entries ({:.1}% hit rate)",
        PROP_CACHE_STATS.hit_count(),
        PROP_CACHE_STATS.miss_count(),
        PROP_CACHE_STATS.flush_count(),
        PROP_CACHE_STATS.num_entries(),
        PROP_CACHE_STATS.hit_rate()
    );

    info!(
        "Verb cache: {} hits, {} misses, {} flushes, {} entries ({:.1}% hit rate)",
        VERB_CACHE_STATS.hit_count(),
        VERB_CACHE_STATS.miss_count(),
        VERB_CACHE_STATS.flush_count(),
        VERB_CACHE_STATS.num_entries(),
        VERB_CACHE_STATS.hit_rate()
    );

    info!(
        "Ancestry cache: {} hits, {} misses, {} flushes, {} entries ({:.1}% hit rate)",
        ANCESTRY_CACHE_STATS.hit_count(),
        ANCESTRY_CACHE_STATS.miss_count(),
        ANCESTRY_CACHE_STATS.flush_count(),
        ANCESTRY_CACHE_STATS.num_entries(),
        ANCESTRY_CACHE_STATS.hit_rate()
    );

    Ok(RetNil)
}

/// Helper function to convert builtin function information to a MOO list.
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
        v_arc_str(bf.name.as_arc_str()),
        min_args,
        max_args,
        v_list_iter(types),
    ])
}

/// Usage: `list function_info([str function_name])`
/// Returns `{name, min_args, max_args, {types...}}` for a builtin, or a list of all
/// builtins if no argument. Args of -1 mean unlimited, types of -1 mean any.
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

pub const BF_SERVER_EVAL_TRAMPOLINE_START_INITIALIZE: usize = 0;
pub const BF_SERVER_EVAL_TRAMPOLINE_RESUME: usize = 1;

/// Compiles and evaluates a MOO expression or statement.
/// Usage: `list eval(str program [, map initial_env [, int verbosity [, int output_mode]]])`
///
/// Arguments:
///   - program: MOO code to compile and execute
///   - initial_env: Optional map of variable bindings to pre-populate in the eval frame.
///     Keys are variable names (strings or symbols), values can be any MOO type.
///     Only variables that are assigned to in the program will be populated.
///   - verbosity: Controls error detail level (default: 0)
///     - 0=summary: Brief error message only
///     - 1=context: Message with error location (graphical display when output_mode > 0)
///     - 2=detailed: Message, location, and diagnostic hints
///     - 3=structured map: Returns error data as map for programmatic handling
///   - output_mode: Controls error formatting style (default: 0)
///     - 0=plain text
///     - 1=graphics: Unicode box-drawing characters
///     - 2=graphics+color: Graphics with ANSI color codes
///
/// When verbosity=3, error result is a map with diagnostic data instead of formatted strings.
/// Use `format_compile_error()` to format the structured map into human-readable text.
fn bf_eval(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_programmer()
        .map_err(world_state_bf_err)?;
    if bf_args.args.is_empty() || bf_args.args.len() > 4 {
        return Err(ErrValue(E_ARGS.msg("bf_eval() requires 1-4 arguments")));
    }
    let Some(program_code) = bf_args.args[0].as_string() else {
        return Err(ErrValue(
            E_TYPE.msg("bf_eval() requires a string as the first argument"),
        ));
    };

    // Parse optional initial environment map (2nd argument)
    let initial_env = if bf_args.args.len() >= 2 {
        let Some(map) = bf_args.args[1].as_map() else {
            return Err(ErrValue(
                E_TYPE.msg("bf_eval() requires a map as the second argument"),
            ));
        };
        let mut env = Vec::with_capacity(map.len());
        for (key, value) in map.iter() {
            let Ok(key_sym) = key.as_symbol() else {
                return Err(ErrValue(
                    E_TYPE.msg("bf_eval() initial_env map keys must be strings"),
                ));
            };
            env.push((key_sym, value));
        }
        Some(env)
    } else {
        None
    };

    // Parse optional verbosity (default 0 for eval) and output_mode (default 0)
    let verbosity = if bf_args.args.len() >= 3 {
        bf_args.args[2].as_integer()
    } else {
        Some(0) // Default to summary for eval
    };

    let output_mode = if bf_args.args.len() >= 4 {
        bf_args.args[3].as_integer()
    } else {
        Some(0)
    };

    let diagnostic_output = parse_diagnostic_options(verbosity, output_mode)?;

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
                Err(e) => {
                    let error_result = match diagnostic_output {
                        DiagnosticOutput::Formatted(options) => {
                            let formatted = format_compile_error(
                                &e,
                                Some(program_code_string.as_str()),
                                options,
                            );
                            let error_vars: Vec<Var> =
                                formatted.into_iter().map(v_string).collect();
                            v_list(error_vars.as_slice())
                        }
                        DiagnosticOutput::Structured => compile_error_to_map(
                            &e,
                            Some(program_code_string.as_str()),
                            bf_args.config.symbol_type,
                        ),
                    };
                    return Ok(Ret(v_list(&[v_int(0), error_result])));
                }
            };
            let bf_frame = bf_args.bf_frame_mut();
            bf_frame.bf_trampoline = Some(BF_SERVER_EVAL_TRAMPOLINE_RESUME);
            // Now we have to construct things to set up for eval. Which means tramping through with a
            // setup-for-eval result here.
            Ok(VmInstr(ExecutionResult::DispatchEval {
                permissions: bf_args.task_perms_who(),
                player: bf_args.exec_state.top().player,
                program,
                initial_env,
            }))
        }
        BF_SERVER_EVAL_TRAMPOLINE_RESUME => {
            // Value must be on in our activation's "return value"
            let value = bf_args.exec_state.top().frame.return_value();
            Ok(Ret(v_list(&[bf_args.v_bool(true), value])))
        }
        _ => {
            panic!("Invalid trampoline value for bf_eval: {tramp}");
        }
    }
}

/// Usage: `bool dump_database([int blocking])`
/// Triggers a database checkpoint. If blocking is true, waits for completion. Wizard-only.
fn bf_dump_database(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.len() > 1 {
        return Err(ErrValue(
            E_ARGS.msg("dump_database() requires 0 or 1 arguments"),
        ));
    }

    let blocking = if bf_args.args.len() == 1 {
        bf_args.args[0].is_true()
    } else {
        false
    };

    match current_task_scheduler_client().checkpoint_with_blocking(blocking) {
        Ok(()) => Ok(Ret(bf_args.v_bool(true))),
        Err(e) => {
            tracing::error!(?e, "dump_database() checkpoint failed");
            Ok(Ret(bf_args.v_bool(false)))
        }
    }
}

/// Usage: `none gc_collect()`
/// Forces garbage collection of anonymous objects. Wizard-only.
/// Collection is scheduled asynchronously and only affects anonymous objects.
fn bf_gc_collect(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("gc_collect() does not take any arguments"),
        ));
    }

    // Must be wizard.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    // Send ForceGC message to scheduler
    current_task_scheduler_client()
        .control_sender()
        .send((bf_args.exec_state.task_id, TaskControlMsg::ForceGC))
        .expect("Could not deliver GC request to scheduler");

    Ok(RetNil)
}

/// Usage: `list memory_usage()`
/// Returns `{block_size, pages_used, pages_free}` for the server process. Wizard-only.
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

/// Usage: `int db_disk_size()`
/// Returns the number of bytes currently occupied by the database on disk. Wizard-only.
fn db_disk_size(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
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

    let disk_size = with_current_transaction(|world_state| world_state.db_usage())
        .map_err(world_state_bf_err)?;

    Ok(Ret(v_int(disk_size as i64)))
}

/// Usage: `none load_server_options()`
/// Reloads server options from $server_options properties. Wizard-only.
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

    current_task_scheduler_client().refresh_server_options();

    Ok(RetNil)
}

/// Helper function to convert performance counters to a MOO map.
fn counter_map(counters: &[&PerfCounter], use_symbols: bool) -> Var {
    let mut result = vec![];
    for c in counters {
        let op_name = if use_symbols {
            v_sym(c.operation)
        } else {
            v_arc_str(c.operation.as_arc_str())
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

/// Usage: `map bf_counters()`
/// Returns performance counters for builtin functions as `{name -> {count, nanos}}`. Wizard-only.
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

/// Usage: `map db_counters()`
/// Returns performance counters for database operations as `{name -> {count, nanos}}`. Wizard-only.
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

/// Usage: `map sched_counters()`
/// Returns performance counters for scheduler operations as `{name -> {count, nanos}}`. Wizard-only.
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

/// Usage: `str rotate_enrollment_token()`
/// Generates a new enrollment token for host enrollment and returns it. Wizard-only.
fn bf_rotate_enrollment_token(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("rotate_enrollment_token() takes no arguments"),
        ));
    }

    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    match current_task_scheduler_client().rotate_enrollment_token() {
        Ok(token) => Ok(Ret(v_str(&token))),
        Err(err) => Err(ErrValue(err)),
    }
}

fn parse_optional_timestamp(arg: &Var, label: &str) -> Result<Option<SystemTime>, BfErr> {
    if arg.type_code() == TYPE_NONE {
        return Ok(None);
    }

    if let Some(int_value) = arg.as_integer() {
        if int_value < 0 {
            return Err(ErrValue(E_INVARG.with_msg(|| {
                format!("{label} must be a non-negative UNIX timestamp")
            })));
        }
        return Ok(Some(UNIX_EPOCH + Duration::from_secs(int_value as u64)));
    }

    if let Some(float_value) = arg.as_float() {
        if !float_value.is_finite() || float_value < 0.0 {
            return Err(ErrValue(E_INVARG.with_msg(|| {
                format!("{label} must be a non-negative finite timestamp")
            })));
        }
        return Ok(Some(UNIX_EPOCH + Duration::from_secs_f64(float_value)));
    }

    Err(ErrValue(E_TYPE.with_msg(|| {
        format!("{label} must be an integer, float, or none")
    })))
}

/// Usage: `list player_event_log_stats(obj player [, num since [, num until]])`
/// Returns `{total_events, earliest_time, latest_time}` for a player's event log.
/// Caller must own the player or be a wizard.
fn bf_player_event_log_stats(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 1 || bf_args.args.len() > 3 {
        return Err(ErrValue(
            E_ARGS.msg("player_event_log_stats() takes 1 to 3 arguments"),
        ));
    }

    let Some(player) = bf_args.args[0].as_object() else {
        return Err(ErrValue(E_TYPE.msg(
            "player_event_log_stats() requires an object as the first argument",
        )));
    };

    // Ensure caller has permission to manage the target player's history.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_obj_owner_perms(&player)
        .map_err(world_state_bf_err)?;

    let since = if bf_args.args.len() >= 2 {
        parse_optional_timestamp(&bf_args.args[1], "since")?
    } else {
        None
    };
    let until = if bf_args.args.len() == 3 {
        parse_optional_timestamp(&bf_args.args[2], "until")?
    } else {
        None
    };

    let stats = current_task_scheduler_client()
        .player_event_log_stats(player, since, until)
        .map_err(ErrValue)?;

    let total_events = if stats.total_events > i64::MAX as u64 {
        i64::MAX
    } else {
        stats.total_events as i64
    };
    let earliest_var = if let Some(time) = stats.earliest {
        let secs = time
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ErrValue(E_INVARG.msg("earliest timestamp predates UNIX epoch")))?;
        v_int(secs.as_secs() as i64)
    } else {
        v_none()
    };
    let latest_var = if let Some(time) = stats.latest {
        let secs = time
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ErrValue(E_INVARG.msg("latest timestamp predates UNIX epoch")))?;
        v_int(secs.as_secs() as i64)
    } else {
        v_none()
    };

    Ok(Ret(v_list(&[
        v_int(total_events),
        earliest_var,
        latest_var,
    ])))
}

/// Usage: `list purge_player_event_log(obj player [, num before [, bool drop_pubkey]])`
/// Deletes events before timestamp (or all if omitted). Returns `{deleted_count, pubkey_deleted}`.
/// If drop_pubkey is true, also removes the player's stored public key.
fn bf_purge_player_event_log(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 1 || bf_args.args.len() > 3 {
        return Err(ErrValue(
            E_ARGS.msg("purge_player_event_log() takes 1 to 3 arguments"),
        ));
    }

    let Some(player) = bf_args.args[0].as_object() else {
        return Err(ErrValue(E_TYPE.msg(
            "purge_player_event_log() requires an object as the first argument",
        )));
    };

    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_obj_owner_perms(&player)
        .map_err(world_state_bf_err)?;

    let before = if bf_args.args.len() >= 2 {
        parse_optional_timestamp(&bf_args.args[1], "before")?
    } else {
        None
    };
    let drop_pubkey = if bf_args.args.len() == 3 {
        bf_args.args[2].is_true()
    } else {
        false
    };

    let result = current_task_scheduler_client()
        .purge_player_event_log(player, before, drop_pubkey)
        .map_err(ErrValue)?;

    let deleted_events = if result.deleted_events > i64::MAX as u64 {
        i64::MAX
    } else {
        result.deleted_events as i64
    };

    Ok(Ret(v_list(&[
        v_int(deleted_events),
        bf_args.v_bool(result.pubkey_deleted),
    ])))
}

/// Helper function to convert cache statistics to a LambdaMOO-compatible list.
fn make_cache_stats_list(cache_stats: &moor_db::CacheStats) -> Var {
    // Return a LambdaMOO-compatible list: [hits, negative_hits, misses, generation, histogram]
    // - hits: cache hits where a value was found
    // - negative_hits: cache hits where we cached "not found"
    // - misses: actual cache misses (not in cache at all)
    // - generation: flush count (closest analog)
    // - histogram: simplified - just [0, total_entries] since we don't track chain depths

    let hits = cache_stats.hit_count() as i64;
    let negative_hits = cache_stats.negative_hit_count() as i64;
    let misses = cache_stats.miss_count() as i64;
    let flushes = cache_stats.flush_count() as i64;
    let num_entries = cache_stats.num_entries() as i64;

    // Create histogram - simplified to just show total cache entries
    let histogram = v_list(&[v_int(0), v_int(num_entries)]);

    v_list(&[
        v_int(hits),
        v_int(negative_hits),
        v_int(misses),
        v_int(flushes), // generation (using flush count)
        histogram,
    ])
}

/// Usage: `list verb_cache_stats()`
/// Returns `{hits, negative_hits, misses, flushes, histogram}` for the verb cache. Wizard-only.
/// `histogram` is a simplified `{0, entries}` list and `flushes` is the closest
/// analog to the LambdaMOO cache generation counter.
fn bf_verb_cache_stats(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("verb_cache_stats() does not take any arguments"),
        ));
    }
    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_is_wizard()
        .map_err(world_state_bf_err)?
    {
        return Err(ErrValue(
            E_PERM.msg("Only wizards may call verb_cache_stats()"),
        ));
    }

    Ok(Ret(make_cache_stats_list(&VERB_CACHE_STATS)))
}

/// Usage: `list property_cache_stats()`
/// Returns `{hits, negative_hits, misses, flushes, histogram}` for the property cache. Wizard-only.
/// `histogram` is a simplified `{0, entries}` list and `flushes` is the closest
/// analog to the LambdaMOO cache generation counter.
fn bf_property_cache_stats(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("property_cache_stats() does not take any arguments"),
        ));
    }
    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_is_wizard()
        .map_err(world_state_bf_err)?
    {
        return Err(ErrValue(
            E_PERM.msg("Only wizards may call property_cache_stats()"),
        ));
    }

    Ok(Ret(make_cache_stats_list(&PROP_CACHE_STATS)))
}

/// Usage: `list ancestry_cache_stats()`
/// Returns `{hits, negative_hits, misses, flushes, histogram}` for the ancestry cache. Wizard-only.
/// `histogram` is a simplified `{0, entries}` list and `flushes` is the closest
/// analog to the LambdaMOO cache generation counter.
fn bf_ancestry_cache_stats(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("ancestry_cache_stats() does not take any arguments"),
        ));
    }
    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_is_wizard()
        .map_err(world_state_bf_err)?
    {
        return Err(ErrValue(
            E_PERM.msg("Only wizards may call ancestry_cache_stats()"),
        ));
    }

    Ok(Ret(make_cache_stats_list(&ANCESTRY_CACHE_STATS)))
}

/// Usage: `none flush_caches()`
/// Clears all internal caches (verb, property, and ancestry resolution). Wizard-only.
fn bf_flush_caches(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("flush_caches() does not take any arguments"),
        ));
    }
    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_is_wizard()
        .map_err(world_state_bf_err)?
    {
        return Err(ErrValue(E_PERM.msg("Only wizards may call flush_caches()")));
    }

    with_current_transaction_mut(|tx| tx.flush_caches());
    Ok(RetNil)
}

/// Usage: `list function_help(str builtin_name)`
/// Returns a list of documentation strings for the specified builtin function.
/// Raises E_INVARG if the builtin doesn't exist or has no documentation.
fn bf_function_help(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(
            E_ARGS.msg("function_help() requires exactly one argument"),
        ));
    }

    let name = bf_args.args[0].as_symbol().map_err(|_| Code(E_TYPE))?;

    let docs = &crate::vm::builtins::docs::BUILTIN_DOCS;

    let Some(lines) = docs.get(name.as_string().as_str()) else {
        return Err(ErrValue(
            E_INVARG.msg(format!("No documentation found for builtin '{name}'")),
        ));
    };

    let doc_list: Vec<Var> = lines.iter().map(|s| v_str(s)).collect();
    Ok(Ret(v_list_iter(doc_list)))
}

pub(crate) fn register_bf_server(builtins: &mut [BuiltinFunction]) {
    builtins[offset_for_builtin("is_player")] = bf_is_player;
    builtins[offset_for_builtin("caller_perms")] = bf_caller_perms;
    builtins[offset_for_builtin("set_task_perms")] = bf_set_task_perms;
    builtins[offset_for_builtin("time")] = bf_time;
    builtins[offset_for_builtin("ftime")] = bf_ftime;
    builtins[offset_for_builtin("ctime")] = bf_ctime;
    builtins[offset_for_builtin("raise")] = bf_raise;
    builtins[offset_for_builtin("server_version")] = bf_server_version;
    builtins[offset_for_builtin("shutdown")] = bf_shutdown;
    builtins[offset_for_builtin("boot_player")] = bf_boot_player;
    builtins[offset_for_builtin("call_function")] = bf_call_function;
    builtins[offset_for_builtin("server_log")] = bf_server_log;
    builtins[offset_for_builtin("function_info")] = bf_function_info;
    builtins[offset_for_builtin("function_help")] = bf_function_help;
    builtins[offset_for_builtin("eval")] = bf_eval;
    builtins[offset_for_builtin("dump_database")] = bf_dump_database;
    builtins[offset_for_builtin("gc_collect")] = bf_gc_collect;
    builtins[offset_for_builtin("memory_usage")] = bf_memory_usage;
    builtins[offset_for_builtin("db_disk_size")] = db_disk_size;
    builtins[offset_for_builtin("load_server_options")] = load_server_options;
    builtins[offset_for_builtin("bf_counters")] = bf_bf_counters;
    builtins[offset_for_builtin("db_counters")] = bf_db_counters;
    builtins[offset_for_builtin("sched_counters")] = bf_sched_counters;
    builtins[offset_for_builtin("log_cache_stats")] = bf_log_cache_stats;
    builtins[offset_for_builtin("verb_cache_stats")] = bf_verb_cache_stats;
    builtins[offset_for_builtin("property_cache_stats")] = bf_property_cache_stats;
    builtins[offset_for_builtin("ancestry_cache_stats")] = bf_ancestry_cache_stats;
    builtins[offset_for_builtin("flush_caches")] = bf_flush_caches;
    builtins[offset_for_builtin("rotate_enrollment_token")] = bf_rotate_enrollment_token;
    builtins[offset_for_builtin("player_event_log_stats")] = bf_player_event_log_stats;
    builtins[offset_for_builtin("purge_player_event_log")] = bf_purge_player_event_log;
}
