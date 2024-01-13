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

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, error, trace};

use moor_values::model::objects::ObjFlag;
use moor_values::model::{world_state_err, WorldStateError};
use moor_values::util::bitenum::BitEnum;
use moor_values::var::error::Error;
use moor_values::var::error::Error::{E_INVARG, E_NACC, E_TYPE};
use moor_values::var::variant::Variant;
use moor_values::var::{v_bool, v_int, v_list, v_none, v_objid, v_str};
use moor_values::NOTHING;

use crate::bf_declare;
use crate::builtins::BfRet::{Ret, VmInstr};
use crate::builtins::{BfCallState, BfRet, BuiltinFunction};
use crate::tasks::VerbCall;
use crate::vm::ExecutionResult::ContinueVerb;
use crate::vm::VM;
use moor_compiler::builtins::offset_for_builtin;
use moor_values::model::defset::Named;

/*
Function: int valid (obj object)
Returns a non-zero integer (i.e., a true value) if object is a valid object (one that has been created and not yet recycled) and zero (i.e., a false value) otherwise.
*/
async fn bf_valid<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(E_TYPE);
    };
    let is_valid = bf_args
        .world_state
        .valid(*obj)
        .await
        .map_err(world_state_err)?;
    Ok(Ret(v_bool(is_valid)))
}
bf_declare!(valid, bf_valid);

async fn bf_parent<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(E_TYPE);
    };
    if obj.0 < 0 {
        return Err(E_INVARG);
    }
    let parent = bf_args
        .world_state
        .parent_of(bf_args.task_perms_who(), *obj)
        .await
        .map_err(world_state_err)?;
    Ok(Ret(v_objid(parent)))
}
bf_declare!(parent, bf_parent);

async fn bf_chparent<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 2 {
        return Err(E_INVARG);
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(E_TYPE);
    };
    let Variant::Obj(new_parent) = bf_args.args[1].variant() else {
        return Err(E_TYPE);
    };
    bf_args
        .world_state
        .change_parent(bf_args.task_perms_who(), *obj, *new_parent)
        .await
        .map_err(world_state_err)?;
    Ok(Ret(v_none()))
}
bf_declare!(chparent, bf_chparent);

async fn bf_children<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(E_TYPE);
    };
    let children = bf_args
        .world_state
        .children_of(bf_args.task_perms_who(), *obj)
        .await
        .map_err(world_state_err)?;

    debug!("Children: {:?} {:?}", obj, children);
    let children = children.iter().map(v_objid).collect::<Vec<_>>();
    debug!("Children: {:?} {:?}", obj, children);
    Ok(Ret(v_list(&children)))
}
bf_declare!(children, bf_children);

/*
Syntax:  create (obj <parent> [, obj <owner>])   => obj
 */
const BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE: usize = 0;
const BF_CREATE_OBJECT_TRAMPOLINE_DONE: usize = 1;

async fn bf_create<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(E_INVARG);
    }
    let Variant::Obj(parent) = bf_args.args[0].variant() else {
        return Err(E_TYPE);
    };
    let owner = if bf_args.args.len() == 2 {
        let Variant::Obj(owner) = bf_args.args[1].variant() else {
            return Err(E_TYPE);
        };
        *owner
    } else {
        bf_args.task_perms_who()
    };

    let tramp = bf_args
        .exec_state
        .top()
        .bf_trampoline
        .unwrap_or(BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE);

    match tramp {
        BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE => {
            let new_obj = bf_args
                .world_state
                .create_object(bf_args.task_perms_who(), *parent, owner, BitEnum::new())
                .await
                .map_err(world_state_err)?;

            // We're going to try to call :initialize on the new object.
            // Then trampoline into the done case.
            // If :initialize doesn't exist, we'll just skip ahead.
            let Ok(initialize) = bf_args
                .world_state
                .find_method_verb_on(bf_args.task_perms_who(), new_obj, "initialize")
                .await
            else {
                return Ok(Ret(v_objid(new_obj)));
            };

            return Ok(VmInstr(ContinueVerb {
                permissions: bf_args.task_perms_who(),
                resolved_verb: initialize,
                call: VerbCall {
                    verb_name: "initialize".to_string(),
                    location: new_obj,
                    this: new_obj,
                    player: bf_args.exec_state.top().player,
                    args: vec![],
                    argstr: "".to_string(),
                    caller: bf_args.exec_state.top().this,
                },
                trampoline: Some(BF_CREATE_OBJECT_TRAMPOLINE_DONE),
                command: None,
                trampoline_arg: Some(v_objid(new_obj)),
            }));
        }
        BF_CREATE_OBJECT_TRAMPOLINE_DONE => {
            // The trampoline argument is the object we just created.
            let Some(new_obj) = bf_args.exec_state.top().bf_trampoline_arg.clone() else {
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
async fn bf_recycle<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(E_TYPE);
    };

    // Before actually recycling the object, we need to move all its contents to #-1. While
    // `recycle_object` will actually do this, we need to make sure :exitfunc is called on each
    // object, so we'll do it manually here.

    'outer: loop {
        let tramp = bf_args.exec_state.top().bf_trampoline;
        match tramp {
            None => {
                // Starting out, we need to call "recycle" on the object, if it exists.
                // The next point in the trampoline is CALL_EXITFUNC and it will expect a list of
                // objects to move/call :exitfunc on. So let's get the initial list of objects
                // now
                let object_contents = bf_args
                    .world_state
                    .contents_of(bf_args.task_perms_who(), *obj)
                    .await
                    .map_err(world_state_err)?;
                // Filter contents for objects that have an :exitfunc verb.
                let mut contents = vec![];
                for o in object_contents.iter() {
                    match bf_args
                        .world_state
                        .find_method_verb_on(bf_args.task_perms_who(), o, "exitfunc")
                        .await
                    {
                        Ok(_) => {
                            contents.push(v_objid(o));
                        }
                        Err(WorldStateError::VerbNotFound(_, _)) => {}
                        Err(e) => {
                            error!("Error looking up exitfunc verb: {:?}", e);
                            return Err(E_NACC);
                        }
                    }
                }
                let contents = v_list(&contents);
                match bf_args
                    .world_state
                    .find_method_verb_on(bf_args.task_perms_who(), *obj, "recycle")
                    .await
                {
                    Ok(dispatch) => {
                        return Ok(VmInstr(ContinueVerb {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb: dispatch,
                            call: VerbCall {
                                verb_name: "recycle".to_string(),
                                location: *obj,
                                this: *obj,
                                player: bf_args.exec_state.top().player,
                                args: vec![],
                                argstr: "".to_string(),
                                caller: bf_args.exec_state.top().this,
                            },
                            trampoline: Some(BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC),
                            trampoline_arg: Some(contents),
                            command: None,
                        }));
                    }
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        // Short-circuit fake-tramp state change.
                        bf_args.exec_state.top_mut().bf_trampoline =
                            Some(BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC);
                        bf_args.exec_state.top_mut().bf_trampoline_arg = Some(contents);
                        // Fall through to the next case.
                    }
                    Err(e) => {
                        error!("Error looking up recycle verb: {:?}", e);
                        return Err(E_NACC);
                    }
                }
            }
            Some(BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC) => {
                // Check the arguments, which must be a list of objects. IF it's empty, we can
                // move onto DONE_MOVE, if not, take the head of the list, and call :exitfunc on it
                // (if it exists), and then back to this state.
                let contents = bf_args.exec_state.top().bf_trampoline_arg.clone().unwrap();
                let Variant::List(contents) = contents.variant() else {
                    panic!("Invalid trampoline argument for bf_recycle");
                };
                'inner: loop {
                    debug!(?obj, contents = ?contents, "Calling :exitfunc for objects contents");
                    if contents.is_empty() {
                        bf_args.exec_state.top_mut().bf_trampoline_arg = None;
                        bf_args.exec_state.top_mut().bf_trampoline =
                            Some(BF_RECYCLE_TRAMPOLINE_DONE_MOVE);
                        continue 'outer;
                    }
                    let (head_obj, contents) = contents.pop_front();
                    let Variant::Obj(head_obj) = head_obj.variant() else {
                        panic!("Invalid trampoline argument for bf_recycle");
                    };
                    // :exitfunc *should* exist because we looked for it earlier, and we're supposed to
                    // be transactionally isolated. But we need to do resolution anyways, so we will
                    // look again anyways.
                    let Ok(exitfunc) = bf_args
                        .world_state
                        .find_method_verb_on(bf_args.task_perms_who(), *head_obj, "exitfunc")
                        .await
                    else {
                        // If there's no :exitfunc, we can just move on to the next object.
                        bf_args.exec_state.top_mut().bf_trampoline_arg = Some(contents);
                        continue 'inner;
                    };
                    // Call :exitfunc on the head object.
                    return Ok(VmInstr(ContinueVerb {
                        permissions: bf_args.task_perms_who(),
                        resolved_verb: exitfunc,
                        call: VerbCall {
                            verb_name: "exitfunc".to_string(),
                            location: *head_obj,
                            this: *head_obj,
                            player: bf_args.exec_state.top().player,
                            args: vec![v_objid(*obj)],
                            argstr: "".to_string(),
                            caller: bf_args.exec_state.top().this,
                        },
                        trampoline: Some(BF_RECYCLE_TRAMPOLINE_CALL_EXITFUNC),
                        trampoline_arg: Some(contents),
                        command: None,
                    }));
                }
            }
            Some(BF_RECYCLE_TRAMPOLINE_DONE_MOVE) => {
                debug!(obj = ?*obj, "Recycling object");
                bf_args
                    .world_state
                    .recycle_object(bf_args.task_perms_who(), *obj)
                    .await
                    .map_err(world_state_err)?;
                return Ok(Ret(v_none()));
            }
            Some(unknown) => {
                panic!("Invalid trampoline for bf_recycle {}", unknown)
            }
        }
    }
}
bf_declare!(recycle, bf_recycle);

async fn bf_max_object<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if !bf_args.args.is_empty() {
        return Err(E_INVARG);
    }
    let max_obj = bf_args
        .world_state
        .max_object(bf_args.task_perms_who())
        .await
        .map_err(world_state_err)?;
    Ok(Ret(v_objid(max_obj)))
}
bf_declare!(max_object, bf_max_object);

const BF_MOVE_TRAMPOLINE_START_ACCEPT: usize = 0;
const BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC: usize = 1;
const BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC: usize = 2;
const BF_MOVE_TRAMPOLINE_DONE: usize = 3;

async fn bf_move<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 2 {
        return Err(E_INVARG);
    }
    let Variant::Obj(what) = bf_args.args[0].variant() else {
        return Err(E_TYPE);
    };
    let Variant::Obj(whereto) = bf_args.args[1].variant() else {
        return Err(E_TYPE);
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

    let mut tramp = bf_args
        .exec_state
        .top()
        .bf_trampoline
        .unwrap_or(BF_MOVE_TRAMPOLINE_START_ACCEPT);
    trace!(what = ?what, where_to = ?*whereto, tramp, "move: looking up :accept verb");

    let perms = bf_args.task_perms().await.map_err(world_state_err)?;
    let mut shortcircuit = false;
    loop {
        match tramp {
            BF_MOVE_TRAMPOLINE_START_ACCEPT => {
                if *whereto == NOTHING {
                    shortcircuit = true;
                    tramp = BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC;
                    continue;
                }
                match bf_args
                    .world_state
                    .find_method_verb_on(bf_args.task_perms_who(), *whereto, "accept")
                    .await
                {
                    Ok(dispatch) => {
                        return Ok(VmInstr(ContinueVerb {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb: dispatch,
                            call: VerbCall {
                                verb_name: "accept".to_string(),
                                location: *whereto,
                                this: *whereto,
                                player: bf_args.exec_state.top().player,
                                args: vec![v_objid(*what)],
                                argstr: "".to_string(),
                                caller: bf_args.exec_state.top().this,
                            },
                            trampoline: Some(BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC),
                            trampoline_arg: None,
                            command: None,
                        }));
                    }
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        if !perms.check_is_wizard().map_err(world_state_err)? {
                            return Err(E_NACC);
                        }
                        // Short-circuit fake-tramp state change.
                        tramp = BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC;
                        shortcircuit = true;
                        continue;
                    }
                    Err(e) => {
                        error!("Error looking up accept verb: {:?}", e);
                        return Err(E_NACC);
                    }
                }
            }
            BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC => {
                trace!(what = ?what, where_to = ?*whereto, tramp, "move: moving object, calling exitfunc");

                // Accept verb has been called, and returned. Check the result. Should be on stack,
                // unless short-circuited, in which case we assume *false*
                let result = if !shortcircuit {
                    bf_args.exec_state.top().peek_top().unwrap()
                } else {
                    v_int(0)
                };
                // If the result is false, and we're not a wizard, then raise E_NACC.
                if !result.is_true() && !perms.check_is_wizard().map_err(world_state_err)? {
                    return Err(E_NACC);
                }

                // Otherwise, ask the world state to move the object.
                trace!(what = ?what, where_to = ?*whereto, tramp, "move: moving object & calling enterfunc");

                let original_location = bf_args
                    .world_state
                    .location_of(bf_args.task_perms_who(), *what)
                    .await
                    .map_err(world_state_err)?;

                // Failure here is likely due to permissions, so we'll just propagate that error.
                bf_args
                    .world_state
                    .move_object(bf_args.task_perms_who(), *what, *whereto)
                    .await
                    .map_err(world_state_err)?;

                // If the object has no location, then we can move on to the enterfunc.
                if original_location == NOTHING {
                    tramp = BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC;
                    continue;
                }

                // Call exitfunc...
                match bf_args
                    .world_state
                    .find_method_verb_on(bf_args.task_perms_who(), original_location, "exitfunc")
                    .await
                {
                    Ok(dispatch) => {
                        let continuation = ContinueVerb {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb: dispatch,
                            call: VerbCall {
                                verb_name: "exitfunc".to_string(),
                                location: original_location,
                                this: original_location,
                                player: bf_args.exec_state.top().player,
                                args: vec![v_objid(*what)],
                                argstr: "".to_string(),
                                caller: bf_args.exec_state.top().this,
                            },
                            command: None,
                            trampoline: Some(BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC),
                            trampoline_arg: None,
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
                        return Err(E_NACC);
                    }
                }
            }
            BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC => {
                if *whereto == NOTHING {
                    tramp = BF_MOVE_TRAMPOLINE_DONE;
                    continue;
                }
                trace!(what = ?what, where_to = ?*whereto, tramp, "move: calling enterfunc");

                // Exitfunc has been called, and returned. Result is irrelevant. Prepare to call
                // :enterfunc on the destination.
                match bf_args
                    .world_state
                    .find_method_verb_on(bf_args.task_perms_who(), *whereto, "enterfunc")
                    .await
                {
                    Ok(dispatch) => {
                        return Ok(VmInstr(ContinueVerb {
                            permissions: bf_args.task_perms_who(),
                            resolved_verb: dispatch,
                            call: VerbCall {
                                verb_name: "enterfunc".to_string(),
                                location: *whereto,
                                this: *whereto,
                                player: bf_args.exec_state.top().player,
                                args: vec![v_objid(*what)],
                                argstr: "".to_string(),
                                caller: bf_args.exec_state.top().this,
                            },
                            command: None,
                            trampoline: Some(3),
                            trampoline_arg: None,
                        }));
                    }
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        // Short-circuit fake-tramp state change.
                        tramp = BF_MOVE_TRAMPOLINE_DONE;
                        continue;
                    }
                    Err(e) => {
                        error!("Error looking up enterfunc verb: {:?}", e);
                        return Err(E_NACC);
                    }
                }
            }
            BF_MOVE_TRAMPOLINE_DONE => {
                trace!(what = ?what, where_to = ?*whereto, tramp, "move: completed");

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

async fn bf_verbs<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(E_TYPE);
    };
    let verbs = bf_args
        .world_state
        .verbs(bf_args.task_perms_who(), *obj)
        .await
        .map_err(world_state_err)?;
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
async fn bf_properties<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 1 {
        return Err(E_INVARG);
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(E_TYPE);
    };
    let props = bf_args
        .world_state
        .properties(bf_args.task_perms_who(), *obj)
        .await
        .map_err(world_state_err)?;
    let props: Vec<_> = props.iter().map(|p| v_str(p.name())).collect();
    Ok(Ret(v_list(&props)))
}
bf_declare!(properties, bf_properties);

async fn bf_set_player_flag<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 2 {
        return Err(E_INVARG);
    }

    let (Variant::Obj(obj), Variant::Int(f)) =
        (bf_args.args[0].variant(), bf_args.args[1].variant())
    else {
        return Err(E_INVARG);
    };

    let f = *f == 1;

    // User must be a wizard.
    bf_args
        .task_perms()
        .await
        .map_err(world_state_err)?
        .check_wizard()
        .map_err(world_state_err)?;

    // Get and set object flags
    let mut flags = bf_args
        .world_state
        .flags_of(*obj)
        .await
        .map_err(world_state_err)?;

    if f {
        flags.set(ObjFlag::User);
    } else {
        flags.clear(ObjFlag::User);
    }

    bf_args
        .world_state
        .set_flags_of(bf_args.task_perms_who(), *obj, flags)
        .await
        .map_err(world_state_err)?;

    // If the object was player, update the VM's copy of the perms.
    if *obj == bf_args.task_perms().await.map_err(world_state_err)?.who {
        bf_args.exec_state.set_task_perms(*obj);
    }

    Ok(Ret(v_none()))
}
bf_declare!(set_player_flag, bf_set_player_flag);

async fn bf_players<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if !bf_args.args.is_empty() {
        return Err(E_INVARG);
    }
    let players = bf_args
        .world_state
        .players()
        .await
        .map_err(world_state_err)?;

    Ok(Ret(v_list(
        players.iter().map(v_objid).collect::<Vec<_>>().as_slice(),
    )))
}
bf_declare!(players, bf_players);

impl VM {
    pub(crate) fn register_bf_objects(&mut self) {
        self.builtins[offset_for_builtin("create")] = Arc::new(BfCreate {});
        self.builtins[offset_for_builtin("valid")] = Arc::new(BfValid {});
        self.builtins[offset_for_builtin("verbs")] = Arc::new(BfVerbs {});
        self.builtins[offset_for_builtin("properties")] = Arc::new(BfProperties {});
        self.builtins[offset_for_builtin("parent")] = Arc::new(BfParent {});
        self.builtins[offset_for_builtin("children")] = Arc::new(BfChildren {});
        self.builtins[offset_for_builtin("move")] = Arc::new(BfMove {});
        self.builtins[offset_for_builtin("chparent")] = Arc::new(BfChparent {});
        self.builtins[offset_for_builtin("set_player_flag")] = Arc::new(BfSetPlayerFlag {});
        self.builtins[offset_for_builtin("recycle")] = Arc::new(BfRecycle {});
        self.builtins[offset_for_builtin("max_object")] = Arc::new(BfMaxObject {});
        self.builtins[offset_for_builtin("players")] = Arc::new(BfPlayers {});
    }
}
