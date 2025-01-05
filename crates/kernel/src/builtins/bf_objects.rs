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

// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
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

use moor_compiler::offset_for_builtin;
use moor_values::model::Named;
use moor_values::model::WorldStateError;
use moor_values::model::{ObjFlag, ValSet};
use moor_values::util::BitEnum;
use moor_values::Error::{E_ARGS, E_INVARG, E_NACC, E_PERM, E_TYPE};
use moor_values::{v_bool, v_int, v_none, v_obj, v_str};
use moor_values::{v_list, Sequence, Symbol};
use moor_values::{v_list_iter, NOTHING};
use moor_values::{List, Variant};

use crate::bf_declare;
use crate::builtins::BfRet::{Ret, VmInstr};
use crate::builtins::{world_state_bf_err, BfCallState, BfErr, BfRet, BuiltinFunction};
use crate::tasks::VerbCall;
use crate::vm::ExecutionResult::DispatchVerb;

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
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let is_valid = bf_args.world_state.valid(obj).map_err(world_state_bf_err)?;
    Ok(Ret(v_bool(is_valid)))
}
bf_declare!(valid, bf_valid);

fn bf_parent(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !obj.is_positive() {
        return Err(BfErr::Code(E_INVARG));
    }
    let parent = bf_args
        .world_state
        .parent_of(&bf_args.task_perms_who(), obj)
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_obj(parent)))
}
bf_declare!(parent, bf_parent);

fn bf_chparent(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Variant::Obj(new_parent) = bf_args.args[1].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    bf_args
        .world_state
        .change_parent(&bf_args.task_perms_who(), obj, new_parent)
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_none()))
}
bf_declare!(chparent, bf_chparent);

fn bf_children(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let children = bf_args
        .world_state
        .children_of(&bf_args.task_perms_who(), obj)
        .map_err(world_state_bf_err)?;

    let children = children.iter().map(v_obj).collect::<Vec<_>>();
    Ok(Ret(v_list(&children)))
}
bf_declare!(children, bf_children);

/*
Syntax:  create (obj <parent> [, obj <owner>])   => obj
 */
const BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE: usize = 0;
const BF_CREATE_OBJECT_TRAMPOLINE_DONE: usize = 1;

fn bf_create(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(parent) = bf_args.args[0].variant().clone() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let owner = if bf_args.args.len() == 2 {
        let Variant::Obj(owner) = bf_args.args[1].variant().clone() else {
            return Err(BfErr::Code(E_TYPE));
        };
        owner
    } else {
        bf_args.task_perms_who()
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
            let Ok((binary, resolved_verb)) = bf_args.world_state.find_method_verb_on(
                &bf_args.task_perms_who(),
                &new_obj,
                *INITIALIZE_SYM,
            ) else {
                return Ok(Ret(v_obj(new_obj)));
            };

            let bf_frame = bf_args.bf_frame_mut();
            bf_frame.bf_trampoline = Some(BF_CREATE_OBJECT_TRAMPOLINE_DONE);
            bf_frame.bf_trampoline_arg = Some(v_obj(new_obj.clone()));
            Ok(VmInstr(DispatchVerb {
                permissions: bf_args.task_perms_who(),
                resolved_verb,
                binary,
                call: VerbCall {
                    verb_name: *INITIALIZE_SYM,
                    location: v_obj(new_obj.clone()),
                    this: v_obj(new_obj),
                    player: bf_args.exec_state.top().player.clone(),
                    args: List::mk_list(&[]),
                    argstr: "".to_string(),
                    caller: bf_args.exec_state.top().this.clone(),
                },
                command: None,
            }))
        }
        BF_CREATE_OBJECT_TRAMPOLINE_DONE => {
            // The trampoline argument is the object we just created.
            let Some(new_obj) = bf_args.bf_frame().bf_trampoline_arg.clone() else {
                panic!("Missing/invalid trampoline argument for bf_create");
            };

            Ok(Ret(new_obj))
        }
        _ => {
            panic!("Invalid trampoline for bf_create {}", tramp)
        }
    }
}
bf_declare!(create, bf_create);
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
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant().clone() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let valid = bf_args.world_state.valid(&obj);
    if valid == Ok(false)
        || valid
            .err()
            .map(|e| e.database_error_msg() == Some("NotFound"))
            .unwrap_or_default()
    {
        return Err(BfErr::Code(E_INVARG));
    }

    // Check if the given task perms can control the object before continuing.
    if !bf_args
        .world_state
        .controls(&bf_args.task_perms_who(), &obj)
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_PERM));
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
                            return Err(BfErr::Code(E_NACC));
                        }
                    }
                }
                let contents = v_list(&contents);
                match bf_args.world_state.find_method_verb_on(
                    &bf_args.task_perms_who(),
                    &obj,
                    *RECYCLE_SYM,
                ) {
                    Ok((binary, resolved_verb)) => {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline = Some(BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC);
                        bf_frame.bf_trampoline_arg = Some(contents);

                        return Ok(VmInstr(DispatchVerb {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb,
                            binary,
                            call: VerbCall {
                                verb_name: *RECYCLE_SYM,
                                location: v_obj(obj.clone()),
                                this: v_obj(obj),
                                player: bf_args.exec_state.top().player.clone(),
                                args: List::mk_list(&[]),
                                argstr: "".to_string(),
                                caller: bf_args.exec_state.top().this.clone(),
                            },
                            command: None,
                        }));
                    }
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        // Short-circuit fake-tramp state change.
                        let bf_frame = bf_args.bf_frame_mut();

                        bf_frame.bf_trampoline = Some(BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC);
                        bf_frame.bf_trampoline_arg = Some(contents);
                        // Fall through to the next case.
                    }
                    Err(e) => {
                        error!("Error looking up recycle verb: {:?}", e);
                        return Err(BfErr::Code(E_NACC));
                    }
                }
            }
            Some(BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC) => {
                // Check the arguments, which must be a list of objects. IF it's empty, we can
                // move onto DONE_MOVE, if not, take the head of the list, and call :exitfunc on it
                // (if it exists), and then back to this state.

                let contents = bf_args.bf_frame().bf_trampoline_arg.clone().unwrap();
                let Variant::List(contents) = contents.variant() else {
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
                    let Variant::Obj(head_obj) = head_obj.variant() else {
                        panic!("Invalid trampoline argument for bf_recycle");
                    };
                    // :exitfunc *should* exist because we looked for it earlier, and we're supposed to
                    // be transactionally isolated. But we need to do resolution anyways, so we will
                    // look again anyways.
                    let Ok((binary, resolved_verb)) = bf_args.world_state.find_method_verb_on(
                        &bf_args.task_perms_who(),
                        head_obj,
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
                    return Ok(VmInstr(DispatchVerb {
                        permissions: bf_args.task_perms_who(),
                        resolved_verb,
                        binary,
                        call: VerbCall {
                            verb_name: *EXITFUNC_SYM,
                            location: v_obj(head_obj.clone()),
                            this: v_obj(head_obj.clone()),
                            player: bf_args.exec_state.top().player.clone(),
                            args: List::mk_list(&[v_obj(obj)]),
                            argstr: "".to_string(),
                            caller: bf_args.exec_state.top().this.clone(),
                        },
                        command: None,
                    }));
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
                panic!("Invalid trampoline for bf_recycle {}", unknown)
            }
        }
    }
}
bf_declare!(recycle, bf_recycle);

fn bf_max_object(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }
    let max_obj = bf_args
        .world_state
        .max_object(&bf_args.task_perms_who())
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_obj(max_obj)))
}
bf_declare!(max_object, bf_max_object);

const BF_MOVE_TRAMPOLINE_START_ACCEPT: usize = 0;
const BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC: usize = 1;
const BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC: usize = 2;
const BF_MOVE_TRAMPOLINE_DONE: usize = 3;

fn bf_move(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(what) = bf_args.args[0].variant().clone() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Variant::Obj(whereto) = bf_args.args[1].variant().clone() else {
        return Err(BfErr::Code(E_TYPE));
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
    //    3    => return v_none

    let bf_frame = bf_args.bf_frame_mut();
    let mut tramp = bf_frame
        .bf_trampoline
        .take()
        .unwrap_or(BF_MOVE_TRAMPOLINE_START_ACCEPT);
    trace!(what = ?what, where_to = ?whereto, tramp, "move: looking up :accept verb");

    let perms = bf_args.task_perms().map_err(world_state_bf_err)?;
    let mut shortcircuit = false;
    loop {
        match tramp {
            BF_MOVE_TRAMPOLINE_START_ACCEPT => {
                if whereto.is_nothing() {
                    shortcircuit = true;
                    tramp = BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC;
                    continue;
                }
                match bf_args.world_state.find_method_verb_on(
                    &bf_args.task_perms_who(),
                    &whereto,
                    *ACCEPT_SYM,
                ) {
                    Ok((binary, resolved_verb)) => {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline = Some(BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC);
                        bf_frame.bf_trampoline_arg = None;
                        return Ok(VmInstr(DispatchVerb {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb,
                            binary,
                            call: VerbCall {
                                verb_name: *ACCEPT_SYM,
                                location: v_obj(whereto.clone()),
                                this: v_obj(whereto.clone()),
                                player: bf_args.exec_state.top().player.clone(),
                                args: List::mk_list(&[v_obj(what)]),
                                argstr: "".to_string(),
                                caller: bf_args.exec_state.top().this.clone(),
                            },
                            command: None,
                        }));
                    }
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        if !perms.check_is_wizard().map_err(world_state_bf_err)? {
                            return Err(BfErr::Code(E_NACC));
                        }
                        // Short-circuit fake-tramp state change.
                        tramp = BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC;
                        shortcircuit = true;
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
                // Unless short-circuited, in which case we assume *false*
                let result = if !shortcircuit {
                    bf_args.exec_state.top().frame.return_value()
                } else {
                    v_int(0)
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
                    Ok((binary, resolved_verb)) => {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline = Some(BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC);
                        bf_frame.bf_trampoline_arg = None;

                        let continuation = DispatchVerb {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb,
                            binary,
                            call: VerbCall {
                                verb_name: *EXITFUNC_SYM,
                                location: v_obj(original_location.clone()),
                                this: v_obj(original_location),
                                player: bf_args.exec_state.top().player.clone(),
                                args: List::mk_list(&[v_obj(what)]),
                                argstr: "".to_string(),
                                caller: bf_args.exec_state.top().this.clone(),
                            },
                            command: None,
                        };
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
                    Ok((binary, resolved_verb)) => {
                        let bf_frame = bf_args.bf_frame_mut();
                        bf_frame.bf_trampoline = Some(BF_MOVE_TRAMPOLINE_DONE);
                        bf_frame.bf_trampoline_arg = None;

                        return Ok(VmInstr(DispatchVerb {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb,
                            binary,
                            call: VerbCall {
                                verb_name: *ENTERFUNC_SYM,
                                location: v_obj(whereto.clone()),
                                this: v_obj(whereto),
                                player: bf_args.exec_state.top().player.clone(),
                                args: List::mk_list(&[v_obj(what)]),
                                argstr: "".to_string(),
                                caller: bf_args.exec_state.top().this.clone(),
                            },
                            command: None,
                        }));
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
                return Ok(Ret(v_none()));
            }
            _ => {
                panic!("Invalid trampoline state: {} in bf_move", tramp);
            }
        }
    }
}
bf_declare!(move, bf_move);

fn bf_verbs(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let verbs = bf_args
        .world_state
        .verbs(&bf_args.task_perms_who(), obj)
        .map_err(world_state_bf_err)?;
    let verbs: Vec<_> = verbs
        .iter()
        .map(|v| v_str(v.names().first().unwrap()))
        .collect();
    Ok(Ret(v_list(&verbs)))
}
bf_declare!(verbs, bf_verbs);

/*
Function: list properties (obj object)
Returns a list of the names of the properties defined directly on the given object, not inherited from its parent. If object is not valid, then E_INVARG is raised. If the programmer does not have read permission on object, then E_PERM is raised.
 */
fn bf_properties(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let props = bf_args
        .world_state
        .properties(&bf_args.task_perms_who(), obj)
        .map_err(world_state_bf_err)?;
    let props: Vec<_> = props.iter().map(|p| v_str(p.name())).collect();
    Ok(Ret(v_list(&props)))
}
bf_declare!(properties, bf_properties);

fn bf_set_player_flag(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let (Variant::Obj(obj), Variant::Int(f)) =
        (bf_args.args[0].variant(), bf_args.args[1].variant())
    else {
        return Err(BfErr::Code(E_INVARG));
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
        bf_args.exec_state.set_task_perms(obj.clone());
    }

    Ok(Ret(v_none()))
}
bf_declare!(set_player_flag, bf_set_player_flag);

fn bf_players(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.args.is_empty() {
        return Err(BfErr::Code(E_ARGS));
    }
    let players = bf_args.world_state.players().map_err(world_state_bf_err)?;

    Ok(Ret(v_list_iter(players.iter().map(v_obj))))
}
bf_declare!(players, bf_players);

pub(crate) fn register_bf_objects(builtins: &mut [Box<dyn BuiltinFunction>]) {
    builtins[offset_for_builtin("create")] = Box::new(BfCreate {});
    builtins[offset_for_builtin("valid")] = Box::new(BfValid {});
    builtins[offset_for_builtin("verbs")] = Box::new(BfVerbs {});
    builtins[offset_for_builtin("properties")] = Box::new(BfProperties {});
    builtins[offset_for_builtin("parent")] = Box::new(BfParent {});
    builtins[offset_for_builtin("children")] = Box::new(BfChildren {});
    builtins[offset_for_builtin("move")] = Box::new(BfMove {});
    builtins[offset_for_builtin("chparent")] = Box::new(BfChparent {});
    builtins[offset_for_builtin("set_player_flag")] = Box::new(BfSetPlayerFlag {});
    builtins[offset_for_builtin("recycle")] = Box::new(BfRecycle {});
    builtins[offset_for_builtin("max_object")] = Box::new(BfMaxObject {});
    builtins[offset_for_builtin("players")] = Box::new(BfPlayers {});
}
