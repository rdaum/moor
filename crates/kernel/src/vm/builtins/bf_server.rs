// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

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
    tasks::{sched_counters, task_program_cache::program_cache_global_stats},
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
    model::{
        CapabilityGrant, CapabilityGrants, DispatchFlagsSource, HasUuid, Named, ObjFlag,
        VerbDispatch, VerbLookup, WorldStateError,
    },
    util::{
        MetricEntriesVisitor, MetricEntry, scale_hot_sample_sum_nanos, scale_rare_sample_sum_nanos,
    },
};
use moor_compiler::{
    ArgCount, ArgType, BUILTINS, Builtin, compile, compile_error_to_map, format_compile_error,
    offset_for_builtin,
};
use moor_db::{
    ANCESTRY_CACHE_STATS, DatabaseRelation, PROP_CACHE_STATS, VERB_CACHE_STATS, db_counters,
};
use moor_var::{
    Associative, E_ARGS, E_INVARG, E_INVIND, E_QUOTA, E_TYPE, Error, Obj, Symbol, Var,
    VarType::TYPE_NONE, v_arc_str, v_float, v_int, v_list, v_list_iter, v_map, v_obj, v_str,
    v_string, v_sym,
};
use uuid::Uuid;

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
/// Returns the permissions in use by the verb that called the currently-executing verb.
/// If the currently-executing verb was not called by another verb (i.e., it is the first
/// verb called in a command or server task), then returns #-1.
fn bf_caller_perms(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("caller_perms() does not take any arguments"),
        ));
    }

    Ok(Ret(v_obj(bf_args.caller_perms())))
}

/// Usage: `list task_perms()`
/// Returns `{principal, grant, ...}` for the current task permissions.
fn bf_task_perms(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("task_perms() does not take any arguments"),
        ));
    }

    let permissions = bf_args.task_permissions();
    let mut result = vec![v_obj(permissions.principal())];
    for grant in permissions.grants().iter() {
        if let Some(grant_var) = task_permission_grant_to_var(&permissions, grant)? {
            result.push(grant_var);
        }
    }

    Ok(Ret(v_list(&result)))
}

fn task_permission_grant_to_var(
    permissions: &moor_common::model::TaskPermissions,
    grant: CapabilityGrant,
) -> Result<Option<Var>, BfErr> {
    let grant_var = match grant {
        CapabilityGrant::ObjectRead(obj) => v_list(&[v_str("object_read"), v_obj(obj)]),
        CapabilityGrant::ObjectWrite(obj) => v_list(&[v_str("object_write"), v_obj(obj)]),
        CapabilityGrant::ObjectRename(obj) => v_list(&[v_str("object_rename"), v_obj(obj)]),
        CapabilityGrant::ObjectMove(obj) => v_list(&[v_str("object_move"), v_obj(obj)]),
        CapabilityGrant::ObjectRecycle(obj) => v_list(&[v_str("object_recycle"), v_obj(obj)]),
        CapabilityGrant::ObjectChparent(obj) => v_list(&[v_str("object_chparent"), v_obj(obj)]),
        CapabilityGrant::ObjectList => v_list(&[v_str("object_list")]),
        CapabilityGrant::PropertyRead { obj, prop } => {
            v_list(&[v_str("property_read"), v_obj(obj), v_sym(prop)])
        }
        CapabilityGrant::PropertyWrite { obj, prop } => {
            v_list(&[v_str("property_write"), v_obj(obj), v_sym(prop)])
        }
        CapabilityGrant::PropertyDefine(obj) => v_list(&[v_str("property_define"), v_obj(obj)]),
        CapabilityGrant::PropertyDelete { obj, prop } => {
            v_list(&[v_str("property_delete"), v_obj(obj), v_sym(prop)])
        }
        CapabilityGrant::VerbRead { obj, verb } => {
            let Some(names) = grant_verb_names(permissions, obj, verb)? else {
                return Ok(None);
            };
            v_list(&[v_str("verb_read"), v_obj(obj), v_string(names)])
        }
        CapabilityGrant::VerbWrite { obj, verb } => {
            let Some(names) = grant_verb_names(permissions, obj, verb)? else {
                return Ok(None);
            };
            v_list(&[v_str("verb_write"), v_obj(obj), v_string(names)])
        }
        CapabilityGrant::VerbProgram { obj, verb } => {
            let Some(names) = grant_verb_names(permissions, obj, verb)? else {
                return Ok(None);
            };
            v_list(&[v_str("verb_program"), v_obj(obj), v_string(names)])
        }
        CapabilityGrant::VerbAdd(obj) => v_list(&[v_str("verb_add"), v_obj(obj)]),
        CapabilityGrant::VerbCall { obj, verb } => {
            let Some(names) = grant_verb_names(permissions, obj, verb)? else {
                return Ok(None);
            };
            v_list(&[v_str("verb_call"), v_obj(obj), v_string(names)])
        }
        CapabilityGrant::BuiltinCall(builtin) => v_list(&[v_str("builtin_call"), v_sym(builtin)]),
    };

    Ok(Some(grant_var))
}

fn grant_verb_names(
    permissions: &moor_common::model::TaskPermissions,
    obj: Obj,
    verb: Uuid,
) -> Result<Option<String>, BfErr> {
    match with_current_transaction(|world_state| {
        world_state.get_verb_by_id(permissions, &obj, verb)
    }) {
        Ok(verbdef) => Ok(Some(
            verbdef
                .names()
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>()
                .join(" "),
        )),
        Err(WorldStateError::VerbNotFound(_, _)) => Ok(None),
        Err(err) => Err(world_state_bf_err(err)),
    }
}

fn parse_capability_grants(
    value: &Var,
    mut resolve_verb: impl FnMut(VerbGrantKind, Obj, Symbol) -> Result<Uuid, BfErr>,
) -> Result<CapabilityGrants, BfErr> {
    let Some(grants_list) = value.as_list() else {
        return Err(ErrValue(
            E_TYPE.msg("set_task_perms() capability grants must be a list"),
        ));
    };

    let mut grants = Vec::with_capacity(grants_list.len());
    for grant in grants_list.iter() {
        grants.push(parse_capability_grant(&grant, &mut resolve_verb)?);
    }

    Ok(CapabilityGrants::from_vec(grants))
}

#[derive(Clone, Copy)]
enum VerbGrantKind {
    Definition,
    Call,
}

fn parse_capability_grant(
    value: &Var,
    resolve_verb: &mut impl FnMut(VerbGrantKind, Obj, Symbol) -> Result<Uuid, BfErr>,
) -> Result<CapabilityGrant, BfErr> {
    let Some(spec) = value.as_list() else {
        return Err(ErrValue(
            E_TYPE.msg("set_task_perms() capability grant entries must be lists"),
        ));
    };
    let spec = spec.iter().collect::<Vec<_>>();
    if spec.is_empty() {
        return Err(ErrValue(
            E_INVARG.msg("set_task_perms() capability grant entry cannot be empty"),
        ));
    }

    let grant_name = spec[0].as_symbol().map_err(ErrValue)?;
    match grant_name.as_str() {
        "object_read" => parse_object_grant(&spec, CapabilityGrant::ObjectRead),
        "object_write" => parse_object_grant(&spec, CapabilityGrant::ObjectWrite),
        "object_rename" => parse_object_grant(&spec, CapabilityGrant::ObjectRename),
        "object_move" => parse_object_grant(&spec, CapabilityGrant::ObjectMove),
        "object_recycle" => parse_object_grant(&spec, CapabilityGrant::ObjectRecycle),
        "object_chparent" => parse_object_grant(&spec, CapabilityGrant::ObjectChparent),
        "object_list" => parse_zero_argument_grant(&spec, CapabilityGrant::ObjectList),
        "property_read" => parse_property_grant(&spec, |obj, prop| CapabilityGrant::PropertyRead {
            obj,
            prop,
        }),
        "property_write" => parse_property_grant(&spec, |obj, prop| {
            CapabilityGrant::PropertyWrite { obj, prop }
        }),
        "property_define" => parse_object_grant(&spec, CapabilityGrant::PropertyDefine),
        "property_delete" => parse_property_grant(&spec, |obj, prop| {
            CapabilityGrant::PropertyDelete { obj, prop }
        }),
        "verb_read" => parse_verb_grant(
            &spec,
            resolve_verb,
            VerbGrantKind::Definition,
            |obj, verb| CapabilityGrant::VerbRead { obj, verb },
        ),
        "verb_write" => parse_verb_grant(
            &spec,
            resolve_verb,
            VerbGrantKind::Definition,
            |obj, verb| CapabilityGrant::VerbWrite { obj, verb },
        ),
        "verb_add" => parse_object_grant(&spec, CapabilityGrant::VerbAdd),
        "verb_program" => parse_verb_grant(
            &spec,
            resolve_verb,
            VerbGrantKind::Definition,
            |obj, verb| CapabilityGrant::VerbProgram { obj, verb },
        ),
        "verb_call" => parse_verb_grant(&spec, resolve_verb, VerbGrantKind::Call, |obj, verb| {
            CapabilityGrant::VerbCall { obj, verb }
        }),
        "builtin_call" => parse_builtin_call_grant(&spec),
        _ => Err(ErrValue(
            E_INVARG.msg("set_task_perms() capability grant name is not supported"),
        )),
    }
}

fn parse_builtin_call_grant(spec: &[Var]) -> Result<CapabilityGrant, BfErr> {
    if spec.len() != 2 {
        return Err(ErrValue(E_INVARG.msg(
            "set_task_perms() builtin_call capability grants require 2 elements",
        )));
    }
    let builtin = spec[1].as_symbol().map_err(ErrValue)?;
    if BUILTINS.find_builtin(builtin).is_none() {
        return Err(ErrValue(E_INVARG.msg(
            "set_task_perms() builtin_call capability target must name a builtin",
        )));
    }
    Ok(CapabilityGrant::BuiltinCall(builtin))
}

fn parse_zero_argument_grant(
    spec: &[Var],
    grant: CapabilityGrant,
) -> Result<CapabilityGrant, BfErr> {
    if spec.len() != 1 {
        return Err(ErrValue(E_INVARG.msg(
            "set_task_perms() zero-argument capability grants require 1 element",
        )));
    }
    Ok(grant)
}

fn parse_object_grant(
    spec: &[Var],
    grant: impl FnOnce(Obj) -> CapabilityGrant,
) -> Result<CapabilityGrant, BfErr> {
    if spec.len() != 2 {
        return Err(ErrValue(E_INVARG.msg(
            "set_task_perms() object capability grants require 2 elements",
        )));
    }
    let Some(obj) = spec[1].as_object() else {
        return Err(ErrValue(E_TYPE.msg(
            "set_task_perms() object capability target must be an object",
        )));
    };
    Ok(grant(obj))
}

fn parse_property_grant(
    spec: &[Var],
    grant: impl FnOnce(Obj, Symbol) -> CapabilityGrant,
) -> Result<CapabilityGrant, BfErr> {
    if spec.len() != 3 {
        return Err(ErrValue(E_INVARG.msg(
            "set_task_perms() property capability grants require 3 elements",
        )));
    }
    let Some(obj) = spec[1].as_object() else {
        return Err(ErrValue(E_TYPE.msg(
            "set_task_perms() property capability target must be an object",
        )));
    };
    let prop = spec[2].as_symbol().map_err(ErrValue)?;
    Ok(grant(obj, prop))
}

fn parse_verb_grant(
    spec: &[Var],
    resolve_verb: &mut impl FnMut(VerbGrantKind, Obj, Symbol) -> Result<Uuid, BfErr>,
    kind: VerbGrantKind,
    grant: impl FnOnce(Obj, Uuid) -> CapabilityGrant,
) -> Result<CapabilityGrant, BfErr> {
    if spec.len() != 3 {
        return Err(ErrValue(
            E_INVARG.msg("set_task_perms() verb capability grants require 3 elements"),
        ));
    }
    let Some(obj) = spec[1].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("set_task_perms() verb capability target must be an object"),
        ));
    };
    let verb = spec[2].as_symbol().map_err(ErrValue)?;
    let uuid = resolve_verb(kind, obj, verb)?;
    Ok(grant(obj, uuid))
}

/// Usage: `none set_task_perms(obj perms[, list grants])`
/// Changes the permissions of the current task to those of the given object.
/// Raises E_PERM if caller is not a wizard and perms is not the caller. The optional grants list
/// is wizard-only.
fn bf_set_task_perms(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !(1..=2).contains(&bf_args.args.len()) {
        return Err(ErrValue(
            E_ARGS.msg("set_task_perms() requires 1 or 2 arguments"),
        ));
    }
    let Some(perms_for) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("set_task_perms() requires an object as the first argument"),
        ));
    };

    if bf_args.args.len() == 2 {
        bf_args.require_wizard_msg(
            "set_task_perms() with capability grants requires wizard permissions",
        )?;
        let permissions = bf_args.task_permissions();
        let grants = parse_capability_grants(&bf_args.args[1], |kind, obj, verb| {
            with_current_transaction(|world_state| match kind {
                VerbGrantKind::Definition => world_state
                    .get_verb(&permissions, &obj, verb)
                    .map(|verbdef| verbdef.uuid()),
                VerbGrantKind::Call => world_state
                    .dispatch_verb(
                        &permissions,
                        VerbDispatch::new(
                            VerbLookup {
                                object: &obj,
                                verb_name: verb,
                                argspec: None,
                                flagspec: None,
                            },
                            DispatchFlagsSource::VerbOwner,
                        ),
                    )
                    .and_then(|result| {
                        result
                            .map(|verb_result| verb_result.program_key.verb_uuid)
                            .ok_or_else(|| WorldStateError::VerbNotFound(obj, verb.to_string()))
                    }),
            })
            .map_err(world_state_bf_err)
        })?;
        bf_args.exec_state.set_task_perms_with_grants(
            &mut crate::vm::kernel_host::KernelHost,
            perms_for,
            grants,
        );
    } else {
        bf_args.require_controls_msg(
            &perms_for,
            "set_task_perms() requires the caller to be a wizard or the caller itself",
        )?;
        bf_args
            .exec_state
            .set_task_perms(&mut crate::vm::kernel_host::KernelHost, perms_for);
    }

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

    bf_args.require_wizard_or_builtin_call()?;

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
        err.msg().map(str::to_string)
    };

    let value = if bf_args.args.len() > 2 {
        Some(bf_args.args[2].clone())
    } else {
        err.value().cloned()
    };

    Err(BfErr::Raise(Error::new(err.err_type(), msg, value)))
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

    bf_args.require_controls_or_builtin_call_msg(
        &player,
        "boot_player() requires the caller to be a wizard or the caller itself",
    )?;

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

    bf_args
        .require_wizard_or_builtin_call_msg("server_log() requires the caller to be a wizard")?;

    if is_error {
        error!(
            "SERVER_LOG {}: {}",
            bf_args.exec_state.top().player(),
            message
        );
    } else {
        info!(
            "SERVER_LOG {}: {}",
            bf_args.exec_state.top().player(),
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

    bf_args.require_wizard_or_builtin_call_msg("Only wizards may call log_cache_stats()")?;

    let log_moor_cache = |name: &str, stats: &moor_db::CacheStats| {
        let hits = stats.hit_count();
        let negative_hits = stats.negative_hit_count();
        let misses = stats.miss_count();
        let total = hits + negative_hits + misses;
        let hit_rate = if total == 0 {
            0.0
        } else {
            ((hits + negative_hits) as f64 * 100.0) / total as f64
        };
        info!(
            "{name} cache: {hits} hits, {negative_hits} negative hits, {misses} misses, {} flushes, {} entries ({hit_rate:.1}% lookup hit rate)",
            stats.flush_count(),
            stats.num_entries(),
        );
    };

    log_moor_cache("Property", &PROP_CACHE_STATS);
    log_moor_cache("Verb", &VERB_CACHE_STATS);
    log_moor_cache("Ancestry", &ANCESTRY_CACHE_STATS);

    let global_program = program_cache_global_stats();
    let global_total = global_program.hits + global_program.misses;
    let global_hit_rate = if global_total == 0 {
        0.0
    } else {
        (global_program.hits as f64 * 100.0) / global_total as f64
    };
    info!(
        "Program cache (global): {} hits, {} misses, {} inserts, {} reclaimed, {} live slots ({global_hit_rate:.1}% hit rate)",
        global_program.hits,
        global_program.misses,
        global_program.inserts,
        global_program.reclaimed,
        global_program.live_slots,
    );

    let task_program = bf_args.exec_state.program_cache_stats;
    let task_total = task_program.hits + task_program.misses;
    let task_hit_rate = if task_total == 0 {
        0.0
    } else {
        (task_program.hits as f64 * 100.0) / task_total as f64
    };
    info!(
        "Program cache (task): {} hits, {} misses, {} inserts, {} reclaimed, {} total slots, {} live slots, {} keys ({task_hit_rate:.1}% hit rate)",
        task_program.hits,
        task_program.misses,
        task_program.inserts,
        task_program.reclaimed,
        bf_args.exec_state.program_cache_total_slots,
        bf_args.exec_state.program_cache_live_slots,
        bf_args.exec_state.program_cache_key_count,
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
    bf_args.require_programmer()?;
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
                authority_principal: bf_args.task_authority_principal(),
                player: bf_args.exec_state.top().player(),
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
    bf_args.require_wizard_or_builtin_call()?;

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

    if blocking {
        return Ok(VmInstr(ExecutionResult::TaskSuspend(
            crate::vm::TaskSuspend::Checkpoint,
        )));
    }

    match current_task_scheduler_client().checkpoint() {
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
    bf_args.require_wizard_or_builtin_call()?;

    // Send ForceGC message to scheduler
    current_task_scheduler_client().force_gc();

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
    bf_args.require_wizard_or_builtin_call()?;

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
    bf_args.require_wizard_or_builtin_call()?;

    let disk_size = with_current_transaction(|world_state| world_state.db_usage())
        .map_err(world_state_bf_err)?;

    Ok(Ret(v_int(disk_size as i64)))
}

/// Usage: `list db_compact([list relations])`
/// Major-compacts selected database relations on a maintenance thread. Wizard-only.
fn bf_db_compact(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args.require_wizard_or_builtin_call()?;

    let argument = (!bf_args.args.is_empty()).then(|| &bf_args.args[0]);
    let relations = parse_db_compact_relations(argument).map_err(ErrValue)?;

    Ok(VmInstr(ExecutionResult::TaskSuspend(
        crate::vm::TaskSuspend::StorageCompaction(
            relations
                .into_iter()
                .map(|relation| Symbol::mk(relation.as_str()))
                .collect(),
        ),
    )))
}

fn parse_db_compact_relations(argument: Option<&Var>) -> Result<Vec<DatabaseRelation>, Error> {
    if let Some(argument) = argument {
        let Some(names) = argument.as_list() else {
            return Err(E_TYPE.msg("db_compact() requires a list of relation names"));
        };
        if names.is_empty() {
            return Err(E_INVARG.msg("db_compact() requires at least one relation name"));
        }

        let mut relations = Vec::with_capacity(names.len());
        for value in names.iter() {
            let name = value.as_symbol()?;
            let Some(relation) = DatabaseRelation::named(name.as_str()) else {
                return Err(E_INVARG
                    .with_msg(|| format!("db_compact() does not recognize relation {name}")));
            };
            if relations.contains(&relation) {
                return Err(E_INVARG.with_msg(|| {
                    format!("db_compact() relation {name} was selected more than once")
                }));
            }
            relations.push(relation);
        }
        return Ok(relations);
    }

    Ok(DatabaseRelation::ALL.to_vec())
}

/// Usage: `none load_server_options()`
/// Reloads server options from $server_options properties. Wizard-only.
fn load_server_options(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("load_server_options() does not take any arguments"),
        ));
    }

    bf_args.require_wizard_or_builtin_call()?;

    current_task_scheduler_client().refresh_server_options();

    Ok(RetNil)
}

/// Helper function to convert performance timers and counters to a MOO map.
fn counter_map_from_entries(entries: &[MetricEntry], use_symbols: bool) -> Var {
    let mut result = vec![];
    for (name_sym, count, cumulative_ns) in entries {
        let op_name = if use_symbols {
            v_sym(*name_sym)
        } else {
            v_arc_str(name_sym.as_arc_str())
        };

        result.push((
            op_name,
            v_list(&[v_int(*count as i64), v_int(*cumulative_ns as i64)]),
        ));
    }

    v_map(&result)
}

fn db_counter_entries() -> Vec<MetricEntry> {
    let mut visitor = MetricEntriesVisitor::new(|metric_name, sum| {
        if metric_name == "db_timers_rare_samples" {
            return scale_rare_sample_sum_nanos(sum);
        }

        scale_hot_sample_sum_nanos(sum)
    });
    db_counters().visit_metrics(&mut visitor);
    visitor.into_entries()
}

fn sched_counter_entries() -> Vec<MetricEntry> {
    let mut visitor = MetricEntriesVisitor::new(|_, sum| scale_hot_sample_sum_nanos(sum));
    sched_counters().visit_metrics(&mut visitor);
    visitor.into_entries()
}

fn bf_counter_entries() -> Vec<MetricEntry> {
    let mut visitor = MetricEntriesVisitor::new(|_, sum| scale_hot_sample_sum_nanos(sum));
    bf_perf_counters().visit_metrics(&mut visitor);
    visitor.into_entries()
}

/// Usage: `map bf_counters()`
/// Returns performance counters for builtin functions as `{name -> {count, nanos}}`. Wizard-only.
fn bf_bf_counters(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args.require_wizard_or_builtin_call()?;

    let _counters = bf_perf_counters();
    Ok(Ret(counter_map_from_entries(
        &bf_counter_entries(),
        bf_args.config.use_symbols_in_builtins && bf_args.config.symbol_type,
    )))
}

/// Usage: `map db_counters()`
/// Returns performance counters for database operations as `{name -> {count, nanos}}`. Wizard-only.
fn bf_db_counters(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args.require_wizard_or_builtin_call()?;

    let mut entries = db_counter_entries();
    entries.extend(with_current_transaction(|world_state| {
        world_state
            .persistence_metrics()
            .into_iter()
            .map(|(name, value)| (Symbol::mk(name), value.min(isize::MAX as u64) as isize, 0))
            .collect::<Vec<_>>()
    }));
    Ok(Ret(counter_map_from_entries(
        &entries,
        bf_args.config.use_symbols_in_builtins && bf_args.config.symbol_type,
    )))
}

/// Usage: `map sched_counters()`
/// Returns performance counters for scheduler operations as `{name -> {count, nanos}}`. Wizard-only.
fn bf_sched_counters(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args.require_wizard_or_builtin_call()?;

    let _counters = sched_counters();
    Ok(Ret(counter_map_from_entries(
        &sched_counter_entries(),
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

    bf_args.require_wizard_or_builtin_call()?;

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
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
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
    bf_args.require_controls_or_builtin_call(&player)?;

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
        v_int(0)
    };
    let latest_var = if let Some(time) = stats.latest {
        let secs = time
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ErrValue(E_INVARG.msg("latest timestamp predates UNIX epoch")))?;
        v_int(secs.as_secs() as i64)
    } else {
        v_int(0)
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
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Err(ErrValue(
            E_ARGS.msg("purge_player_event_log() takes 1 to 3 arguments"),
        ));
    }

    let Some(player) = bf_args.args[0].as_object() else {
        return Err(ErrValue(E_TYPE.msg(
            "purge_player_event_log() requires an object as the first argument",
        )));
    };

    bf_args.require_controls_or_builtin_call(&player)?;

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
    bf_args.require_wizard_or_builtin_call_msg("Only wizards may call verb_cache_stats()")?;

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
    bf_args.require_wizard_or_builtin_call_msg("Only wizards may call property_cache_stats()")?;

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
    bf_args.require_wizard_or_builtin_call_msg("Only wizards may call ancestry_cache_stats()")?;

    Ok(Ret(make_cache_stats_list(&ANCESTRY_CACHE_STATS)))
}

/// Usage: `map program_cache_stats()`
/// Returns task-local and global program-cache stats. Wizard-only.
///
/// Shape:
/// - `global`: `{hits, misses, inserts, reclaimed, live_slots}`
/// - `task`: `{hits, misses, inserts, reclaimed, total_slots, live_slots, key_count}`
fn bf_program_cache_stats(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("program_cache_stats() does not take any arguments"),
        ));
    }
    bf_args.require_wizard_or_builtin_call_msg("Only wizards may call program_cache_stats()")?;

    let global = program_cache_global_stats();
    let task = bf_args.exec_state.program_cache_stats;

    Ok(Ret(v_map(&[
        (
            v_str("global"),
            v_map(&[
                (v_str("hits"), v_int(global.hits)),
                (v_str("misses"), v_int(global.misses)),
                (v_str("inserts"), v_int(global.inserts)),
                (v_str("reclaimed"), v_int(global.reclaimed)),
                (v_str("live_slots"), v_int(global.live_slots)),
            ]),
        ),
        (
            v_str("task"),
            v_map(&[
                (v_str("hits"), v_int(task.hits)),
                (v_str("misses"), v_int(task.misses)),
                (v_str("inserts"), v_int(task.inserts)),
                (v_str("reclaimed"), v_int(task.reclaimed)),
                (
                    v_str("total_slots"),
                    v_int(bf_args.exec_state.program_cache_total_slots as i64),
                ),
                (
                    v_str("live_slots"),
                    v_int(bf_args.exec_state.program_cache_live_slots as i64),
                ),
                (
                    v_str("key_count"),
                    v_int(bf_args.exec_state.program_cache_key_count as i64),
                ),
            ]),
        ),
    ])))
}

/// Usage: `none flush_caches()`
/// Clears all internal caches (verb, property, and ancestry resolution). Wizard-only.
fn bf_flush_caches(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("flush_caches() does not take any arguments"),
        ));
    }
    bf_args.require_wizard_or_builtin_call_msg("Only wizards may call flush_caches()")?;

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
    builtins[offset_for_builtin("task_perms")] = bf_task_perms;
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
    builtins[offset_for_builtin("db_compact")] = bf_db_compact;
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
    builtins[offset_for_builtin("program_cache_stats")] = bf_program_cache_stats;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_capability_grants() {
        let obj = Obj::mk_id(10);
        let verb_read = Uuid::from_u128(1);
        let verb_write = Uuid::from_u128(2);
        let verb_program = Uuid::from_u128(3);
        let verb_call = Uuid::from_u128(4);
        let mut verb_ids = [verb_read, verb_write, verb_program, verb_call].into_iter();
        let grants = parse_capability_grants(
            &v_list(&[
                v_list(&[v_str("object_read"), v_obj(obj)]),
                v_list(&[v_str("object_write"), v_obj(obj)]),
                v_list(&[v_str("object_rename"), v_obj(obj)]),
                v_list(&[v_str("object_move"), v_obj(obj)]),
                v_list(&[v_str("object_recycle"), v_obj(obj)]),
                v_list(&[v_str("object_chparent"), v_obj(obj)]),
                v_list(&[v_str("object_list")]),
                v_list(&[v_str("property_define"), v_obj(obj)]),
                v_list(&[v_str("property_write"), v_obj(obj), v_str("p")]),
                v_list(&[v_str("property_delete"), v_obj(obj), v_str("p")]),
                v_list(&[v_str("verb_add"), v_obj(obj)]),
                v_list(&[v_sym(Symbol::mk("verb_read")), v_obj(obj), v_str("v")]),
                v_list(&[v_str("verb_write"), v_obj(obj), v_str("v")]),
                v_list(&[v_str("verb_program"), v_obj(obj), v_str("v")]),
                v_list(&[v_str("verb_call"), v_obj(obj), v_str("v")]),
                v_list(&[v_str("builtin_call"), v_str("server_log")]),
            ]),
            |_, _, _| Ok(verb_ids.next().unwrap()),
        )
        .unwrap();

        let parsed = grants.iter().collect::<Vec<_>>();
        assert_eq!(
            parsed,
            vec![
                CapabilityGrant::ObjectRead(obj),
                CapabilityGrant::ObjectWrite(obj),
                CapabilityGrant::ObjectRename(obj),
                CapabilityGrant::ObjectMove(obj),
                CapabilityGrant::ObjectRecycle(obj),
                CapabilityGrant::ObjectChparent(obj),
                CapabilityGrant::ObjectList,
                CapabilityGrant::PropertyDefine(obj),
                CapabilityGrant::PropertyWrite {
                    obj,
                    prop: Symbol::mk("p"),
                },
                CapabilityGrant::PropertyDelete {
                    obj,
                    prop: Symbol::mk("p"),
                },
                CapabilityGrant::VerbAdd(obj),
                CapabilityGrant::VerbRead {
                    obj,
                    verb: verb_read,
                },
                CapabilityGrant::VerbWrite {
                    obj,
                    verb: verb_write,
                },
                CapabilityGrant::VerbProgram {
                    obj,
                    verb: verb_program,
                },
                CapabilityGrant::VerbCall {
                    obj,
                    verb: verb_call,
                },
                CapabilityGrant::BuiltinCall(Symbol::mk("server_log")),
            ]
        );
    }

    #[test]
    fn rejects_unsupported_capability_grants() {
        let obj = Obj::mk_id(10);
        assert!(
            parse_capability_grants(
                &v_list(&[v_list(&[v_str("object_takeover"), v_obj(obj)])]),
                |_, _, _| panic!("unsupported grant should not resolve verbs")
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_unknown_builtin_call_grants() {
        assert!(
            parse_capability_grants(
                &v_list(&[v_list(&[v_str("builtin_call"), v_str("not_a_builtin")])]),
                |_, _, _| panic!("builtin grants should not resolve verbs")
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_arguments_for_zero_argument_capability_grants() {
        let obj = Obj::mk_id(10);
        assert!(
            parse_capability_grants(
                &v_list(&[v_list(&[v_str("object_list"), v_obj(obj)])]),
                |_, _, _| panic!("zero-argument grants should not resolve verbs")
            )
            .is_err()
        );
    }

    #[test]
    fn db_compact_defaults_to_all_relations() {
        assert_eq!(
            parse_db_compact_relations(None).unwrap(),
            DatabaseRelation::ALL
        );
    }

    #[test]
    fn db_compact_preserves_selected_relation_order() {
        let argument = v_list(&[v_str("object_propvalues"), v_str("object_propflags")]);

        assert_eq!(
            parse_db_compact_relations(Some(&argument)).unwrap(),
            vec![
                DatabaseRelation::ObjectPropvalues,
                DatabaseRelation::ObjectPropflags,
            ]
        );
    }

    #[test]
    fn db_compact_rejects_unknown_and_duplicate_relations() {
        assert!(parse_db_compact_relations(Some(&v_list(&[v_str("not_a_relation")]))).is_err());
        assert!(
            parse_db_compact_relations(Some(&v_list(&[
                v_str("object_name"),
                v_str("object_name"),
            ])))
            .is_err()
        );
    }
}
