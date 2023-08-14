use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, error, trace};

use moor_value::model::objects::ObjFlag;
use moor_value::model::WorldStateError;
use moor_value::var::error::Error::{E_INVARG, E_NACC, E_TYPE};
use moor_value::var::objid::NOTHING;
use moor_value::var::variant::Variant;
use moor_value::var::{v_bool, v_int, v_list, v_none, v_objid, v_str};

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::tasks::VerbCall;
use crate::vm::builtin::BfRet::{Error, Ret, VmInstr};
use crate::vm::builtin::{BfCallState, BfRet, BuiltinFunction};
use crate::vm::ExecutionResult::ContinueVerb;
use crate::vm::VM;

/*
Function: int valid (obj object)
Returns a non-zero integer (i.e., a true value) if object is a valid object (one that has been created and not yet recycled) and zero (i.e., a false value) otherwise.
*/
async fn bf_valid<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let is_valid = bf_args.world_state.valid(*obj).await?;
    Ok(Ret(v_bool(is_valid)))
}
bf_declare!(valid, bf_valid);

async fn bf_parent<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    if obj.0 < 0 {
        return Ok(Error(E_INVARG));
    }
    let parent = bf_args
        .world_state
        .parent_of(bf_args.task_perms_who(), *obj)
        .await?;
    Ok(Ret(v_objid(parent)))
}
bf_declare!(parent, bf_parent);

async fn bf_chparent<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Variant::Obj(new_parent) = bf_args.args[1].variant() else {
        return Ok(Error(E_TYPE));
    };
    bf_args
        .world_state
        .change_parent(bf_args.task_perms_who(), *obj, *new_parent)
        .await?;
    Ok(Ret(v_none()))
}
bf_declare!(chparent, bf_chparent);

async fn bf_children<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let children = bf_args
        .world_state
        .children_of(bf_args.task_perms_who(), *obj)
        .await?;
    debug!("Children: {:?} {:?}", obj, children);
    let children = children.iter().map(|c| v_objid(*c)).collect::<Vec<_>>();
    debug!("Children: {:?} {:?}", obj, children);
    Ok(Ret(v_list(children)))
}
bf_declare!(children, bf_children);

/*
Syntax:  create (obj <parent> [, obj <owner>])   => obj
 */
const BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE: usize = 0;
const BF_CREATE_OBJECT_TRAMPOLINE_DONE: usize = 1;

async fn bf_create<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(parent) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let owner = if bf_args.args.len() == 2 {
        let Variant::Obj(owner) = bf_args.args[1].variant() else {
            return Ok(Error(E_TYPE));
        };
        *owner
    } else {
        bf_args.task_perms_who()
    };

    let tramp = bf_args
        .vm
        .top()
        .bf_trampoline
        .unwrap_or(BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE);

    match tramp {
        BF_CREATE_OBJECT_TRAMPOLINE_START_CALL_INITIALIZE => {
            let new_obj = bf_args
                .world_state
                .create_object(bf_args.task_perms_who(), *parent, owner)
                .await?;

            // We're going to try to call :initialize on the new object.
            // Then trampoline into the done case.
            // If :initialize doesn't exist, we'll just skip ahead.
            let Ok(initialize) = bf_args.world_state.find_method_verb_on(
                bf_args.task_perms_who(),
                new_obj,
                "initialize",
            ).await else {
                return Ok(Ret(v_objid(new_obj)));
            };

            return Ok(VmInstr(ContinueVerb {
                permissions: bf_args.task_perms_who(),
                resolved_verb: initialize,
                call: VerbCall {
                    verb_name: "initialize".to_string(),
                    location: new_obj,
                    this: new_obj,
                    player: bf_args.vm.top().player,
                    args: vec![],
                    caller: bf_args.vm.top().this,
                },
                trampoline: Some(BF_CREATE_OBJECT_TRAMPOLINE_DONE),
                command: None,
                trampoline_arg: Some(v_objid(new_obj)),
            }));
        }
        BF_CREATE_OBJECT_TRAMPOLINE_DONE => {
            // The trampoline argument is the object we just created.
            let Some(new_obj) = bf_args.vm.top().bf_trampoline_arg.clone() else {
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

/*
Function: int object_bytes (obj object)
Returns the number of bytes of the server's memory required to store the given object, including the space used by the values of all of its non-clear properties and by the verbs and properties defined directly on the object. Raised E_INVARG if object is not a valid object and E_PERM if the programmer is not a wizard.
 */

/*
Function: obj max_object ()
Returns the largest object number yet assigned to a created object. Note that the object with this number may no longer exist; it may have been recycled. The next object created will be assigned the object number one larger than the value of max_object().
 */

const BF_MOVE_TRAMPOLINE_START_ACCEPT: usize = 0;
const BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC: usize = 1;
const BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC: usize = 2;
const BF_MOVE_TRAMPOLINE_DONE: usize = 3;

async fn bf_move<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(what) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Variant::Obj(whereto) = bf_args.args[1].variant() else {
        return Ok(Error(E_TYPE));
    };

    // Before actually doing any work, reject recursive moves.
    //   If the destination is self, E_RECMOVE
    //   If the destination is something that's inside me, that's also E_RECMOVE
    //   And so on...

    // 'Trampoline' state machine:
    //    None => look up :accept, if it exists, set tramp to 1, and ask for it to be invoked.
    //            if it doesn't & perms not wizard, set raise E_NACC (as if :accept returned false)
    //    1    => if verb call was a success (look at stack), set tramp to 2, move the object,
    //            then prepare :exitfunc on the original object source
    //            if :exitfunc doesn't exist, proceed to 3 (enterfunc)
    //    2    => set tramp to 3, call :enterfunc on the destination if it exists, result is ignored.
    //    3    => return v_none

    let mut tramp = bf_args
        .vm
        .top()
        .bf_trampoline
        .unwrap_or(BF_MOVE_TRAMPOLINE_START_ACCEPT);
    trace!(what = ?what, where_to = ?*whereto, tramp, "move: looking up :accept verb");

    let perms = bf_args.terk_perms().await?;
    let mut shortcircuit = false;
    loop {
        match tramp {
            BF_MOVE_TRAMPOLINE_START_ACCEPT => {
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
                                player: bf_args.vm.top().player,
                                args: vec![v_objid(*what)],
                                caller: bf_args.vm.top().this,
                            },
                            trampoline: Some(BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC),
                            trampoline_arg: None,
                            command: None,
                        }));
                    }
                    Err(WorldStateError::VerbNotFound(_, _)) => {
                        if !perms.check_is_wizard()? {
                            return Ok(Error(E_NACC));
                        }
                        // Short-circuit fake-tramp state change.
                        tramp = 1;
                        shortcircuit = true;
                        continue;
                    }
                    Err(e) => {
                        error!("Error looking up accept verb: {:?}", e);
                        return Ok(Error(E_NACC));
                    }
                }
            }
            BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC => {
                trace!(what = ?what, where_to = ?*whereto, tramp, "move: moving object, calling exitfunc");

                // Accept verb has been called, and returned. Check the result. Should be on stack,
                // unless short-circuited, in which case we assume *false*
                let result = if !shortcircuit {
                    bf_args.vm.top().peek_top().unwrap()
                } else {
                    v_int(0)
                };
                // If the result is false, and we're not a wizard, then raise E_NACC.
                if !result.is_true() && !perms.check_is_wizard()? {
                    return Ok(Error(E_NACC));
                }

                // Otherwise, ask the world state to move the object.
                trace!(what = ?what, where_to = ?*whereto, tramp, "move: moving object & calling enterfunc");

                let original_location = bf_args
                    .world_state
                    .location_of(bf_args.task_perms_who(), *what)
                    .await?;

                // Failure here is likely due to permissions, so we'll just propagate that error.
                bf_args
                    .world_state
                    .move_object(bf_args.task_perms_who(), *what, *whereto)
                    .await?;

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
                                player: bf_args.vm.top().player,
                                args: vec![v_objid(*what)],
                                caller: bf_args.vm.top().this,
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
                        return Ok(Error(E_NACC));
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
                                player: bf_args.vm.top().player,
                                args: vec![v_objid(*what)],
                                caller: bf_args.vm.top().this,
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
                        return Ok(Error(E_NACC));
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

async fn bf_verbs<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let verbs = bf_args
        .world_state
        .verbs(bf_args.task_perms_who(), *obj)
        .await?;
    let verbs = verbs
        .iter()
        .map(|v| v_str(v.names.first().unwrap()))
        .collect();
    Ok(Ret(v_list(verbs)))
}
bf_declare!(verbs, bf_verbs);

/*
Function: list properties (obj object)
Returns a list of the names of the properties defined directly on the given object, not inherited from its parent. If object is not valid, then E_INVARG is raised. If the programmer does not have read permission on object, then E_PERM is raised.
 */
async fn bf_properties<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let props = bf_args
        .world_state
        .properties(bf_args.task_perms_who(), *obj)
        .await?;
    let props = props.iter().map(|p| v_str(&p.name)).collect();
    Ok(Ret(v_list(props)))
}
bf_declare!(properties, bf_properties);

async fn bf_set_player_flag<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }

    let (Variant::Obj(obj), Variant::Int(f)) = (bf_args.args[0].variant(), bf_args.args[1].variant()) else {
        return Ok(Error(E_INVARG));
    };

    let f = *f == 1;

    // User must be a wizard.
    bf_args.terk_perms().await?.check_wizard()?;

    // Get and set object flags
    let mut flags = bf_args.world_state.flags_of(*obj).await?;

    if f {
        flags.set(ObjFlag::User);
    } else {
        flags.clear(ObjFlag::User);
    }

    bf_args
        .world_state
        .set_flags_of(bf_args.task_perms_who(), *obj, flags)
        .await?;

    // If the object was player, update the VM's copy of the perms.
    if *obj == bf_args.terk_perms().await?.who {
        bf_args.vm.set_task_perms(*obj);
    }

    Ok(Ret(v_none()))
}
bf_declare!(set_player_flag, bf_set_player_flag);

impl VM {
    pub(crate) fn register_bf_objects(&mut self) -> Result<(), anyhow::Error> {
        self.builtins[offset_for_builtin("create")] = Arc::new(Box::new(BfCreate {}));
        self.builtins[offset_for_builtin("valid")] = Arc::new(Box::new(BfValid {}));
        self.builtins[offset_for_builtin("verbs")] = Arc::new(Box::new(BfVerbs {}));
        self.builtins[offset_for_builtin("properties")] = Arc::new(Box::new(BfProperties {}));
        self.builtins[offset_for_builtin("parent")] = Arc::new(Box::new(BfParent {}));
        self.builtins[offset_for_builtin("children")] = Arc::new(Box::new(BfChildren {}));
        self.builtins[offset_for_builtin("move")] = Arc::new(Box::new(BfMove {}));
        self.builtins[offset_for_builtin("chparent")] = Arc::new(Box::new(BfChparent {}));
        self.builtins[offset_for_builtin("set_player_flag")] =
            Arc::new(Box::new(BfSetPlayerFlag {}));
        Ok(())
    }
}
