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

//! Built-in functions for object manipulation and hierarchy management.

use lazy_static::lazy_static;
use std::collections::HashMap;
use tracing::{debug, error, trace};

use moor_common::matching::{CommandParser, DefaultParseCommand, ObjectNameMatcher, Preposition};
use moor_common::model::{ObjSet, PrepSpec, verb_perms_string};
use moor_common::{
    model::{Named, ObjFlag, ObjectKind, ValSet, WorldStateError},
    util::BitEnum,
};
use moor_compiler::offset_for_builtin;
use moor_var::{
    Associative, E_ARGS, E_INVARG, E_NACC, E_PERM, E_TYPE, List, NOTHING, Obj, Sequence, Symbol,
    Variant, v_arc_string, v_bool, v_int, v_list, v_list_iter, v_map_iter, v_obj, v_str, v_string,
    v_sym,
};

use crate::{
    task_context::{with_current_transaction, with_current_transaction_mut},
    vm::{
        VerbExecutionRequest,
        builtins::{
            BfCallState, BfErr, BfRet,
            BfRet::{Ret, RetNil, VmInstr},
            BuiltinFunction, world_state_bf_err,
        },
        vm_host::ExecutionResult::DispatchVerb,
    },
};

lazy_static! {
    static ref INITIALIZE_SYM: Symbol = Symbol::mk("initialize");
    static ref EXITFUNC_SYM: Symbol = Symbol::mk("exitfunc");
    static ref ENTERFUNC_SYM: Symbol = Symbol::mk("enterfunc");
    static ref CREATE_SYM: Symbol = Symbol::mk("create");
    static ref RECYCLE_SYM: Symbol = Symbol::mk("recycle");
    static ref ACCEPT_SYM: Symbol = Symbol::mk("accept");
}

/// Helper function to create an object and call its :initialize verb if it exists.
/// Used by both `create` and `create_at` functions.
fn create_object_with_initialize(
    bf_args: &mut BfCallState<'_>,
    parent: &Obj,
    owner: &Obj,
    init_args: Option<&List>,
    obj_kind: ObjectKind,
) -> Result<BfRet, BfErr> {
    let new_obj = with_current_transaction_mut(|ws| {
        ws.create_object(
            &bf_args.task_perms_who(),
            parent,
            owner,
            BitEnum::new(),
            obj_kind,
        )
    })
    .map_err(world_state_bf_err)?;

    // Try to call :initialize on the new object
    let Ok((program, resolved_verb)) = with_current_transaction(|world_state| {
        world_state.find_method_verb_on(&bf_args.task_perms_who(), &new_obj, *INITIALIZE_SYM)
    }) else {
        return Ok(Ret(v_obj(new_obj)));
    };

    let bf_frame = bf_args.bf_frame_mut();
    bf_frame.bf_trampoline = Some(BF_CREATE_OBJECT_TRAMPOLINE_DONE);
    bf_frame.bf_trampoline_arg = Some(v_obj(new_obj));

    let initialize_args = if let Some(init_args) = init_args {
        init_args.clone()
    } else {
        List::mk_list(&[])
    };

    let ve = VerbExecutionRequest {
        permissions: bf_args.task_perms_who(),
        resolved_verb,
        verb_name: *INITIALIZE_SYM,
        this: v_obj(new_obj),
        player: bf_args.exec_state.top().player,
        args: initialize_args,
        caller: bf_args.exec_state.top().this.clone(),
        argstr: "".to_string(),
        program,
    };
    Ok(VmInstr(DispatchVerb(Box::new(ve))))
}
/// Returns true if object is a valid object (created and not yet recycled).
/// MOO: `int valid(obj object)`
fn bf_valid(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("valid() takes 1 argument")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("valid() first argument must be an object"),
        ));
    };
    let is_valid = with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?;
    Ok(Ret(bf_args.v_bool(is_valid)))
}

/// Returns the parent of the given object.
/// MOO: `obj parent(obj object)`
fn bf_parent(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("parent() takes 1 argument")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("parent() first argument must be an object"),
        ));
    };
    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("parent() argument must be a valid object"),
        ));
    }
    let parent = with_current_transaction(|world_state| {
        world_state.parent_of(&bf_args.task_perms_who(), &obj)
    })
    .map_err(world_state_bf_err)?;
    Ok(Ret(v_obj(parent)))
}

/// Changes the parent of object to be new-parent.
/// MOO: `none chparent(obj object, obj new-parent)`
fn bf_chparent(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(E_ARGS.msg("chparent() takes 2 arguments")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("chparent() first argument must be an object"),
        ));
    };
    let Some(new_parent) = bf_args.args[1].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("chparent() second argument must be an object"),
        ));
    };

    // If object is not valid, or if new-parent is neither valid nor equal to #-1, then E_INVARG is raised.
    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
        || !(new_parent.is_nothing()
            || with_current_transaction(|world_state| world_state.valid(&new_parent))
                .map_err(world_state_bf_err)?)
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("chparent() arguments must be valid objects"),
        ));
    }

    with_current_transaction_mut(|world_state| {
        world_state.change_parent(&bf_args.task_perms_who(), &obj, &new_parent)
    })
    .map_err(world_state_bf_err)?;
    Ok(RetNil)
}

/// Returns a list of the children of object.
/// MOO: `list children(obj object)`
fn bf_children(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("children() takes 1 argument")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("children() first argument must be an object"),
        ));
    };
    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("children() argument must be a valid object"),
        ));
    }
    let children = with_current_transaction(|world_state| {
        world_state.children_of(&bf_args.task_perms_who(), &obj)
    })
    .map_err(world_state_bf_err)?;

    let children = children.iter().map(v_obj).collect::<Vec<_>>();
    Ok(Ret(v_list(&children)))
}

/// Returns a list of all nested children of object.
/// MOO: `list descendants(obj object)`
fn bf_descendants(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("descendants() takes 1 argument"),
        ));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("descendants() first argument must be an object"),
        ));
    };
    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("descendants() argument must be a valid object"),
        ));
    }
    let descendants = with_current_transaction(|world_state| {
        world_state.descendants_of(&bf_args.task_perms_who(), &obj, false)
    })
    .map_err(world_state_bf_err)?;

    let descendants = descendants.iter().map(v_obj).collect::<Vec<_>>();
    Ok(Ret(v_list(&descendants)))
}

/// Returns a list of all ancestors of object in order ascending up the inheritance hierarchy.
/// MOO: `list ancestors(obj object [, int full])`
fn bf_ancestors(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 2 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("ancestors() takes 1 or 2 arguments"),
        ));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("ancestors() first argument must be an object"),
        ));
    };
    let add_self = if bf_args.args.len() == 2 {
        bf_args.args[1].is_true()
    } else {
        false
    };

    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("ancestors() argument must be a valid object"),
        ));
    }
    let ancestors = with_current_transaction(|world_state| {
        world_state.ancestors_of(&bf_args.task_perms_who(), &obj, add_self)
    })
    .map_err(world_state_bf_err)?;

    let ancestors = ancestors.iter().map(v_obj).collect::<Vec<_>>();
    Ok(Ret(v_list(&ancestors)))
}

/// Returns true if object is a descendant of possible_ancestor.
/// MOO: `int isa(obj object, obj parent)`
fn bf_isa(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(E_ARGS.msg("isa() takes 2 arguments")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("isa() first argument must be an object"),
        ));
    };
    let Some(possible_ancestor) = bf_args.args[1].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("isa() second argument must be an object"),
        ));
    };

    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
        || !with_current_transaction(|world_state| world_state.valid(&possible_ancestor))
            .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("isa() arguments must be valid objects"),
        ));
    }

    let ancestors = with_current_transaction(|world_state| {
        world_state.ancestors_of(&bf_args.task_perms_who(), &obj, true)
    })
    .map_err(world_state_bf_err)?;

    let isa = ancestors.contains(possible_ancestor);

    Ok(Ret(v_bool(isa)))
}

/// Recursively builds a list of an object's location, its location's location, and so forth until
/// hitting #nothing. If stop is provided, it stops before that object. If is-parent is true, stop
/// is treated as a parent and stops when any location is a child of that parent.
/// MOO: `list locations(obj object [, obj stop [, int is-parent]])`
fn bf_locations(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("locations() takes 1 to 3 arguments"),
        ));
    }

    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("locations() first argument must be an object"),
        ));
    };

    let stop_obj = if bf_args.args.len() >= 2 {
        let Some(stop) = bf_args.args[1].as_object() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("locations() second argument must be an object"),
            ));
        };
        Some(stop)
    } else {
        None
    };

    let is_parent = if bf_args.args.len() == 3 {
        bf_args.args[2].is_true()
    } else {
        false
    };

    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("locations() argument must be a valid object"),
        ));
    }

    let mut locations = Vec::new();
    let mut current = obj;

    loop {
        // Get the location of the current object
        let location = with_current_transaction(|world_state| {
            world_state.location_of(&bf_args.task_perms_who(), &current)
        })
        .map_err(world_state_bf_err)?;

        // Stop if we've hit #nothing
        if location.is_nothing() {
            break;
        }

        // Handle stop conditions before adding to the list
        if let Some(stop) = stop_obj
            && !is_parent
        {
            // Simple equality check - stop before adding this location
            if location == stop {
                break;
            }
        }

        // Add this location to our list
        locations.push(v_obj(location));

        // Handle is_parent stop condition after adding to the list
        if let Some(stop) = stop_obj
            && is_parent
        {
            // If is_parent is true, check if location is a child of stop
            let ancestors = with_current_transaction(|world_state| {
                world_state.ancestors_of(&bf_args.task_perms_who(), &location, false)
            })
            .map_err(world_state_bf_err)?;
            if ancestors.contains(stop) {
                break;
            }
        }

        current = location;
    }

    Ok(Ret(v_list(&locations)))
}

const BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE: usize = 0;
const BF_CREATE_OBJECT_TRAMPOLINE_DONE: usize = 1;

/// Creates and returns a new object whose parent is parent and whose owner is as described below.
/// MOO: `obj create(obj parent [, obj owner] [, int obj_type] [, list init_args])`
/// obj_type: 0=numbered, 1=anonymous, 2=UUID
/// Also accepts boolean for backward compatibility: false=numbered, true=anonymous
fn bf_create(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 4 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("create() takes 1 to 4 arguments"),
        ));
    }
    let Some(parent) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("create() first argument must be an object"),
        ));
    };
    let owner = if bf_args.args.len() >= 2 {
        let Some(owner) = bf_args.args[1].as_object() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("create() second argument must be an object"),
            ));
        };
        owner
    } else {
        bf_args.task_perms_who()
    };

    // Third argument is object type: 0=numbered, 1=anon (unsupported), 2=uuid
    // For backward compatibility, also accept boolean (false=numbered, true=anon which is unsupported)
    let obj_kind = if bf_args.args.len() >= 3 {
        let arg = &bf_args.args[2];

        let obj_type = match arg.variant() {
            Variant::Int(i) => *i,
            Variant::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            _ => {
                return Err(BfErr::ErrValue(
                    E_TYPE.msg("create() third argument must be an integer or bool"),
                ));
            }
        };

        // Convert integer to ObjectKind, validating as we go
        match obj_type {
            0 => ObjectKind::NextObjid,
            1 => {
                if !bf_args.config.anonymous_objects {
                    return Err(BfErr::ErrValue(E_INVARG.msg(
                        "Anonymous objects not available (anonymous_objects feature is disabled)",
                    )));
                }
                ObjectKind::Anonymous
            }
            2 => {
                if !bf_args.config.use_uuobjids {
                    return Err(BfErr::ErrValue(
                        E_INVARG.msg("UUID objects not available (use_uuobjids is false)"),
                    ));
                }
                ObjectKind::UuObjId
            }
            _ => {
                return Err(BfErr::ErrValue(E_INVARG.msg(
                    "create() object type must be 0 (numbered), 1 (anonymous), or 2 (UUID)",
                )));
            }
        }
    } else {
        // No object type specified, use default based on config
        if bf_args.config.use_uuobjids {
            ObjectKind::UuObjId
        } else {
            ObjectKind::NextObjid
        }
    };

    // Fourth argument is "init-args" - must be a list if provided
    let init_args = if bf_args.args.len() == 4 {
        let Some(init_args) = bf_args.args[3].as_list() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("create() fourth argument must be a list"),
            ));
        };
        Some(init_args)
    } else {
        None
    };

    let tramp = bf_args
        .bf_frame_mut()
        .bf_trampoline
        .take()
        .unwrap_or(BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE);

    match tramp {
        BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE => {
            create_object_with_initialize(bf_args, &parent, &owner, init_args, obj_kind)
        }
        BF_CREATE_OBJECT_TRAMPOLINE_DONE => {
            let Some(new_obj) = bf_args.bf_frame().bf_trampoline_arg.clone() else {
                panic!("Missing/invalid trampoline argument for bf_create");
            };
            Ok(Ret(new_obj))
        }
        _ => {
            panic!("Invalid trampoline for bf_create {tramp}")
        }
    }
}

/// Creates and returns a new object at the specified object ID. This function is wizard-only.
/// MOO: `obj create_at(obj id, obj parent [, obj owner] [, list init-args])`
fn bf_create_at(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("create_at() takes 2 to 4 arguments"),
        ));
    }

    // First argument is the object ID to create at
    let Some(obj_id) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("create_at() first argument must be an object"),
        ));
    };

    // create_at only accepts numeric object IDs, not UUID-based ones
    if obj_id.is_uuobjid() {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("create_at() requires a numeric object ID"),
        ));
    }

    // Second argument is parent
    let Some(parent) = bf_args.args[1].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("create_at() second argument must be an object"),
        ));
    };

    // Third argument is owner (optional)
    let owner = if bf_args.args.len() >= 3 {
        let Some(owner) = bf_args.args[2].as_object() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("create_at() third argument must be an object"),
            ));
        };
        owner
    } else {
        bf_args.task_perms_who()
    };

    // Fourth argument is "init-args" - must be a list if provided
    let init_args = if bf_args.args.len() == 4 {
        let Some(init_args) = bf_args.args[3].as_list() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("create_at() fourth argument must be a list"),
            ));
        };
        Some(init_args)
    } else {
        None
    };

    // create_at is wizard-only
    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    task_perms.check_wizard().map_err(world_state_bf_err)?;

    let tramp = bf_args
        .bf_frame_mut()
        .bf_trampoline
        .take()
        .unwrap_or(BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE);

    match tramp {
        BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE => create_object_with_initialize(
            bf_args,
            &parent,
            &owner,
            init_args,
            ObjectKind::Objid(obj_id),
        ),
        BF_CREATE_OBJECT_TRAMPOLINE_DONE => {
            let Some(new_obj) = bf_args.bf_frame().bf_trampoline_arg.clone() else {
                panic!("Missing/invalid trampoline argument for bf_create_at");
            };
            Ok(Ret(new_obj))
        }
        _ => {
            panic!("Invalid trampoline for bf_create_at {tramp}")
        }
    }
}

/// The given object is destroyed, irrevocably. The programmer must either own object or be a wizard; otherwise, E_PERM is raised.
/// If object is not valid, then E_INVARG is raised. The children of object are reparented to the parent of object.
/// Before object is recycled, each object in its contents is moved to #-1 (implying a call to object's exitfunc verb, if any)
/// and then object's `recycle' verb, if any, is called with no arguments.
/// MOO: `none recycle(obj object)`
// This is invoked with a list of objects to move/call :exitfunc on. When the list is empty, the
// next trampoline is called, to do the actual recycle.
const BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC: usize = 0;
// Do the recycle.
const BF_RECYCLE_TRAMPOLINE_DONE_MOVE: usize = 1;
fn bf_recycle(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("recycle() takes 1 argument")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("recycle() first argument must be an object"),
        ));
    };

    if obj.is_anonymous() {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("cannot recycle() anonymous objects"),
        ));
    }

    let valid = with_current_transaction(|world_state| world_state.valid(&obj));
    if valid == Ok(false)
        || valid
            .err()
            .map(|e| e.database_error_msg() == Some("NotFound"))
            .unwrap_or_default()
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("recycle() argument must be a valid object"),
        ));
    }

    // Check if the given task perms can control the object before continuing.
    if !with_current_transaction(|world_state| {
        world_state.controls(&bf_args.task_perms_who(), &obj)
    })
    .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(E_PERM.msg("recycle() permission denied")));
    }

    // Before actually recycling the object, we need to move all its contents to #-1. While
    // `recycle_object` will actually do this, we need to make sure :exitfunc is called on each
    // object, so we'll do it manually here.

    'outer: loop {
        let tramp = bf_args.bf_frame_mut().bf_trampoline.take();
        match tramp {
            None => {
                // Starting out, we need to call "recycle" on the object, if it exists.
                // The next point in the trampoline is CALL_EXITFUNC and it will expect a list of
                // objects to move/call :exitfunc on. So let's get the initial list of objects
                // now
                let object_contents = with_current_transaction(|world_state| {
                    world_state.contents_of(&bf_args.task_perms_who(), &obj)
                })
                .map_err(world_state_bf_err)?;
                // Filter contents for objects that have an :exitfunc verb.
                let mut contents = vec![];
                for o in object_contents.iter() {
                    match with_current_transaction(|world_state| {
                        world_state.find_method_verb_on(
                            &bf_args.task_perms_who(),
                            &o,
                            *EXITFUNC_SYM,
                        )
                    }) {
                        Ok(_) => {
                            contents.push(v_obj(o));
                        }
                        Err(WorldStateError::VerbNotFound(_, _)) => {}
                        Err(e) => {
                            error!("Error looking up exitfunc verb: {:?}", e);
                            return Err(BfErr::ErrValue(
                                E_NACC.msg("recycle() error looking up exitfunc"),
                            ));
                        }
                    }
                }
                let contents = v_list(&contents);
                match with_current_transaction(|world_state| {
                    world_state.find_method_verb_on(&bf_args.task_perms_who(), &obj, *RECYCLE_SYM)
                }) {
                    Ok((program, resolved_verb)) => {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline = Some(BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC);
                        bf_frame.bf_trampoline_arg = Some(contents);

                        return Ok(VmInstr(DispatchVerb(Box::new(VerbExecutionRequest {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb,
                            verb_name: *RECYCLE_SYM,
                            this: v_obj(obj),
                            player: bf_args.exec_state.top().player,
                            args: List::mk_list(&[]),
                            caller: bf_args.exec_state.top().this.clone(),
                            argstr: "".to_string(),
                            program,
                        }))));
                    }
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        // Short-circuit fake-tramp state change.
                        let bf_frame = bf_args.bf_frame_mut();

                        bf_frame.bf_trampoline = Some(BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC);
                        bf_frame.bf_trampoline_arg = Some(contents);
                        // Fall through to the next case.
                    }
                    Err(_) => {
                        return Err(BfErr::ErrValue(
                            E_NACC.msg("recycle() error looking up recycle"),
                        ));
                    }
                }
            }
            Some(BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC) => {
                // Check the arguments, which must be a list of objects. IF it's empty, we can
                // move onto DONE_MOVE, if not, take the head of the list, and call :exitfunc on it
                // (if it exists), and then back to this state.

                let contents = bf_args.bf_frame().bf_trampoline_arg.clone().unwrap();
                let Some(contents) = contents.as_list() else {
                    panic!("Invalid trampoline argument for bf_recycle");
                };
                'inner: loop {
                    if contents.is_empty() {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline_arg = None;
                        bf_frame.bf_trampoline = Some(BF_RECYCLE_TRAMPOLINE_DONE_MOVE);
                        continue 'outer;
                    }
                    let (head_obj, contents) =
                        contents.pop_front().map_err(|_| BfErr::Code(E_INVARG))?;
                    let Some(head_obj) = head_obj.as_object() else {
                        panic!("Invalid trampoline argument for bf_recycle");
                    };
                    // :exitfunc *should* exist because we looked for it earlier, and we're supposed to
                    // be transactionally isolated. But we need to do resolution anyways, so we will
                    // look again anyways.
                    let Ok((program, resolved_verb)) = with_current_transaction(|world_state| {
                        world_state.find_method_verb_on(
                            &bf_args.task_perms_who(),
                            &head_obj,
                            *EXITFUNC_SYM,
                        )
                    }) else {
                        // If there's no :exitfunc, we can just move on to the next object.
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline_arg = Some(contents);
                        continue 'inner;
                    };
                    let bf_frame = bf_args.bf_frame_mut();
                    bf_frame.bf_trampoline_arg = Some(contents);
                    bf_frame.bf_trampoline = Some(BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC);

                    // Call :exitfunc on the head object.
                    return Ok(VmInstr(DispatchVerb(Box::new(VerbExecutionRequest {
                        permissions: bf_args.task_perms_who(),
                        resolved_verb,
                        verb_name: *EXITFUNC_SYM,
                        this: v_obj(head_obj),
                        player: bf_args.exec_state.top().player,
                        args: List::mk_list(&[v_obj(obj)]),
                        caller: bf_args.exec_state.top().this.clone(),
                        argstr: "".to_string(),
                        program,
                    }))));
                }
            }
            Some(BF_RECYCLE_TRAMPOLINE_DONE_MOVE) => {
                debug!(obj = ?obj, "Recycling object");
                with_current_transaction_mut(|world_state| {
                    world_state.recycle_object(&bf_args.task_perms_who(), &obj)
                })
                .map_err(world_state_bf_err)?;
                return Ok(Ret(v_int(0)));
            }
            Some(unknown) => {
                panic!("Invalid trampoline for bf_recycle {unknown}")
            }
        }
    }
}

/// Returns the largest object number ever assigned to a created object.
/// MOO: `obj max_object()`
fn bf_max_object(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("max_object() takes no arguments"),
        ));
    }
    let max_obj =
        with_current_transaction(|world_state| world_state.max_object(&bf_args.task_perms_who()))
            .map_err(world_state_bf_err)?;
    Ok(Ret(v_obj(max_obj)))
}

const BF_MOVE_TRAMPOLINE_START_ACCEPT: usize = 0;
const BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC: usize = 1;
const BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC: usize = 2;
const BF_MOVE_TRAMPOLINE_DONE: usize = 3;

/// Changes what's location to be where.
/// MOO: `none move(obj what, obj where)`
fn bf_move(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(E_ARGS.msg("move() takes 2 arguments")));
    }
    let Some(what) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("move() first argument must be an object"),
        ));
    };
    let Some(whereto) = bf_args.args[1].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("move() second argument must be an object"),
        ));
    };

    // World state will reject this move if it's recursive.
    //   If the destination is self, E_RECMOVE
    //   If the destination is something that's inside me, that's also E_RECMOVE
    //   And so on...

    // 'Trampoline' state machine:
    //    None => look up :accept, if it exists, set tramp to 1, and ask for it to be invoked.
    //            if it doesn't & perms not wizard, set raise E_NACC (as if :accept returned false)
    //            If the destination is #-1 (NOTHING), we can skip straight through to 1/.
    //    1    => if verb call was a success (look at stack), set tramp to 2, move the object,
    //            then prepare :exitfunc on the original object source
    //            if :exitfunc doesn't exist, proceed to 3 (enterfunc)
    //    2    => set tramp to 3, call :enterfunc on the destination if it exists, result is ignored.
    //    3    => return out (retnil)

    let bf_frame = bf_args.bf_frame_mut();
    let mut tramp = bf_frame
        .bf_trampoline
        .take()
        .unwrap_or(BF_MOVE_TRAMPOLINE_START_ACCEPT);
    trace!(what = ?what, where_to = ?whereto, tramp, "move: looking up :accept verb");

    let perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    let mut shortcircuit = None;
    loop {
        match tramp {
            BF_MOVE_TRAMPOLINE_START_ACCEPT => {
                if whereto.is_nothing() {
                    shortcircuit = Some(1);
                    tramp = BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC;
                    continue;
                }
                match with_current_transaction(|world_state| {
                    world_state.find_method_verb_on(
                        &bf_args.task_perms_who(),
                        &whereto,
                        *ACCEPT_SYM,
                    )
                }) {
                    Ok((program, resolved_verb)) => {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline = Some(BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC);
                        bf_frame.bf_trampoline_arg = None;
                        return Ok(VmInstr(DispatchVerb(Box::new(VerbExecutionRequest {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb,
                            verb_name: *ACCEPT_SYM,
                            this: v_obj(whereto),
                            player: bf_args.exec_state.top().player,
                            args: List::mk_list(&[v_obj(what)]),
                            caller: bf_args.exec_state.top().this.clone(),
                            argstr: "".to_string(),
                            program,
                        }))));
                    }
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        if !perms.check_is_wizard().map_err(world_state_bf_err)? {
                            return Err(BfErr::Code(E_NACC));
                        }
                        // Short-circuit fake-tramp state change.
                        tramp = BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC;
                        shortcircuit = Some(0);
                        continue;
                    }
                    Err(e) => {
                        error!("Error looking up accept verb: {:?}", e);
                        return Err(BfErr::Code(E_NACC));
                    }
                }
            }
            BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC => {
                trace!(what = ?what, where_to = ?whereto, tramp, "move: moving object, calling exitfunc");

                // Accept verb has been called, and returned. Check the result. Should be in our
                // activation's return-value.
                let result = match shortcircuit {
                    None => bf_args.exec_state.top().frame.return_value(),
                    Some(n) => v_int(n),
                };
                // If the result is false, and we're not a wizard, then raise E_NACC.
                if !result.is_true() && !perms.check_is_wizard().map_err(world_state_bf_err)? {
                    return Err(BfErr::Code(E_NACC));
                }

                // Otherwise, ask the world state to move the object.
                trace!(what = ?what, where_to = ?whereto, tramp, "move: moving object & calling enterfunc");

                let original_location = with_current_transaction(|world_state| {
                    world_state.location_of(&bf_args.task_perms_who(), &what)
                })
                .map_err(world_state_bf_err)?;

                // Failure here is likely due to permissions, so we'll just propagate that error.
                with_current_transaction_mut(|world_state| {
                    world_state.move_object(&bf_args.task_perms_who(), &what, &whereto)
                })
                .map_err(world_state_bf_err)?;

                // If the object has no location, then we can move on to the enterfunc.
                if original_location == NOTHING {
                    tramp = BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC;
                    continue;
                }

                // Call exitfunc...
                match with_current_transaction(|world_state| {
                    world_state.find_method_verb_on(
                        &bf_args.task_perms_who(),
                        &original_location,
                        *EXITFUNC_SYM,
                    )
                }) {
                    Ok((program, resolved_verb)) => {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline = Some(BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC);
                        bf_frame.bf_trampoline_arg = None;

                        let continuation = DispatchVerb(Box::new(VerbExecutionRequest {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb,
                            verb_name: *EXITFUNC_SYM,
                            this: v_obj(original_location),
                            player: bf_args.exec_state.top().player,
                            args: List::mk_list(&[v_obj(what)]),
                            caller: bf_args.exec_state.top().this.clone(),
                            argstr: "".to_string(),
                            program,
                        }));
                        return Ok(VmInstr(continuation));
                    }
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        // Short-circuit fake-tramp state change.
                        tramp = 2;
                        continue;
                    }
                    Err(e) => {
                        error!("Error looking up exitfunc verb: {:?}", e);
                        return Err(BfErr::Code(E_NACC));
                    }
                }
            }
            BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC => {
                if whereto == NOTHING {
                    tramp = BF_MOVE_TRAMPOLINE_DONE;
                    continue;
                }
                trace!(what = ?what, where_to = ?whereto, tramp, "move: calling enterfunc");

                // Exitfunc has been called, and returned. Result is irrelevant. Prepare to call
                // :enterfunc on the destination.
                match with_current_transaction(|world_state| {
                    world_state.find_method_verb_on(
                        &bf_args.task_perms_who(),
                        &whereto,
                        *ENTERFUNC_SYM,
                    )
                }) {
                    Ok((program, resolved_verb)) => {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline = Some(BF_MOVE_TRAMPOLINE_DONE);
                        bf_frame.bf_trampoline_arg = None;

                        return Ok(VmInstr(DispatchVerb(Box::new(VerbExecutionRequest {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb,
                            verb_name: *ENTERFUNC_SYM,
                            this: v_obj(whereto),
                            player: bf_args.exec_state.top().player,
                            args: List::mk_list(&[v_obj(what)]),
                            caller: bf_args.exec_state.top().this.clone(),
                            argstr: "".to_string(),
                            program,
                        }))));
                    }
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        // Short-circuit fake-tramp state change.
                        tramp = BF_MOVE_TRAMPOLINE_DONE;
                        continue;
                    }
                    Err(e) => {
                        error!("Error looking up enterfunc verb: {:?}", e);
                        return Err(BfErr::Code(E_NACC));
                    }
                }
            }
            BF_MOVE_TRAMPOLINE_DONE => {
                trace!(what = ?what, where_to = ?whereto, tramp, "move: completed");

                // Enter func was called, and returned. Result is irrelevant. We're done.
                // Return v_none.
                return Ok(RetNil);
            }
            _ => {
                panic!("Invalid trampoline state: {tramp} in bf_move");
            }
        }
    }
}

/// Returns a list of the names of the verbs defined directly on the given object, not inherited from its parent.
/// MOO: `list verbs(obj object)`
fn bf_verbs(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("verbs() takes 1 argument")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("verbs() first argument must be an object"),
        ));
    };
    let verbs =
        with_current_transaction(|world_state| world_state.verbs(&bf_args.task_perms_who(), &obj))
            .map_err(world_state_bf_err)?;
    let verbs: Vec<_> = verbs
        .iter()
        .map(|v| v_arc_string(v.names().first().unwrap().as_arc_string()))
        .collect();
    Ok(Ret(v_list(&verbs)))
}

/// Returns a list of the names of the properties defined directly on the given object, not inherited from its parent.
/// If object is not valid, then E_INVARG is raised. If the programmer does not have read permission on object, then E_PERM is raised.
/// MOO: `list properties(obj object)`
fn bf_properties(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("properties() takes 1 argument")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("properties() first argument must be an object"),
        ));
    };
    let props = with_current_transaction(|world_state| {
        world_state.properties(&bf_args.task_perms_who(), &obj)
    })
    .map_err(world_state_bf_err)?;
    let props: Vec<_> = if bf_args.config.use_symbols_in_builtins {
        props.iter().map(|p| v_sym(p.name())).collect()
    } else {
        props
            .iter()
            .map(|p| v_arc_string(p.name().as_arc_string()))
            .collect()
    };
    Ok(Ret(v_list(&props)))
}

/// Confers or removes the "player object" status of the given object, depending upon the truth value of value.
/// MOO: `none set_player_flag(obj object, value)`
fn bf_set_player_flag(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("set_player_flag() takes 2 arguments"),
        ));
    }

    let (Variant::Obj(obj), Variant::Int(f)) =
        (bf_args.args[0].variant(), bf_args.args[1].variant())
    else {
        return Err(BfErr::ErrValue(E_INVARG.msg(
            "set_player_flag() arguments must be an object and an integer",
        )));
    };

    let f = *f == 1;

    // User must be a wizard.
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    // Get and set object flags
    let mut flags = with_current_transaction(|world_state| world_state.flags_of(obj))
        .map_err(world_state_bf_err)?;

    if f {
        flags.set(ObjFlag::User);
    } else {
        flags.clear(ObjFlag::User);
    }

    with_current_transaction_mut(|world_state| {
        world_state.set_flags_of(&bf_args.task_perms_who(), obj, flags)
    })
    .map_err(world_state_bf_err)?;

    // If the object was player, update the VM's copy of the perms.
    if obj.eq(&bf_args.task_perms().map_err(world_state_bf_err)?.who) {
        bf_args.exec_state.set_task_perms(*obj);
    }

    Ok(RetNil)
}

/// Returns a list of the object numbers of all player objects in the database.
/// MOO: `list players()`
fn bf_players(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::ErrValue(E_ARGS.msg("players() takes no arguments")));
    }
    let players = with_current_transaction(|world_state| world_state.players())
        .map_err(world_state_bf_err)?;

    Ok(Ret(v_list_iter(players.iter().map(v_obj))))
}

/// Returns a list of all valid objects in the database. Wizard-only.
/// MOO: `list objects()`
fn bf_objects(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .check_wizard()
        .map_err(world_state_bf_err)?;

    if !bf_args.args.is_empty() {
        return Err(BfErr::ErrValue(E_ARGS.msg("objects() takes no arguments")));
    }

    let objects = with_current_transaction(|world_state| world_state.all_objects())
        .map_err(world_state_bf_err)?;

    Ok(Ret(v_list_iter(objects.iter().map(v_obj))))
}

/// Returns a list of objects owned by the given object. If owner is not valid, then E_INVARG is raised.
/// If the programmer does not have read permission on owner, then E_PERM is raised.
/// MOO: `list owned_objects(obj owner)`
fn bf_owned_objects(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("owned_objects() takes 1 argument"),
        ));
    }
    let Some(owner) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("owned_objects() first argument must be an object"),
        ));
    };
    if !with_current_transaction(|world_state| world_state.valid(&owner))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("owned_objects() argument must be a valid object"),
        ));
    }

    let owned_objects = with_current_transaction(|world_state| {
        world_state.owned_objects(&bf_args.task_perms_who(), &owner)
    })
    .map_err(world_state_bf_err)?;

    let owned_objects = owned_objects.iter().map(v_obj).collect::<Vec<_>>();
    Ok(Ret(v_list(&owned_objects)))
}

/// Renumber an object to a new object number
/// MOO: `obj renumber(obj)`
/// MOO: `obj renumber(obj, obj target)`
fn bf_renumber(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 1 || bf_args.args.len() > 2 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("renumber() takes 1 or 2 arguments"),
        ));
    }

    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("renumber() first argument must be an object"),
        ));
    };

    let target = if bf_args.args.len() == 2 {
        let Some(target_obj) = bf_args.args[1].as_object() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("renumber() second argument must be an object"),
            ));
        };
        Some(target_obj)
    } else {
        None
    };

    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("renumber() argument must be a valid object"),
        ));
    }

    // Check target object if provided - for validation we just check it's a valid object reference
    if let Some(_target) = &target {
        // We don't validate if target exists - that's handled by the renumber implementation
    }

    let task_perms = bf_args.task_perms().map_err(world_state_bf_err)?;

    // Only wizards can renumber objects
    task_perms.check_wizard().map_err(world_state_bf_err)?;

    // Call the world state renumber_object method
    let new_obj = with_current_transaction_mut(|world_state| {
        world_state.renumber_object(&task_perms.who, &obj, target.as_ref())
    })
    .map_err(world_state_bf_err)?;

    Ok(Ret(v_obj(new_obj)))
}

/// Determine if the given OBJ is an anonymous reference or "regular" object id
fn bf_is_anonymous(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("is_anonymous() takes 1 argument"),
        ));
    }

    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("is_anonymous() argument must be an object"),
        ));
    };

    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("is_anonymous() argument must be a valid object"),
        ));
    }

    Ok(Ret(v_bool(obj.is_anonymous())))
}

/// Determine if the given OBJ is a UUID-obj reference.
fn bf_is_uuobjid(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("is_uuobjid() takes 1 argument")));
    }

    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("is_uuobjid() argument must be an object"),
        ));
    };

    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("is_uuobjid() argument must be a valid object"),
        ));
    }

    Ok(Ret(v_bool(obj.is_uuobjid())))
}

/// Parse a command string and return its components as a map.
/// MOO: `map parse_command(str command, list environment, [bool complex])`
///
/// The environment is a list of objects and/or {object, names ... } entries to search for object name matching.
/// For example: `parse_command("look frobozicon", {player, player.location, {#666, "frob", "frobozzicon"}})`
/// returns `=> ["args" -> {"frobozicon"}, "argstr" -> "frobozicon", "dobj" -> #666,
///              "dobjstr" -> "frobozicon", "iobj" -> #-1, "iobjstr" -> "",
///              "prep" -> -1, "prepstr" -> "", "verb" -> "look"]`
///
/// Returns a map with the following keys:
/// - `verb`: The verb symbol (e.g., "look")
/// - `argstr`: The full argument string after the verb
/// - `args`: List of individual argument strings
/// - `dobjstr`: The direct object string that was matched (or empty string)
/// - `dobj`: The direct object found (or #-1 if none)
/// - `prepstr`: The preposition string (or empty string)
/// - `prep`: Integer representing the preposition (-2=any, -1=none, 0-14=specific prepositions)
/// - `iobjstr`: The indirect object string (or empty string)
/// - `iobj`: The indirect object found (or #-1 if none)
///
/// The third `complex` argument enables advanced matching features:
/// - When true, uses fuzzy matching (Levenshtein distance) for object names
/// - Handles ordinal descriptors ("first", "second", "third") to differentiate multiple matching objects
/// - Provides more flexible and forgiving object name matching
fn bf_parse_command(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("parse_command() takes 2 or 3 arguments"),
        ));
    }

    let Some(command_str) = bf_args.args[0].as_string() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("parse_command() first argument must be a string"),
        ));
    };

    let Some(environment_list) = bf_args.args[1].as_list() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("parse_command() second argument must be a list"),
        ));
    };

    let complex_match = bf_args.args.len() >= 3 && bf_args.args[2].is_true();
    let use_symbols = bf_args.config.use_symbols_in_builtins && bf_args.config.symbol_type;
    let mk_sym_or_str = |s: Symbol| {
        if use_symbols {
            v_sym(s)
        } else {
            v_str(s.as_arc_str().as_ref())
        }
    };
    let use_sym_or_str = |sym| {
        if use_symbols { v_sym(sym) } else { v_str(sym) }
    };

    struct DelegatingObjectMatcher(Box<dyn ObjectNameMatcher>);
    impl ObjectNameMatcher for DelegatingObjectMatcher {
        fn match_object(&self, name: &str) -> Result<Option<Obj>, WorldStateError> {
            self.0.match_object(name)
        }
    }

    // Create a custom MatchEnvironment that uses the provided environment list
    struct ListMatchEnvironment {
        who: Obj,
        name_map: std::collections::HashMap<Obj, Vec<String>>,
    }

    impl ListMatchEnvironment {
        pub fn new(who: Obj, name_map: HashMap<Obj, Vec<String>>) -> Result<Self, WorldStateError> {
            Ok(Self { who, name_map })
        }
    }

    impl moor_common::matching::MatchEnvironment for ListMatchEnvironment {
        fn obj_valid(&self, oid: &Obj) -> Result<bool, WorldStateError> {
            // Check if the object is in our name map (which means it's in our environment)
            Ok(self.name_map.contains_key(oid))
        }

        fn get_names(&self, oid: &Obj) -> Result<Vec<String>, WorldStateError> {
            // Check if we have custom names for this object
            if let Some(custom_names) = self.name_map.get(oid)
                && !custom_names.is_empty()
            {
                return Ok(custom_names.clone());
            }

            // Fall back to getting names from world state
            let names =
                with_current_transaction(|world_state| world_state.names_of(&self.who, oid))?;
            let mut result = vec![names.0];
            result.extend(names.1);
            Ok(result)
        }

        fn get_surroundings(&self, _player: &Obj) -> Result<ObjSet, WorldStateError> {
            // For our environment, return all objects from our name map
            let mut surroundings = ObjSet::default();
            for obj in self.name_map.keys() {
                surroundings = surroundings.with_inserted(*obj);
            }
            Ok(surroundings)
        }

        fn location_of(&self, _player: &Obj) -> Result<Obj, WorldStateError> {
            // We don't have a real location in this environment, return #-1
            Ok(NOTHING)
        }
    }

    // Process the environment list to build the name mapping
    let mut name_map = HashMap::new();
    for env_entry in environment_list.iter() {
        // Handle simple object entries
        if let Some(obj) = env_entry.as_object() {
            name_map.insert(obj, Vec::new());
            continue;
        }

        // Handle list entries: {obj: object, names: list_of_names}
        let Some(names) = env_entry.as_list() else {
            return Err(BfErr::ErrValue(
                E_INVARG.msg("invalid {obj, names ... } name entry"),
            ));
        };

        let Ok(obj) = names.index(0) else {
            return Err(BfErr::ErrValue(
                E_INVARG.msg("invalid {obj, names ... } name entry"),
            ));
        };

        let Some(obj) = obj.as_object() else {
            return Err(BfErr::ErrValue(
                E_INVARG.msg("invalid {obj, names ... } name entry"),
            ));
        };

        let Ok(obj_names) = names.remove_at(0) else {
            return Err(BfErr::ErrValue(
                E_INVARG.msg("invalid {obj, names ... } name entry"),
            ));
        };

        let Some(names_list) = obj_names.as_list() else {
            return Err(BfErr::ErrValue(
                E_INVARG.msg("invalid {obj, names ... } name entry"),
            ));
        };

        let mut names = vec![];
        for name_var in names_list.iter() {
            let Some(name_str) = name_var.as_string() else {
                continue;
            };
            names.push(name_str.to_string());
        }

        name_map.insert(obj, names);
    }
    let env = ListMatchEnvironment::new(bf_args.task_perms_who(), name_map).map_err(|e| {
        BfErr::ErrValue(
            E_INVARG.with_msg(|| format!("parse_command() error creating environment: {e}")),
        )
    })?;

    // Use the DefaultObjectNameMatcher with our custom environment
    let matcher: Box<dyn ObjectNameMatcher> = if complex_match {
        Box::new(moor_common::matching::ComplexObjectNameMatcher {
            env,
            player: bf_args.task_perms_who(),
        })
    } else {
        Box::new(moor_common::matching::DefaultObjectNameMatcher {
            env,
            player: bf_args.task_perms_who(),
        })
    };

    let matcher = DelegatingObjectMatcher(matcher);
    let parser = DefaultParseCommand::new();
    let parsed = parser.parse_command(command_str, &matcher).map_err(|e| {
        BfErr::ErrValue(E_INVARG.with_msg(|| format!("parse_command() error: {e}")))
    })?;

    // Convert the parsed command to a map
    let mut result_map = vec![];

    result_map.push((use_sym_or_str("verb"), mk_sym_or_str(parsed.verb)));
    result_map.push((v_str("argstr"), use_sym_or_str(&parsed.argstr)));
    result_map.push((use_sym_or_str("args"), v_list(&parsed.args)));

    // dobjstr: string or ""
    result_map.push((
        use_sym_or_str("dobjstr"),
        if let Some(ref s) = parsed.dobjstr {
            v_str(s)
        } else {
            v_str("")
        },
    ));

    // dobj: object or #-1
    result_map.push((
        use_sym_or_str("dobj"),
        if let Some(obj) = parsed.dobj {
            v_obj(obj)
        } else {
            v_obj(NOTHING)
        },
    ));

    // prepstr: string or ""
    result_map.push((
        use_sym_or_str("prepstr"),
        if let Some(ref s) = parsed.prepstr {
            v_str(s)
        } else {
            v_str("")
        },
    ));

    // prep: integer (0=none, 1=at/to, 2=in front of, 3=into/in, 4=on top of/on, 5=out of, 6=over, 7=through, 8=under, 9=behind, 10=beside, 11=for/about, 12=is, 13=as, 14=off/off of)
    let prep_value = match parsed.prep {
        PrepSpec::Any => -2,
        PrepSpec::None => -1,
        PrepSpec::Other(p) => p as i64,
    };
    result_map.push((v_str("prep"), v_int(prep_value)));

    // iobjstr: string or 0
    result_map.push((
        use_sym_or_str("iobjstr"),
        if let Some(ref s) = parsed.iobjstr {
            v_str(s)
        } else {
            v_str("")
        },
    ));

    // iobj: object or #-1
    result_map.push((
        use_sym_or_str("iobj"),
        if let Some(obj) = parsed.iobj {
            v_obj(obj)
        } else {
            v_obj(NOTHING)
        },
    ));

    Ok(Ret(v_map_iter(result_map.iter())))
}

/// Finds command verbs matching the parsed command specification (as returned from parse_command)
/// on the given environmental targets.
/// MOO: `list find_command_verb(map parsed_command_spec, list targets_to_search)`
///
/// This function searches for command verbs on the specified targets that match the parsed command
/// specification. It returns a list of [target_object, verb_info] pairs where:
/// - `target_object` is the object where the matching verb was found
/// - `verb_info` is a list [owner, permissions, verb_names] describing the verb
///
/// When used together with `parse_command`, these two functions emulate the behavior of the
/// built-in MOO command parser:
/// 1. First call `parse_command` to parse the command string into a specification
/// 2. Then call `find_command_verb` with the specification and target objects
/// 3. Dispatch to the found verb (if any) to execute the command
///
/// This allows custom command parsing and dispatching while maintaining compatibility with
/// the standard MOO command parsing logic.
fn bf_find_command_verb(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("find_command_verb() takes 2 arguments"),
        ));
    }

    let Some(parsed_command_spec) = bf_args.args[0].as_map() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("find_command_verb() first argument must be a map"),
        ));
    };

    let Some(targets_to_search) = bf_args.args[1].as_list() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("find_command_verb() second argument must be a list"),
        ));
    };

    // Extract parsed command components from the map
    let verb_sym = match parsed_command_spec.get(&v_str("verb")) {
        Ok(verb_var) => verb_var.as_symbol().map_err(|_| {
            BfErr::ErrValue(
                E_TYPE.msg("find_command_verb() parsed_command_spec.verb must be a symbol"),
            )
        })?,
        _ => {
            return Err(BfErr::ErrValue(
                E_INVARG.msg("find_command_verb() parsed_command_spec missing 'verb' key"),
            ));
        }
    };
    let use_symbols = bf_args.config.use_symbols_in_builtins && bf_args.config.symbol_type;
    let sym_or_str = |sym| {
        if use_symbols { v_sym(sym) } else { v_str(sym) }
    };
    let dobj = parsed_command_spec
        .get(&sym_or_str("dobj"))
        .ok()
        .and_then(|dobj_var| dobj_var.as_object())
        .unwrap_or(NOTHING);

    let prep = parsed_command_spec
        .get(&sym_or_str("prep"))
        .ok()
        .and_then(|prep_var| prep_var.as_integer())
        .map(|prep_int| {
            // Convert back from the integer representation used in parse_command
            match prep_int {
                -2 => PrepSpec::Any,
                -1 => PrepSpec::None,
                p => PrepSpec::Other(
                    Preposition::from_repr(p as u16).unwrap_or(Preposition::WithUsing),
                ),
            }
        })
        .unwrap_or(PrepSpec::None);

    let iobj = parsed_command_spec
        .get(&sym_or_str("iobj"))
        .ok()
        .and_then(|iobj_var| iobj_var.as_object())
        .unwrap_or(NOTHING);

    let mut matches = Vec::new();

    // Search for command verbs on each target
    for target_var in targets_to_search.iter() {
        let Some(target) = target_var.as_object() else {
            continue; // Skip non-object entries
        };

        // Check if target is valid
        if !with_current_transaction(|world_state| world_state.valid(&target))
            .map_err(world_state_bf_err)?
        {
            continue; // Skip invalid objects
        }

        // Look for command verb on this target
        let match_result = with_current_transaction(|world_state| {
            world_state.find_command_verb_on(
                &bf_args.task_perms_who(),
                &target,
                verb_sym,
                &dobj,
                prep,
                &iobj,
            )
        });

        match match_result {
            Ok(Some((_, verbdef))) => {
                let owner = verbdef.owner();
                let perms = verbdef.flags();
                let names = verbdef.names();

                let perms_string = verb_perms_string(perms);

                // Join names into a single string, this is how MOO presents it.
                let verb_names = names
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");

                let verb_info =
                    v_list(&[v_obj(owner), v_string(perms_string), v_string(verb_names)]);
                matches.push(v_list(&[v_obj(target), verb_info]));
            }
            Ok(None) => {
                // No match on this target, continue
            }
            Err(WorldStateError::VerbPermissionDenied)
            | Err(WorldStateError::ObjectPermissionDenied)
            | Err(WorldStateError::PropertyPermissionDenied) => {
                // Permission denied, skip this target
            }
            Err(e) => {
                error!("Error finding command verb: {:?}", e);
                // Continue with other targets
            }
        }
    }

    Ok(Ret(v_list(&matches)))
}

pub(crate) fn register_bf_objects(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("create")] = Box::new(bf_create);
    builtins[offset_for_builtin("create_at")] = Box::new(bf_create_at);
    builtins[offset_for_builtin("valid")] = Box::new(bf_valid);
    builtins[offset_for_builtin("verbs")] = Box::new(bf_verbs);
    builtins[offset_for_builtin("properties")] = Box::new(bf_properties);
    builtins[offset_for_builtin("parent")] = Box::new(bf_parent);
    builtins[offset_for_builtin("children")] = Box::new(bf_children);
    builtins[offset_for_builtin("ancestors")] = Box::new(bf_ancestors);
    builtins[offset_for_builtin("isa")] = Box::new(bf_isa);
    builtins[offset_for_builtin("descendants")] = Box::new(bf_descendants);
    builtins[offset_for_builtin("move")] = Box::new(bf_move);
    builtins[offset_for_builtin("chparent")] = Box::new(bf_chparent);
    builtins[offset_for_builtin("set_player_flag")] = Box::new(bf_set_player_flag);
    builtins[offset_for_builtin("recycle")] = Box::new(bf_recycle);
    builtins[offset_for_builtin("max_object")] = Box::new(bf_max_object);
    builtins[offset_for_builtin("players")] = Box::new(bf_players);
    builtins[offset_for_builtin("objects")] = Box::new(bf_objects);
    builtins[offset_for_builtin("locations")] = Box::new(bf_locations);
    builtins[offset_for_builtin("owned_objects")] = Box::new(bf_owned_objects);
    builtins[offset_for_builtin("renumber")] = Box::new(bf_renumber);
    builtins[offset_for_builtin("is_anonymous")] = Box::new(bf_is_anonymous);
    builtins[offset_for_builtin("is_uuobjid")] = Box::new(bf_is_uuobjid);
    builtins[offset_for_builtin("parse_command")] = Box::new(bf_parse_command);
    builtins[offset_for_builtin("find_command_verb")] = Box::new(bf_find_command_verb);
}
