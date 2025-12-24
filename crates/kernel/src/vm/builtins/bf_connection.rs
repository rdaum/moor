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

use crate::task_context::{
    current_session, current_task_scheduler_client, with_current_transaction,
};
use crate::vm::TaskSuspend;
use crate::vm::builtins::BfErr::{Code, ErrValue};
use crate::vm::builtins::BfRet::{Ret, RetNil, VmInstr};
use crate::vm::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction, world_state_bf_err};
use crate::vm::vm_host::ExecutionResult;
use moor_common::builtins::offset_for_builtin;
use moor_common::tasks::Event::{Present, Unpresent};
use moor_common::tasks::{NarrativeEvent, Presentation, SessionError};
use moor_var::VarType::TYPE_STR;
use moor_var::{
    E_ARGS, E_INVARG, E_PERM, E_TYPE, Sequence, Symbol, Var, Variant, v_arc_str, v_float, v_int,
    v_list, v_list_iter, v_map, v_obj, v_str, v_string, v_sym,
};
use std::time::{Duration, SystemTime};
use tracing::{info, warn};

/// Usage: `int force_input(obj conn, str line [, int at_front])`
/// Queues input as if it came from the player's connection. Returns the task ID
/// of the spawned command task. Caller must be player or wizard.
fn bf_force_input(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
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

    match current_task_scheduler_client().force_input(conn, line.to_string()) {
        Ok(task_id) => Ok(Ret(v_int(task_id as i64))),
        Err(e) => Err(ErrValue(e)),
    }
}

/// Usage: `any worker_request(symbol worker_type, list args [, map options])`
/// Sends a request to an external worker (e.g., HTTP). Suspends until complete.
/// Options may include `timeout_seconds`. Wizard-only.
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
                    if key.as_string() == "timeout_seconds"
                        && let Some(secs) = v.as_float()
                    {
                        timeout = Some(Duration::from_secs_f64(secs));
                    }
                }
            }
            Variant::List(l) => {
                for pair in l.iter() {
                    match pair.variant() {
                        Variant::List(pair_list) if pair_list.len() == 2 => {
                            let key = pair_list[0].as_symbol().map_err(ErrValue)?;
                            if key.as_string() == "timeout_seconds"
                                && let Some(secs) = pair_list[1].as_float()
                            {
                                timeout = Some(Duration::from_secs_f64(secs));
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

/// Usage: `list connections([obj player])`
/// Returns connection info for player (or all if omitted). Each entry is
/// `{conn_obj, peer_addr, idle_secs, content_types, options}`.
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

    let connection_details = match current_session().connection_details(player) {
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
                    .map(|ct| v_arc_str(ct.as_arc_str())),
            )
        };

        // Get connection options for this connection
        let connection_options =
            match current_session().connection_attributes(detail.connection_obj) {
                Ok(attributes) => {
                    // Convert attributes map to list of {name, value} pairs
                    match attributes.variant() {
                        moor_var::Variant::Map(m) => {
                            let pairs: Vec<Var> = m
                                .iter()
                                .map(|(k, v)| v_list(&[k.clone(), v.clone()]))
                                .collect();
                            v_list(&pairs)
                        }
                        _ => v_list(&[]), // Empty list if not a map
                    }
                }
                Err(_) => v_list(&[]), // Empty list on error
            };

        let connection_list = v_list(&[
            v_obj(detail.connection_obj),
            v_str(&detail.peer_addr),
            v_float(detail.idle_seconds),
            content_types,
            connection_options,
        ]);
        connections_list.push(connection_list);
    }

    Ok(Ret(v_list_iter(connections_list)))
}

/// Usage: `obj connection()`
/// Returns the connection object (negative ID) for the current task's session.
fn bf_connection(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("connection() does not take any arguments"),
        ));
    }

    // Get connection details for the current session (None means "this session")
    let connection_details = match current_session().connection_details(None) {
        Ok(result) => result,
        Err(SessionError::NoConnectionForPlayer(_)) => {
            return Err(ErrValue(E_INVARG.msg("No connection for current task")));
        }
        Err(_) => {
            return Err(ErrValue(
                E_INVARG.msg("Unable to get connection information"),
            ));
        }
    };

    // There should be exactly one connection for the current session
    let connection_obj = connection_details
        .first()
        .ok_or_else(|| ErrValue(E_INVARG.msg("No connection found for current task")))?
        .connection_obj;

    Ok(Ret(v_obj(connection_obj)))
}

/// Usage: `none switch_player(obj new_player)`
/// Switches the current session to a different player object. The target must be
/// a valid player object. Wizard-only.
fn bf_switch_player(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(E_ARGS.msg("switch_player() requires 1 argument")));
    }

    let Some(new_player) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("switch_player() requires an object as the first argument"),
        ));
    };

    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;

    // Only wizards can switch players
    if !task_perms.check_is_wizard().map_err(world_state_bf_err)? {
        return Err(ErrValue(
            E_PERM.msg("switch_player() requires wizard permissions"),
        ));
    }

    // Check if we're already the target player - if so, no-op
    if task_perms.who == new_player {
        return Ok(RetNil);
    }

    // Validate that the target player object exists and is accessible
    if !with_current_transaction(|ws| ws.valid(&new_player)).map_err(world_state_bf_err)? {
        return Err(ErrValue(
            E_INVARG.msg("switch_player() target player object does not exist"),
        ));
    }

    // Check that the new player is a valid player object
    let player_flags =
        with_current_transaction(|ws| ws.flags_of(&new_player)).map_err(world_state_bf_err)?;
    if !player_flags.contains(moor_common::model::ObjFlag::User) {
        return Err(ErrValue(
            E_INVARG.msg("switch_player() requires a player object"),
        ));
    }

    // Log the switch attempt for audit trail
    info!(
        wizard = ?task_perms.who,
        from_player = ?task_perms.who,
        to_player = ?new_player,
        task_id = ?bf_args.exec_state.task_id,
        "Player switch requested"
    );

    // Request the switch through the task scheduler
    match current_task_scheduler_client().switch_player(new_player) {
        Ok(_) => {
            info!(
                wizard = ?task_perms.who,
                from_player = ?task_perms.who,
                to_player = ?new_player,
                "Player switch completed successfully"
            );
            Ok(RetNil)
        }
        Err(e) => {
            warn!(
                wizard = ?task_perms.who,
                from_player = ?task_perms.who,
                to_player = ?new_player,
                error = ?e,
                "Player switch failed"
            );
            Err(ErrValue(e))
        }
    }
}

/// Usage: `list workers()`
/// Returns info about external workers. Each entry is `{type, count, queue_size,
/// avg_response_ms, last_ping_secs}`. Wizard-only.
fn bf_workers(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(ErrValue(
            E_ARGS.msg("workers() does not take any arguments"),
        ));
    }

    // Must be wizard
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    let workers_info = current_task_scheduler_client().workers_info();

    // Convert worker information to MOO list format
    // Each entry: [worker_type, worker_count, total_queue_size, avg_response_time, last_ping_ago]
    let result = workers_info.iter().map(|worker_info| {
        let worker_type = if bf_args.config.symbol_type {
            v_sym(worker_info.worker_type)
        } else {
            v_arc_str(worker_info.worker_type.as_arc_str())
        };

        v_list(&[
            worker_type,
            v_int(worker_info.worker_count as i64),
            v_int(worker_info.total_queue_size as i64),
            v_float(worker_info.avg_response_time_ms),
            v_float(worker_info.last_ping_ago_secs),
        ])
    });

    Ok(Ret(v_list_iter(result)))
}

/// Usage: `list output_delimiters(obj player)`
/// Returns `{prefix, suffix}` strings used to delimit command output for the player.
fn bf_output_delimiters(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(
            E_ARGS.msg("output_delimiters() requires exactly 1 argument"),
        ));
    }

    let Some(player) = bf_args.args[0].as_object() else {
        return Err(ErrValue(E_TYPE.msg("Player must be an object")));
    };

    // Permission check: can only query own delimiters unless wizard
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    if player != task_perms.who && !task_perms.check_is_wizard().map_err(world_state_bf_err)? {
        return Err(ErrValue(E_PERM.msg("Permission denied")));
    }

    // Get the attributes from the connection registry
    let attributes_var = match current_session().connection_attributes(player) {
        Ok(attributes) => attributes,
        Err(SessionError::NoConnectionForPlayer(_)) => {
            // No active connection, return empty delimiters
            return Ok(Ret(v_list(&[v_str(""), v_str("")])));
        }
        Err(_) => return Err(ErrValue(E_INVARG.msg("Unable to get output delimiters"))),
    };

    // Extract prefix and suffix from the attributes
    // For players, we get a list of [connection_obj, attributes] pairs - use first connection
    // For connection objects, we get the attributes map directly
    use moor_var::IndexMode;

    // Extract prefix and suffix from the attributes
    let (prefix, suffix) = if let Some(connections_list) = attributes_var.as_list() {
        // Player object - extract from first connection's attributes
        if let Some(first_connection) = connections_list.iter().next() {
            if let Some(connection_pair) = first_connection.as_list() {
                if connection_pair.len() >= 2 {
                    let attrs = &connection_pair[1];
                    let prefix = attrs
                        .get(
                            &v_sym(Symbol::mk("line-output-prefix")),
                            IndexMode::ZeroBased,
                        )
                        .ok()
                        .and_then(|v| v.as_string().map(|s| s.to_string()))
                        .unwrap_or_default();
                    let suffix = attrs
                        .get(
                            &v_sym(Symbol::mk("line-output-suffix")),
                            IndexMode::ZeroBased,
                        )
                        .ok()
                        .and_then(|v| v.as_string().map(|s| s.to_string()))
                        .unwrap_or_default();
                    (prefix, suffix)
                } else {
                    (String::new(), String::new())
                }
            } else {
                (String::new(), String::new())
            }
        } else {
            (String::new(), String::new())
        }
    } else {
        // Connection object - extract directly from attributes map
        let prefix = attributes_var
            .get(
                &v_sym(Symbol::mk("line-output-prefix")),
                IndexMode::ZeroBased,
            )
            .ok()
            .and_then(|v| v.as_string().map(|s| s.to_string()))
            .unwrap_or_default();
        let suffix = attributes_var
            .get(
                &v_sym(Symbol::mk("line-output-suffix")),
                IndexMode::ZeroBased,
            )
            .ok()
            .and_then(|v| v.as_string().map(|s| s.to_string()))
            .unwrap_or_default();
        (prefix, suffix)
    };

    Ok(Ret(v_list(&[v_str(&prefix), v_str(&suffix)])))
}

/// Helper function to check if the current user owns the given connection object.
/// Returns true if the user is a wizard or owns the connection.
fn check_connection_ownership(
    connection_obj: moor_var::Obj,
    bf_args: &mut BfCallState<'_>,
) -> Result<bool, BfErr> {
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;

    // Wizards can access any connection
    if task_perms.check_is_wizard().map_err(world_state_bf_err)? {
        return Ok(true);
    }

    // Get all connections for this user
    let user_connections = match current_session().connection_details(Some(task_perms.who)) {
        Ok(connections) => connections,
        Err(_) => return Ok(false), // No connections for this user
    };

    // Check if the connection_obj is among the user's connections
    Ok(user_connections
        .iter()
        .any(|detail| detail.connection_obj == connection_obj))
}

/// Usage: `list connection_options(obj conn)`
/// Returns all options for connection object as `{name, value}` pairs.
/// Argument must be a connection object (negative ID), not a player.
fn bf_connection_options(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(
            E_ARGS.msg("connection_options() requires exactly 1 argument"),
        ));
    }

    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(ErrValue(E_TYPE.msg("Argument must be an object")));
    };

    // connection_options() requires a connection object (negative ID), not a player object
    if obj.is_positive() {
        return Err(ErrValue(E_INVARG.msg(
            "connection_options() requires a connection object, not a player object",
        )));
    }

    // Permission check: can only query connection options if user owns the connection or is wizard
    if !check_connection_ownership(obj, bf_args)? {
        return Err(ErrValue(E_PERM.msg("Permission denied")));
    }

    // Get the attributes from the connection registry
    let attributes = match current_session().connection_attributes(obj) {
        Ok(attributes) => attributes,
        Err(SessionError::NoConnectionForPlayer(_)) => {
            // No active connection, return empty list
            return Ok(Ret(v_list(&[])));
        }
        Err(_) => {
            return Err(ErrValue(E_INVARG.msg("Unable to get connection options")));
        }
    };

    // Convert attributes to list of {name, value} pairs as expected by MOO
    // Since we only accept connection objects, we should get a map back
    let moor_var::Variant::Map(m) = attributes.variant() else {
        // For connection objects, we should always get a map, but handle gracefully
        return Ok(Ret(v_list(&[])));
    };

    // Convert map to list of {name, value} pairs
    let pairs: Vec<Var> = m
        .iter()
        .map(|(k, v)| v_list(&[k.clone(), v.clone()]))
        .collect();
    Ok(Ret(v_list(&pairs)))
}

/// Usage: `any connection_option(obj conn, str name)`
/// Returns the value of a specific connection option. Raises E_INVARG if not found.
/// Argument must be a connection object (negative ID), not a player.
fn bf_connection_option(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(ErrValue(
            E_ARGS.msg("connection_option() requires exactly 2 arguments"),
        ));
    }

    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(ErrValue(E_TYPE.msg("First argument must be an object")));
    };

    let Some(option_name) = bf_args.args[1].as_string() else {
        return Err(ErrValue(E_TYPE.msg("Second argument must be a string")));
    };

    // connection_option() requires a connection object (negative ID), not a player object
    if obj.is_positive() {
        return Err(ErrValue(E_INVARG.msg(
            "connection_option() requires a connection object, not a player object",
        )));
    }

    // Permission check: can only query connection options if user owns the connection or is wizard
    if !check_connection_ownership(obj, bf_args)? {
        return Err(ErrValue(E_PERM.msg("Permission denied")));
    }

    // Get the attributes from the connection registry
    let attributes = match current_session().connection_attributes(obj) {
        Ok(attributes) => attributes,
        Err(SessionError::NoConnectionForPlayer(_)) => {
            // No active connection
            return Err(ErrValue(E_INVARG.msg("No connection found")));
        }
        Err(_) => {
            return Err(ErrValue(E_INVARG.msg("Unable to get connection options")));
        }
    };

    // Look for the specific option in the attributes map
    let Variant::Map(m) = attributes.variant() else {
        // For connection objects, we should always get a map
        return Err(ErrValue(E_INVARG.msg("Unable to get connection options")));
    };

    // Search for the option by name (case-sensitive)
    for (key, value) in m.iter() {
        if let Some(key_str) = key.as_string()
            && key_str == option_name
        {
            return Ok(Ret(value.clone()));
        }
    }
    // Option not found
    Err(ErrValue(E_INVARG.msg(format!(
        "Connection option '{option_name}' not found"
    ))))
}

/// Usage: `none set_connection_option(obj conn, symbol option_name, any value)`
/// Sets a connection option on a connection object (negative ID).
fn bf_set_connection_option(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 3 {
        return Err(ErrValue(
            E_ARGS.msg("set_connection_option() requires exactly 3 arguments"),
        ));
    }

    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(ErrValue(E_TYPE.msg("First argument must be an object")));
    };

    let Ok(option_symbol) = bf_args.args[1].as_symbol() else {
        return Err(ErrValue(E_TYPE.msg("Second argument must be a string")));
    };

    let value = bf_args.args[2].clone();

    // set_connection_option() requires a connection object (negative ID), not a player object
    if obj.is_positive() {
        return Err(ErrValue(E_INVARG.msg(
            "set_connection_option() requires a connection object, not a player object",
        )));
    }

    if !check_connection_ownership(obj, bf_args)? {
        return Err(ErrValue(E_PERM.msg("Permission denied")));
    }

    let event =
        NarrativeEvent::set_connection_option(bf_args.exec_state.this(), obj, option_symbol, value);
    current_task_scheduler_client().notify(obj, Box::new(event));

    Ok(Ret(v_int(0)))
}

/// Usage: `int notify(obj player, str|any message [, int no_flush [, int no_newline [, symbol content_type [, map metadata]]]])`
/// Sends output to a player. If no_flush is true, buffering is allowed. If no_newline
/// is true, no newline is appended. With rich_notify enabled, message can be any value.
fn bf_notify(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // If in non rich-mode `notify` can only send text.
    // Otherwise, it can send any value, and it's up to the host/client to interpret it.
    if !bf_args.config.rich_notify {
        if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
            return Err(ErrValue(E_ARGS.msg("notify() requires 2 to 4 arguments")));
        }
        if bf_args.args[1].type_code() != TYPE_STR {
            return Err(ErrValue(
                E_TYPE.msg("notify() requires a string as the second argument"),
            ));
        }
    }

    if bf_args.args.len() < 2 || bf_args.args.len() > 6 {
        return Err(ErrValue(E_ARGS.msg("notify() requires 2 to 6 arguments")));
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

    let no_flush = if bf_args.args.len() > 2 {
        bf_args.args[2].is_true()
    } else {
        false
    };

    let no_newline = if bf_args.args.len() > 3 {
        bf_args.args[3].is_true()
    } else {
        false
    };

    let content_type = if bf_args.config.rich_notify && bf_args.args.len() >= 5 {
        let content_type = bf_args.args[4].as_symbol().map_err(ErrValue)?;
        Some(content_type)
    } else {
        None
    };

    let metadata = if bf_args.config.rich_notify && bf_args.args.len() == 6 {
        let metadata_arg = &bf_args.args[5];
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
                                    "notify() metadata alist must contain {key, value} pairs",
                                )));
                            }
                            let key_sym = pair[0].as_symbol().map_err(ErrValue)?;
                            metadata_vec.push((key_sym, pair[1].clone()));
                        }
                        _ => {
                            return Err(ErrValue(
                                E_TYPE
                                    .msg("notify() metadata alist must contain {key, value} pairs"),
                            ));
                        }
                    }
                }
            }
            _ => {
                return Err(ErrValue(
                    E_TYPE.msg("notify() metadata must be a map or alist"),
                ));
            }
        }

        Some(metadata_vec)
    } else {
        None
    };

    let event = NarrativeEvent::notify(
        bf_args.exec_state.this(),
        bf_args.args[1].clone(),
        content_type,
        no_flush,
        no_newline,
        metadata,
    );
    current_task_scheduler_client().notify(player, Box::new(event));

    // MOO docs say this should return none, but in reality it returns 1?
    Ok(Ret(v_int(1)))
}

/// Usage: `int event_log(obj player, str|any message [, symbol content_type [, map metadata]])`
/// Logs an event to the player's persistent event log without broadcasting to connections.
/// This allows sending formatted output to connections separately while still logging a
/// canonical format to the event log. With rich_notify enabled, message can be any value.
fn bf_event_log(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // If in non rich-mode `event_log` can only log text.
    // Otherwise, it can log any value.
    if !bf_args.config.rich_notify {
        if bf_args.args.len() < 2 || bf_args.args.len() > 2 {
            return Err(ErrValue(
                E_ARGS.msg("event_log() requires exactly 2 arguments"),
            ));
        }
        if bf_args.args[1].type_code() != TYPE_STR {
            return Err(ErrValue(
                E_TYPE.msg("event_log() requires a string as the second argument"),
            ));
        }
    }

    if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
        return Err(ErrValue(
            E_ARGS.msg("event_log() requires 2 to 4 arguments"),
        ));
    }

    let Some(player) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("event_log() requires an object as the first argument"),
        ));
    };

    // Must be player object, not connection object
    if !player.is_positive() {
        return Err(ErrValue(
            E_INVARG.msg("event_log() requires a player object, not a connection object"),
        ));
    }

    // If player is not the calling task perms, or a caller is not a wizard, raise E_PERM.
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    task_perms
        .check_obj_owner_perms(&player)
        .map_err(world_state_bf_err)?;

    let content_type = if bf_args.config.rich_notify && bf_args.args.len() >= 3 {
        let content_type = bf_args.args[2].as_symbol().map_err(ErrValue)?;
        Some(content_type)
    } else {
        None
    };

    let metadata = if bf_args.config.rich_notify && bf_args.args.len() == 4 {
        let metadata_arg = &bf_args.args[3];
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
                                    "event_log() metadata alist must contain {key, value} pairs",
                                )));
                            }
                            let key_sym = pair[0].as_symbol().map_err(ErrValue)?;
                            metadata_vec.push((key_sym, pair[1].clone()));
                        }
                        _ => {
                            return Err(ErrValue(
                                E_TYPE.msg(
                                    "event_log() metadata alist must contain {key, value} pairs",
                                ),
                            ));
                        }
                    }
                }
            }
            _ => {
                return Err(ErrValue(
                    E_TYPE.msg("event_log() metadata must be a map or alist"),
                ));
            }
        }

        Some(metadata_vec)
    } else {
        None
    };

    // Create the event - no_flush and no_newline are always false for event log
    let event = NarrativeEvent::notify(
        bf_args.exec_state.this(),
        bf_args.args[1].clone(),
        content_type,
        false,
        false,
        metadata,
    );
    current_task_scheduler_client().log_event(player, Box::new(event));

    Ok(Ret(v_int(1)))
}

/// Usage: `none present(obj player, str id [, str content_type, str target, str content [, list attrs]])`
/// Shows rich content (popup, panel, etc.) to the player. With only player and id,
/// dismisses that presentation. Requires rich_notify enabled.
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
        current_task_scheduler_client().notify(player, Box::new(event));

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

    current_task_scheduler_client().notify(player, Box::new(event));

    Ok(RetNil)
}

/// Usage: `list connected_players([int include_all])`
/// Returns a list of all connected player objects. If include_all is true,
/// includes connection objects (negative IDs) as well.
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

    let connected_player_set = current_session()
        .connected_players()
        .expect("Connected players should always be available");
    let map = connected_player_set.iter().filter_map(|p| {
        if !p.is_positive() && !include_all {
            return None;
        }
        Some(v_obj(*p))
    });
    Ok(Ret(v_list_iter(map)))
}

/// Usage: `int idle_seconds(obj player)`
/// Returns the number of seconds since the player last sent input.
fn bf_idle_seconds(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(ErrValue(E_ARGS.msg("idle_seconds() requires 1 argument")));
    }
    let Some(who) = bf_args.args[0].as_object() else {
        return Err(ErrValue(
            E_TYPE.msg("idle_seconds() requires an object as the first argument"),
        ));
    };
    let Ok(idle_seconds) = current_session().idle_seconds(who) else {
        return Err(ErrValue(E_INVARG.msg(
            "idle_seconds() requires a valid object as the first argument",
        )));
    };

    Ok(Ret(v_int(idle_seconds as i64)))
}

/// Usage: `int connected_seconds(obj player)`
/// Returns the number of seconds since the player connected.
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
    let Ok(connected_seconds) = current_session().connected_seconds(who) else {
        return Err(ErrValue(E_INVARG.msg(
            "connected_seconds() requires a valid object as the first argument",
        )));
    };

    Ok(Ret(v_int(connected_seconds as i64)))
}

/// Usage: `str connection_name(obj player)`
/// Returns a network-specific string identifying the player's connection (e.g., IP address).
/// Caller must be the player or a wizard.
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

    let Ok(connection_name) = current_session().connection_name(player) else {
        return Err(ErrValue(E_ARGS.msg(
            "connection_name() requires a valid object as the first argument",
        )));
    };

    Ok(Ret(v_string(connection_name)))
}

/// Usage: `list listeners([obj|int search])`
/// Returns active network listeners. Each entry is `{object, port, options}`.
/// Options is a map of Symbol->Var pairs (e.g., ["tls" -> 1, "print_messages" -> 1]).
/// If search is given, returns only the matching listener (by object or port).
fn bf_listeners(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(ErrValue(
            E_ARGS.msg("listeners() requires 0 or 1 arguments"),
        ));
    }

    let listeners = current_task_scheduler_client().listeners();

    // Helper to convert options Vec to a map Var
    let options_to_map = |options: &[(Symbol, Var)]| -> Var {
        let pairs: Vec<(Var, Var)> = options
            .iter()
            .map(|(k, v)| (v_sym(*k), v.clone()))
            .collect();
        v_map(&pairs)
    };

    // If an argument is provided, try to find the specific listener
    if bf_args.args.len() == 1 {
        let find_arg = &bf_args.args[0];

        // Look for a listener that matches the argument (could be object, port, etc.)
        for listener in listeners.iter() {
            // Check if the argument matches the listener object
            if let Some(obj) = find_arg.as_object() {
                if obj == listener.0 {
                    let options_map = options_to_map(&listener.3);
                    return Ok(Ret(v_list(&[
                        v_obj(listener.0),
                        v_int(listener.2 as i64),
                        options_map,
                    ])));
                }
            }
            // Check if the argument matches the port
            else if let Some(port) = find_arg.as_integer()
                && port == listener.2 as i64
            {
                let options_map = options_to_map(&listener.3);
                return Ok(Ret(v_list(&[
                    v_obj(listener.0),
                    v_int(listener.2 as i64),
                    options_map,
                ])));
            }
        }
        // If not found, return empty list.
        return Ok(Ret(v_list(&[])));
    }

    // No argument provided, return all listeners
    let listeners = listeners.iter().map(|listener| {
        let options_map = options_to_map(&listener.3);
        v_list(&[v_obj(listener.0), v_int(listener.2 as i64), options_map])
    });

    let listeners = v_list_iter(listeners);

    Ok(Ret(listeners))
}

/// Usage: `none unlisten(int port [, str host_type])`
/// Stops listening on the specified port. Host_type defaults to "tcp". Wizard-only.
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

    if let Some(err) = current_task_scheduler_client().unlisten(host_type, port) {
        return Err(ErrValue(err));
    }

    Ok(RetNil)
}

/// Usage: `int listen(obj object, int port [, int|map options])`
///
/// Starts listening for connections. Object receives connection callbacks instead of #0.
/// Third argument can be:
///
///   - An integer (legacy): treated as print_messages flag (0 or 1)
///   - A map (new style): options like ["tls" -> 1, "print_messages" -> 1]
///
/// Returns the canonical port. Wizard-only.
fn bf_listen(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // Requires wizard permissions.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(ErrValue(E_ARGS.msg("listen() requires 2 to 3 arguments")));
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

    // Parse options - can be an integer (legacy print_messages) or a map
    let options: Vec<(Symbol, Var)> = if bf_args.args.len() >= 3 {
        let options_arg = &bf_args.args[2];

        if let Some(options_map) = options_arg.as_map() {
            // New mode: map of options
            let mut opts = Vec::new();
            for (key, value) in options_map.iter() {
                let Ok(key_sym) = key.as_symbol() else {
                    return Err(ErrValue(
                        E_TYPE.msg("listen() options map keys must be strings or symbols"),
                    ));
                };
                opts.push((key_sym, value.clone()));
            }
            opts
        } else if options_arg.is_scalar() {
            // Legacy mode: scalar (int/bool/float) argument is print_messages flag
            if options_arg.is_true() {
                vec![(Symbol::mk("print_messages"), v_int(1))]
            } else {
                vec![]
            }
        } else {
            return Err(ErrValue(
                E_TYPE.msg("listen() third argument must be an integer or map"),
            ));
        }
    } else {
        vec![]
    };

    // Host type defaults to TCP
    let host_type = "tcp".to_string();

    // Ask the scheduler to broadcast a listen request out to all the hosts.
    if let Some(error) =
        current_task_scheduler_client().listen(object, host_type, port, options)
    {
        return Err(ErrValue(error));
    }

    // "Listen() returns canon, a `canonicalized' version of point, with any configuration-specific defaulting or aliasing accounted for. "
    // Uh, ok for now we'll just return the port.
    Ok(Ret(v_int(port as i64)))
}

pub(crate) fn register_bf_connection(builtins: &mut [BuiltinFunction]) {
    builtins[offset_for_builtin("notify")] = bf_notify;
    builtins[offset_for_builtin("connected_players")] = bf_connected_players;
    builtins[offset_for_builtin("force_input")] = bf_force_input;
    builtins[offset_for_builtin("present")] = bf_present;
    builtins[offset_for_builtin("worker_request")] = bf_worker_request;
    builtins[offset_for_builtin("connections")] = bf_connections;
    builtins[offset_for_builtin("connection")] = bf_connection;
    builtins[offset_for_builtin("switch_player")] = bf_switch_player;
    builtins[offset_for_builtin("workers")] = bf_workers;
    builtins[offset_for_builtin("output_delimiters")] = bf_output_delimiters;
    builtins[offset_for_builtin("connection_options")] = bf_connection_options;
    builtins[offset_for_builtin("connection_option")] = bf_connection_option;
    builtins[offset_for_builtin("set_connection_option")] = bf_set_connection_option;
    builtins[offset_for_builtin("idle_seconds")] = bf_idle_seconds;
    builtins[offset_for_builtin("connected_seconds")] = bf_connected_seconds;
    builtins[offset_for_builtin("connection_name")] = bf_connection_name;
    builtins[offset_for_builtin("listeners")] = bf_listeners;
    builtins[offset_for_builtin("listen")] = bf_listen;
    builtins[offset_for_builtin("unlisten")] = bf_unlisten;
    builtins[offset_for_builtin("event_log")] = bf_event_log;
}
