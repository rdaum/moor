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
    current_task_scheduler_client, with_current_transaction, with_loader_interface,
};
use crate::vm::builtins::BfRet::Ret;
use crate::vm::builtins::{
    BfCallState, BfErr, BfRet, BuiltinFunction, DiagnosticOutput, parse_diagnostic_options,
    world_state_bf_err,
};
use lazy_static::lazy_static;
use moor_common::builtins::offset_for_builtin;
use moor_common::model::{ObjectKind, obj_flags_string, prop_flags_string};
use moor_compiler::{DiagnosticRenderOptions, format_compile_error};
use moor_objdef::{
    ConflictEntity, ConflictMode, Constants, Entity, ObjDefLoaderOptions, ObjdefLoaderError,
};
use moor_var::{
    E_ARGS, E_INVARG, E_TYPE, Sequence, Symbol, Var, Variant, v_empty_map, v_list, v_obj, v_str,
    v_sym,
};

lazy_static! {
    // Entity type symbols
    static ref OBJECT_FLAGS_SYM: Symbol = Symbol::mk("object_flags");
    static ref BUILTIN_PROPS_SYM: Symbol = Symbol::mk("builtin_props");
    static ref PARENTAGE_SYM: Symbol = Symbol::mk("parentage");
    static ref PROPERTY_DEF_SYM: Symbol = Symbol::mk("property_def");
    static ref PROPERTY_VALUE_SYM: Symbol = Symbol::mk("property_value");
    static ref PROPERTY_FLAG_SYM: Symbol = Symbol::mk("property_flag");
    static ref VERB_DEF_SYM: Symbol = Symbol::mk("verb_def");
    static ref VERB_PROGRAM_SYM: Symbol = Symbol::mk("verb_program");

    // Options symbols
    static ref DRY_RUN_SYM: Symbol = Symbol::mk("dry_run");
    static ref CONFLICT_MODE_SYM: Symbol = Symbol::mk("conflict_mode");
    static ref TARGET_OBJECT_SYM: Symbol = Symbol::mk("target_object");
    static ref CREATE_NEW_SYM: Symbol = Symbol::mk("create_new");
    static ref CONSTANTS_SYM: Symbol = Symbol::mk("constants");
    static ref OVERRIDES_SYM: Symbol = Symbol::mk("overrides");
    static ref REMOVALS_SYM: Symbol = Symbol::mk("removals");
    static ref RETURN_CONFLICTS_SYM: Symbol = Symbol::mk("return_conflicts");
    static ref DIAGNOSTICS_SYM: Symbol = Symbol::mk("diagnostics");

    // Conflict mode symbols
    static ref CLOBBER_SYM: Symbol = Symbol::mk("clobber");
    static ref SKIP_SYM: Symbol = Symbol::mk("skip");
    static ref DETECT_SYM: Symbol = Symbol::mk("detect");
}

/// Returns a list of strings representing the object definition in objdef format.
/// The caller must own the object or be a wizard. Options are currently ignored (keeping simple for phase 1).
/// MOO: `list dump_object(obj object [, map options])`
fn bf_dump_object(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("dump_object() takes 1 or 2 arguments"),
        ));
    }

    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("dump_object() first argument must be an object"),
        ));
    };

    // Validate options argument if provided (currently ignored but must be a map)
    if bf_args.args.len() == 2 && bf_args.args[1].as_map().is_none() {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("dump_object() second argument must be a map"),
        ));
    }
    // Options are currently ignored for phase 1 simplicity

    // Check that object is valid
    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("dump_object() argument must be a valid object"),
        ));
    }

    // Check permissions: wizard only (object dumps can expose properties owned by others)
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    task_perms.check_wizard().map_err(world_state_bf_err)?;

    // Use the task scheduler client to request the dump from the scheduler
    let lines = current_task_scheduler_client()
        .dump_object(obj)
        .map_err(|e| BfErr::ErrValue(E_INVARG.msg(format!("Failed to dump object: {e}"))))?;

    // Convert to MOO list of strings
    let string_vars: Vec<_> = lines.iter().map(|line| v_str(line)).collect();
    Ok(Ret(v_list(&string_vars)))
}

/// Convert a MOO entity specification to an internal Entity enum.
/// Entity specs can be:
/// - `object_flags / "object_flags"
/// - `builtin_props / "builtin_props"
/// - `parentage / "parentage"
/// - {`property_def, propname} / {"property_def", propname}
/// - {`property_value, propname} / {"property_value", propname}
/// - {`property_flag, propname} / {"property_flag", propname}
/// - {`verb_def, {name1, name2, ...}} / {"verb_def", {name1, name2, ...}}
/// - {`verb_program, {name1, name2, ...}} / {"verb_program", {name1, name2, ...}}
fn moo_entity_to_entity(_bf_args: &mut BfCallState<'_>, moo_entity: &Var) -> Result<Entity, BfErr> {
    match moo_entity.variant() {
        Variant::Str(_) | Variant::Sym(_) => {
            let sym = moo_entity.as_symbol().map_err(BfErr::ErrValue)?;
            if sym == *OBJECT_FLAGS_SYM {
                Ok(Entity::ObjectFlags)
            } else if sym == *BUILTIN_PROPS_SYM {
                Ok(Entity::BuiltinProps)
            } else if sym == *PARENTAGE_SYM {
                Ok(Entity::Parentage)
            } else {
                Err(BfErr::ErrValue(E_INVARG.msg("Invalid entity type")))
            }
        }
        Variant::List(l) => {
            if l.len() != 2 {
                return Err(BfErr::ErrValue(
                    E_INVARG.msg("Entity specification must be {type, specifier}"),
                ));
            }
            let entity_type = l.index(0).map_err(BfErr::ErrValue)?;
            let specifier = l.index(1).map_err(BfErr::ErrValue)?;

            let type_sym = entity_type.as_symbol().map_err(BfErr::ErrValue)?;

            if type_sym == *PROPERTY_DEF_SYM {
                let prop_name = specifier.as_symbol().map_err(BfErr::ErrValue)?;
                Ok(Entity::PropertyDef(prop_name))
            } else if type_sym == *PROPERTY_VALUE_SYM {
                let prop_name = specifier.as_symbol().map_err(BfErr::ErrValue)?;
                Ok(Entity::PropertyValue(prop_name))
            } else if type_sym == *PROPERTY_FLAG_SYM {
                let prop_name = specifier.as_symbol().map_err(BfErr::ErrValue)?;
                Ok(Entity::PropertyFlag(prop_name))
            } else if type_sym == *VERB_DEF_SYM {
                let Some(names_list) = specifier.as_list() else {
                    return Err(BfErr::ErrValue(E_TYPE.msg("Verb names must be a list")));
                };
                let mut names = Vec::new();
                for name_var in names_list.iter() {
                    let name = name_var.as_symbol().map_err(BfErr::ErrValue)?;
                    names.push(name);
                }
                Ok(Entity::VerbDef(names))
            } else if type_sym == *VERB_PROGRAM_SYM {
                let Some(names_list) = specifier.as_list() else {
                    return Err(BfErr::ErrValue(E_TYPE.msg("Verb names must be a list")));
                };
                let mut names = Vec::new();
                for name_var in names_list.iter() {
                    let name = name_var.as_symbol().map_err(BfErr::ErrValue)?;
                    names.push(name);
                }
                Ok(Entity::VerbProgram(names))
            } else {
                Err(BfErr::ErrValue(E_INVARG.msg("Invalid entity type")))
            }
        }
        _ => Err(BfErr::ErrValue(
            E_TYPE.msg("Entity must be string/symbol or {type, specifier}"),
        )),
    }
}

/// Convert an internal ConflictEntity back to MOO format for return values.
fn conflict_entity_to_moo(bf_args: &mut BfCallState<'_>, conflict: &ConflictEntity) -> Var {
    let use_symbols = bf_args.config.use_symbols_in_builtins && bf_args.config.symbol_type;
    let sym_or_str = |sym: Symbol| {
        if use_symbols {
            v_sym(sym)
        } else {
            v_str(&sym.as_string())
        }
    };

    match conflict {
        ConflictEntity::ObjectFlags(flags) => v_list(&[
            sym_or_str(*OBJECT_FLAGS_SYM),
            v_str(&obj_flags_string(*flags)),
        ]),
        ConflictEntity::BuiltinProps(prop, value) => {
            v_list(&[sym_or_str(*BUILTIN_PROPS_SYM), v_sym(*prop), value.clone()])
        }
        ConflictEntity::Parentage(parent) => v_list(&[sym_or_str(*PARENTAGE_SYM), v_obj(*parent)]),
        ConflictEntity::PropertyDef(prop, _def) => {
            v_list(&[sym_or_str(*PROPERTY_DEF_SYM), v_sym(*prop)])
        }
        ConflictEntity::PropertyValue(prop, value) => {
            v_list(&[sym_or_str(*PROPERTY_VALUE_SYM), v_sym(*prop), value.clone()])
        }
        ConflictEntity::PropertyFlag(prop, flags) => v_list(&[
            sym_or_str(*PROPERTY_FLAG_SYM),
            v_sym(*prop),
            v_str(&prop_flags_string(*flags)),
        ]),
        ConflictEntity::VerbDef(names, _def) => {
            let name_vars = names.iter().map(|n| v_sym(*n)).collect::<Vec<_>>();
            v_list(&[sym_or_str(*VERB_DEF_SYM), v_list(&name_vars)])
        }
        ConflictEntity::VerbProgram(names, _program) => {
            let name_vars = names.iter().map(|n| v_sym(*n)).collect::<Vec<_>>();
            v_list(&[sym_or_str(*VERB_PROGRAM_SYM), v_list(&name_vars)])
        }
    }
}

/// Parse object kind specification from the third argument
fn parse_object_kind_spec(
    bf_args: &BfCallState<'_>,
    arg: &Var,
) -> Result<Option<ObjectKind>, BfErr> {
    match arg.variant() {
        Variant::Int(0) => Ok(Some(ObjectKind::NextObjid)),
        Variant::Int(1) => {
            if !bf_args.config.anonymous_objects {
                return Err(BfErr::ErrValue(E_INVARG.msg(
                    "Anonymous objects not available (anonymous_objects feature is disabled)",
                )));
            }
            Ok(Some(ObjectKind::Anonymous))
        }
        Variant::Int(2) => {
            if !bf_args.config.use_uuobjids {
                return Err(BfErr::ErrValue(
                    E_INVARG.msg("UUID objects not available (use_uuobjids is false)"),
                ));
            }
            Ok(Some(ObjectKind::UuObjId))
        }
        Variant::Int(_) => Err(BfErr::ErrValue(E_INVARG.msg(
            "load_object() object_spec must be 0 (NextObjid), 1 (Anonymous), 2 (UuObjId), or an object ID",
        ))),
        Variant::Obj(obj) => Ok(Some(ObjectKind::Objid(*obj))),
        _ => Err(BfErr::ErrValue(E_TYPE.msg(
            "load_object() third argument must be an integer (0, 1, 2) or an object ID",
        ))),
    }
}

/// Parse a single override/removal pair: {obj, entity}
fn parse_obj_entity_pair(
    bf_args: &mut BfCallState<'_>,
    pair: &Var,
    pair_type: &str,
) -> Result<(moor_var::Obj, Entity), BfErr> {
    let Some(pair_list) = pair.as_list() else {
        return Err(BfErr::ErrValue(E_TYPE.msg(format!(
            "{pair_type} must be a list of {{obj, entity}} pairs"
        ))));
    };

    if pair_list.len() != 2 {
        return Err(BfErr::ErrValue(E_ARGS.msg(format!(
            "{pair_type} pairs must have exactly 2 elements: {{obj, entity}}"
        ))));
    }

    let obj_var = pair_list.index(0).map_err(BfErr::ErrValue)?;
    let Some(obj) = obj_var.as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg(format!("{pair_type} object must be an object")),
        ));
    };

    let entity_var = pair_list.index(1).map_err(BfErr::ErrValue)?;
    let entity = moo_entity_to_entity(bf_args, &entity_var)?;

    Ok((obj, entity))
}

/// Parse conflict mode from symbol
fn parse_conflict_mode(mode_sym: Symbol) -> Result<(ConflictMode, bool, bool), BfErr> {
    if mode_sym == *CLOBBER_SYM {
        Ok((ConflictMode::Clobber, false, false))
    } else if mode_sym == *SKIP_SYM {
        Ok((ConflictMode::Skip, false, false))
    } else if mode_sym == *DETECT_SYM {
        // "detect" mode is essentially dry_run + return_conflicts
        Ok((ConflictMode::Clobber, true, true))
    } else {
        Err(BfErr::ErrValue(
            E_INVARG.msg("conflict_mode must be `clobber, `skip, or `detect"),
        ))
    }
}

/// Format load result for return to MOO code
fn format_load_result(
    bf_args: &mut BfCallState<'_>,
    result: &moor_objdef::ObjDefLoaderResults,
    return_conflicts: bool,
) -> Result<Var, BfErr> {
    if !return_conflicts {
        // Return simple object ID (backward compatibility)
        if result.loaded_objects.is_empty() {
            return Err(BfErr::ErrValue(E_INVARG.msg("No objects were loaded")));
        }
        return Ok(v_obj(result.loaded_objects[0]));
    }

    // Return detailed result: {success, conflicts, loaded_objects}
    let conflicts: Vec<_> = result
        .conflicts
        .iter()
        .map(|(obj, conflict)| v_list(&[v_obj(*obj), conflict_entity_to_moo(bf_args, conflict)]))
        .collect();

    let loaded_objects: Vec<_> = result
        .loaded_objects
        .iter()
        .map(|obj| v_obj(*obj))
        .collect();

    Ok(v_list(&[
        bf_args.v_bool(result.commit),
        v_list(&conflicts),
        v_list(&loaded_objects),
    ]))
}

/// Loads a single object definition from a list of strings and creates it in the database.
/// This creates the object and all its properties/verbs. Wizard-only.
/// MOO: `obj load_object(list object_lines [, map options] [, obj|int object_spec])`
/// object_spec: 0=NextObjid, 1=Anonymous, 2=UuObjId, #N=specific ID, omitted=use objdef's ID
fn bf_load_object(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("load_object() requires 1-3 arguments"),
        ));
    }

    let Some(lines_list) = bf_args.args[0].as_list() else {
        return Err(BfErr::ErrValue(E_TYPE.msg(
            "load_object() requires a list of strings as the first argument",
        )));
    };

    // Convert list of values to list of strings, joining with newlines
    let mut lines = Vec::new();
    for line_val in lines_list.iter() {
        let Some(line_str) = line_val.as_string() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("load_object() requires a list of strings"),
            ));
        };
        lines.push(line_str.to_string());
    }
    let object_definition = lines.join("\n");

    // Parse options map (second argument)
    let options_map = if bf_args.args.len() >= 2 {
        bf_args.map_or_alist_to_map(&bf_args.args[1])?
    } else {
        v_empty_map().as_map().unwrap().clone()
    };

    // Parse the object specification (third argument)
    let object_kind = if bf_args.args.len() == 3 {
        parse_object_kind_spec(bf_args, &bf_args.args[2])?
    } else {
        None
    };

    // Extract options from the map using symbol constants
    let mut dry_run = false;
    let mut conflict_mode = ConflictMode::Clobber;
    let mut constants: Option<Constants> = None;
    let mut overrides = Vec::new();
    let mut return_conflicts = false;
    let mut diagnostic_options = DiagnosticRenderOptions::default();

    for (key, value) in options_map.iter() {
        let key_sym = key.as_symbol().map_err(BfErr::ErrValue)?;

        if key_sym == *DRY_RUN_SYM {
            dry_run = value.is_true();
            continue;
        }

        if key_sym == *CONFLICT_MODE_SYM {
            let mode_sym = value.as_symbol().map_err(BfErr::ErrValue)?;
            let (mode, dr, rc) = parse_conflict_mode(mode_sym)?;
            conflict_mode = mode;
            if dr {
                dry_run = true;
            }
            if rc {
                return_conflicts = true;
            }
            continue;
        }

        if key_sym == *CONSTANTS_SYM {
            let const_map = bf_args.map_or_alist_to_map(&value)?;
            constants = Some(Constants::Map(const_map));
            continue;
        }

        if key_sym == *OVERRIDES_SYM {
            let Some(overrides_list) = value.as_list() else {
                return Err(BfErr::ErrValue(
                    E_TYPE.msg("overrides must be a list of {obj, entity} pairs"),
                ));
            };
            for override_pair in overrides_list.iter() {
                let (obj, entity) = parse_obj_entity_pair(bf_args, &override_pair, "overrides")?;
                overrides.push((obj, entity));
            }
            continue;
        }

        if key_sym == *RETURN_CONFLICTS_SYM {
            return_conflicts = value.is_true();
            continue;
        }

        if key_sym == *DIAGNOSTICS_SYM {
            // Parse diagnostic options from a map with "verbosity" and "output_mode" fields
            let Some(diag_map) = value.as_map() else {
                return Err(BfErr::ErrValue(
                    E_TYPE.msg("diagnostics must be a map"),
                ));
            };

            let mut verbosity = None;
            let mut output_mode = None;

            for (k, v) in diag_map.iter() {
                let Some(key_str) = k.as_string() else {
                    continue;
                };

                if key_str == "verbosity" {
                    verbosity = v.as_integer();
                } else if key_str == "output_mode" {
                    output_mode = v.as_integer();
                }
            }

            let diagnostic_output = parse_diagnostic_options(verbosity, output_mode)?;
            // obj_load only uses formatted output
            diagnostic_options = match diagnostic_output {
                DiagnosticOutput::Formatted(options) => options,
                DiagnosticOutput::Structured => DiagnosticRenderOptions::default(),
            };
            continue;
        }
    }

    // Check permissions: wizard only (object creation with arbitrary properties/verbs)
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    task_perms.check_wizard().map_err(world_state_bf_err)?;

    // Create options object for the loader
    let loader_options = ObjDefLoaderOptions {
        dry_run,
        conflict_mode,
        object_kind,
        constants: constants.clone(),
        overrides,
        validate_parent_changes: true, // Individual loads should validate parent changes
    };

    // Get the compile options from the config
    let compile_options = bf_args.config.compile_options();

    let loader_result: Result<_, ObjdefLoaderError> = with_loader_interface(|loader| {
        let mut object_loader = moor_objdef::ObjectDefinitionLoader::new(loader);
        object_loader.load_single_object(&object_definition, compile_options, loader_options)
    });

    let result = match loader_result {
        Ok(results) => results,
        Err(e) => {
            if let Some((_, compile_error)) = e.compile_error() {
                let formatted = format_compile_error(
                    compile_error,
                    Some(&object_definition),
                    diagnostic_options,
                );
                let message = formatted.join("\n");
                return Err(BfErr::ErrValue(E_INVARG.msg(message)));
            }

            return Err(BfErr::ErrValue(
                E_INVARG.msg(format!("Failed to load object: {e}")),
            ));
        }
    };

    // Format and return the result
    let return_value = format_load_result(bf_args, &result, return_conflicts)?;
    Ok(Ret(return_value))
}

/// Completely replaces an existing object with a new definition from objdef format.
/// This deletes all properties and verbs not in the new definition.
/// MOO: `obj reload_object(list object_lines [, map constants] [, obj target])`
fn bf_reload_object(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("reload_object() requires 1-3 arguments"),
        ));
    }

    let Some(lines_list) = bf_args.args[0].as_list() else {
        return Err(BfErr::ErrValue(E_TYPE.msg(
            "reload_object() requires a list of strings as the first argument",
        )));
    };

    // Convert list of values to list of strings, joining with newlines
    let mut lines = Vec::new();
    for line_val in lines_list.iter() {
        let Some(line_str) = line_val.as_string() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("reload_object() requires a list of strings"),
            ));
        };
        lines.push(line_str.to_string());
    }
    let object_definition = lines.join("\n");

    // Parse constants map (second argument)
    let constants = if bf_args.args.len() >= 2 {
        let Ok(const_map) = bf_args.map_or_alist_to_map(&bf_args.args[1]) else {
            return Err(BfErr::ErrValue(E_TYPE.with_msg( ||
                format!("invalid second argument for reload_object(); was {}, should be map or alist of constant substitutions",
                         bf_args.args[1].type_code().to_literal())
            )));
        };
        Some(Constants::Map(const_map))
    } else {
        None
    };

    // Parse target object (third argument)
    let target_obj = if bf_args.args.len() == 3 {
        let Some(obj) = bf_args.args[2].as_object() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("reload_object() target must be an object"),
            ));
        };

        // Verify the target object exists
        if !with_current_transaction(|world_state| world_state.valid(&obj))
            .map_err(world_state_bf_err)?
        {
            return Err(BfErr::ErrValue(
                E_INVARG.msg("reload_object() target object does not exist"),
            ));
        }

        Some(obj)
    } else {
        None
    };

    // Check permissions: wizard only
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    task_perms.check_wizard().map_err(world_state_bf_err)?;

    // Use the current task's transaction via loader interface
    let result = match with_loader_interface(|loader| {
        let mut object_loader = moor_objdef::ObjectDefinitionLoader::new(loader);

        // Reload the object with the provided constants and target
        object_loader.reload_single_object(&object_definition, constants, target_obj)
    }) {
        Ok(result) => result,
        Err(e) => {
            return Err(BfErr::ErrValue(
                E_INVARG.with_msg(|| format!("failed to load object: {e}")),
            ));
        }
    };

    // Return the loaded object ID (should be exactly one)
    if result.loaded_objects.is_empty() {
        return Err(BfErr::ErrValue(E_INVARG.msg("No objects were loaded")));
    }

    Ok(Ret(v_obj(result.loaded_objects[0])))
}

pub(crate) fn register_bf_obj_load(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("dump_object")] = Box::new(bf_dump_object);
    builtins[offset_for_builtin("load_object")] = Box::new(bf_load_object);
    builtins[offset_for_builtin("reload_object")] = Box::new(bf_reload_object);
}
