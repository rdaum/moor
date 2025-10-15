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
use crate::vm::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction, world_state_bf_err};
use lazy_static::lazy_static;
use moor_common::builtins::offset_for_builtin;
use moor_common::model::{
    ArgSpec, ObjFlag, ObjectMutation, PropFlag, VerbArgsSpec, VerbFlag, obj_flags_string,
    parse_preposition_spec, prop_flags_string,
};
use moor_common::util::BitEnum;
use moor_objdef::{ConflictEntity, ConflictMode, Entity, ObjDefLoaderOptions};
use moor_var::program::ProgramType;
use moor_var::{
    E_ARGS, E_INVARG, E_TYPE, Error, Sequence, Symbol, Var, Variant, v_empty_map, v_error, v_int,
    v_list, v_map, v_obj, v_str, v_sym,
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

    // Conflict mode symbols
    static ref CLOBBER_SYM: Symbol = Symbol::mk("clobber");
    static ref SKIP_SYM: Symbol = Symbol::mk("skip");
    static ref DETECT_SYM: Symbol = Symbol::mk("detect");

    // Mutation action symbols
    static ref DEFINE_PROPERTY_SYM: Symbol = Symbol::mk("define_property");
    static ref DELETE_PROPERTY_SYM: Symbol = Symbol::mk("delete_property");
    static ref SET_PROPERTY_VALUE_SYM: Symbol = Symbol::mk("set_property_value");
    static ref SET_PROPERTY_FLAGS_SYM: Symbol = Symbol::mk("set_property_flags");
    static ref CLEAR_PROPERTY_SYM: Symbol = Symbol::mk("clear_property");
    static ref DEFINE_VERB_SYM: Symbol = Symbol::mk("define_verb");
    static ref DELETE_VERB_SYM: Symbol = Symbol::mk("delete_verb");
    static ref UPDATE_VERB_PROGRAM_SYM: Symbol = Symbol::mk("update_verb_program");
    static ref UPDATE_VERB_METADATA_SYM: Symbol = Symbol::mk("update_verb_metadata");
    static ref CREATE_OBJECT_SYM: Symbol = Symbol::mk("create_object");
    static ref RECYCLE_OBJECT_SYM: Symbol = Symbol::mk("recycle_object");
    static ref SET_OBJECT_FLAGS_SYM: Symbol = Symbol::mk("set_object_flags");
    static ref SET_PARENT_SYM: Symbol = Symbol::mk("set_parent");
    static ref SET_LOCATION_SYM: Symbol = Symbol::mk("set_location");
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

/// Convert an internal Entity back to MOO format for return values.
fn entity_to_moo(bf_args: &mut BfCallState<'_>, entity: &Entity) -> Var {
    let use_symbols = bf_args.config.use_symbols_in_builtins && bf_args.config.symbol_type;
    let sym_or_str = |sym: Symbol| {
        if use_symbols {
            v_sym(sym)
        } else {
            v_str(&sym.as_string())
        }
    };

    match entity {
        Entity::ObjectFlags => sym_or_str(*OBJECT_FLAGS_SYM),
        Entity::BuiltinProps => sym_or_str(*BUILTIN_PROPS_SYM),
        Entity::Parentage => sym_or_str(*PARENTAGE_SYM),
        Entity::PropertyDef(prop) => v_list(&[sym_or_str(*PROPERTY_DEF_SYM), v_sym(*prop)]),
        Entity::PropertyValue(prop) => v_list(&[sym_or_str(*PROPERTY_VALUE_SYM), v_sym(*prop)]),
        Entity::PropertyFlag(prop) => v_list(&[sym_or_str(*PROPERTY_FLAG_SYM), v_sym(*prop)]),
        Entity::VerbDef(names) => {
            let name_vars = names.iter().map(|n| v_sym(*n)).collect::<Vec<_>>();
            v_list(&[sym_or_str(*VERB_DEF_SYM), v_list(&name_vars)])
        }
        Entity::VerbProgram(names) => {
            let name_vars = names.iter().map(|n| v_sym(*n)).collect::<Vec<_>>();
            v_list(&[sym_or_str(*VERB_PROGRAM_SYM), v_list(&name_vars)])
        }
    }
}

/// Loads a single object definition from a list of strings and creates it in the database.
/// This creates the object and all its properties/verbs. Wizard-only.
/// MOO: `obj load_object(list object_lines [, map options])`
fn bf_load_object(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 1 || bf_args.args.len() > 2 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("load_object() requires 1-2 arguments"),
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
    let options_map = if bf_args.args.len() == 2 {
        bf_args.map_or_alist_to_map(&bf_args.args[1])?
    } else {
        v_empty_map().as_map().unwrap().clone()
    };

    // Extract options from the map using symbol constants
    let mut dry_run = false;
    let mut conflict_mode = ConflictMode::Clobber;
    let mut target_object = None;
    let mut create_new = false;
    let mut constants = None;
    let mut overrides = Vec::new();
    let mut removals = Vec::new();
    let mut return_conflicts = false;

    for (key, value) in options_map.iter() {
        let key_sym = key.as_symbol().map_err(BfErr::ErrValue)?;

        if key_sym == *DRY_RUN_SYM {
            dry_run = value.is_true();
        } else if key_sym == *CONFLICT_MODE_SYM {
            let mode_sym = value.as_symbol().map_err(BfErr::ErrValue)?;
            if mode_sym == *CLOBBER_SYM {
                conflict_mode = ConflictMode::Clobber;
            } else if mode_sym == *SKIP_SYM {
                conflict_mode = ConflictMode::Skip;
            } else if mode_sym == *DETECT_SYM {
                // "detect" mode is essentially dry_run + return_conflicts
                dry_run = true;
                return_conflicts = true;
            } else {
                return Err(BfErr::ErrValue(
                    E_INVARG.msg("conflict_mode must be `clobber, `skip, or `detect"),
                ));
            }
        } else if key_sym == *TARGET_OBJECT_SYM {
            target_object =
                Some(value.as_object().ok_or_else(|| {
                    BfErr::ErrValue(E_TYPE.msg("target_object must be an object"))
                })?);
        } else if key_sym == *CREATE_NEW_SYM {
            create_new = value.is_true();
        } else if key_sym == *CONSTANTS_SYM {
            let const_map = bf_args.map_or_alist_to_map(&value)?;
            constants = Some(const_map);
        } else if key_sym == *OVERRIDES_SYM {
            let Some(overrides_list) = value.as_list() else {
                return Err(BfErr::ErrValue(
                    E_TYPE.msg("overrides must be a list of {obj, entity} pairs"),
                ));
            };
            for override_pair in overrides_list.iter() {
                let Some(pair_list) = override_pair.as_list() else {
                    return Err(BfErr::ErrValue(
                        E_TYPE.msg("overrides must be a list of {obj, entity} pairs"),
                    ));
                };
                if pair_list.len() != 2 {
                    return Err(BfErr::ErrValue(
                        E_ARGS.msg("override pairs must have exactly 2 elements: {obj, entity}"),
                    ));
                }
                let obj = pair_list
                    .index(0)
                    .map_err(BfErr::ErrValue)?
                    .as_object()
                    .ok_or_else(|| {
                        BfErr::ErrValue(E_TYPE.msg("override object must be an object"))
                    })?;
                let entity =
                    moo_entity_to_entity(bf_args, &pair_list.index(1).map_err(BfErr::ErrValue)?)?;
                overrides.push((obj, entity));
            }
        } else if key_sym == *REMOVALS_SYM {
            let Some(removals_list) = value.as_list() else {
                return Err(BfErr::ErrValue(
                    E_TYPE.msg("removals must be a list of {obj, entity} pairs"),
                ));
            };
            for removal_pair in removals_list.iter() {
                let Some(pair_list) = removal_pair.as_list() else {
                    return Err(BfErr::ErrValue(
                        E_TYPE.msg("removals must be a list of {obj, entity} pairs"),
                    ));
                };
                if pair_list.len() != 2 {
                    return Err(BfErr::ErrValue(
                        E_ARGS.msg("removal pairs must have exactly 2 elements: {obj, entity}"),
                    ));
                }
                let obj = pair_list
                    .index(0)
                    .map_err(BfErr::ErrValue)?
                    .as_object()
                    .ok_or_else(|| {
                        BfErr::ErrValue(E_TYPE.msg("removal object must be an object"))
                    })?;
                let entity =
                    moo_entity_to_entity(bf_args, &pair_list.index(1).map_err(BfErr::ErrValue)?)?;
                removals.push((obj, entity));
            }
        } else if key_sym == *RETURN_CONFLICTS_SYM {
            return_conflicts = value.is_true();
        }
    }

    // Validate mutual exclusivity of target_object and create_new
    if target_object.is_some() && create_new {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("Cannot specify both target_object and create_new options"),
        ));
    }

    // Check permissions: wizard only (object creation with arbitrary properties/verbs)
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    task_perms.check_wizard().map_err(world_state_bf_err)?;

    // Create options object for the loader
    let loader_options = ObjDefLoaderOptions {
        dry_run,
        conflict_mode,
        target_object,
        create_new,
        constants,
        overrides,
        removals,
    };

    // Get the compile options from the config
    let compile_options = bf_args.config.compile_options();

    // Use the current task's transaction via loader interface
    // This avoids creating a separate transaction and the TaskSuspend::Commit issue
    let result = with_loader_interface(|loader| {
        let mut object_loader = moor_objdef::ObjectDefinitionLoader::new(loader);

        // Load the single object with provided options
        let target_obj = loader_options.target_object;
        let constants_clone = loader_options.constants.clone();
        let results = object_loader
            .load_single_object(
                &object_definition,
                compile_options,
                target_obj,
                constants_clone,
                loader_options,
            )
            .map_err(|e| {
                moor_common::model::WorldStateError::DatabaseError(format!(
                    "Failed to load object: {e}"
                ))
            })?;

        // Note: We don't commit here - the loader operations are happening in the task's transaction
        // The transaction will be committed normally when the task completes

        Ok(results)
    })
    .map_err(world_state_bf_err)?;

    // Format the return value based on return_conflicts flag
    let return_value = if return_conflicts {
        // Return detailed result: {success, conflicts, removals, loaded_objects}
        let conflicts = result
            .conflicts
            .iter()
            .map(|(obj, conflict)| {
                v_list(&[v_obj(*obj), conflict_entity_to_moo(bf_args, conflict)])
            })
            .collect::<Vec<_>>();

        let removals_result = result
            .removals
            .iter()
            .map(|(obj, entity)| v_list(&[v_obj(*obj), entity_to_moo(bf_args, entity)]))
            .collect::<Vec<_>>();

        let loaded_objects = result
            .loaded_objects
            .iter()
            .map(|obj| v_obj(*obj))
            .collect::<Vec<_>>();

        v_list(&[
            bf_args.v_bool(result.commit),
            v_list(&conflicts),
            v_list(&removals_result),
            v_list(&loaded_objects),
        ])
    } else {
        // Return simple object ID (backward compatibility)
        if result.loaded_objects.is_empty() {
            return Err(BfErr::ErrValue(E_INVARG.msg("No objects were loaded")));
        }
        v_obj(result.loaded_objects[0])
    };

    // Return the result - the loaded objects are already in the current transaction
    // No need for TaskSuspend::Commit since we used the same transaction
    Ok(Ret(return_value))
}

/// Parse verb flags from a string like "rwxd"
pub fn parse_verb_flags(flags_str: &str) -> Result<BitEnum<VerbFlag>, Error> {
    VerbFlag::parse_str(flags_str)
        .ok_or_else(|| E_INVARG.msg(format!("Invalid verb flags: {flags_str}")))
}

/// Parse property flags from a string like "rwc"
pub fn parse_prop_flags(flags_str: &str) -> Result<BitEnum<PropFlag>, Error> {
    PropFlag::parse_str(flags_str)
        .ok_or_else(|| E_INVARG.msg(format!("Invalid property flags: {flags_str}")))
}

/// Parse object flags from a string like "pwr"
pub fn parse_object_flags(flags_str: &str) -> Result<BitEnum<ObjFlag>, Error> {
    ObjFlag::parse_str(flags_str)
        .ok_or_else(|| E_INVARG.msg(format!("Invalid object flags: {flags_str}")))
}

/// Convert a MOO Var representing a mutation specification into an ObjectMutation
pub fn var_to_mutation(mutation_var: &Var) -> Result<ObjectMutation, Error> {
    let list = mutation_var
        .as_list()
        .ok_or_else(|| E_TYPE.msg("Mutation must be a list"))?;

    if list.is_empty() {
        return Err(E_ARGS.msg("Mutation list cannot be empty"));
    }

    // First element is the action keyword (symbol or string)
    let action = list.index(0)?;
    let action_sym = action.as_symbol()?;

    // Property operations
    if action_sym == *DEFINE_PROPERTY_SYM {
        if list.len() != 5 {
            return Err(E_ARGS
                .msg("define_property requires 5 elements: action, name, owner, flags, value"));
        }
        let name = list.index(1)?.as_symbol()?;
        let owner = list
            .index(2)?
            .as_object()
            .ok_or_else(|| E_TYPE.msg("Owner must be an object"))?;
        let flags_val = list.index(3)?;
        let flags_str = flags_val
            .as_string()
            .ok_or_else(|| E_TYPE.msg("Flags must be a string"))?;
        let flags = parse_prop_flags(flags_str)?;
        let value = list.index(4)?;

        return Ok(ObjectMutation::DefineProperty {
            name,
            owner,
            flags,
            value: Some(value),
        });
    }

    if action_sym == *DELETE_PROPERTY_SYM {
        if list.len() != 2 {
            return Err(E_ARGS.msg("delete_property requires 2 elements: action, name"));
        }
        let name = list.index(1)?.as_symbol()?;
        return Ok(ObjectMutation::DeleteProperty { name });
    }

    if action_sym == *SET_PROPERTY_VALUE_SYM {
        if list.len() != 3 {
            return Err(E_ARGS.msg("set_property_value requires 3 elements: action, name, value"));
        }
        let name = list.index(1)?.as_symbol()?;
        let value = list.index(2)?;
        return Ok(ObjectMutation::SetPropertyValue { name, value });
    }

    if action_sym == *SET_PROPERTY_FLAGS_SYM {
        if list.len() != 4 {
            return Err(
                E_ARGS.msg("set_property_flags requires 4 elements: action, name, owner, flags")
            );
        }
        let name = list.index(1)?.as_symbol()?;
        let owner = list
            .index(2)?
            .as_object()
            .ok_or_else(|| E_TYPE.msg("Owner must be an object"))?;
        let flags_val = list.index(3)?;
        let flags_str = flags_val
            .as_string()
            .ok_or_else(|| E_TYPE.msg("Flags must be a string"))?;
        let flags = parse_prop_flags(flags_str)?;

        return Ok(ObjectMutation::SetPropertyFlags {
            name,
            owner: Some(owner),
            flags,
        });
    }

    if action_sym == *CLEAR_PROPERTY_SYM {
        if list.len() != 2 {
            return Err(E_ARGS.msg("clear_property requires 2 elements: action, name"));
        }
        let name = list.index(1)?.as_symbol()?;
        return Ok(ObjectMutation::ClearProperty { name });
    }

    // Verb operations
    if action_sym == *DEFINE_VERB_SYM {
        if list.len() != 6 {
            return Err(E_ARGS.msg(
                "define_verb requires 6 elements: action, names, owner, flags, argspec, program",
            ));
        }
        let names_val = list.index(1)?;
        let names_list = names_val
            .as_list()
            .ok_or_else(|| E_TYPE.msg("Verb names must be a list"))?;
        let mut names = Vec::new();
        for name_var in names_list.iter() {
            names.push(name_var.as_symbol()?);
        }

        let owner = list
            .index(2)?
            .as_object()
            .ok_or_else(|| E_TYPE.msg("Owner must be an object"))?;
        let flags_val = list.index(3)?;
        let flags_str = flags_val
            .as_string()
            .ok_or_else(|| E_TYPE.msg("Flags must be a string"))?;
        let flags = parse_verb_flags(flags_str)?;

        let argspec_val = list.index(4)?;
        let argspec_list = argspec_val
            .as_list()
            .ok_or_else(|| E_TYPE.msg("Argspec must be a list"))?;
        let argspec = parse_verb_argspec(argspec_list)?;

        let _program_str = list
            .index(5)?
            .as_string()
            .ok_or_else(|| E_TYPE.msg("Program must be a string"))?;
        // TODO: Actually compile the program - for now use empty MooR program
        let program = ProgramType::MooR(moor_compiler::Program::new());

        return Ok(ObjectMutation::DefineVerb {
            names,
            owner,
            flags,
            argspec,
            program,
        });
    }

    if action_sym == *DELETE_VERB_SYM {
        if list.len() != 2 {
            return Err(E_ARGS.msg("delete_verb requires 2 elements: action, names"));
        }
        let names_val = list.index(1)?;
        let names_list = names_val
            .as_list()
            .ok_or_else(|| E_TYPE.msg("Verb names must be a list"))?;
        let mut names = Vec::new();
        for name_var in names_list.iter() {
            names.push(name_var.as_symbol()?);
        }
        return Ok(ObjectMutation::DeleteVerb { names });
    }

    if action_sym == *UPDATE_VERB_PROGRAM_SYM {
        if list.len() != 3 {
            return Err(
                E_ARGS.msg("update_verb_program requires 3 elements: action, names, program")
            );
        }
        let names_val = list.index(1)?;
        let names_list = names_val
            .as_list()
            .ok_or_else(|| E_TYPE.msg("Verb names must be a list"))?;
        let mut names = Vec::new();
        for name_var in names_list.iter() {
            names.push(name_var.as_symbol()?);
        }

        let _program_str = list
            .index(2)?
            .as_string()
            .ok_or_else(|| E_TYPE.msg("Program must be a string"))?;
        // TODO: Actually compile the program
        let program = ProgramType::MooR(moor_compiler::Program::new());

        return Ok(ObjectMutation::UpdateVerbProgram { names, program });
    }

    if action_sym == *UPDATE_VERB_METADATA_SYM {
        if list.len() != 6 {
            return Err(E_ARGS.msg("update_verb_metadata requires 6 elements: action, names, new_names, owner, flags, argspec"));
        }
        let names_val = list.index(1)?;
        let names_list = names_val
            .as_list()
            .ok_or_else(|| E_TYPE.msg("Verb names must be a list"))?;
        let mut names = Vec::new();
        for name_var in names_list.iter() {
            names.push(name_var.as_symbol()?);
        }

        let new_names_var = list.index(2)?;
        let new_names = if new_names_var.variant() == &Variant::Int(0) {
            None
        } else {
            let new_names_list = new_names_var
                .as_list()
                .ok_or_else(|| E_TYPE.msg("New verb names must be a list or 0"))?;
            let mut nn = Vec::new();
            for name_var in new_names_list.iter() {
                nn.push(name_var.as_symbol()?);
            }
            Some(nn)
        };

        let owner_var = list.index(3)?;
        let owner = if owner_var.variant() == &Variant::Int(0) {
            None
        } else {
            Some(
                owner_var
                    .as_object()
                    .ok_or_else(|| E_TYPE.msg("Owner must be an object or 0"))?,
            )
        };

        let flags_var = list.index(4)?;
        let flags = if flags_var.variant() == &Variant::Int(0) {
            None
        } else {
            let flags_str = flags_var
                .as_string()
                .ok_or_else(|| E_TYPE.msg("Flags must be a string or 0"))?;
            Some(parse_verb_flags(flags_str)?)
        };

        let argspec_var = list.index(5)?;
        let argspec = if argspec_var.variant() == &Variant::Int(0) {
            None
        } else {
            let argspec_list = argspec_var
                .as_list()
                .ok_or_else(|| E_TYPE.msg("Argspec must be a list or 0"))?;
            Some(parse_verb_argspec(argspec_list)?)
        };

        return Ok(ObjectMutation::UpdateVerbMetadata {
            names,
            new_names,
            owner,
            flags,
            argspec,
        });
    }

    // Object lifecycle operations
    if action_sym == *CREATE_OBJECT_SYM {
        // create_object: {action, objid_or_0, parent, location, owner, flags}
        if list.len() != 6 {
            return Err(E_ARGS.msg(
                "create_object requires 6 elements: action, objid, parent, location, owner, flags",
            ));
        }

        let objid_val = list.index(1)?;
        let objid = if objid_val.variant() == &Variant::Int(0) {
            None
        } else {
            Some(
                objid_val
                    .as_object()
                    .ok_or_else(|| E_TYPE.msg("Object ID must be an object or 0"))?,
            )
        };

        let parent = list
            .index(2)?
            .as_object()
            .ok_or_else(|| E_TYPE.msg("Parent must be an object"))?;
        let location = list
            .index(3)?
            .as_object()
            .ok_or_else(|| E_TYPE.msg("Location must be an object"))?;
        let owner = list
            .index(4)?
            .as_object()
            .ok_or_else(|| E_TYPE.msg("Owner must be an object"))?;
        let flags_val = list.index(5)?;
        let flags_str = flags_val
            .as_string()
            .ok_or_else(|| E_TYPE.msg("Flags must be a string"))?;
        let flags = parse_object_flags(flags_str)?;

        return Ok(ObjectMutation::CreateObject {
            objid,
            parent,
            location,
            owner,
            flags,
        });
    }

    if action_sym == *RECYCLE_OBJECT_SYM {
        if list.len() != 1 {
            return Err(E_ARGS.msg("recycle_object requires 1 element: action"));
        }
        return Ok(ObjectMutation::RecycleObject);
    }

    // Object attribute operations
    if action_sym == *SET_OBJECT_FLAGS_SYM {
        if list.len() != 2 {
            return Err(E_ARGS.msg("set_object_flags requires 2 elements: action, flags"));
        }
        let flags_val = list.index(1)?;
        let flags_str = flags_val
            .as_string()
            .ok_or_else(|| E_TYPE.msg("Flags must be a string"))?;
        let flags = parse_object_flags(flags_str)?;
        return Ok(ObjectMutation::SetObjectFlags { flags });
    }

    if action_sym == *SET_PARENT_SYM {
        if list.len() != 2 {
            return Err(E_ARGS.msg("set_parent requires 2 elements: action, parent"));
        }
        let parent = list
            .index(1)?
            .as_object()
            .ok_or_else(|| E_TYPE.msg("Parent must be an object"))?;
        return Ok(ObjectMutation::SetParent { parent });
    }

    if action_sym == *SET_LOCATION_SYM {
        if list.len() != 2 {
            return Err(E_ARGS.msg("set_location requires 2 elements: action, location"));
        }
        let location = list
            .index(1)?
            .as_object()
            .ok_or_else(|| E_TYPE.msg("Location must be an object"))?;
        return Ok(ObjectMutation::SetLocation { location });
    }

    Err(E_INVARG.msg(format!(
        "Unknown mutation action: {}",
        action_sym.as_string()
    )))
}

/// Parse a verb argspec list like {"this", "none", "this"} into VerbArgsSpec
fn parse_verb_argspec(argspec_list: &moor_var::List) -> Result<VerbArgsSpec, Error> {
    if argspec_list.len() != 3 {
        return Err(E_ARGS.msg("Verb argspec must have 3 elements: dobj, prep, iobj"));
    }

    let dobj_val = argspec_list.index(0)?;
    let dobj_str = dobj_val
        .as_string()
        .ok_or_else(|| E_TYPE.msg("dobj must be a string"))?;
    let prep_val = argspec_list.index(1)?;
    let prep_str = prep_val
        .as_string()
        .ok_or_else(|| E_TYPE.msg("prep must be a string"))?;
    let iobj_val = argspec_list.index(2)?;
    let iobj_str = iobj_val
        .as_string()
        .ok_or_else(|| E_TYPE.msg("iobj must be a string"))?;

    let dobj = ArgSpec::from_string(dobj_str)
        .ok_or_else(|| E_INVARG.msg(format!("Invalid dobj spec: {dobj_str}")))?;
    let prep = parse_preposition_spec(prep_str)
        .ok_or_else(|| E_INVARG.msg(format!("Invalid prep spec: {prep_str}")))?;
    let iobj = ArgSpec::from_string(iobj_str)
        .ok_or_else(|| E_INVARG.msg(format!("Invalid iobj spec: {iobj_str}")))?;

    Ok(VerbArgsSpec { dobj, prep, iobj })
}

/// Apply a batch of mutations to multiple objects. Wizard-only.
/// MOO: `map mutate_objects(list changelist)`
/// changelist format: {{obj, {mutation1, mutation2, ...}}, {obj2, {mutation1, ...}}, ...}
fn bf_mutate_objects(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("mutate_objects() requires 1 argument"),
        ));
    }

    let Some(changelist) = bf_args.args[0].as_list() else {
        return Err(BfErr::ErrValue(E_TYPE.msg(
            "mutate_objects() argument must be a list of {obj, mutations} pairs",
        )));
    };

    // Check permissions: wizard only
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    task_perms.check_wizard().map_err(world_state_bf_err)?;

    // Parse the changelist: {{obj, {mutation1, mutation2}}, {obj2, {mutation1}}, ...}
    let mut parsed_changelist = Vec::new();
    for entry_var in changelist.iter() {
        let Some(entry_list) = entry_var.as_list() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("Each changelist entry must be {obj, mutations}"),
            ));
        };

        if entry_list.len() != 2 {
            return Err(BfErr::ErrValue(E_ARGS.msg(
                "Each changelist entry must have exactly 2 elements: {obj, mutations}",
            )));
        }

        let target_val = entry_list.index(0).map_err(BfErr::ErrValue)?;
        let Some(target) = target_val.as_object() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("First element of changelist entry must be an object"),
            ));
        };

        let mutations_val = entry_list.index(1).map_err(BfErr::ErrValue)?;
        let Some(mutations_list) = mutations_val.as_list() else {
            return Err(BfErr::ErrValue(E_TYPE.msg(
                "Second element of changelist entry must be a list of mutations",
            )));
        };

        // Convert MOO mutations to internal ObjectMutation structs
        let mut mutations = Vec::new();
        for mutation_var in mutations_list.iter() {
            let mutation = var_to_mutation(&mutation_var).map_err(BfErr::ErrValue)?;
            mutations.push(mutation);
        }

        parsed_changelist.push((target, mutations));
    }

    // Use the current task's transaction via loader interface to apply mutations
    let results = with_loader_interface(|loader| {
        let mut batch_results = Vec::new();

        for (target, mutations) in parsed_changelist {
            let result = moor_common::model::loader::batch_mutate(loader, &target, &mutations);
            batch_results.push(result);
        }

        Ok(batch_results)
    })
    .map_err(world_state_bf_err)?;

    // Format the return value as a list of per-object results
    // Each entry: {`target -> obj, `success -> 0/1, `results -> {{`index, `success, `error?}, ...}}
    let mut object_results = Vec::new();
    for batch_result in results.iter() {
        let mut mutation_results = Vec::new();
        for mutation_result in batch_result.results.iter() {
            let success = mutation_result.result.is_ok();
            let mut fields = vec![
                (
                    v_sym(Symbol::mk("index")),
                    v_int((mutation_result.index + 1) as i64),
                ),
                (v_sym(Symbol::mk("success")), bf_args.v_bool(success)),
            ];

            if let Err(ref error) = mutation_result.result {
                // Convert WorldStateError to MOO Error and then to Var
                let moo_error = error.to_error();
                fields.push((v_sym(Symbol::mk("error")), v_error(moo_error)));
            }

            mutation_results.push(v_map(&fields));
        }

        let obj_success = batch_result.all_succeeded();
        let obj_result = v_map(&[
            (v_sym(Symbol::mk("target")), v_obj(batch_result.target)),
            (v_sym(Symbol::mk("success")), bf_args.v_bool(obj_success)),
            (v_sym(Symbol::mk("results")), v_list(&mutation_results)),
        ]);
        object_results.push(obj_result);
    }

    // Return the result - mutations are already in the current transaction
    // No need for TaskSuspend::Commit since we used the same transaction
    Ok(Ret(v_list(&object_results)))
}

pub(crate) fn register_bf_obj_load(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("dump_object")] = Box::new(bf_dump_object);
    builtins[offset_for_builtin("load_object")] = Box::new(bf_load_object);
    builtins[offset_for_builtin("mutate_objects")] = Box::new(bf_mutate_objects);
}
