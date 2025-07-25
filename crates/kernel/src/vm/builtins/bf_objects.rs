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

use lazy_static::lazy_static;
use tracing::{debug, error, trace};

use moor_common::model::Named;
use moor_common::model::WorldStateError;
use moor_common::model::{ObjFlag, ValSet};
use moor_common::util::BitEnum;
use moor_compiler::offset_for_builtin;
use moor_var::{E_ARGS, E_INVARG, E_NACC, E_PERM, E_TYPE, v_arc_string, v_sym};
use moor_var::{List, Variant, v_bool};
use moor_var::{NOTHING, v_list_iter};
use moor_var::{Sequence, Symbol, v_list};
use moor_var::{v_int, v_obj};

use crate::vm::builtins::BfRet::{Ret, RetNil, VmInstr};
use crate::vm::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction, world_state_bf_err};
use crate::vm::vm_host::ExecutionResult::DispatchVerb;
use crate::vm::{VerbCall, VerbExecutionRequest};

lazy_static! {
    static ref INITIALIZE_SYM: Symbol = Symbol::mk("initialize");
    static ref EXITFUNC_SYM: Symbol = Symbol::mk("exitfunc");
    static ref ENTERFUNC_SYM: Symbol = Symbol::mk("enterfunc");
    static ref CREATE_SYM: Symbol = Symbol::mk("create");
    static ref RECYCLE_SYM: Symbol = Symbol::mk("recycle");
    static ref ACCEPT_SYM: Symbol = Symbol::mk("accept");
}
/*
Function: int valid (obj object)
Returns a non-zero integer (i.e., a true value) if object is a valid object (one that has been created and not yet recycled) and zero (i.e., a false value) otherwise.
*/
fn bf_valid(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("valid() takes 1 argument")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("valid() first argument must be an object"),
        ));
    };
    let is_valid = bf_args
        .world_state
        .valid(&obj)
        .map_err(world_state_bf_err)?;
    Ok(Ret(bf_args.v_bool(is_valid)))
}

fn bf_parent(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("parent() takes 1 argument")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("parent() first argument must be an object"),
        ));
    };
    if !obj.is_positive()
        || !bf_args
            .world_state
            .valid(&obj)
            .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("parent() argument must be a valid object"),
        ));
    }
    let parent = bf_args
        .world_state
        .parent_of(&bf_args.task_perms_who(), &obj)
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_obj(parent)))
}

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
    if !bf_args
        .world_state
        .valid(&obj)
        .map_err(world_state_bf_err)?
        || !(new_parent.is_nothing()
            || bf_args
                .world_state
                .valid(&new_parent)
                .map_err(world_state_bf_err)?)
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("chparent() arguments must be valid objects"),
        ));
    }

    bf_args
        .world_state
        .change_parent(&bf_args.task_perms_who(), &obj, &new_parent)
        .map_err(world_state_bf_err)?;
    Ok(RetNil)
}

fn bf_children(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("children() takes 1 argument")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("children() first argument must be an object"),
        ));
    };
    if !bf_args
        .world_state
        .valid(&obj)
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("children() argument must be a valid object"),
        ));
    }
    let children = bf_args
        .world_state
        .children_of(&bf_args.task_perms_who(), &obj)
        .map_err(world_state_bf_err)?;

    let children = children.iter().map(v_obj).collect::<Vec<_>>();
    Ok(Ret(v_list(&children)))
}

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
    if !bf_args
        .world_state
        .valid(&obj)
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("descendants() argument must be a valid object"),
        ));
    }
    let descendants = bf_args
        .world_state
        .descendants_of(&bf_args.task_perms_who(), &obj, false)
        .map_err(world_state_bf_err)?;

    let descendants = descendants.iter().map(v_obj).collect::<Vec<_>>();
    Ok(Ret(v_list(&descendants)))
}

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

    if !bf_args
        .world_state
        .valid(&obj)
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("ancestors() argument must be a valid object"),
        ));
    }
    let ancestors = bf_args
        .world_state
        .ancestors_of(&bf_args.task_perms_who(), &obj, add_self)
        .map_err(world_state_bf_err)?;

    let ancestors = ancestors.iter().map(v_obj).collect::<Vec<_>>();
    Ok(Ret(v_list(&ancestors)))
}

/*
Syntax: isa (obj <object>, obj <possible_ancestor>) => int
*/
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

    if !bf_args
        .world_state
        .valid(&obj)
        .map_err(world_state_bf_err)?
        || !bf_args
            .world_state
            .valid(&possible_ancestor)
            .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("isa() arguments must be valid objects"),
        ));
    }

    let ancestors = bf_args
        .world_state
        .ancestors_of(&bf_args.task_perms_who(), &obj, true)
        .map_err(world_state_bf_err)?;

    let isa = ancestors.contains(possible_ancestor);

    Ok(Ret(v_bool(isa)))
}

/*
Function: list locations (obj object [, obj stop [, int is-parent]])
Recursively builds a list of an object's location, its location's location, and so forth until
hitting #nothing. If stop is provided, it stops before that object. If is-parent is true, stop
is treated as a parent and stops when any location is a child of that parent.
*/
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

    if !bf_args
        .world_state
        .valid(&obj)
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
        let location = bf_args
            .world_state
            .location_of(&bf_args.task_perms_who(), &current)
            .map_err(world_state_bf_err)?;

        // Stop if we've hit #nothing
        if location.is_nothing() {
            break;
        }

        // Handle stop conditions before adding to the list
        if let Some(stop) = stop_obj {
            if !is_parent {
                // Simple equality check - stop before adding this location
                if location == stop {
                    break;
                }
            }
        }

        // Add this location to our list
        locations.push(v_obj(location));

        // Handle is_parent stop condition after adding to the list
        if let Some(stop) = stop_obj {
            if is_parent {
                // If is_parent is true, check if location is a child of stop
                let ancestors = bf_args
                    .world_state
                    .ancestors_of(&bf_args.task_perms_who(), &location, false)
                    .map_err(world_state_bf_err)?;
                if ancestors.contains(stop) {
                    break;
                }
            }
        }

        current = location;
    }

    Ok(Ret(v_list(&locations)))
}

/*
Syntax:  create (obj <parent> [, obj <owner>])   => obj
 */
const BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE: usize = 0;
const BF_CREATE_OBJECT_TRAMPOLINE_DONE: usize = 1;

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

    // Third argument is "is-anon" - we ignore it but validate it exists
    if bf_args.args.len() >= 3 {
        // is-anon argument exists but is ignored
    }

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
            let new_obj = bf_args
                .world_state
                .create_object(&bf_args.task_perms_who(), &parent, &owner, BitEnum::new())
                .map_err(world_state_bf_err)?;

            // We're going to try to call :initialize on the new object.
            // Then trampoline into the done case.
            // If :initialize doesn't exist, we'll just skip ahead.
            let Ok((program, resolved_verb)) = bf_args.world_state.find_method_verb_on(
                &bf_args.task_perms_who(),
                &new_obj,
                *INITIALIZE_SYM,
            ) else {
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
                program,
                call: Box::new(VerbCall {
                    verb_name: *INITIALIZE_SYM,
                    location: v_obj(new_obj),
                    this: v_obj(new_obj),
                    player: bf_args.exec_state.top().player,
                    args: initialize_args,
                    argstr: "".to_string(),
                    caller: bf_args.exec_state.top().this.clone(),
                }),
                command: None,
            };
            Ok(VmInstr(DispatchVerb(Box::new(ve))))
        }
        BF_CREATE_OBJECT_TRAMPOLINE_DONE => {
            // The trampoline argument is the object we just created.
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
/*
Function: none recycle (obj object)
The given object is destroyed, irrevocably. The programmer must either own object or be a wizard; otherwise, E_PERM is raised. If object is not valid, then E_INVARG is raised. The children of object are reparented to the parent of object. Before object is recycled, each object in its contents is moved to #-1 (implying a call to object's exitfunc verb, if any) and then object's `recycle' verb, if any, is called with no arguments.
 */
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

    let valid = bf_args.world_state.valid(&obj);
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
    if !bf_args
        .world_state
        .controls(&bf_args.task_perms_who(), &obj)
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
                let object_contents = bf_args
                    .world_state
                    .contents_of(&bf_args.task_perms_who(), &obj)
                    .map_err(world_state_bf_err)?;
                // Filter contents for objects that have an :exitfunc verb.
                let mut contents = vec![];
                for o in object_contents.iter() {
                    match bf_args.world_state.find_method_verb_on(
                        &bf_args.task_perms_who(),
                        &o,
                        *EXITFUNC_SYM,
                    ) {
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
                match bf_args.world_state.find_method_verb_on(
                    &bf_args.task_perms_who(),
                    &obj,
                    *RECYCLE_SYM,
                ) {
                    Ok((program, resolved_verb)) => {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline = Some(BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC);
                        bf_frame.bf_trampoline_arg = Some(contents);

                        return Ok(VmInstr(DispatchVerb(Box::new(VerbExecutionRequest {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb,
                            program,
                            call: Box::new(VerbCall {
                                verb_name: *RECYCLE_SYM,
                                location: v_obj(obj),
                                this: v_obj(obj),
                                player: bf_args.exec_state.top().player,
                                args: List::mk_list(&[]),
                                argstr: "".to_string(),
                                caller: bf_args.exec_state.top().this.clone(),
                            }),
                            command: None,
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
                    let Ok((program, resolved_verb)) = bf_args.world_state.find_method_verb_on(
                        &bf_args.task_perms_who(),
                        &head_obj,
                        *EXITFUNC_SYM,
                    ) else {
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
                        program,
                        call: Box::new(VerbCall {
                            verb_name: *EXITFUNC_SYM,
                            location: v_obj(head_obj),
                            this: v_obj(head_obj),
                            player: bf_args.exec_state.top().player,
                            args: List::mk_list(&[v_obj(obj)]),
                            argstr: "".to_string(),
                            caller: bf_args.exec_state.top().this.clone(),
                        }),
                        command: None,
                    }))));
                }
            }
            Some(BF_RECYCLE_TRAMPOLINE_DONE_MOVE) => {
                debug!(obj = ?obj, "Recycling object");
                bf_args
                    .world_state
                    .recycle_object(&bf_args.task_perms_who(), &obj)
                    .map_err(world_state_bf_err)?;
                return Ok(Ret(v_int(0)));
            }
            Some(unknown) => {
                panic!("Invalid trampoline for bf_recycle {unknown}")
            }
        }
    }
}

fn bf_max_object(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("max_object() takes no arguments"),
        ));
    }
    let max_obj = bf_args
        .world_state
        .max_object(&bf_args.task_perms_who())
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_obj(max_obj)))
}

const BF_MOVE_TRAMPOLINE_START_ACCEPT: usize = 0;
const BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC: usize = 1;
const BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC: usize = 2;
const BF_MOVE_TRAMPOLINE_DONE: usize = 3;

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
                match bf_args.world_state.find_method_verb_on(
                    &bf_args.task_perms_who(),
                    &whereto,
                    *ACCEPT_SYM,
                ) {
                    Ok((program, resolved_verb)) => {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline = Some(BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC);
                        bf_frame.bf_trampoline_arg = None;
                        return Ok(VmInstr(DispatchVerb(Box::new(VerbExecutionRequest {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb,
                            program,
                            call: Box::new(VerbCall {
                                verb_name: *ACCEPT_SYM,
                                location: v_obj(whereto),
                                this: v_obj(whereto),
                                player: bf_args.exec_state.top().player,
                                args: List::mk_list(&[v_obj(what)]),
                                argstr: "".to_string(),
                                caller: bf_args.exec_state.top().this.clone(),
                            }),
                            command: None,
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

                let original_location = bf_args
                    .world_state
                    .location_of(&bf_args.task_perms_who(), &what)
                    .map_err(world_state_bf_err)?;

                // Failure here is likely due to permissions, so we'll just propagate that error.
                bf_args
                    .world_state
                    .move_object(&bf_args.task_perms_who(), &what, &whereto)
                    .map_err(world_state_bf_err)?;

                // If the object has no location, then we can move on to the enterfunc.
                if original_location == NOTHING {
                    tramp = BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC;
                    continue;
                }

                // Call exitfunc...
                match bf_args.world_state.find_method_verb_on(
                    &bf_args.task_perms_who(),
                    &original_location,
                    *EXITFUNC_SYM,
                ) {
                    Ok((program, resolved_verb)) => {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline = Some(BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC);
                        bf_frame.bf_trampoline_arg = None;

                        let continuation = DispatchVerb(Box::new(VerbExecutionRequest {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb,
                            program,
                            call: Box::new(VerbCall {
                                verb_name: *EXITFUNC_SYM,
                                location: v_obj(original_location),
                                this: v_obj(original_location),
                                player: bf_args.exec_state.top().player,
                                args: List::mk_list(&[v_obj(what)]),
                                argstr: "".to_string(),
                                caller: bf_args.exec_state.top().this.clone(),
                            }),
                            command: None,
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
                match bf_args.world_state.find_method_verb_on(
                    &bf_args.task_perms_who(),
                    &whereto,
                    *ENTERFUNC_SYM,
                ) {
                    Ok((program, resolved_verb)) => {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline = Some(BF_MOVE_TRAMPOLINE_DONE);
                        bf_frame.bf_trampoline_arg = None;

                        return Ok(VmInstr(DispatchVerb(Box::new(VerbExecutionRequest {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb,
                            program,
                            call: Box::new(VerbCall {
                                verb_name: *ENTERFUNC_SYM,
                                location: v_obj(whereto),
                                this: v_obj(whereto),
                                player: bf_args.exec_state.top().player,
                                args: List::mk_list(&[v_obj(what)]),
                                argstr: "".to_string(),
                                caller: bf_args.exec_state.top().this.clone(),
                            }),
                            command: None,
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

fn bf_verbs(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("verbs() takes 1 argument")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("verbs() first argument must be an object"),
        ));
    };
    let verbs = bf_args
        .world_state
        .verbs(&bf_args.task_perms_who(), &obj)
        .map_err(world_state_bf_err)?;
    let verbs: Vec<_> = verbs
        .iter()
        .map(|v| v_arc_string(v.names().first().unwrap().as_arc_string()))
        .collect();
    Ok(Ret(v_list(&verbs)))
}

/*
Function: list properties (obj object)
Returns a list of the names of the properties defined directly on the given object, not inherited from its parent. If object is not valid, then E_INVARG is raised. If the programmer does not have read permission on object, then E_PERM is raised.
 */
fn bf_properties(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("properties() takes 1 argument")));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("properties() first argument must be an object"),
        ));
    };
    let props = bf_args
        .world_state
        .properties(&bf_args.task_perms_who(), &obj)
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
    let mut flags = bf_args
        .world_state
        .flags_of(obj)
        .map_err(world_state_bf_err)?;

    if f {
        flags.set(ObjFlag::User);
    } else {
        flags.clear(ObjFlag::User);
    }

    bf_args
        .world_state
        .set_flags_of(&bf_args.task_perms_who(), obj, flags)
        .map_err(world_state_bf_err)?;

    // If the object was player, update the VM's copy of the perms.
    if obj.eq(&bf_args.task_perms().map_err(world_state_bf_err)?.who) {
        bf_args.exec_state.set_task_perms(*obj);
    }

    Ok(RetNil)
}

fn bf_players(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::ErrValue(E_ARGS.msg("players() takes no arguments")));
    }
    let players = bf_args.world_state.players().map_err(world_state_bf_err)?;

    Ok(Ret(v_list_iter(players.iter().map(v_obj))))
}

/*
Function: list owned_objects (obj owner)
Returns a list of objects owned by the given object. If owner is not valid, then E_INVARG is raised.
If the programmer does not have read permission on owner, then E_PERM is raised.
*/
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
    if !bf_args
        .world_state
        .valid(&owner)
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("owned_objects() argument must be a valid object"),
        ));
    }

    let owned_objects = bf_args
        .world_state
        .owned_objects(&bf_args.task_perms_who(), &owner)
        .map_err(world_state_bf_err)?;

    let owned_objects = owned_objects.iter().map(v_obj).collect::<Vec<_>>();
    Ok(Ret(v_list(&owned_objects)))
}

pub(crate) fn register_bf_objects(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("create")] = Box::new(bf_create);
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
    builtins[offset_for_builtin("locations")] = Box::new(bf_locations);
    builtins[offset_for_builtin("owned_objects")] = Box::new(bf_owned_objects);
}
